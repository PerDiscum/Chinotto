//! Music libraries on the computer: named sets of music folders whose songs
//! Chinotto lists, browses and plays. Music files are only ever read.
//!
//! Each library is one JSON file in the app's data folder
//! (`libraries/<id>.json`), written whole (temporary file, then rename).
//!
//! Missing files: a song is removed only when its folder is reachable and the
//! file is gone. If the folder itself isn't there (its drive is unplugged),
//! its songs stay, marked unavailable.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::tags;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Tags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub composer: Option<String>,
    pub comment: Option<String>,
    pub grouping: Option<String>,
    pub sort_title: Option<String>,
    pub sort_artist: Option<String>,
    pub sort_album: Option<String>,
    pub sort_album_artist: Option<String>,
    pub sort_composer: Option<String>,
    pub year: u32,
    pub track_nr: u32,
    pub track_count: u32,
    pub disc_nr: u32,
    pub disc_count: u32,
    pub compilation: bool,
    pub bpm: u32,
    pub length_ms: u32,
    pub bitrate: u32,
    pub sample_rate: u32,
    pub kind: String,
}

impl Tags {
    fn from_info(t: chinotto_gpod::TrackInfo, kind: &str) -> Self {
        let u = |v: i32| v.max(0) as u32;
        Tags {
            title: t.title,
            artist: t.artist,
            album_artist: t.album_artist,
            album: t.album,
            genre: t.genre,
            composer: t.composer,
            comment: t.comment,
            grouping: t.grouping,
            sort_title: t.sort_title,
            sort_artist: t.sort_artist,
            sort_album: t.sort_album,
            sort_album_artist: t.sort_album_artist,
            sort_composer: t.sort_composer,
            year: u(t.year),
            track_nr: u(t.track_nr),
            track_count: u(t.track_count),
            disc_nr: u(t.disc_nr),
            disc_count: u(t.disc_count),
            compilation: t.compilation,
            bpm: t.bpm.max(0) as u32,
            length_ms: u(t.length_ms),
            bitrate: u(t.bitrate_kbps),
            sample_rate: t.sample_rate,
            kind: t.filetype.unwrap_or_else(|| kind.to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibTrack {
    pub id: u32,
    pub path: String,
    pub size: u64,
    /// File modification time (Unix seconds); with `size`, tells whether to re-read tags.
    pub mtime: i64,
    pub added: i64,
    #[serde(default)]
    pub play_count: u32,
    #[serde(default)]
    pub rating: u8,
    #[serde(default)]
    pub played: Option<i64>,
    /// Has an embedded picture, or a cover image in its folder.
    #[serde(default)]
    pub cover: bool,
    pub tags: Tags,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LibPlaylist {
    pub id: u32,
    pub name: String,
    pub tracks: Vec<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Library {
    pub id: String,
    pub name: String,
    pub folders: Vec<String>,
    pub next_id: u32,
    pub tracks: Vec<LibTrack>,
    pub last_scan: Option<i64>,
    pub playlists: Vec<LibPlaylist>,
    pub next_playlist_id: u32,
    /// Files taken out of the library by hand: scans skip them.
    pub excluded: Vec<String>,
    /// Set for the songs of an MSC player (see msc.rs): its settings' id.
    pub device: Option<String>,
    /// MSC player: the folder its playlist files are read from.
    pub playlist_dir: Option<String>,
    /// Never saved (an MTP device's songs, read from it while it's open).
    #[serde(skip)]
    pub ephemeral: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FolderState {
    pub path: String,
    /// The folder is there (its drive is connected).
    pub available: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub id: String,
    pub name: String,
    pub folders: Vec<FolderState>,
    pub songs: usize,
    pub last_scan: Option<i64>,
    /// An MSC player's songs, shown under Devices rather than Libraries.
    pub device: Option<String>,
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn mtime_of(m: &std::fs::Metadata) -> i64 {
    m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0)
}

pub fn folder_available(path: &str) -> bool {
    Path::new(path).is_dir()
}

impl Library {
    pub fn new(name: &str, folders: Vec<String>) -> Self {
        let id = format!("{:x}{:04x}", now(), std::process::id() & 0xffff);
        Library { id, name: name.to_string(), folders: clean_folders(folders), next_id: 1, ..Default::default() }
    }

    pub fn summary(&self) -> Summary {
        Summary {
            id: self.id.clone(),
            name: self.name.clone(),
            folders: self.folders.iter().map(|f| FolderState { path: f.clone(), available: folder_available(f) }).collect(),
            songs: self.tracks.len(),
            last_scan: self.last_scan,
            device: self.device.clone(),
        }
    }

    /// The library folder holding `path` (the longest one that contains it).
    pub fn folder_of(&self, path: &str) -> Option<&str> {
        self.folders.iter().filter(|f| Path::new(path).starts_with(f)).max_by_key(|f| f.len()).map(String::as_str)
    }

    pub fn track(&self, id: u32) -> Option<&LibTrack> {
        self.tracks.iter().find(|t| t.id == id)
    }

    /// Drop playlist entries whose song is gone.
    pub fn prune_playlists(&mut self) {
        let ids: HashSet<u32> = self.tracks.iter().map(|t| t.id).collect();
        for p in &mut self.playlists {
            p.tracks.retain(|t| ids.contains(t));
        }
    }

    pub fn new_playlist(&mut self, name: &str, tracks: Vec<u32>) -> u32 {
        self.next_playlist_id = self.next_playlist_id.max(1);
        let id = self.next_playlist_id;
        self.next_playlist_id += 1;
        self.playlists.push(LibPlaylist { id, name: name.to_string(), tracks });
        self.prune_playlists();
        id
    }

    /// Take songs out of the library (their files stay; later scans skip them).
    pub fn remove_tracks(&mut self, ids: &HashSet<u32>) -> usize {
        let before = self.tracks.len();
        let mut gone = Vec::new();
        self.tracks.retain(|t| {
            let keep = !ids.contains(&t.id);
            if !keep {
                gone.push(t.path.clone());
            }
            keep
        });
        self.excluded.extend(gone);
        self.excluded.sort();
        self.excluded.dedup();
        self.prune_playlists();
        before - self.tracks.len()
    }

    /// Re-read one song's file (after its tags were written).
    pub fn reread(&mut self, id: u32) {
        let Some(t) = self.tracks.iter_mut().find(|t| t.id == id) else { return };
        let p = PathBuf::from(&t.path);
        if let Ok(m) = std::fs::metadata(&p) {
            t.size = m.len();
            t.mtime = mtime_of(&m);
        }
        let (tags, cover) = read_track(&p, &mut HashMap::new());
        t.tags = tags;
        t.cover = cover;
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("{}.json", self.id));
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec(self).map_err(|e| e.to_string())?)
            .map_err(|e| format!("Couldn't save the library: {e}"))?;
        std::fs::rename(&tmp, &path).map_err(|e| format!("Couldn't save the library: {e}"))
    }

    /// Drop songs whose folder isn't there (only when the user asks).
    pub fn remove_unavailable(&mut self) -> usize {
        let gone: HashSet<&str> = self.folders.iter().filter(|f| !folder_available(f)).map(String::as_str).collect();
        let before = self.tracks.len();
        let folders = self.folders.clone();
        self.tracks.retain(|t| {
            let f = folders.iter().filter(|f| Path::new(&t.path).starts_with(f)).max_by_key(|f| f.len());
            // Songs outside every folder (a folder was taken out of the library) go too.
            f.is_some_and(|f| !gone.contains(f.as_str()))
        });
        let n = before - self.tracks.len();
        self.prune_playlists();
        n
    }
}

/// Absolute, without duplicates or folders inside another listed one.
fn clean_folders(folders: Vec<String>) -> Vec<String> {
    let mut v: Vec<String> = folders.into_iter().map(|f| f.trim_end_matches('/').to_string()).filter(|f| !f.is_empty()).collect();
    v.sort();
    v.dedup();
    let all = v.clone();
    v.retain(|f| !all.iter().any(|o| o != f && Path::new(f).starts_with(o)));
    v
}

pub fn set_folders(lib: &mut Library, folders: Vec<String>) {
    lib.folders = clean_folders(folders);
}

/// What a scan found (for the progress report and the log).
#[derive(Debug, Default, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    /// Folders that weren't there (their songs were kept).
    pub unavailable_folders: usize,
}

/// Files found in a folder: (path, size, mtime).
fn walk(dir: &Path, out: &mut Vec<(PathBuf, u64, i64)>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let name = e.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue; // hidden files, including macOS "._" companions
        }
        let p = e.path();
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_dir() {
            walk(&p, out);
        } else if ft.is_file() && tags::library_kind(&p).is_some() {
            if let Ok(m) = e.metadata() {
                out.push((p, m.len(), mtime_of(&m)));
            }
        }
    }
}

/// A cover image beside the music (cover.jpg, folder.png, front.jpg…).
pub fn folder_cover(dir: &Path) -> Option<PathBuf> {
    const NAMES: [&str; 4] = ["cover", "folder", "front", "album"];
    let rd = std::fs::read_dir(dir).ok()?;
    let mut found: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
            let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
            NAMES.contains(&stem.as_str()) && matches!(ext.as_str(), "jpg" | "jpeg" | "png")
        })
        .collect();
    found.sort_by_key(|p| NAMES.iter().position(|n| p.file_stem().and_then(|s| s.to_str()).is_some_and(|s| s.eq_ignore_ascii_case(n))));
    found.into_iter().next()
}

