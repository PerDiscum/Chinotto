//! Commands that change the iPod. Each one:
//! 1. checks the user allowed changes to this iPod,
//! 2. applies the change to libgpod's copy of the database,
//! 3. saves (chinotto-gpod backs up the old iTunesDB first),
//! 4. re-reads the iPod and returns the fresh view to the window.
//!
//! If anything fails, libgpod's in-memory copy is thrown away, so the next
//! change starts again from what's actually on the iPod.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chinotto_gpod::{Database, Field};
use serde::Serialize;
use serde_json::Value;
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, Manager};

use crate::commands::view;
use crate::state::AppState;
use crate::tags;

fn parse_ids(ids: &[String]) -> Result<Vec<u64>, String> {
    ids.iter().map(|s| s.parse::<u64>().map_err(|_| format!("bad id {s}"))).collect()
}

/// Run `f` against the writer for the iPod at `root`, save, and return the new view
/// (plus anything `f` wants to report, as `extra`).
fn with_writer<T: Serialize>(app: &AppHandle, root: &Path, f: impl FnOnce(&mut Database) -> Result<T, String>) -> Result<Response, String> {
    let state = app.state::<AppState>();
    let session = state.session().filter(|s| s.root == root).ok_or("That iPod isn't open")?;
    let identity =
        session.sysinfo.identity().ok_or("This iPod doesn't report a FireWire ID or serial number, so Chinotto can't change it")?;
    if !state.is_writable(&identity) {
        return Err("Changes to this iPod are switched off. Turn on “Allow changes” on its page first.".into());
    }

    let mut guard = state.writer.lock().unwrap();
    if guard.as_ref().is_none_or(|w| w.root() != root) {
        *guard = Some(Database::open(root).map_err(|e| e.to_string())?);
    }
    let db = guard.as_mut().unwrap();
    let result = f(db).and_then(|extra| db.save().map(|_| extra).map_err(|e| e.to_string()));
    let extra = match result {
        Ok(x) => x,
        Err(e) => {
            // Discard libgpod's copy; it may hold changes that weren't saved.
            *guard = None;
            return Err(e);
        }
    };
    drop(guard);

    let fresh = state.open(root)?;
    let view = view(&fresh, &state)?;
    let extra = serde_json::to_string(&extra).map_err(|e| e.to_string())?;
    Ok(Response::new(format!("{{\"view\":{view},\"extra\":{extra}}}")))
}

async fn blocking(app: AppHandle, f: impl FnOnce(AppHandle) -> Result<Response, String> + Send + 'static) -> Result<Response, String> {
    tauri::async_runtime::spawn_blocking(move || f(app)).await.map_err(|e| e.to_string())?
}

/// Turn "Allow changes" on or off for the open iPod.
#[tauri::command]
pub fn set_allow_changes(path: String, allow: bool, app: AppHandle) -> Result<Response, String> {
    let state = app.state::<AppState>();
    let session = state.session().filter(|s| s.root == Path::new(&path)).ok_or("That iPod isn't open")?;
    let identity =
        session.sysinfo.identity().ok_or("This iPod doesn't report a FireWire ID or serial number, so Chinotto can't change it")?;
    state.set_writable(&identity, allow)?;
    if !allow {
        *state.writer.lock().unwrap() = None;
    }
    view(&session, &state).map(Response::new)
}

/// Fields from the Get Info window: only the keys present are changed.
/// Strings: text, or null/"" to clear. Rating is in stars (0–5).
fn fields(changes: &HashMap<String, Value>) -> Result<Vec<Field>, String> {
    let text = |v: &Value| v.as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let int = |k: &str, v: &Value| -> Result<i32, String> {
        match v {
            Value::Null => Ok(0),
            Value::Number(n) => n.as_i64().map(|n| n.clamp(0, 9999) as i32).ok_or(format!("{k}: not a whole number")),
            Value::String(s) if s.trim().is_empty() => Ok(0),
            Value::String(s) => s.trim().parse().map_err(|_| format!("{k}: not a number")),
            _ => Err(format!("{k}: not a number")),
        }
    };
    let mut out = Vec::new();
    for (k, v) in changes {
        out.push(match k.as_str() {
            "title" => Field::Title(text(v)),
            "artist" => Field::Artist(text(v)),
            "albumArtist" => Field::AlbumArtist(text(v)),
            "album" => Field::Album(text(v)),
            "genre" => Field::Genre(text(v)),
            "composer" => Field::Composer(text(v)),
            "comment" => Field::Comment(text(v)),
            "grouping" => Field::Grouping(text(v)),
            "sortTitle" => Field::SortTitle(text(v)),
            "sortArtist" => Field::SortArtist(text(v)),
            "sortAlbum" => Field::SortAlbum(text(v)),
            "sortAlbumArtist" => Field::SortAlbumArtist(text(v)),
            "sortComposer" => Field::SortComposer(text(v)),
            "year" => Field::Year(int(k, v)?),
            "trackNr" => Field::TrackNr(int(k, v)?),
            "trackCount" => Field::TrackCount(int(k, v)?),
            "discNr" => Field::DiscNr(int(k, v)?),
            "discCount" => Field::DiscCount(int(k, v)?),
            "bpm" => Field::Bpm(int(k, v)?.min(i16::MAX as i32) as i16),
            "rating" => Field::Rating((int(k, v)?.clamp(0, 5) * 20) as u32),
            "compilation" => Field::Compilation(v.as_bool().ok_or("compilation: not true/false")?),
            other => return Err(format!("unknown field {other}")),
        });
    }
    Ok(out)
}

