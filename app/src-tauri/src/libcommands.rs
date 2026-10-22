//! Commands for music libraries (see library.rs), the hourly scan, and the
//! `libart:` URL scheme serving their covers.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::http::{header, Request, Response as HttpResponse, StatusCode};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::library::{self, Library, Summary};
use crate::state::AppState;

/// All libraries, kept in memory; each saved to `<dir>/<id>.json`.
#[derive(Default)]
pub struct Libraries {
    pub dir: Option<PathBuf>,
    pub list: Vec<Library>,
    pub scanning: HashSet<String>,
}

impl Libraries {
    pub fn load(dir: PathBuf) -> Self {
        let mut list = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().is_some_and(|x| x == "json") {
                    match Library::load(&p) {
                        Ok(l) => list.push(l),
                        Err(e) => eprintln!("Skipping library {e}"),
                    }
                }
            }
        }
        list.sort_by_key(|l| l.name.to_lowercase());
        Libraries { dir: Some(dir), list, scanning: HashSet::new() }
    }

    fn get_mut(&mut self, id: &str) -> Result<&mut Library, String> {
        self.list.iter_mut().find(|l| l.id == id).ok_or_else(|| "That library doesn't exist any more".to_string())
    }

    pub(crate) fn save(&self, id: &str) -> Result<(), String> {
        let dir = self.dir.as_deref().ok_or("No place to keep libraries")?;
        let lib = self.list.iter().find(|l| l.id == id).ok_or("No such library")?;
        if lib.ephemeral {
            return Ok(());
        }
        lib.save(dir)
    }

    fn summaries(&self) -> Vec<LibrarySummary> {
        self.list.iter().map(|l| LibrarySummary { scanning: self.scanning.contains(&l.id), summary: l.summary() }).collect()
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySummary {
    #[serde(flatten)]
    summary: Summary,
    scanning: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress<'a> {
    id: &'a str,
    done: usize,
    total: usize,
    current: &'a str,
}

fn emit_list(app: &AppHandle) {
    let list = app.state::<AppState>().libraries.lock().unwrap().summaries();
    let _ = app.emit("libraries-changed", list);
}

/// Scan a library in the background (nothing happens if it's already scanning).
pub fn start_scan(app: &AppHandle, id: &str) {
    let snapshot = {
        let state = app.state::<AppState>();
        let mut libs = state.libraries.lock().unwrap();
        if libs.scanning.contains(id) {
            return;
        }
        let Some(lib) = libs.list.iter().find(|l| l.id == id).cloned() else { return };
        // An MTP device's songs come from the device, not from scanning folders.
        if lib.ephemeral {
            return;
        }
        libs.scanning.insert(id.to_string());
        lib
    };
    emit_list(app);
    let app = app.clone();
    std::thread::spawn(move || {
        let mut lib = snapshot;
        let id = lib.id.clone();
        let mut last = Instant::now();
        let result = library::scan(&mut lib, |done, total, current| {
            if done == total || last.elapsed() > Duration::from_millis(250) {
                last = Instant::now();
                let _ = app.emit("library-progress", Progress { id: &id, done, total, current });
            }
        });
        let state = app.state::<AppState>();
        let mut libs = state.libraries.lock().unwrap();
        libs.scanning.remove(&id);
        let mut rescan = false;
        if let Ok(cur) = libs.get_mut(&id) {
            // Keep a rename made meanwhile; scan again if the folders changed.
            rescan = cur.folders != lib.folders;
            // Keep what changed meanwhile: ratings and plays, songs taken out,
            // and tags just written (those files are re-read next scan anyway).
            let now: std::collections::HashMap<u32, &library::LibTrack> = cur.tracks.iter().map(|t| (t.id, t)).collect();
            let excluded: HashSet<&str> = cur.excluded.iter().map(String::as_str).collect();
            let mut merged = lib.tracks;
            merged.retain(|t| !excluded.contains(t.path.as_str()));
            for t in &mut merged {
                if let Some(c) = now.get(&t.id) {
                    t.rating = c.rating;
                    t.play_count = c.play_count;
                    t.played = c.played;
                    if c.mtime > t.mtime {
                        t.tags = c.tags.clone();
                        t.cover = c.cover;
                        t.mtime = c.mtime;
                        t.size = c.size;
                    }
                }
            }
            cur.tracks = merged;
            cur.next_id = cur.next_id.max(lib.next_id);
            cur.last_scan = lib.last_scan;
            // A player's playlists are its playlist files.
            if cur.device.is_some() {
                crate::msc::import_playlists(cur);
            }
            cur.prune_playlists();
            if let Err(e) = libs.save(&id) {
                let _ = app.emit("player-error", e);
            }
        }
        drop(libs);
        eprintln!("Scanned library {id}: {result:?}");
        emit_list(&app);
        let _ = app.emit("library-updated", &id);
        if rescan {
            start_scan(&app, &id);
        }
    });
}

/// Scan every library shortly after start-up, then every hour.
pub fn start_hourly(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(10));
        loop {
            let ids: Vec<String> = app.state::<AppState>().libraries.lock().unwrap().list.iter().map(|l| l.id.clone()).collect();
            for id in ids {
                start_scan(&app, &id);
            }
            std::thread::sleep(Duration::from_secs(3600));
        }
    });
}

