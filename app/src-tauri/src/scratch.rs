//! The Scratch list: songs opened from outside the libraries (double-clicked
//! in Finder, "Open With", dropped on the window, Open File…). They play, but
//! are never added to a library, and the list is forgotten when Chinotto quits.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::library::{self, Tags};
use crate::state::AppState;
use crate::tags;

pub struct ScratchTrack {
    id: u32,
    path: String,
    size: u64,
    mtime: i64,
    cover: bool,
    tags: Tags,
}

#[derive(Default)]
pub struct Scratch {
    next: u32,
    tracks: Vec<ScratchTrack>,
    /// Song to start playing once the window is ready (opened at launch).
    pending_play: Option<u32>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Changed {
    /// Play this song now (the first of the files just opened).
    play: Option<u32>,
}

/// Music files in `paths` (folders are looked through).
fn music_files(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for p in paths {
        if p.is_dir() {
            let mut kids: Vec<PathBuf> = std::fs::read_dir(p).into_iter().flatten().flatten().map(|e| e.path()).collect();
            kids.sort();
            kids.retain(|k| !k.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')));
            out.extend(music_files(&kids));
        } else if tags::library_kind(p).is_some() {
            out.push(p.clone());
        }
    }
    out
}

/// Add files to the Scratch list (a file already there isn't added twice).
/// Returns the id of the first one, to play.
pub fn add(app: &AppHandle, paths: &[PathBuf]) -> Option<u32> {
    let files = music_files(paths);
    if files.is_empty() {
        return None;
    }
    let mut covers = HashMap::new();
    let state = app.state::<AppState>();
    let mut s = state.scratch.lock().unwrap();
    let mut first = None;
    for f in files {
        let path = f.to_string_lossy().into_owned();
        let id = if let Some(t) = s.tracks.iter().find(|t| t.path == path) {
            t.id
        } else {
            s.next += 1;
            let id = s.next;
            let (tags, cover) = library::read_track(&f, &mut covers);
            let m = std::fs::metadata(&f).ok();
            let mtime = m
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            s.tracks.push(ScratchTrack { id, path, size: m.map(|m| m.len()).unwrap_or(0), mtime, cover, tags });
            id
        };
        first.get_or_insert(id);
    }
    s.pending_play = first;
    drop(s);
    let _ = app.emit("scratch-changed", Changed { play: first });
    first
}

/// The file of a Scratch song, from its `scratch:<n>` id.
pub fn track_file(app: &AppHandle, dbid: &str) -> Result<PathBuf, String> {
    let id: u32 = dbid.strip_prefix("scratch:").and_then(|n| n.parse().ok()).ok_or("bad id")?;
    let state = app.state::<AppState>();
    let s = state.scratch.lock().unwrap();
    let t = s.tracks.iter().find(|t| t.id == id).ok_or("That song isn't in the Scratch list any more")?;
    let p = PathBuf::from(&t.path);
    if !p.is_file() {
        return Err(format!("The file isn't there any more ({})", p.display()));
    }
    Ok(p)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TrackView<'a> {
    id: u32,
    dbid: String,
    #[serde(flatten)]
    tags: &'a Tags,
    location: &'a str,
    size: u64,
    rating: u8,
    play_count: u32,
    media_type: u32,
    modified: i64,
    art: u32,
    has_cover: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct View<'a> {
    tracks: Vec<TrackView<'a>>,
    /// Set once, at launch: the song opened with Chinotto, to play.
    play: Option<u32>,
}

#[tauri::command]
pub fn scratch_view(state: State<'_, AppState>) -> Result<tauri::ipc::Response, String> {
    let mut s = state.scratch.lock().unwrap();
    let play = s.pending_play.take();
    let v = View {
        tracks: s
            .tracks
            .iter()
            .map(|t| TrackView {
                id: t.id,
                dbid: format!("scratch:{}", t.id),
                tags: &t.tags,
                location: &t.path,
                size: t.size,
                rating: 0,
                play_count: 0,
                media_type: 1,
                modified: t.mtime,
                art: 0,
                has_cover: t.cover,
            })
            .collect(),
        play,
    };
    serde_json::to_string(&v).map(tauri::ipc::Response::new).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn scratch_add(paths: Vec<String>, app: AppHandle) {
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    add(&app, &paths);
}

/// Take songs out of the Scratch list (`ids` empty: all of them). Files aren't touched.
#[tauri::command]
pub fn scratch_remove(ids: Vec<u32>, app: AppHandle) {
    {
        let state = app.state::<AppState>();
        let mut s = state.scratch.lock().unwrap();
        if ids.is_empty() {
            s.tracks.clear();
        } else {
            s.tracks.retain(|t| !ids.contains(&t.id));
        }
        s.pending_play = None;
    }
    let _ = app.emit("scratch-changed", Changed { play: None });
}

/// Files given on the command line (Linux "Open With", a second launch).
pub fn files_in_args(args: &[String]) -> Vec<PathBuf> {
    args.iter().skip(1).filter(|a| !a.starts_with('-')).map(PathBuf::from).filter(|p| Path::new(p).is_file()).collect()
}