pub(crate) fn read_track(path: &Path, cover_dirs: &mut HashMap<PathBuf, bool>) -> (Tags, bool) {
    let kind = tags::library_kind(path).unwrap_or("Audio file");
    let (tags, picture) = match tags::read_tags(path, kind) {
        Ok((info, pic)) => (Tags::from_info(info, kind), pic),
        // Unreadable tags (or a format lofty doesn't know, like WMA): list it by file name.
        Err(_) => (
            Tags { title: path.file_stem().map(|s| s.to_string_lossy().into_owned()), kind: kind.to_string(), ..Default::default() },
            false,
        ),
    };
    let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let in_folder = *cover_dirs.entry(dir.clone()).or_insert_with(|| folder_cover(&dir).is_some());
    (tags, picture || in_folder)
}

/// Bring `lib` up to date with its folders. Only new and changed files (by
/// size and modification time) have their tags read, or every file when
/// `full`. `progress(done, total, current)` is called as files are read.
pub fn scan(lib: &mut Library, full: bool, mut progress: impl FnMut(usize, usize, &str)) -> ScanResult {
    let mut res = ScanResult::default();
    let mut found: Vec<(PathBuf, u64, i64)> = Vec::new();
    let mut reachable: Vec<String> = Vec::new();
    for f in &lib.folders {
        if folder_available(f) {
            reachable.push(f.clone());
            walk(Path::new(f), &mut found);
        } else {
            res.unavailable_folders += 1;
        }
    }

    if !lib.excluded.is_empty() {
        let ex: HashSet<&str> = lib.excluded.iter().map(String::as_str).collect();
        found.retain(|(p, _, _)| !ex.contains(&*p.to_string_lossy()));
    }
    let by_path: HashMap<String, usize> = lib.tracks.iter().enumerate().map(|(i, t)| (t.path.clone(), i)).collect();
    let mut seen = vec![false; lib.tracks.len()];
    let todo: Vec<&(PathBuf, u64, i64)> = found
        .iter()
        .filter(|(p, size, mtime)| match by_path.get(&*p.to_string_lossy()) {
            Some(&i) => {
                seen[i] = true;
                let t = &lib.tracks[i];
                full || t.size != *size || t.mtime != *mtime
            }
            None => true,
        })
        .collect();

    let total = todo.len();
    let mut cover_dirs = HashMap::new();
    let mut new_tracks = Vec::new();
    for (n, (p, size, mtime)) in todo.into_iter().enumerate() {
        let path = p.to_string_lossy().into_owned();
        progress(n, total, p.file_name().and_then(|s| s.to_str()).unwrap_or(""));
        let (tags, cover) = read_track(p, &mut cover_dirs);
        // Gone while it was read (a player unplugged mid-scan): no time, so
        // the next scan reads it again rather than keeping these tags.
        let mtime = if p.exists() { *mtime } else { 0 };
        if let Some(&i) = by_path.get(&path) {
            let t = &mut lib.tracks[i];
            t.size = *size;
            t.mtime = mtime;
            t.tags = tags;
            t.cover = cover;
            res.updated += 1;
        } else {
            new_tracks.push(LibTrack {
                id: lib.next_id,
                path,
                size: *size,
                mtime,
                added: now(),
                play_count: 0,
                rating: 0,
                played: None,
                cover,
                tags,
            });
            lib.next_id += 1;
            res.added += 1;
        }
    }
    progress(total, total, "");

    // Gone: in a reachable folder and not found there. Songs of unreachable
    // folders (unplugged drive) are kept.
    let mut i = 0;
    lib.tracks.retain(|t| {
        let keep = seen[i] || (!reachable.iter().any(|f| Path::new(&t.path).starts_with(f)) && lib_folder_listed(&lib.folders, &t.path));
        i += 1;
        if !keep {
            res.removed += 1;
        }
        keep
    });
    lib.tracks.extend(new_tracks);
    lib.prune_playlists();
    lib.last_scan = Some(now());
    res
}