#[tauri::command]
pub fn list_libraries(state: State<'_, AppState>) -> Vec<LibrarySummary> {
    state.libraries.lock().unwrap().summaries()
}

#[tauri::command]
pub fn create_library(name: String, folders: Vec<String>, app: AppHandle) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Give the library a name".into());
    }
    let lib = Library::new(name, folders);
    let id = lib.id.clone();
    {
        let state = app.state::<AppState>();
        let mut libs = state.libraries.lock().unwrap();
        libs.list.push(lib);
        libs.save(&id)?;
    }
    start_scan(&app, &id);
    Ok(id)
}

#[tauri::command]
pub fn update_library(id: String, name: Option<String>, folders: Option<Vec<String>>, app: AppHandle) -> Result<(), String> {
    let changed_folders;
    {
        let state = app.state::<AppState>();
        let mut libs = state.libraries.lock().unwrap();
        let lib = libs.get_mut(&id)?;
        if let Some(n) = name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
            lib.name = n.to_string();
        }
        let before = lib.folders.clone();
        if let Some(f) = folders {
            library::set_folders(lib, f);
        }
        changed_folders = lib.folders != before;
        libs.save(&id)?;
    }
    emit_list(&app);
    if changed_folders {
        start_scan(&app, &id);
    }
    Ok(())
}

/// Forget a library: only Chinotto's own list of it is deleted, never the music.
#[tauri::command]
pub fn delete_library(id: String, app: AppHandle) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        let mut libs = state.libraries.lock().unwrap();
        let dir = libs.dir.clone().ok_or("No place to keep libraries")?;
        libs.list.retain(|l| l.id != id);
        let file = dir.join(format!("{id}.json"));
        if file.is_file() {
            std::fs::remove_file(&file).map_err(|e| format!("Couldn't remove the library: {e}"))?;
        }
    }
    emit_list(&app);
    Ok(())
}

#[tauri::command]
pub fn scan_library(id: String, app: AppHandle) {
    start_scan(&app, &id);
}

#[tauri::command]
pub fn remove_unavailable(id: String, app: AppHandle) -> Result<usize, String> {
    let n = {
        let state = app.state::<AppState>();
        let mut libs = state.libraries.lock().unwrap();
        let n = libs.get_mut(&id)?.remove_unavailable();
        libs.save(&id)?;
        n
    };
    emit_list(&app);
    let _ = app.emit("library-updated", &id);
    Ok(n)
}

