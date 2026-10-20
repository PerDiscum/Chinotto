//! ArtworkDB reader / appender (iPod Classic layout).
//!
//! ```text
//! mhfd                          header (+0x08 total length, +0x1C last image id)
//!   mhsd type 1 → mhli → mhii*  one entry per track cover
//!     mhii                      +0x10 id, +0x14 song dbid (u64), +0x30 source pixels + 1
//!       mhod type 2 → mhni      one per thumbnail format: +0x10 format id,
//!                               +0x14 offset in .ithmb, +0x18 bytes, +0x20 height, +0x22 width
//!         mhod type 3           file name, UTF-16LE (":F1055_1.ithmb")
//!       mhod type 6             trailing block (copied as-is)
//!   mhsd type 2 → mhla          albums
//!   mhsd type 3 → mhlf → mhif*  one per format (+0x10 format id, +0x14 bytes)
//! ```
//!
//! New entries are byte-for-byte clones of an existing entry from the same
//! iPod with only ids, offsets and file names changed, so they match whatever
//! the iPod's previous manager wrote. Existing entries are never modified.

use std::collections::HashMap;

use crate::bytes::{decode_utf16le, encode_utf16le, put_u32, put_u64, Bytes};
use crate::Error;

#[derive(Debug, Clone, PartialEq)]
pub struct Thumb {
    pub format: u32,
    pub offset: u32,
    pub size: u32,
    pub width: u16,
    pub height: u16,
    /// e.g. "F1055_1.ithmb" (without the leading ':')
    pub file: String,
    /// Positions relative to the start of the owning mhii (for cloning).
    mhni_at: usize,
    name_at: Option<usize>,
    name_bytes: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub id: u32,
    pub song_dbid: u64,
    pub source_pixels: u32,
    pub thumbs: Vec<Thumb>,
    at: usize,
    len: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Format {
    pub format: u32,
    pub width: u16,
    pub height: u16,
    /// Slot size in bytes (may hold fewer rows than `height`, e.g. 56×55 for a 56×56 format).
    pub size: u32,
}

#[derive(Debug, Clone)]
pub struct ArtworkDb {
    bytes: Vec<u8>,
    pub last_id: u32,
    pub images: Vec<Image>,
    image_section: (usize, usize, usize, usize), // (mhsd at, mhsd len, mhli at, count)
}

#[derive(Debug, Clone)]
pub struct NewThumb {
    pub format: u32,
    pub file: String,
    pub offset: u32,
    /// The picture's size inside the slot, written to the thumbnail's header.
    pub width: u16,
    pub height: u16,
}

pub struct NewImage {
    pub id: u32,
    pub song_dbid: u64,
    pub source_pixels: u32,
    pub thumbs: Vec<NewThumb>,
}

impl ArtworkDb {
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        let b = Bytes::new(data);
        b.expect(0, b"mhfd")?;
        if b.u32(8)? as usize != data.len() {
            return Err(Error::Invalid("ArtworkDB length mismatch".into()));
        }
        let mut images = Vec::new();
        let mut image_section = None;
        let mut pos = b.u32(4)? as usize;
        while pos < data.len() {
            b.expect(pos, b"mhsd")?;
            let shl = b.u32(pos + 4)? as usize;
            let stl = b.u32(pos + 8)? as usize;
            let stype = b.u16(pos + 12)?;
            let list = pos + shl;
            let count = b.u32(list + 8)? as usize;
            if stype == 1 {
                b.expect(list, b"mhli")?;
                image_section = Some((pos, stl, list, count));
                let mut p = list + b.u32(list + 4)? as usize;
                for _ in 0..count {
                    b.expect(p, b"mhii")?;
                    let img = read_mhii(&b, p)?;
                    p += img.len;
                    images.push(img);
                }
            }
            if stl == 0 {
                return Err(Error::Invalid("zero-length section".into()));
            }
            pos += stl;
        }
        let image_section = image_section.ok_or_else(|| Error::Invalid("ArtworkDB has no image list".into()))?;
        Ok(ArtworkDb { last_id: b.u32(0x1C)?, bytes: data.to_vec(), images, image_section })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn max_id(&self) -> u32 {
        self.images.iter().map(|i| i.id).max().unwrap_or(0).max(self.last_id)
    }

    /// The set of thumbnail formats used by most covers on this iPod.
    pub fn common_formats(&self) -> Vec<Format> {
        let mut counts: HashMap<Vec<Format>, usize> = HashMap::new();
        for im in &self.images {
            let mut key: Vec<Format> =
                im.thumbs.iter().map(|t| Format { format: t.format, width: t.width, height: t.height, size: t.size }).collect();
            key.sort_by_key(|f| f.format);
            *counts.entry(key).or_default() += 1;
        }
        // Prefer the fullest set (iTunes' own covers carry every format the
        // iPod uses), then the most common one.
        counts.into_iter().max_by_key(|(k, n)| (k.len(), *n)).map(|(k, _)| k).unwrap_or_default()
    }

    /// Most recent image using exactly these formats, each in an `F<fmt>_<n>.ithmb` file.
    pub fn find_template(&self, formats: &[u32]) -> Option<&Image> {
        let mut want: Vec<u32> = formats.to_vec();
        want.sort_unstable();
        self.images.iter().rev().find(|im| {
            let mut have: Vec<u32> = im.thumbs.iter().map(|t| t.format).collect();
            have.sort_unstable();
            have == want && im.thumbs.iter().all(|t| is_ithmb_name(&t.file))
        })
    }

