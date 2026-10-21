//! Commands the window calls (`invoke(...)` in the front end).

use std::path::PathBuf;

use chinotto_ipod::device::is_ipod_root;
use chinotto_ipod::itunesdb::Track;
use chinotto_ipod::mac_to_unix;
use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Manager, State};

use crate::devices::{self, DeviceInfo};
use crate::state::{AppState, Session};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IpodView<'a> {
    path: String,
    /// Name the iPod gives itself (its library playlist's name).
    name: String,
    device: DeviceInfo,
    serial: Option<&'a str>,
    db_version: u32,
    generation: u32,
    has_artwork_db: bool,
    /// FireWire GUID or serial; None when the iPod doesn't say (then it can't be made writable).
    identity: Option<String>,
    /// The user allowed Chinotto to change this iPod.
    writable: bool,
    playlists: Vec<PlaylistView<'a>>,
    tracks: Vec<TrackView<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlaylistView<'a> {
    /// 64-bit id as a string (JavaScript numbers can't hold it exactly).
    id: String,
    name: &'a str,
    master: bool,
    smart: bool,
    track_ids: &'a [u32],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TrackView<'a> {
    id: u32,
    /// Permanent 64-bit id as a string; survives saves (unlike `id`).
    dbid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    artist: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    album_artist: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    album: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    genre: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    composer: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    comment: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    grouping: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sort_title: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sort_artist: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sort_album: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sort_album_artist: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sort_composer: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<&'a str>,
    kind: String,
    year: u32,
    track_nr: u32,
    track_count: u32,
    disc_nr: u32,
    disc_count: u32,
    length_ms: u32,
    size: u32,
    bitrate: u32,
    sample_rate: u32,
    /// 0–5 stars.
    rating: u8,
    play_count: u32,
    compilation: bool,
    media_type: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    added: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    played: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    modified: Option<i64>,
    /// ArtworkDB image id when the track has a cover that exists, else 0.
    art: u32,
}

fn kind_of(t: &Track) -> String {
    let ext = t.strings.location.as_deref().and_then(|l| l.rsplit_once('.')).map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    let desc = t.strings.filetype.as_deref().unwrap_or("");
    if desc.contains(' ') {
        return desc.to_string();
    }
    match ext.as_str() {
        "mp3" => "MPEG audio file",
        "m4a" => "MPEG-4 audio file",
        "m4b" => "Audiobook",
        "m4p" => "Protected AAC audio file",
        "aac" => "AAC audio file",
        "wav" => "WAV audio file",
        "aif" | "aiff" => "AIFF audio file",
        "m4v" | "mp4" | "mov" => "Video file",
        _ if !desc.is_empty() => desc,
        _ => "Unknown",
    }
    .to_string()
}

fn track_view<'a>(s: &'a Session, t: &'a Track) -> TrackView<'a> {
    let st = &t.strings;
    let art = if t.has_artwork == 1 && s.art_index.contains_key(&t.mhii_link) { t.mhii_link } else { 0 };
    TrackView {
        id: t.id,
        dbid: t.dbid.to_string(),
        title: st.title.as_deref(),
        artist: st.artist.as_deref(),
        album_artist: st.album_artist.as_deref(),
        album: st.album.as_deref(),
        genre: st.genre.as_deref(),
        composer: st.composer.as_deref(),
        comment: st.comment.as_deref(),
        grouping: st.grouping.as_deref(),
        sort_title: st.sort_title.as_deref(),
        sort_artist: st.sort_artist.as_deref(),
        sort_album: st.sort_album.as_deref(),
        sort_album_artist: st.sort_album_artist.as_deref(),
        sort_composer: st.sort_composer.as_deref(),
        location: st.location.as_deref(),
        kind: kind_of(t),
        year: t.year,
        track_nr: t.track_nr,
        track_count: t.track_count,
        disc_nr: t.disc_nr,
        disc_count: t.disc_count,
        length_ms: t.length_ms,
        size: t.size,
        bitrate: t.bitrate_kbps,
        sample_rate: t.sample_rate,
        rating: (t.rating / 20).min(5),
        play_count: t.play_count,
        compilation: t.compilation,
        media_type: t.media_type,
        added: mac_to_unix(t.time_added),
        played: mac_to_unix(t.time_played),
        modified: mac_to_unix(t.time_modified),
        art,
    }
}

