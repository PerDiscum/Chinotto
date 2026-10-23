//! `.OMA` files as an OpenMG Walkman stores MP3 songs.
//!
//! ```text
//! 0x000  "ea3" tag: an ID3v2.3 tag (magic "ea3"), text frames in UTF-16BE,
//!        zero-padded to 0xC00 bytes
//! 0xC00  "EA3" header, 0x60 bytes:
//!        "EA3" 02 | 00 60 | FF FF (not encrypted) | 00 00 00 00
//!        01 0F 50 00 00 04 00 00 | content ID (12 bytes)
//!        codec (4 bytes: 03 = MP3, then its parameters)
//!        length in ms (u32 BE) | frame count (u32 BE) | zeros
//! 0xC60  the MP3 frames, as they are
//! ```

use crate::mp3::Mp3;

pub const TAG_SIZE: usize = 0xC00;
pub const HEADER_SIZE: usize = 0x60;

/// The bytes before the content ID, in every file and in `05CIDLST`.
pub const CID_PREFIX: [u8; 8] = [0x01, 0x0F, 0x50, 0x00, 0x00, 0x04, 0x00, 0x00];

/// The codec field for MP3 at `bitrate_index`, as the device's own files
/// have it: `03 84 D9 00` at 128 kbps, `03 84 DE 00` at 320 kbps (the low
/// nibble is the bitrate index; only these two are confirmed).
pub fn codec_params(bitrate_index: u8) -> [u8; 4] {
    [0x03, 0x84, 0xD0 | (bitrate_index & 0x0F), 0x00]
}

/// A song's tags as the file carries them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tags {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub year: Option<u32>,
    /// Track and disc number (0: unknown). The Walkman's database has no
    /// place for them, so they're kept in the file's tag (TRCK, TPOS),
    /// where Chinotto reads them to order albums.
    pub track: u32,
    pub disc: u32,
}

fn text_frame(id: &[u8; 4], text: &str) -> Vec<u8> {
    let body: Vec<u8> = std::iter::once(2u8).chain(crate::fit(text).encode_utf16().flat_map(u16::to_be_bytes)).collect();
    let mut f = id.to_vec();
    f.extend_from_slice(&(body.len() as u32).to_be_bytes());
    f.extend_from_slice(&[0, 0]);
    f.extend(body);
    f
}

/// The 0xC00-byte `ea3` tag.
pub fn tag(tags: &Tags, length_ms: u32) -> Vec<u8> {
    let size = TAG_SIZE - 10; // syncsafe: 0xBF6 → 00 00 17 76
    let mut t = b"ea3\x03\x00\x00".to_vec();
    t.extend_from_slice(&[(size >> 21) as u8 & 0x7F, (size >> 14) as u8 & 0x7F, (size >> 7) as u8 & 0x7F, size as u8 & 0x7F]);
    t.extend(text_frame(b"TIT2", &tags.title));
    t.extend(text_frame(b"TPE1", &tags.artist));
    t.extend(text_frame(b"TALB", &tags.album));
    t.extend(text_frame(b"TCON", &tags.genre));
    if let Some(y) = tags.year.filter(|y| *y > 0) {
        t.extend(text_frame(b"TYER", &y.to_string()));
    }
    if tags.track > 0 {
        t.extend(text_frame(b"TRCK", &tags.track.to_string()));
    }
    if tags.disc > 0 {
        t.extend(text_frame(b"TPOS", &tags.disc.to_string()));
    }
    t.extend(text_frame(b"TLEN", &length_ms.to_string()));
    t.resize(TAG_SIZE, 0);
    t
}

/// The 0x60-byte `EA3` header.
pub fn header(cid: &[u8; 12], codec: [u8; 4], length_ms: u32, frames: u32) -> Vec<u8> {
    let mut h = b"EA3\x02\x00\x60\xFF\xFF\x00\x00\x00\x00".to_vec();
    h.extend_from_slice(&CID_PREFIX);
    h.extend_from_slice(cid);
    h.extend_from_slice(&codec);
    h.extend_from_slice(&length_ms.to_be_bytes());
    h.extend_from_slice(&frames.to_be_bytes());
    h.resize(HEADER_SIZE, 0);
    h
}

/// A whole `.OMA` file.
pub fn build(mp3: &Mp3, tags: &Tags, cid: &[u8; 12]) -> Vec<u8> {
    let mut f = tag(tags, mp3.length_ms);
    f.extend(header(cid, codec_params(mp3.bitrate_index), mp3.length_ms, mp3.frames));
    f.extend_from_slice(&mp3.audio);
    f
}