// ---- The library as the window sees it (same shape as an iPod's songs) ----

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TrackView<'a> {
    id: u32,
    dbid: String,
    #[serde(flatten)]
    tags: &'a library::Tags,
    location: &'a str,
    size: u64,
    rating: u8,
    play_count: u32,
    media_type: u32,
    added: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    played: Option<i64>,
    modified: i64,
    art: u32,
    has_cover: bool,
    /// Its folder isn't there (drive unplugged).
    unavailable: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlaylistView {
    id: String,
    name: String,
    master: bool,
    smart: bool,
    track_ids: Vec<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LibraryView<'a> {
    id: &'a str,
    name: &'a str,
    playlists: Vec<PlaylistView>,
    tracks: Vec<TrackView<'a>>,
}

/// The whole library as one JSON string (see `commands::view`).
#[tauri::command]
pub fn library_view(id: String, state: State<'_, AppState>) -> Result<tauri::ipc::Response, String> {
    let libs = state.libraries.lock().unwrap();
    let lib = libs.list.iter().find(|l| l.id == id).ok_or("That library doesn't exist any more")?;
    let missing: HashSet<&str> = lib.folders.iter().filter(|f| !library::folder_available(f)).map(String::as_str).collect();
    let v = LibraryView {
        id: &lib.id,
        name: &lib.name,
        playlists: std::iter::once(PlaylistView {
            id: "all".into(),
            name: lib.name.clone(),
            master: true,
            smart: false,
            track_ids: lib.tracks.iter().map(|t| t.id).collect(),
        })
        .chain(lib.playlists.iter().map(|p| PlaylistView {
            id: format!("p{}", p.id),
            name: p.name.clone(),
            master: false,
            smart: false,
            track_ids: p.tracks.clone(),
        }))
        .collect(),
        tracks: lib
            .tracks
            .iter()
            .map(|t| TrackView {
                id: t.id,
                dbid: format!("lib:{}:{}", lib.id, t.id),
                tags: &t.tags,
                location: &t.path,
                size: t.size,
                rating: t.rating,
                play_count: t.play_count,
                media_type: 1,
                added: t.added,
                played: t.played,
                modified: t.mtime,
                art: 0,
                has_cover: t.cover,
                unavailable: lib.folder_of(&t.path).is_some_and(|f| missing.contains(f)),
            })
            .collect(),
    };
    serde_json::to_string(&v).map(tauri::ipc::Response::new).map_err(|e| e.to_string())
}

/// The file of a library song, from its `lib:<library>:<song>` id.
pub fn track_file(app: &AppHandle, dbid: &str) -> Result<PathBuf, String> {
    let mut parts = dbid.strip_prefix("lib:").ok_or("bad id")?.splitn(2, ':');
    let (lib_id, tid) = (parts.next().ok_or("bad id")?, parts.next().ok_or("bad id")?.parse::<u32>().map_err(|_| "bad id")?);
    let state = app.state::<AppState>();
    let libs = state.libraries.lock().unwrap();
    let lib = libs.list.iter().find(|l| l.id == lib_id).ok_or("That library doesn't exist any more")?;
    let t = lib.track(tid).ok_or("That song isn't in the library any more")?;
    if lib.ephemeral && t.path.starts_with("mtp:") {
        // On an MTP device: copied into a cache folder to be played or copied.
        let p = t.path.clone();
        drop(libs);
        return crate::mtp::fetch(app, &p);
    }
    let path = PathBuf::from(&t.path);
    if !path.is_file() {
        let folder = lib.folder_of(&t.path).unwrap_or("its folder");
        return Err(if library::folder_available(folder) {
            format!("The song's file is missing ({})", path.display())
        } else {
            format!("The song's drive isn't connected ({folder})")
        });
    }
    Ok(path)
}

