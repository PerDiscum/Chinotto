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

use crate::bytes::{decode_utf16le, encode_utf16le, put_u16, put_u32, put_u64, Bytes};
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
        counts.into_iter().max_by_key(|(_, n)| *n).map(|(k, _)| k).unwrap_or_default()
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

    /// New ArtworkDB bytes with `entries` appended to the image list, each a
    /// copy of `template` (an image of this database).
    pub fn append(&self, template: &Image, entries: &[NewImage]) -> Result<Vec<u8>, Error> {
        self.append_from(&self.bytes[template.at..template.at + template.len], template, entries)
    }

    /// Like [`append`](Self::append), with a template made by [`template_image`].
    pub fn append_new(&self, template: &(Vec<u8>, Image), entries: &[NewImage]) -> Result<Vec<u8>, Error> {
        self.append_from(&template.0, &template.1, entries)
    }

    fn append_from(&self, tpl: &[u8], template: &Image, entries: &[NewImage]) -> Result<Vec<u8>, Error> {
        if entries.is_empty() {
            return Ok(self.bytes.clone());
        }
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

/// An empty ArtworkDB (no covers yet) listing `formats`, laid out as iTunes
/// and libgpod write it: image list, album list, file list.
pub fn blank(formats: &[(u32, u32)]) -> Vec<u8> {
    fn header(tag: &[u8; 4], len: usize) -> Vec<u8> {
        let mut h = vec![0u8; len];
        h[..4].copy_from_slice(tag);
        put_u32(&mut h, 4, len as u32);
        put_u32(&mut h, 8, len as u32);
        h
    }
    fn section(kind: u16, list: Vec<u8>) -> Vec<u8> {
        let mut s = header(b"mhsd", 0x60);
        put_u16(&mut s, 0x0C, kind);
        put_u32(&mut s, 8, (0x60 + list.len()) as u32);
        s.extend(list);
        s
    }
    let mhli = header(b"mhli", 0x5C); // count 0 (in the total-length slot)
    let mut mhli = mhli;
    put_u32(&mut mhli, 8, 0);
    let mut mhla = header(b"mhla", 0x5C);
    put_u32(&mut mhla, 8, 0);
    let mut mhlf = header(b"mhlf", 0x5C);
    put_u32(&mut mhlf, 8, formats.len() as u32);
    for (id, size) in formats {
        let mut f = header(b"mhif", 0x7C);
        put_u32(&mut f, 0x10, *id);
        put_u32(&mut f, 0x14, *size);
        mhlf.extend(f);
    }
    let mut out = header(b"mhfd", 0x84);
    put_u32(&mut out, 0x10, 2); // iTunes 4.9+ databases
    put_u32(&mut out, 0x14, 3); // sections
    put_u32(&mut out, 0x1C, 99); // last image id; the first cover gets 100
    out[0x30] = 2;
    out.extend(section(1, mhli));
    out.extend(section(2, mhla));
    out.extend(section(3, mhlf));
    let len = out.len() as u32;
    put_u32(&mut out, 8, len);
    out
}

/// A cover entry to copy for new covers when the iPod has none yet: one
/// thumbnail per format, in `F<format>_1.ithmb` (the file name only sets the
/// name's length; [`ArtworkDb::append_new`] fills in the real ones).
pub fn template_image(formats: &[Format]) -> Result<(Vec<u8>, Image), Error> {
    let mut m = vec![0u8; 0x98];
    m[..4].copy_from_slice(b"mhii");
    put_u32(&mut m, 4, 0x98);
    put_u32(&mut m, 0x0C, formats.len() as u32);
    for f in formats {
        let name = encode_utf16le(&format!(":{}", ithmb_name(f.format, 1)));
        let pad = (4 - name.len() % 4) % 4;
        let mut s = vec![0u8; 0x24];
        s[..4].copy_from_slice(b"mhod");
        put_u32(&mut s, 4, 0x18);
        put_u32(&mut s, 8, (0x24 + name.len() + pad) as u32);
        put_u16(&mut s, 0x0C, 3);
        put_u32(&mut s, 0x18, name.len() as u32);
        s[0x1C] = 2; // UTF-16LE
        s.extend(&name);
        s.extend(std::iter::repeat_n(0u8, pad));
        let mut ni = vec![0u8; 0x4C];
        ni[..4].copy_from_slice(b"mhni");
        put_u32(&mut ni, 4, 0x4C);
        put_u32(&mut ni, 8, (0x4C + s.len()) as u32);
        put_u32(&mut ni, 0x0C, 1);
        put_u32(&mut ni, 0x10, f.format);
        put_u32(&mut ni, 0x18, f.size);
        put_u16(&mut ni, 0x20, f.height);
        put_u16(&mut ni, 0x22, f.width);
        ni.extend(s);
        let mut od = vec![0u8; 0x18];
        od[..4].copy_from_slice(b"mhod");
        put_u32(&mut od, 4, 0x18);
        put_u32(&mut od, 8, (0x18 + ni.len()) as u32);
        put_u16(&mut od, 0x0C, 2);
        od.extend(ni);
        m.extend(od);
    }
    let len = m.len() as u32;
    put_u32(&mut m, 8, len);
    let img = read_mhii(&Bytes::new(&m), 0)?;
    Ok((m, img))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_database_takes_new_covers() {
        let formats =
            [Format { format: 1055, width: 128, height: 128, size: 32768 }, Format { format: 1061, width: 56, height: 56, size: 6160 }];
        let db = ArtworkDb::parse(&blank(&[(1055, 32768), (1061, 6050)])).unwrap();
        assert!(db.images.is_empty());
        let tpl = template_image(&formats).unwrap();
        let entries = [NewImage {
            id: db.max_id() + 1,
            song_dbid: 0xABCD,
            source_pixels: 600 * 600 + 1,
            thumbs: vec![
                NewThumb { format: 1055, file: "F1055_1.ithmb".into(), offset: 0, width: 128, height: 128 },
                NewThumb { format: 1061, file: "F1061_1.ithmb".into(), offset: 6160, width: 56, height: 56 },
            ],
        }];
        let again = ArtworkDb::parse(&db.append_new(&tpl, &entries).unwrap()).unwrap();
        assert_eq!(again.images.len(), 1);
        let im = &again.images[0];
        assert_eq!((im.id, im.song_dbid), (100, 0xABCD));
        assert_eq!(im.thumbs[1].file, "F1061_1.ithmb");
        assert_eq!((im.thumbs[1].offset, im.thumbs[1].size, im.thumbs[1].width), (6160, 6160, 56));
        assert_eq!(again.common_formats(), {
            let mut f = formats.to_vec();
            f.sort_by_key(|f| f.format);
            f
        });
    }
}
