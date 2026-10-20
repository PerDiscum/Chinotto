//! iTunesDB reader.
//!
//! Layout (all little-endian; every record starts with a 4-byte tag, its
//! header length at +4, and either its total length or a child count at +8):
//!
//! ```text
//! mhbd                         database header (+0x14: number of mhsd sections)
//!   mhsd (+0x0C type)          1 = tracks, 2 = playlists, 3 = playlists incl. podcasts,
//!                              4 = albums, 5 = smart "special" playlists, ...
//!     mhlt (+0x08 count)  →  mhit*   track (+0x0C number of mhod children)
//!     mhlp (+0x08 count)  →  mhyp*   playlist → mhod* (name, smart rules…) then mhip* items
//!     mhla (+0x08 count)  →  mhia*   album
//!   mhod                       string or data record; strings: +0x18 encoding
//!                              (1 = UTF-16LE, 2 = UTF-8), +0x1C byte length, text at +0x28
//! ```

use crate::bytes::{decode_utf16le, Bytes};
use crate::Error;

/// String fields of a track (mhod types). Only the ones present are filled.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackStrings {
    pub title: Option<String>,        // 1
    pub location: Option<String>,     // 2  e.g. ":iPod_Control:Music:F40:abcd.m4a"
    pub album: Option<String>,        // 3
    pub artist: Option<String>,       // 4
    pub genre: Option<String>,        // 5
    pub filetype: Option<String>,     // 6  e.g. "AAC audio file"
    pub comment: Option<String>,      // 8
    pub composer: Option<String>,     // 12
    pub grouping: Option<String>,     // 13
    pub album_artist: Option<String>, // 22
    pub sort_artist: Option<String>,  // 23
    pub sort_title: Option<String>,   // 27
    pub sort_album: Option<String>,   // 28
    pub sort_album_artist: Option<String>, // 29
    pub sort_composer: Option<String>,     // 30
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Track {
    /// Track id used by playlists (mhit +0x10).
    pub id: u32,
    /// 64-bit database id (mhit +0x70); ArtworkDB links covers by this.
    pub dbid: u64,
    pub strings: TrackStrings,
    pub compilation: bool,       // +0x1E
    pub rating: u8,              // +0x1F, 0–100 in steps of 20
    pub time_modified: u32,      // +0x20 (Mac epoch)
    pub size: u32,               // +0x24 bytes
    pub length_ms: u32,          // +0x28
    pub track_nr: u32,           // +0x2C
    pub track_count: u32,        // +0x30
    pub year: u32,               // +0x34
    pub bitrate_kbps: u32,       // +0x38
    pub sample_rate: u32,        // +0x3C (stored as rate << 16)
    pub play_count: u32,         // +0x50
    pub time_played: u32,        // +0x58 (Mac epoch)
    pub disc_nr: u32,            // +0x5C
    pub disc_count: u32,         // +0x60
    pub time_added: u32,         // +0x68 (Mac epoch)
    pub artwork_count: u16,      // +0x7C
    pub artwork_size: u32,       // +0x80
    /// 1 = has artwork, 2 = none (as written by Swinsian/iTunes), 0 = unset.
    pub has_artwork: u8,         // +0xA4
    pub media_type: u32,         // +0xD0 (1 = audio)
    /// ArtworkDB image id this track's cover uses (0 = none).
    pub mhii_link: u32,          // +0x160
    /// Offset of this mhit in the file (useful for patching).
    pub offset: usize,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Playlist {
    pub name: String,
    pub id: u64,
    pub is_master: bool,
    pub is_smart: bool,
    /// Track ids (see [`Track::id`]) in playlist order.
    pub track_ids: Vec<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct ITunesDb {
    pub version: u32,
    pub tracks: Vec<Track>,
    pub playlists: Vec<Playlist>,
    /// Section types present, in file order (for diagnostics).
    pub sections: Vec<u32>,
}

impl ITunesDb {
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        let b = Bytes::new(data);
        b.expect(0, b"mhbd")?;
        let header_len = b.u32(4)? as usize;
        let total_len = b.u32(8)? as usize;
        if total_len != data.len() {
            return Err(Error::Invalid(format!(
                "iTunesDB says it is {total_len} bytes but the file is {}",
                data.len()
            )));
        }
        let mut db = ITunesDb { version: b.u32(0x10)?, ..Default::default() };
        let section_count = b.u32(0x14)? as usize;

        let mut playlists_from_type2: Option<Vec<Playlist>> = None;
        let mut playlists_from_type3: Option<Vec<Playlist>> = None;

        let mut pos = header_len;
        for _ in 0..section_count {
            b.expect(pos, b"mhsd")?;
            let shl = b.u32(pos + 4)? as usize;
            let stl = b.u32(pos + 8)? as usize;
            let stype = b.u32(pos + 0x0C)?;
            db.sections.push(stype);
            let list = pos + shl;
            match stype {
                1 => db.tracks = parse_tracks(&b, list)?,
                2 => playlists_from_type2 = Some(parse_playlists(&b, list)?),
                3 => playlists_from_type3 = Some(parse_playlists(&b, list)?),
                _ => {}
            }
            pos = pos.checked_add(stl).ok_or(Error::Invalid("section length overflow".into()))?;
        }
        // Type 2 is the regular playlist list; fall back to type 3 if it's missing.
        db.playlists = playlists_from_type2.or(playlists_from_type3).unwrap_or_default();
        Ok(db)
    }

    pub fn master_playlist(&self) -> Option<&Playlist> {
        self.playlists.iter().find(|p| p.is_master)
    }

    pub fn track_by_id(&self, id: u32) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }
}

