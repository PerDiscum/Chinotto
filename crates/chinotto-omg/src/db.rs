//! The OMGAUDIO database.
//!
//! Every table file has the same layout:
//!
//! ```text
//! magic (4) | 01 01 00 00 | section count (u8) | 7 zeros
//! per section (16 bytes): name (4) | offset (u32) | length (u32) | 4 zeros
//! each section: name (4) | row count (u16) | row size (u16) | 8 bytes | rows
//! ```
//!
//! The song list (`04CNTINF`, 656-byte rows of five 128-byte tag slots) and
//! the content IDs (`05CIDLST`, one per song, same order) are the data;
//! everything else is derived from them and rebuilt on every change, as the
//! device's own software does:
//!
//! | tree (`01TREEnn`) | groups (`03GINFnn`)       | songs in a group by |
//! |-------------------|---------------------------|---------------------|
//! | 01 albums         | album, artist, genre, art | song number         |
//! | 02 artists        | name                      | album, number       |
//! | 03 albums         | name                      | song number         |
//! | 04 genres         | name                      | artist, album, no.  |
//! | 2D artist › album | name + sort name (+ art)  | as 02               |
//!
//! Group names sort by their UTF-16 code units (so "Unknown Artist" comes
//! before "ea933892"); "song number" is the row in `04CNTINF`. A tree's
//! groups point at where they start in its song list (`TPLB`). `02TREINF`
//! holds the total length of all songs for trees 01–04 and 2C. The device
//! rebuilds its other trees (0C, 0E, 0F…) itself.
//!
//! Rebuilding the user's NW-S716F tables this way gives its files byte for
//! byte (test `rebuilds_device_tables`).

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use crate::oma::CID_PREFIX;

pub const SONG_ROW: usize = 0x290;
pub const CID_ROW: usize = 0x30;
const SLOT: usize = 0x80;

/// A song in the database.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Song {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub year: Option<u32>,
    pub length_ms: u32,
    /// The codec field (0 in songs some software added; `oma::codec_params` otherwise).
    pub codec: [u8; 4],
    pub cid: [u8; 12],
    /// Track and disc number (0: unknown), from the file's tag: the tables
    /// have no place for them. An album's songs go in this order.
    pub track: u32,
    pub disc: u32,
    /// The song's file inside OMGAUDIO ("10F00/10000001.OMA"), if found.
    pub file: Option<String>,
}