/// The whole iPod as one JSON string. A `String` body is sent to the window as
/// JSON (a `Vec<u8>` would arrive as raw bytes).
pub(crate) fn view(s: &Session, state: &AppState) -> Result<String, String> {
    let master = s.db.master_playlist();
    let identity = s.sysinfo.identity();
    let writable = identity.as_deref().is_some_and(|i| state.is_writable(i));
    let device = devices::describe(&s.root, false);
    let v = IpodView {
        path: s.root.to_string_lossy().into_owned(),
        name: master.map(|p| p.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| device.volume.clone()),
        device,
        serial: s.sysinfo.serial.as_deref(),
        db_version: s.db.version,
        generation: s.generation,
        has_artwork_db: s.art.is_some(),
        identity,
        writable,
        playlists: s
            .db
            .playlists
            .iter()
            .map(|p| PlaylistView { id: p.id.to_string(), name: &p.name, master: p.is_master, smart: p.is_smart, track_ids: &p.track_ids })
            .collect(),
        tracks: s.db.tracks.iter().map(|t| track_view(s, t)).collect(),
    };
    serde_json::to_string(&v).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_devices(state: State<'_, AppState>) -> Vec<DeviceInfo> {
    state.scanner.lock().unwrap().scan(&state.manual_roots())
}

/// Remember a folder picked with "Open iPod folder…" (e.g. a copy of an iPod on disk).
#[tauri::command]
pub fn add_folder(path: String, state: State<'_, AppState>) -> Result<DeviceInfo, String> {
    let p = PathBuf::from(&path);
    if !is_ipod_root(&p) {
        return Err(format!(
            "{path} isn't an iPod (there's no iPod_Control/iTunes/iTunesDB inside it). Chinotto can only open iPods so far; other players are coming."
        ));
    }
    state.add_manual_root(p.clone());
    Ok(devices::describe(&p, true))
}

#[tauri::command]
pub async fn open_ipod(path: String, app: AppHandle) -> Result<Response, String> {
    // Parsing a large database takes a moment; keep it off the main thread.
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let s = state.open(&PathBuf::from(path))?;
        view(&s, &state).map(Response::new)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn close_ipod(state: State<'_, AppState>) {
    state.close();
}

#[tauri::command]
pub async fn eject(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let p = PathBuf::from(&path);
    // Close any song file playing from the iPod first, or the disk would be busy.
    if state.session().is_some_and(|s| s.root == p) {
        state.player().stop_and_wait();
    }
    state.close_if(&p);
    tauri::async_runtime::spawn_blocking(move || devices::eject(&p)).await.map_err(|e| e.to_string())?
}

/// A folder to open at start-up: first command-line argument or CHINOTTO_OPEN.
#[tauri::command]
pub fn startup_folder(state: State<'_, AppState>) -> Option<String> {
    let p = std::env::args_os()
        .nth(1)
        .filter(|a| !a.to_string_lossy().starts_with('-'))
        .or_else(|| std::env::var_os("CHINOTTO_OPEN"))
        .map(PathBuf::from)?;
    if is_ipod_root(&p) {
        state.add_manual_root(p.clone());
        Some(p.to_string_lossy().into_owned())
    } else {
        None
    }
}

/// Pictures the user chose for devices (identity → picture key).
#[tauri::command]
pub fn device_icons(state: State<'_, AppState>) -> std::collections::HashMap<String, String> {
    state.device_icons()
}

/// Choose a device's picture; `icon: null` goes back to the automatic one.
#[tauri::command]
pub fn set_device_icon(identity: String, icon: Option<String>, state: State<'_, AppState>) -> Result<(), String> {
    state.set_device_icon(&identity, icon)
}

/// The system's preferred languages, e.g. ["de-DE", "en-US"] (for choosing the app's language).
#[tauri::command]
pub fn system_locales() -> Vec<String> {
    sys_locale::get_locales().collect()
}