fn parse_tracks(b: &Bytes, list: usize) -> Result<Vec<Track>, Error> {
    b.expect(list, b"mhlt")?;
    let count = b.u32(list + 8)? as usize;
    let mut p = list + b.u32(list + 4)? as usize;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        b.expect(p, b"mhit")?;
        let hl = b.u32(p + 4)? as usize;
        let tl = b.u32(p + 8)? as usize;
        let nmhod = b.u32(p + 0x0C)? as usize;
        let opt_u32 = |off: usize| if off + 4 <= hl { b.u32(p + off) } else { Ok(0) };
        let opt_u16 = |off: usize| if off + 2 <= hl { b.u16(p + off) } else { Ok(0) };
        let opt_u8 = |off: usize| if off < hl { b.u8(p + off) } else { Ok(0) };
        let mut t = Track {
            id: b.u32(p + 0x10)?,
            dbid: b.u64(p + 0x70)?,
            compilation: b.u8(p + 0x1E)? != 0,
            rating: b.u8(p + 0x1F)?,
            time_modified: b.u32(p + 0x20)?,
            size: b.u32(p + 0x24)?,
            length_ms: b.u32(p + 0x28)?,
            track_nr: b.u32(p + 0x2C)?,
            track_count: b.u32(p + 0x30)?,
            year: b.u32(p + 0x34)?,
            bitrate_kbps: b.u32(p + 0x38)?,
            sample_rate: b.u32(p + 0x3C)? >> 16,
            play_count: b.u32(p + 0x50)?,
            time_played: b.u32(p + 0x58)?,
            disc_nr: b.u32(p + 0x5C)?,
            disc_count: b.u32(p + 0x60)?,
            time_added: b.u32(p + 0x68)?,
            artwork_count: opt_u16(0x7C)?,
            artwork_size: opt_u32(0x80)?,
            has_artwork: opt_u8(0xA4)?,
            media_type: opt_u32(0xD0)?,
            mhii_link: opt_u32(0x160)?,
            offset: p,
            ..Default::default()
        };
        let mut q = p + hl;
        for _ in 0..nmhod {
            b.expect(q, b"mhod")?;
            let mtl = b.u32(q + 8)? as usize;
            let mtype = b.u32(q + 0x0C)?;
            if let Some(slot) = string_slot(&mut t.strings, mtype) {
                *slot = Some(read_string_mhod(b, q)?);
            }
            q += mtl;
        }
        if q > p + tl {
            return Err(Error::Invalid(format!("track at {p}: children overrun its length")));
        }
        out.push(t);
        p += tl;
    }
    Ok(out)
}

fn string_slot(s: &mut TrackStrings, mtype: u32) -> Option<&mut Option<String>> {
    Some(match mtype {
        1 => &mut s.title,
        2 => &mut s.location,
        3 => &mut s.album,
        4 => &mut s.artist,
        5 => &mut s.genre,
        6 => &mut s.filetype,
        8 => &mut s.comment,
        12 => &mut s.composer,
        13 => &mut s.grouping,
        22 => &mut s.album_artist,
        23 => &mut s.sort_artist,
        27 => &mut s.sort_title,
        28 => &mut s.sort_album,
        29 => &mut s.sort_album_artist,
        30 => &mut s.sort_composer,
        _ => return None,
    })
}

/// Read a standard string mhod (types 1–14, 18–31).
fn read_string_mhod(b: &Bytes, q: usize) -> Result<String, Error> {
    let encoding = b.u32(q + 0x18)?;
    let len = b.u32(q + 0x1C)? as usize;
    let raw = b.slice(q + 0x28, len)?;
    Ok(if encoding == 2 {
        String::from_utf8_lossy(raw).into_owned()
    } else {
        decode_utf16le(raw)
    })
}

fn parse_playlists(b: &Bytes, list: usize) -> Result<Vec<Playlist>, Error> {
    b.expect(list, b"mhlp")?;
    let count = b.u32(list + 8)? as usize;
    let mut p = list + b.u32(list + 4)? as usize;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        b.expect(p, b"mhyp")?;
        let hl = b.u32(p + 4)? as usize;
        let tl = b.u32(p + 8)? as usize;
        let nmhod = b.u32(p + 0x0C)? as usize;
        let nitems = b.u32(p + 0x10)? as usize;
        let mut pl = Playlist {
            is_master: b.u8(p + 0x14)? != 0,
            id: b.u64(p + 0x1C)?,
            ..Default::default()
        };
        let mut q = p + hl;
        for _ in 0..nmhod {
            b.expect(q, b"mhod")?;
            let mtype = b.u32(q + 0x0C)?;
            match mtype {
                1 => pl.name = read_string_mhod(b, q)?,
                50 | 51 => pl.is_smart = true,
                _ => {}
            }
            q += b.u32(q + 8)? as usize;
        }
        pl.track_ids.reserve(nitems);
        for _ in 0..nitems {
            b.expect(q, b"mhip")?;
            pl.track_ids.push(b.u32(q + 0x18)?);
            q += b.u32(q + 8)? as usize;
        }
        out.push(pl);
        p += tl;
    }
    Ok(out)
}