/// An album's pictures: 200, 96 and 32 pixels square (baseline JPEG).
#[derive(Debug, Clone, PartialEq)]
pub struct Art {
    pub large: Vec<u8>,
    pub medium: Vec<u8>,
    pub small: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Database {
    pub songs: Vec<Song>,
    /// Pictures by album name.
    pub art: HashMap<String, Art>,
    /// `02TREINF` as read (only its totals change).
    treinf: Vec<u8>,
    /// Rows of `02TREINF` holding the total length (by the tree list).
    total_rows: Vec<usize>,
}

pub type Error = String;

fn be16(b: &[u8], at: usize) -> usize {
    u16::from_be_bytes([b[at], b[at + 1]]) as usize
}
fn be32(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// Section `i` of a table: (rows offset, row count, row size).
fn section(file: &[u8], magic: &[u8; 4], i: usize) -> Result<(usize, usize, usize), Error> {
    let name = String::from_utf8_lossy(magic).into_owned();
    if file.len() < 0x20 || &file[..4] != magic || file[8] as usize <= i {
        return Err(format!("{name}: not the expected table"));
    }
    let at = be32(file, 0x10 + i * 16 + 4) as usize;
    if file.len() < at + 16 {
        return Err(format!("{name}: cut short"));
    }
    let (count, size) = (be16(file, at + 4), be16(file, at + 6));
    if file.len() < at + 16 + count * size {
        return Err(format!("{name}: cut short"));
    }
    Ok((at + 16, count, size))
}

/// The text of a tag slot (`ID` + `00 02` + UTF-16BE), if it's `id`.
fn slot_text(slot: &[u8], id: &[u8; 4]) -> Option<String> {
    if &slot[..4] != id {
        return None;
    }
    let units: Vec<u16> = slot[6..].chunks_exact(2).map(|p| u16::from_be_bytes([p[0], p[1]])).take_while(|u| *u != 0).collect();
    Some(String::from_utf16_lossy(&units))
}

fn slot(id: &[u8; 4], text: &str) -> Vec<u8> {
    let mut s = id.to_vec();
    s.extend_from_slice(&[0, 2]);
    s.extend(crate::fit(text).encode_utf16().flat_map(u16::to_be_bytes));
    s.resize(SLOT, 0);
    s
}

/// A picture tag's file paths ("200200;20P01\1G0001P0.JPG|…").
fn picture_paths(text: &str) -> Vec<String> {
    text.split('|').filter_map(|p| p.split_once(';')).map(|(_, path)| path.replace('\\', "/")).collect()
}

impl Database {
    /// Read the database in `omgaudio` (the OMGAUDIO folder).
    pub fn read(omgaudio: &Path) -> Result<Self, Error> {
        let file = |name: &str| std::fs::read(omgaudio.join(name)).map_err(|e| format!("{name}: {e}"));
        let cnt = file("04CNTINF.DAT")?;
        let cid = file("05CIDLST.DAT")?;
        let (at, n, size) = section(&cnt, b"CNIF", 0)?;
        let (cat, cn, csize) = section(&cid, b"CIDL", 0)?;
        if size != SONG_ROW || csize != CID_ROW || cn != n {
            return Err("04CNTINF and 05CIDLST don't match".into());
        }
        let mut songs = Vec::with_capacity(n);
        for i in 0..n {
            let row = &cnt[at + i * SONG_ROW..][..SONG_ROW];
            let slots = be16(row, 12);
            let mut s = Song { length_ms: be32(row, 8), codec: row[4..8].try_into().unwrap(), ..Default::default() };
            for k in 0..slots.min(5) {
                let sl = &row[16 + k * SLOT..][..SLOT];
                match &sl[..4] {
                    b"TIT2" => s.title = slot_text(sl, b"TIT2").unwrap_or_default(),
                    b"TPE1" => s.artist = slot_text(sl, b"TPE1").unwrap_or_default(),
                    b"TALB" => s.album = slot_text(sl, b"TALB").unwrap_or_default(),
                    b"TCON" => s.genre = slot_text(sl, b"TCON").unwrap_or_default(),
                    b"TYER" => s.year = slot_text(sl, b"TYER").and_then(|y| y.trim().parse().ok()),
                    _ => {}
                }
            }
            let crow = &cid[cat + i * CID_ROW..][..CID_ROW];
            if crow[4..12] != CID_PREFIX {
                return Err(format!("05CIDLST row {} has an unknown layout", i + 1));
            }
            s.cid = crow[12..24].try_into().unwrap();
            songs.push(s);
        }
        // Album pictures, from the album groups' picture tags.
        let mut art = HashMap::new();
        if let Ok(g) = file("03GINF01.DAT") {
            let (at, n, size) = section(&g, b"GPIF", 0)?;
            for i in 0..n {
                let row = &g[at + i * size..][..size];
                let tags: Vec<&[u8]> = (16..size).step_by(SLOT).map(|o| &row[o..o + SLOT]).collect();
                let name = tags.iter().find_map(|t| slot_text(t, b"TIT2"));
                let big = tags.iter().find_map(|t| slot_text(t, b"PICP")).map(|t| picture_paths(&t));
                let thumbs = tags.iter().find_map(|t| slot_text(t, b"PIC0")).map(|t| picture_paths(&t));
                if let (Some(name), Some(big), Some(thumbs)) = (name, big, thumbs) {
                    let read = |p: Option<&String>| p.and_then(|p| std::fs::read(omgaudio.join(p)).ok());
                    if let (Some(large), Some(medium), Some(small)) = (read(big.first()), read(thumbs.first()), read(thumbs.get(1))) {
                        art.insert(name, Art { large, medium, small });
                    }
                }
            }
        }
        // Each song's file (by content ID) and the numbers in its tag.
        let mut by_cid: HashMap<[u8; 12], (String, (u32, u32))> = HashMap::new();
        for dir in std::fs::read_dir(omgaudio).map_err(|e| e.to_string())?.flatten() {
            let d = dir.file_name().to_string_lossy().into_owned();
            if !d.to_ascii_uppercase().starts_with("10F") || !dir.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            for f in std::fs::read_dir(dir.path()).map_err(|e| e.to_string())?.flatten() {
                let n = f.file_name().to_string_lossy().into_owned();
                if n.starts_with('.') || !n.to_ascii_uppercase().ends_with(".OMA") {
                    continue;
                }
                let mut head = vec![0u8; crate::oma::TAG_SIZE + 0x20];
                let read = std::fs::File::open(f.path()).and_then(|mut h| std::io::Read::read_exact(&mut h, &mut head));
                if let (Ok(()), Some(cid)) = (read, crate::oma::content_id(&head)) {
                    by_cid.insert(cid, (format!("{d}/{n}"), crate::oma::numbers(&head)));
                }
            }
        }
        for s in &mut songs {
            if let Some((file, (track, disc))) = by_cid.remove(&s.cid) {
                (s.file, s.track, s.disc) = (Some(file), track, disc);
            }
        }
        let treinf = file("02TREINF.DAT")?;
        let list = file("00GTRLST.DAT")?;
        let (lat, ln, lsize) = section(&list, b"GTLT", 1)?;
        let total_rows = (0..ln).filter(|i| matches!(be16(&list, lat + i * lsize), 0x01..=0x04 | 0x2C)).collect();
        section(&treinf, b"GTIF", 0)?;
        Ok(Database { songs, art, treinf, total_rows })
    }

    /// All table files that depend on the songs, rebuilt: (file name, bytes).
    pub fn tables(&self) -> Vec<(String, Vec<u8>)> {
        let songs = &self.songs;
        let ids: Vec<u16> = (1..=songs.len() as u16).collect();
        let key = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
        let song = |id: u16| &songs[id as usize - 1];
        // Groups of `by`, sorted, each with its songs in `inner` order.
        type Order = (Vec<u16>, Vec<u16>, (bool, u32, u32, u16));
        let grouped = |by: &dyn Fn(&Song) -> &str, inner: &dyn Fn(u16) -> Order| {
            let mut g: BTreeMap<Vec<u16>, (String, Vec<u16>)> = BTreeMap::new();
            for &id in &ids {
                let name = by(song(id));
                g.entry(key(name)).or_insert_with(|| (name.to_string(), Vec::new())).1.push(id);
            }
            g.into_values()
                .map(|(name, mut members)| {
                    members.sort_by_key(|&id| inner(id));
                    (name, members)
                })
                .collect::<Vec<_>>()
        };
        let total = |m: &[u16]| m.iter().map(|&id| song(id).length_ms).sum::<u32>();
        // In an album: by disc and track number, then songs without one, in the order they were added.
        let order = |id: u16| {
            let s = song(id);
            (s.track == 0, s.disc, s.track, id)
        };
        let by_number = |id: u16| (Vec::new(), Vec::new(), order(id));
        let by_album = |id: u16| (key(&song(id).album), Vec::new(), order(id));
        let by_artist_album = |id: u16| (key(&song(id).artist), key(&song(id).album), order(id));

        let mut out = Vec::new();
        // 04CNTINF, 05CIDLST
        let rows: Vec<Vec<u8>> = songs
            .iter()
            .map(|s| {
                let mut r = vec![0u8; 4];
                r.extend_from_slice(&s.codec);
                r.extend_from_slice(&s.length_ms.to_be_bytes());
                r.extend_from_slice(&[0, 5, 0, 0x80]);
                for (id, t) in [(b"TIT2", &s.title), (b"TPE1", &s.artist), (b"TALB", &s.album), (b"TCON", &s.genre)] {
                    r.extend(slot(id, t));
                }
                if let Some(y) = s.year.filter(|y| *y > 0) {
                    r.extend(slot(b"TYER", &y.to_string()));
                }
                r.resize(SONG_ROW, 0);
                r
            })
            .collect();
        out.push(("04CNTINF.DAT".into(), table(b"CNIF", &[(b"CNFB", SONG_ROW, rows, [0; 8], None)])));
        let rows: Vec<Vec<u8>> = songs
            .iter()
            .map(|s| {
                let mut r = vec![0u8; 4];
                r.extend_from_slice(&CID_PREFIX);
                r.extend_from_slice(&s.cid);
                r.resize(CID_ROW, 0);
                r
            })
            .collect();
        out.push(("05CIDLST.DAT".into(), table(b"CIDL", &[(b"CILB", CID_ROW, rows, [0; 8], None)])));

        let pics = |n: usize, prefix: &str| {
            vec![
                slot(b"PICP", &format!("200200;20P{n:02X}\\{prefix}{n:04X}P0.JPG")),
                slot(b"PIC0", &format!("096096;20P{n:02X}\\{prefix}{n:04X}01.JPG|032032;20P{n:02X}\\{prefix}{n:04X}02.JPG")),
            ]
        };
        // 01/03: albums.
        let albums = grouped(&|s| &s.album, &by_number);
        out.push(("01TREE01.DAT".into(), tree(&albums.iter().map(|(_, m)| m.clone()).collect::<Vec<_>>())));
        out.push(("01TREE03.DAT".into(), tree(&albums.iter().map(|(_, m)| m.clone()).collect::<Vec<_>>())));
        let rows = albums
            .iter()
            .enumerate()
            .map(|(i, (name, m))| {
                let first = song(m[0]);
                let mut tags = vec![slot(b"TIT2", name), slot(b"TPE1", &first.artist), slot(b"TCON", &first.genre)];
                if self.art.contains_key(name) {
                    tags.extend(pics(i + 1, "1G"));
                }
                group_row(0x310, total(m), 6, tags)
            })
            .collect();
        out.push(("03GINF01.DAT".into(), groups(0x310, rows)));
        let names =
            |g: &[(String, Vec<u16>)]| g.iter().map(|(n, m)| group_row(0x90, total(m), 1, vec![slot(b"TIT2", n)])).collect::<Vec<_>>();
        out.push(("03GINF03.DAT".into(), groups(0x90, names(&albums))));
        // 02: artists; 04: genres.
        let artists = grouped(&|s| &s.artist, &by_album);
        out.push(("01TREE02.DAT".into(), tree(&artists.iter().map(|(_, m)| m.clone()).collect::<Vec<_>>())));
        out.push(("03GINF02.DAT".into(), groups(0x90, names(&artists))));
        let genres = grouped(&|s| &s.genre, &by_artist_album);
        out.push(("01TREE04.DAT".into(), tree(&genres.iter().map(|(_, m)| m.clone()).collect::<Vec<_>>())));
        out.push(("03GINF04.DAT".into(), groups(0x90, names(&genres))));
        // 2D: each artist, then its albums.
        let mut nodes: Vec<(u8, Vec<u16>)> = Vec::new();
        let mut rows = Vec::new();
        for (artist, members) in &artists {
            nodes.push((1, Vec::new()));
            rows.push(group_row(0x210, 0, 4, vec![slot(b"TIT2", artist), slot(b"XSOT", artist)]));
            let mut by: BTreeMap<Vec<u16>, (String, Vec<u16>)> = BTreeMap::new();
            for &id in members {
                let a = &song(id).album;
                by.entry(key(a)).or_insert_with(|| (a.clone(), Vec::new())).1.push(id);
            }
            for (album, m) in by.into_values() {
                nodes.push((2, m));
                let mut tags = vec![slot(b"TIT2", &album), slot(b"XSOT", &album)];
                if self.art.contains_key(&album) {
                    tags.extend(pics(nodes.len(), "IT"));
                }
                rows.push(group_row(0x210, 0, 4, tags));
            }
        }
        out.push(("01TREE2D.DAT".into(), tree_levels(&nodes)));
        out.push(("03GINF2D.DAT".into(), groups(0x210, rows)));
        // 02TREINF: the total length.
        let mut t = self.treinf.clone();
        let all: u32 = songs.iter().map(|s| s.length_ms).sum();
        if let Ok((at, _, size)) = section(&t, b"GTIF", 0) {
            for &r in &self.total_rows {
                t[at + r * size + 8..][..4].copy_from_slice(&all.to_be_bytes());
            }
        }
        out.push(("02TREINF.DAT".into(), t));
        out
    }

    /// The album pictures the rebuilt tables point at: (path inside OMGAUDIO, bytes).
    pub fn pictures(&self) -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        let mut put = |n: usize, prefix: &str, art: &Art| {
            for (suffix, b) in [("P0", &art.large), ("01", &art.medium), ("02", &art.small)] {
                out.push((format!("20P{n:02X}/{prefix}{n:04X}{suffix}.JPG"), b.clone()));
            }
        };
        let key = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
        let mut albums: Vec<&str> = self.songs.iter().map(|s| s.album.as_str()).collect();
        albums.sort_by_key(|a| key(a));
        albums.dedup();
        for (i, a) in albums.iter().enumerate() {
            if let Some(art) = self.art.get(*a) {
                put(i + 1, "1G", art);
            }
        }
        // 2D numbering: artists and their albums, in order.
        let mut artists: BTreeMap<Vec<u16>, BTreeMap<Vec<u16>, String>> = BTreeMap::new();
        for s in &self.songs {
            artists.entry(key(&s.artist)).or_default().insert(key(&s.album), s.album.clone());
        }
        let mut n = 0;
        for albums in artists.values() {
            n += 1;
            for a in albums.values() {
                n += 1;
                if let Some(art) = self.art.get(a) {
                    put(n, "IT", art);
                }
            }
        }
        out
    }
}

/// A table file from its sections: (name, row size, rows, header bytes 8–15,
/// fixed section length or None for "as long as its rows").
type Section<'a> = (&'a [u8; 4], usize, Vec<Vec<u8>>, [u8; 8], Option<usize>);