    /// Highest `n` in `F<format>_<n>.ithmb` referenced per format.
    pub fn last_file_index(&self) -> HashMap<u32, u32> {
        let mut out = HashMap::new();
        for t in self.images.iter().flat_map(|i| &i.thumbs) {
            if let Some((fmt, n)) = parse_ithmb_name(&t.file) {
                let e = out.entry(fmt).or_insert(0);
                *e = (*e).max(n);
            }
        }
        out
    }

    /// New ArtworkDB bytes with `entries` appended to the image list.
    pub fn append(&self, template: &Image, entries: &[NewImage]) -> Result<Vec<u8>, Error> {
        if entries.is_empty() {
            return Ok(self.bytes.clone());
        }
        let tpl = &self.bytes[template.at..template.at + template.len];
        let mut clones = Vec::with_capacity(entries.len() * tpl.len());
        for e in entries {
            let mut c = tpl.to_vec();
            put_u32(&mut c, 0x10, e.id);
            put_u64(&mut c, 0x14, e.song_dbid);
            put_u32(&mut c, 0x30, e.source_pixels);
            for t in &template.thumbs {
                let n = e
                    .thumbs
                    .iter()
                    .find(|n| n.format == t.format)
                    .ok_or_else(|| Error::Invalid(format!("missing thumbnail for format {}", t.format)))?;
                let file = &n.file;
                put_u32(&mut c, t.mhni_at + 0x14, n.offset);
                // No padding: the picture fills its stated size (the template's
                // may have been a letterboxed cover).
                c[t.mhni_at + 0x1C..t.mhni_at + 0x20].fill(0);
                c[t.mhni_at + 0x20..t.mhni_at + 0x22].copy_from_slice(&n.height.to_le_bytes());
                c[t.mhni_at + 0x22..t.mhni_at + 0x24].copy_from_slice(&n.width.to_le_bytes());
                let name = encode_utf16le(&format!(":{file}"));
                let at = t.name_at.ok_or_else(|| Error::Invalid("template thumbnail has no file name".into()))?;
                if name.len() != t.name_bytes {
                    return Err(Error::Invalid(format!("file name length differs from template: {file}")));
                }
                c[at..at + name.len()].copy_from_slice(&name);
            }
            clones.extend_from_slice(&c);
        }

        let (s_at, s_len, l_at, count) = self.image_section;
        let insert_at = s_at + s_len;
        let mut out = Vec::with_capacity(self.bytes.len() + clones.len());
        out.extend_from_slice(&self.bytes[..insert_at]);
        out.extend_from_slice(&clones);
        out.extend_from_slice(&self.bytes[insert_at..]);
        let new_len = out.len() as u32;
        put_u32(&mut out, 8, new_len);
        put_u32(&mut out, s_at + 8, (s_len + clones.len()) as u32);
        put_u32(&mut out, l_at + 8, (count + entries.len()) as u32);
        let last = entries.iter().map(|e| e.id).max().unwrap_or(0).max(self.last_id);
        put_u32(&mut out, 0x1C, last);
        Ok(out)
    }
}

fn read_mhii(b: &Bytes, p: usize) -> Result<Image, Error> {
    let hl = b.u32(p + 4)? as usize;
    let tl = b.u32(p + 8)? as usize;
    let nchild = b.u32(p + 12)? as usize;
    let mut thumbs = Vec::new();
    let mut q = p + hl;
    for _ in 0..nchild {
        if q >= p + tl {
            break;
        }
        b.expect(q, b"mhod")?;
        let mhl = b.u32(q + 4)? as usize;
        let mtl = b.u32(q + 8)? as usize;
        let mtype = b.u16(q + 12)?;
        if mtype == 2 || mtype == 5 {
            let ni = q + mhl;
            b.expect(ni, b"mhni")?;
            let fq = ni + b.u32(ni + 4)? as usize;
            let (mut file, mut name_at, mut name_bytes) = (String::new(), None, 0);
            if b.tag(fq)? == *b"mhod" && b.u16(fq + 12)? == 3 {
                name_bytes = b.u32(fq + 24)? as usize;
                name_at = Some(fq + 36 - p);
                file = decode_utf16le(b.slice(fq + 36, name_bytes)?).trim_start_matches(':').to_string();
            }
            thumbs.push(Thumb {
                format: b.u32(ni + 0x10)?,
                offset: b.u32(ni + 0x14)?,
                size: b.u32(ni + 0x18)?,
                height: b.u16(ni + 0x20)?,
                width: b.u16(ni + 0x22)?,
                file,
                mhni_at: ni - p,
                name_at,
                name_bytes,
            });
        }
        q += mtl;
    }
    Ok(Image { id: b.u32(p + 0x10)?, song_dbid: b.u64(p + 0x14)?, source_pixels: b.u32(p + 0x30)?, thumbs, at: p, len: tl })
}

pub fn ithmb_name(format: u32, index: u32) -> String {
    format!("F{format}_{index}.ithmb")
}

fn is_ithmb_name(s: &str) -> bool {
    parse_ithmb_name(s).is_some()
}

fn parse_ithmb_name(s: &str) -> Option<(u32, u32)> {
    let rest = s.strip_prefix('F')?.strip_suffix(".ithmb")?;
    let (fmt, n) = rest.split_once('_')?;
    Some((fmt.parse().ok()?, n.parse().ok()?))
}