/// Track and disc number from an `ea3` tag (the file's first 0xC00 bytes).
pub fn numbers(head: &[u8]) -> (u32, u32) {
    let (mut track, mut disc) = (0, 0);
    if head.get(..3) != Some(b"ea3") {
        return (0, 0);
    }
    let mut p = 10;
    while let Some(id) = head.get(p..p + 4).filter(|id| id.iter().all(|b| b.is_ascii_alphanumeric())) {
        let Some(size) = head.get(p + 4..p + 8).map(|s| u32::from_be_bytes(s.try_into().unwrap()) as usize) else { break };
        let Some(body) = head.get(p + 10..p + 10 + size) else { break };
        let text = match body.first() {
            Some(2) => String::from_utf16_lossy(&body[1..].chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect::<Vec<_>>()),
            Some(0) | Some(3) => String::from_utf8_lossy(&body[1..]).into_owned(),
            _ => String::new(),
        };
        let n = text.trim_matches('\0').split('/').next().and_then(|n| n.trim().parse().ok()).unwrap_or(0);
        match id {
            b"TRCK" => track = n,
            b"TPOS" => disc = n,
            _ => {}
        }
        p += 10 + size;
    }
    (track, disc)
}

/// Replace the tag of the `.OMA` file at `path` (its first 0xC00 bytes; the
/// header and audio are left alone).
pub fn rewrite_tag(path: &std::path::Path, tags: &Tags, length_ms: u32) -> std::io::Result<()> {
    use std::io::{Read, Seek, SeekFrom, Write};
    let mut f = std::fs::OpenOptions::new().read(true).write(true).open(path)?;
    let mut head = vec![0u8; TAG_SIZE + 3];
    f.read_exact(&mut head)?;
    if &head[..3] != b"ea3" || &head[TAG_SIZE..] != b"EA3" {
        return Err(std::io::Error::other("not an OpenMG file"));
    }
    f.seek(SeekFrom::Start(0))?;
    f.write_all(&tag(tags, length_ms))?;
    f.sync_all()
}

/// The content ID of an `.OMA` file (from its first 0xC20 bytes).
pub fn content_id(head: &[u8]) -> Option<[u8; 12]> {
    if head.get(TAG_SIZE..TAG_SIZE + 3) != Some(b"EA3") {
        return None;
    }
    head.get(TAG_SIZE + 0x14..TAG_SIZE + 0x20)?.try_into().ok()
}

/// Where song `id` (its row in `04CNTINF`, from 1) lives, inside OMGAUDIO.
pub fn path_for(id: u32) -> String {
    format!("10F{:02X}/{:08X}.OMA", id >> 8, 0x1000_0000 + id)
}

/// A new content ID: 12 bytes that differ from every other file's.
pub fn new_content_id(taken: &[[u8; 12]]) -> [u8; 12] {
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    loop {
        // std's hasher keys are random per process; mix in time and a counter.
        let state = std::collections::hash_map::RandomState::new();
        let mut out = [0u8; 12];
        for (i, chunk) in out.chunks_mut(8).enumerate() {
            let mut h = state.build_hasher();
            h.write_u64(N.fetch_add(1, Ordering::Relaxed));
            h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
            h.write_usize(i);
            chunk.copy_from_slice(&h.finish().to_be_bytes()[..chunk.len()]);
        }
        if !taken.contains(&out) {
            return out;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_layout() {
        let data: Vec<u8> = (0..5).flat_map(|i| crate::mp3::tests::frame(9, i == 2)).collect();
        let mp3 = crate::mp3::prepare(&data).unwrap();
        let tags = Tags {
            title: "If We".into(),
            artist: "Mariah Carey".into(),
            album: "Glitter".into(),
            genre: "Pop".into(),
            year: Some(2001),
            ..Default::default()
        };
        let cid = [0x77, 0xaa, 0x1e, 0x4e, 0x90, 0xed, 0xf6, 0x93, 0xb7, 0x4f, 0x74, 0x93];
        let f = build(&mp3, &tags, &cid);
        // As the device's own files: tag header, first frame, EA3 header.
        assert_eq!(&f[..14], b"ea3\x03\x00\x00\x00\x00\x17\x76TIT2");
        assert_eq!(&f[14..22], &[0, 0, 0, 0x0b, 0, 0, 2, 0]);
        assert_eq!(&f[0xC00..0xC14], b"EA3\x02\x00\x60\xFF\xFF\x00\x00\x00\x00\x01\x0F\x50\x00\x00\x04\x00\x00");
        assert_eq!(content_id(&f), Some(cid));
        assert_eq!(&f[0xC20..0xC24], &[0x03, 0x84, 0xD9, 0x00]);
        assert_eq!(u32::from_be_bytes(f[0xC28..0xC2C].try_into().unwrap()), 5);
        assert_eq!(f.len(), 0xC60 + data.len());
        assert_eq!(&f[0xC60..], &data[..]);
        assert_eq!(codec_params(14), [0x03, 0x84, 0xDE, 0x00]);
        assert_eq!(numbers(&f), (0, 0));
        let numbered = tag(&Tags { track: 3, disc: 1, ..tags }, 1000);
        assert_eq!(numbered.len(), TAG_SIZE);
        assert_eq!(numbers(&numbered), (3, 1));
    }

    #[test]
    fn paths_and_ids() {
        assert_eq!(path_for(7), "10F00/10000007.OMA");
        assert_eq!(path_for(0x10A), "10F01/1000010A.OMA");
        let a = new_content_id(&[]);
        assert_ne!(a, new_content_id(&[a]));
    }
}