fn table(magic: &[u8; 4], sections: &[Section]) -> Vec<u8> {
    let n = sections.len();
    let mut head = magic.to_vec();
    head.extend_from_slice(&[1, 1, 0, 0, n as u8, 0, 0, 0, 0, 0, 0, 0]);
    let mut body = Vec::new();
    let mut offset = 0x10 + 16 * n;
    for (name, size, rows, extra, fixed) in sections {
        let mut s = name.to_vec();
        s.extend_from_slice(&(rows.len() as u16).to_be_bytes());
        s.extend_from_slice(&(*size as u16).to_be_bytes());
        s.extend_from_slice(extra);
        for r in rows {
            s.extend_from_slice(r);
        }
        let len = fixed.unwrap_or(s.len()).max(s.len());
        s.resize(len, 0);
        head.extend_from_slice(*name);
        head.extend_from_slice(&(offset as u32).to_be_bytes());
        head.extend_from_slice(&(len as u32).to_be_bytes());
        head.extend_from_slice(&[0; 4]);
        offset += len;
        body.extend(s);
    }
    head.extend(body);
    head.resize(head.len().div_ceil(16) * 16, 0);
    head
}

fn group_row(size: usize, total_ms: u32, slots: u16, tags: Vec<Vec<u8>>) -> Vec<u8> {
    let mut r = vec![0u8; 8];
    r.extend_from_slice(&total_ms.to_be_bytes());
    r.extend_from_slice(&slots.to_be_bytes());
    r.extend_from_slice(&(SLOT as u16).to_be_bytes());
    for t in tags {
        r.extend(t);
    }
    r.resize(size, 0);
    r
}