/// Songs outside every listed folder (the folder was taken out of the
/// library) are dropped at the next scan.
fn lib_folder_listed(folders: &[String], path: &str) -> bool {
    folders.iter().any(|f| Path::new(path).starts_with(f))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("chinotto-lib-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn keeps_songs_of_unplugged_folders_and_drops_deleted_files() {
        let a = tmp("a");
        let b = tmp("b");
        std::fs::write(a.join("one.mp3"), b"not really audio").unwrap();
        std::fs::write(a.join("two.flac"), b"nor this").unwrap();
        std::fs::write(b.join("three.wma"), b"x").unwrap();
        std::fs::write(b.join("notes.txt"), b"ignored").unwrap();
        let mut lib = Library::new("Test", vec![a.to_string_lossy().into(), b.to_string_lossy().into()]);
        let r = scan(&mut lib, false, |_, _, _| {});
        assert_eq!((r.added, lib.tracks.len()), (3, 3));
        // Unreadable files are listed by name.
        assert!(lib.tracks.iter().any(|t| t.tags.title.as_deref() == Some("three") && t.tags.kind == "Windows Media audio file"));

        // Unchanged files aren't read again.
        let r = scan(&mut lib, false, |_, _, _| {});
        assert_eq!((r.added, r.updated, r.removed), (0, 0, 0));

        // Folder b "unplugged": its song stays.
        let b_moved = b.with_extension("away");
        std::fs::rename(&b, &b_moved).unwrap();
        let r = scan(&mut lib, false, |_, _, _| {});
        assert_eq!((r.removed, r.unavailable_folders, lib.tracks.len()), (0, 1, 3));

        // A file deleted from a reachable folder goes.
        std::fs::remove_file(a.join("two.flac")).unwrap();
        let r = scan(&mut lib, false, |_, _, _| {});
        assert_eq!((r.removed, lib.tracks.len()), (1, 2));

        // Asked to: songs of the missing folder go.
        assert_eq!(lib.remove_unavailable(), 1);
        assert_eq!(lib.tracks.len(), 1);
        let _ = std::fs::remove_dir_all(&a);
        let _ = std::fs::remove_dir_all(&b_moved);
    }

    #[test]
    fn folders_are_cleaned() {
        let v = clean_folders(vec!["/m/a/".into(), "/m/a/sub".into(), "/m/b".into(), "/m/b".into()]);
        assert_eq!(v, vec!["/m/a".to_string(), "/m/b".to_string()]);
    }
}