/// Save a library or Scratch song's cover (embedded, or the folder's picture)
/// as a PNG file at `dest`.
#[tauri::command]
pub async fn save_file_cover(app: AppHandle, dbid: String, dest: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let file = if dbid.starts_with("scratch:") { crate::scratch::track_file(&app, &dbid)? } else { track_file(&app, &dbid)? };
        let bytes = crate::tags::embedded_cover(&file)
            .or_else(|| std::fs::read(library::folder_cover(file.parent()?)?).ok())
            .ok_or("This song has no cover")?;
        let img = crate::cover::decode(&bytes)?;
        img.save_with_format(&dest, image::ImageFormat::Png).map_err(|e| format!("Couldn't save the cover: {e}"))
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---- libart: /<library>-<song>-<px> → JPEG cover ----

pub fn handle_art(app: &AppHandle, req: &Request<Vec<u8>>) -> HttpResponse<Vec<u8>> {
    match render_art(app, req.uri().path()) {
        Some(jpg) => HttpResponse::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/jpeg")
            .header(header::CACHE_CONTROL, "max-age=600")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(jpg)
            .unwrap(),
        None => HttpResponse::builder().status(StatusCode::NOT_FOUND).body(Vec::new()).unwrap(),
    }
}

fn render_art(app: &AppHandle, path: &str) -> Option<Vec<u8>> {
    let mut parts = path.trim_start_matches('/').split('-');
    let (lib, tid, px) = (parts.next()?, parts.next()?, parts.next()?.parse::<u32>().ok()?.clamp(16, 1200));
    // "scratch" stands for the Scratch list.
    let file = if lib == "scratch" {
        crate::scratch::track_file(app, &format!("scratch:{tid}")).ok()?
    } else {
        track_file(app, &format!("lib:{lib}:{tid}")).ok()?
    };
    let bytes = crate::tags::embedded_cover(&file).or_else(|| std::fs::read(library::folder_cover(file.parent()?)?).ok())?;
    cover_jpeg(&bytes, px)
}

fn cover_jpeg(bytes: &[u8], px: u32) -> Option<Vec<u8>> {
    let img = crate::cover::decode(bytes).ok()?;
    let small = if img.width() > px || img.height() > px { img.thumbnail(px, px) } else { img };
    let mut out = Vec::new();
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 88);
    small.to_rgb8().write_with_encoder(enc).ok()?;
    Some(out)
}

// ---- Changing a library ----

/// Run `f` on library `id`, save it, and tell the window.
fn change<T>(app: &AppHandle, id: &str, f: impl FnOnce(&mut Library) -> Result<T, String>) -> Result<T, String> {
    let r = {
        let state = app.state::<AppState>();
        let mut libs = state.libraries.lock().unwrap();
        let r = f(libs.get_mut(id)?)?;
        libs.save(id)?;
        r
    };
    emit_list(app);
    let _ = app.emit("library-updated", id);
    Ok(r)
}

fn playlist_id(s: &str) -> Result<u32, String> {
    s.strip_prefix('p').and_then(|n| n.parse().ok()).ok_or_else(|| "That isn't one of the library's playlists".to_string())
}

#[tauri::command]
pub fn lib_create_playlist(lib: String, name: String, ids: Vec<u32>, app: AppHandle) -> Result<String, String> {
    change(&app, &lib, |l| Ok(format!("p{}", l.new_playlist(name.trim(), ids))))
}

#[tauri::command]
pub fn lib_rename_playlist(lib: String, id: String, name: String, app: AppHandle) -> Result<(), String> {
    let pid = playlist_id(&id)?;
    change(&app, &lib, |l| {
        let p = l.playlists.iter_mut().find(|p| p.id == pid).ok_or("No such playlist")?;
        p.name = name.trim().to_string();
        Ok(())
    })
}

#[tauri::command]
pub fn lib_delete_playlist(lib: String, id: String, app: AppHandle) -> Result<(), String> {
    let pid = playlist_id(&id)?;
    change(&app, &lib, |l| {
        l.playlists.retain(|p| p.id != pid);
        Ok(())
    })
}