#[tauri::command]
pub async fn edit_tracks(path: String, ids: Vec<String>, changes: HashMap<String, Value>, app: AppHandle) -> Result<Response, String> {
    let ids = parse_ids(&ids)?;
    let fields = fields(&changes)?;
    blocking(app, move |app| with_writer(&app, Path::new(&path), |db| db.edit(&ids, &fields).map_err(|e| e.to_string()))).await
}

/// Remove songs from the iPod (library and all playlists); their files are deleted after the save.
#[tauri::command]
pub async fn delete_tracks(path: String, ids: Vec<String>, app: AppHandle) -> Result<Response, String> {
    let ids = parse_ids(&ids)?;
    blocking(app, move |app| with_writer(&app, Path::new(&path), |db| db.remove_tracks(&ids).map_err(|e| e.to_string()))).await
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AddProgress {
    done: usize,
    total: usize,
    current: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AddResult {
    added: Vec<String>,
    /// Files that weren't added, with the reason.
    skipped: Vec<String>,
}

/// Every audio file in `paths`, looking inside folders.
fn expand(paths: &[String]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack: Vec<PathBuf> = paths.iter().rev().map(PathBuf::from).collect();
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            let mut kids: Vec<PathBuf> = std::fs::read_dir(&p).into_iter().flatten().flatten().map(|e| e.path()).collect();
            kids.sort();
            kids.retain(|k| !k.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')));
            stack.extend(kids.into_iter().rev());
        } else {
            out.push(p);
        }
    }
    out
}

/// Copy music files (or folders of them) onto the iPod.
#[tauri::command]
pub async fn add_files(path: String, files: Vec<String>, playlist: Option<String>, app: AppHandle) -> Result<Response, String> {
    let playlist = playlist.map(|p| p.parse::<u64>().map_err(|_| "bad playlist id".to_string())).transpose()?;
    blocking(app, move |app| {
        let files = expand(&files);
        let emitter = app.clone();
        with_writer(&app, Path::new(&path), move |db| {
            let mut res = AddResult { added: Vec::new(), skipped: Vec::new() };
            let total = files.len();
            for (i, f) in files.iter().enumerate() {
                let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let _ = emitter.emit("add-progress", AddProgress { done: i, total, current: name.clone() });
                if tags::ipod_kind(f).is_none() {
                    // Silently skip cover images, playlists etc. inside folders; report audio we can't take.
                    if is_audio_ext(f) {
                        res.skipped.push(format!("{name}: this kind of file doesn't play on an iPod yet (it would need converting)"));
                    }
                    continue;
                }
                match tags::read(f).and_then(|info| db.add_track(f, &info).map_err(|e| e.to_string())) {
                    Ok(id) => res.added.push(id.to_string()),
                    Err(e) => res.skipped.push(e),
                }
            }
            let _ = emitter.emit("add-progress", AddProgress { done: total, total, current: String::new() });
            if let Some(pl) = playlist {
                let mut ids = db.playlists().into_iter().find(|p| p.id == pl).map(|p| p.tracks).unwrap_or_default();
                ids.extend(res.added.iter().filter_map(|s| s.parse::<u64>().ok()));
                db.set_playlist_tracks(pl, &ids).map_err(|e| e.to_string())?;
            }
            if res.added.is_empty() && !res.skipped.is_empty() {
                return Err(res.skipped.join("\n"));
            }
            Ok(res)
        })
    })
    .await
}

fn is_audio_ext(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        matches!(
            e.to_ascii_lowercase().as_str(),
            "flac" | "ogg" | "oga" | "opus" | "wma" | "ape" | "wv" | "alac" | "mp2" | "m4b" | "m4p" | "dsf" | "dff"
        )
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Created {
    id: String,
}

/// New playlist, optionally filled with songs. Returns its id in `extra`.
#[tauri::command]
pub async fn create_playlist(path: String, name: String, ids: Vec<String>, app: AppHandle) -> Result<Response, String> {
    let ids = parse_ids(&ids)?;
    blocking(app, move |app| {
        with_writer(&app, Path::new(&path), |db| {
            let id = db.create_playlist(&name).map_err(|e| e.to_string())?;
            if !ids.is_empty() {
                db.set_playlist_tracks(id, &ids).map_err(|e| e.to_string())?;
            }
            Ok(Created { id: id.to_string() })
        })
    })
    .await
}

#[tauri::command]
pub async fn rename_playlist(path: String, id: String, name: String, app: AppHandle) -> Result<Response, String> {
    let id = parse_ids(&[id])?[0];
    blocking(app, move |app| with_writer(&app, Path::new(&path), |db| db.rename_playlist(id, &name).map_err(|e| e.to_string()))).await
}

/// Delete a playlist; its songs stay on the iPod.
#[tauri::command]
pub async fn delete_playlist(path: String, id: String, app: AppHandle) -> Result<Response, String> {
    let id = parse_ids(&[id])?[0];
    blocking(app, move |app| with_writer(&app, Path::new(&path), |db| db.delete_playlist(id).map_err(|e| e.to_string()))).await
}

/// Replace a playlist's songs (adding, removing and reordering all go through this).
#[tauri::command]
pub async fn set_playlist_tracks(path: String, id: String, ids: Vec<String>, app: AppHandle) -> Result<Response, String> {
    let id = parse_ids(&[id])?[0];
    let ids = parse_ids(&ids)?;
    blocking(app, move |app| with_writer(&app, Path::new(&path), |db| db.set_playlist_tracks(id, &ids).map_err(|e| e.to_string()))).await
}