fn groups(size: usize, rows: Vec<Vec<u8>>) -> Vec<u8> {
    table(b"GPIF", &[(b"GPFB", size, rows, [0; 8], None)])
}

/// A one-level tree: each group's songs, in order.
fn tree(groups: &[Vec<u16>]) -> Vec<u8> {
    tree_levels(&groups.iter().map(|m| (1, m.clone())).collect::<Vec<_>>())
}

/// A tree of (level, songs) nodes; a node without songs (an artist above its
/// albums) points at song 0.
fn tree_levels(nodes: &[(u8, Vec<u16>)]) -> Vec<u8> {
    let mut order: Vec<u16> = Vec::new();
    let rows: Vec<Vec<u8>> = nodes
        .iter()
        .enumerate()
        .map(|(i, (level, songs))| {
            let start = if songs.is_empty() { 0 } else { order.len() as u16 + 1 };
            order.extend(songs);
            let mut r = ((i + 1) as u16).to_be_bytes().to_vec();
            r.extend_from_slice(&[*level, 0]);
            r.extend_from_slice(&start.to_be_bytes());
            r.extend_from_slice(&[0, 0]);
            r
        })
        .collect();
    let count = |n: usize| {
        let mut e = [0u8; 8];
        e[..4].copy_from_slice(&(n as u32).to_be_bytes());
        e
    };
    let songs = order.iter().map(|id| id.to_be_bytes().to_vec()).collect::<Vec<_>>();
    table(b"TREE", &[(b"GPLB", 8, rows, count(nodes.len()), Some(0x4010)), (b"TPLB", 2, songs, count(order.len()), None)])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(title: &str, artist: &str, album: &str, genre: &str, ms: u32) -> Song {
        Song { title: title.into(), artist: artist.into(), album: album.into(), genre: genre.into(), length_ms: ms, ..Default::default() }
    }

    /// A database with no songs, as on a Walkman just set up (only the
    /// table files the rebuild reads).
    fn empty(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("04CNTINF.DAT"), table(b"CNIF", &[(b"CNFB", SONG_ROW, vec![], [0; 8], None)])).unwrap();
        std::fs::write(dir.join("05CIDLST.DAT"), table(b"CIDL", &[(b"CILB", CID_ROW, vec![], [0; 8], None)])).unwrap();
        let list_row = |tree: u16| {
            let mut r = tree.to_be_bytes().to_vec();
            r.resize(0x50, 0);
            r
        };
        std::fs::write(
            dir.join("00GTRLST.DAT"),
            table(
                b"GTLT",
                &[
                    (b"SYSB", 0x50, vec![vec![0; 0x50]], [0; 8], None),
                    (b"GTLB", 0x50, [1, 2, 3, 4, 0x2D, 0x2C].map(list_row).to_vec(), [0; 8], None),
                ],
            ),
        )
        .unwrap();
        std::fs::write(dir.join("02TREINF.DAT"), table(b"GTIF", &[(b"GTFB", 0x90, vec![vec![0; 0x90]; 6], [0; 8], None)])).unwrap();
    }

    #[test]
    fn rebuilds_and_reads_back() {
        let dir = std::env::temp_dir().join(format!("chinotto-omg-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        empty(&dir);
        let mut db = Database::read(&dir).unwrap();
        assert!(db.songs.is_empty());
        db.songs = vec![
            song("b", "Zed", "Second", "Rock", 1000),
            song("a", "Abe", "First", "Pop", 2000),
            Song { year: Some(1995), ..song("c", "Zed", "First", "Rock", 3000) },
        ];
        for (i, s) in db.songs.iter_mut().enumerate() {
            s.cid = [i as u8 + 1; 12];
        }
        for (name, bytes) in db.tables() {
            std::fs::write(dir.join(name), bytes).unwrap();
        }
        let back = Database::read(&dir).unwrap();
        assert_eq!(back.songs, db.songs);
        let t = |name: &str| std::fs::read(dir.join(name)).unwrap();
        // Albums: First (songs 2, 3), Second (1).
        let albums = t("01TREE01.DAT");
        let (at, n, _) = section(&albums, b"TREE", 1).unwrap();
        assert_eq!(n, 3);
        assert_eq!(&albums[at..at + 6], &[0, 2, 0, 3, 0, 1]);
        // Artists: Abe (2), Zed (album First: 3, then Second: 1).
        let artists = t("01TREE02.DAT");
        let (at, _, _) = section(&artists, b"TREE", 1).unwrap();
        assert_eq!(&artists[at..at + 6], &[0, 2, 0, 3, 0, 1]);
        // 2D: Abe, First, Zed, First, Second.
        let tree = t("01TREE2D.DAT");
        let (at, n, _) = section(&tree, b"TREE", 0).unwrap();
        assert_eq!(n, 5);
        let starts: Vec<usize> = (0..n).map(|i| be16(&tree, at + i * 8 + 4)).collect();
        assert_eq!(starts, [0, 1, 0, 2, 3]);
        // Totals in 02TREINF rows of trees 01–04 and 2C, not 2D.
        let inf = t("02TREINF.DAT");
        let (at, _, size) = section(&inf, b"GTIF", 0).unwrap();
        let totals: Vec<u32> = (0..6).map(|r| be32(&inf, at + r * size + 8)).collect();
        assert_eq!(totals, [6000, 6000, 6000, 6000, 0, 6000]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn albums_go_in_track_order() {
        let dir = std::env::temp_dir().join(format!("chinotto-omg-order-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        empty(&dir);
        let mut db = Database::read(&dir).unwrap();
        // Added as: track 4, unnumbered, track 1, disc 2 track 1, track 3.
        db.songs = vec![
            Song { track: 4, ..song("Ironic", "A", "J", "R", 1) },
            song("Bonus", "A", "J", "R", 1),
            Song { track: 1, ..song("All I Really Want", "A", "J", "R", 1) },
            Song { track: 1, disc: 2, ..song("Disc two", "A", "J", "R", 1) },
            Song { track: 3, ..song("Head Over Feet", "A", "J", "R", 1) },
        ];
        let tables: HashMap<String, Vec<u8>> = db.tables().into_iter().collect();
        for name in ["01TREE01.DAT", "01TREE02.DAT", "01TREE04.DAT", "01TREE2D.DAT"] {
            let t = &tables[name];
            let (at, n, _) = section(t, b"TREE", 1).unwrap();
            let order: Vec<usize> = (0..n).map(|i| be16(t, at + i * 2)).collect();
            assert_eq!(order, [3, 5, 1, 4, 2], "{name}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The tables rebuilt from a real Walkman's songs are its own tables.
    /// Set CHINOTTO_TEST_OMG to a copy of an OMGAUDIO folder (never committed).
    #[test]
    fn rebuilds_device_tables() {
        let Ok(dir) = std::env::var("CHINOTTO_TEST_OMG") else { return };
        let dir = Path::new(&dir);
        let db = Database::read(dir).unwrap();
        for (name, bytes) in db.tables() {
            assert!(std::fs::read(dir.join(&name)).unwrap() == bytes, "{name} differs");
        }
        for (path, bytes) in db.pictures() {
            assert!(std::fs::read(dir.join(&path)).unwrap() == bytes, "{path} differs");
        }
    }
}