#[tauri::command]
pub fn lib_set_playlist_tracks(lib: String, id: String, ids: Vec<u32>, app: AppHandle) -> Result<(), String> {
    let pid = playlist_id(&id)?;
    change(&app, &lib, |l| {
        l.playlists.iter_mut().find(|p| p.id == pid).ok_or("No such playlist")?.tracks = ids;
        l.prune_playlists();
        Ok(())
    })
}

/// Take songs out of the library. Their files aren't touched; scans skip them.
#[tauri::command]
pub fn lib_remove_tracks(lib: String, ids: Vec<u32>, app: AppHandle) -> Result<usize, String> {
    let ids: HashSet<u32> = ids.into_iter().collect();
    change(&app, &lib, |l| Ok(l.remove_tracks(&ids)))
}

/// A cover for embedding: shrunk by the 600 px rule, JPEG or PNG.
fn cover_bytes(image: &str) -> Result<(Vec<u8>, lofty::picture::MimeType), String> {
    use lofty::picture::MimeType;
    let raw = std::fs::read(image).map_err(|e| format!("Couldn't open the image: {e}"))?;
    let img = crate::cover::decode(&raw)?;
    let small = crate::cover::limit_600(img.clone());
    let is = |sig: &[u8]| raw.starts_with(sig);
    if small.width() == img.width() && small.height() == img.height() {
        if is(&[0xFF, 0xD8]) {
            return Ok((raw, MimeType::Jpeg));
        }
        if is(b"\x89PNG") {
            return Ok((raw, MimeType::Png));
        }
    }
    let mut out = Vec::new();
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 92);
    small.to_rgb8().write_with_encoder(enc).map_err(|e| e.to_string())?;
    Ok((out, MimeType::Jpeg))
}

/// Song Details for library songs: tags (and the cover) are written into the
/// files; the rating is kept in the library. `cover`: an image file, "" to
/// remove the cover, or null to leave it. Returns the files that couldn't be
/// changed, with the reason.
#[tauri::command]
pub async fn lib_edit_tracks(
    lib: String,
    ids: Vec<u32>,
    mut changes: std::collections::HashMap<String, serde_json::Value>,
    cover: Option<String>,
    app: AppHandle,
) -> Result<Vec<String>, String> {
    let rating = changes.remove("rating").and_then(|v| v.as_u64()).map(|r| r.min(5) as u8);
    let fields = crate::write::fields(&changes)?;
    let cover = match cover.as_deref() {
        None => None,
        Some("") => Some(crate::tagwrite::CoverChange::Remove),
        Some(path) => {
            let (bytes, mime) = cover_bytes(path)?;
            Some(crate::tagwrite::CoverChange::Set(bytes, mime))
        }
    };
    // A player's songs follow its rules (and need its "Allow changes").
    let rules = crate::msc::edit_rules(&app, &lib)?;
    let paths: Vec<(u32, PathBuf)> = {
        let state = app.state::<AppState>();
        let libs = state.libraries.lock().unwrap();
        let l = libs.list.iter().find(|l| l.id == lib).ok_or("That library doesn't exist any more")?;
        ids.iter().filter_map(|id| l.track(*id).map(|t| (*id, PathBuf::from(&t.path)))).collect()
    };
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut problems = Vec::new();
        let mut done = Vec::new();
        if !fields.is_empty() || cover.is_some() {
            for (id, path) in &paths {
                match crate::tagwrite::write(path, &fields, cover.as_ref()) {
                    Ok(()) => {
                        if let Some(p) = &rules {
                            crate::msc::after_edit(path, p, cover.as_ref());
                        }
                        done.push(*id)
                    }
                    Err(e) => problems.push(e),
                }
            }
        }
        change(&app2, &lib, |l| {
            for id in &done {
                l.reread(*id);
            }
            if let Some(r) = rating {
                for t in l.tracks.iter_mut().filter(|t| ids.contains(&t.id)) {
                    t.rating = r;
                }
            }
            Ok(())
        })?;
        Ok(problems)
    })
    .await
    .map_err(|e| e.to_string())?
}
