//! MTP players in the app: found on USB (chinotto-mtp), opened when the user
//! clicks one, and shown like a library (a `Library` with `device` =
//! "mtp:<key>" that's never saved or scanned: its songs come from the
//! device's own database each time it's opened).
//!
//! Songs are played by copying them from the device into a cache folder
//! first. This step reads devices; copying to them comes next.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chinotto_mtp::{Device, RawDevice};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::devices::DeviceInfo;
use crate::library::{LibPlaylist, LibTrack, Library, Tags};
use crate::state::AppState;

#[derive(Default)]
pub struct MtpState {
    fingerprint: u64,
    found: Vec<RawDevice>,
    open: Option<Opened>,
}

struct Opened {
    key: String,
    dev: Device,
    info: MtpInfo,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MtpInfo {
    name: String,
    model: Option<String>,
    manufacturer: Option<String>,
    serial: Option<String>,
    battery: Option<u8>,
    capacity: u64,
    free: u64,
    songs: usize,
    library: String,
}

/// The library id for a device key ("mtp:1-5" → "m1x5"; no dashes: libart URLs split on them).
fn lib_id(key: &str) -> String {
    format!("m{}", key.trim_start_matches("mtp:").replace('-', "x"))
}

/// A picture for the device, from its maker and name.
fn icon_for(r: &RawDevice) -> &'static str {
    let name = format!("{} {}", r.vendor.as_deref().unwrap_or(""), r.product.as_deref().unwrap_or("")).to_lowercase();
    let pick = [
        ("zen vision:m", "creative-zen-vision-m"),
        ("zen vision w", "creative-zen-vision-w"),
        ("zen micro", "creative-zen-micro"),
        ("zen stone", "creative-zen-stone"),
        ("zen", "creative-zen-touch"),
        ("sansa e2", "sansa-e200"),
        ("sansa e1", "sansa-e130"),
        ("sansa m2", "sansa-m200"),
        ("sansa clip", "sansa-clip"),
        ("sansa fuze", "sansa-fuze"),
        ("walkman", "sony-nwz"),
        ("nwz-", "sony-nwz"),
        ("iaudio", "cowon-iaudio-x5"),
        ("iriver", "iriver-h100"),
        ("zune", "generic"),
    ];
    for (k, v) in pick {
        if name.contains(k) {
            return v;
        }
    }
    // Phones and other Android devices (Google's and most makers' MTP is Android's).
    if r.vendor_id == 0x18d1 || name.contains("android") || name.contains("retroid") {
        return "android";
    }
    "generic"
}

fn describe(r: &RawDevice, open: Option<&Opened>) -> DeviceInfo {
    let key = r.key();
    let info = open.filter(|o| o.key == key).map(|o| &o.info);
    let fallback = [r.vendor.as_deref(), r.product.as_deref()].into_iter().flatten().collect::<Vec<_>>().join(" ");
    DeviceInfo {
        path: key.clone(),
        volume: info.map(|i| i.name.clone()).unwrap_or_else(|| if fallback.is_empty() { "MTP device".into() } else { fallback }),
        model: r.product.clone(),
        model_number: None,
        icon: icon_for(r).to_string(),
        identity: Some(format!("mtp:{:04x}:{:04x}", r.vendor_id, r.product_id)),
        capacity: info.map(|i| i.capacity).unwrap_or(0),
        free: info.map(|i| i.free).unwrap_or(0),
        // MTP devices aren't ejected: closing them in Chinotto is enough.
        manual: true,
        needs_setup: false,
        kind: "mtp",
        library: info.map(|i| i.library.clone()),
    }
}

/// MTP devices connected now (detected again only when USB devices change).
pub fn poll(app: &AppHandle) -> Vec<DeviceInfo> {
    let state = app.state::<AppState>();
    let mut m = state.mtp.lock().unwrap();
    let fp = chinotto_mtp::usb_fingerprint();
    if fp != m.fingerprint {
        m.fingerprint = fp;
        m.found = chinotto_mtp::detect();
        // The open device was unplugged.
        if let Some(o) = &m.open {
            if !m.found.iter().any(|r| r.key() == o.key) {
                let lib = o.info.library.clone();
                m.open = None;
                state.libraries.lock().unwrap().list.retain(|l| l.id != lib);
            }
        }
    }
    m.found.iter().map(|r| describe(r, m.open.as_ref())).collect()
}

/// Open the device (closing another one that's open), read its songs and
/// playlists into a library, and say what it is.
#[tauri::command]
pub async fn mtp_open(key: String, app: AppHandle) -> Result<MtpInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let raw = {
            let mut m = state.mtp.lock().unwrap();
            if let Some(o) = m.open.as_ref().filter(|o| o.key == key) {
                return Ok(o.info.clone());
            }
            if let Some(o) = m.open.take() {
                state.libraries.lock().unwrap().list.retain(|l| l.id != o.info.library);
            }
            m.found.iter().find(|r| r.key() == key).cloned().ok_or("That device isn't connected any more")?
        };
        let mut dev = Device::open(&raw).map_err(|e| e.to_string())?;
        let storages = dev.storages();
        let tracks = dev.tracks(|_, _| {});
        let playlists = dev.playlists();
        let lib = lib_id(&key);
        let name = dev.friendly_name().or_else(|| dev.model()).unwrap_or_else(|| describe(&raw, None).volume);
        let info = MtpInfo {
            name: name.clone(),
            model: dev.model(),
            manufacturer: dev.manufacturer(),
            serial: dev.serial(),
            battery: dev.battery(),
            capacity: storages.iter().map(|s| s.capacity).sum(),
            free: storages.iter().map(|s| s.free).sum(),
            songs: tracks.len(),
            library: lib.clone(),
        };
        let mut l = Library::new(&name, Vec::new());
        l.id = lib.clone();
        l.device = Some(key.clone());
        l.ephemeral = true;
        l.tracks = tracks
            .iter()
            .map(|t| {
                let file = t.filename.clone().unwrap_or_else(|| format!("{}.mp3", t.id));
                LibTrack {
                    id: t.id,
                    path: format!("{key}/{}/{file}", t.id),
                    size: t.size,
                    mtime: t.modified,
                    added: t.modified,
                    play_count: t.play_count,
                    rating: (t.rating / 20).min(5) as u8,
                    played: None,
                    cover: false,
                    tags: Tags {
                        title: t.title.clone().or_else(|| t.filename.clone()),
                        artist: t.artist.clone(),
                        album: t.album.clone(),
                        genre: t.genre.clone(),
                        composer: t.composer.clone(),
                        year: t.year().max(0) as u32,
                        track_nr: t.track_nr as u32,
                        length_ms: t.duration_ms,
                        bitrate: t.bitrate / 1000,
                        sample_rate: t.sample_rate,
                        kind: format!("{} audio file", chinotto_mtp::filetype_name(t.filetype)),
                        ..Default::default()
                    },
                }
            })
            .collect();
        let known: std::collections::HashSet<u32> = tracks.iter().map(|t| t.id).collect();
        l.playlists = playlists
            .into_iter()
            .map(|p| LibPlaylist { id: p.id, name: p.name, tracks: p.tracks.into_iter().filter(|t| known.contains(t)).collect() })
            .collect();
        l.next_playlist_id = l.playlists.iter().map(|p| p.id).max().unwrap_or(0) + 1;
        {
            let mut libs = state.libraries.lock().unwrap();
            libs.list.retain(|x| x.id != lib);
            libs.list.push(l);
        }
        state.mtp.lock().unwrap().open = Some(Opened { key, dev, info: info.clone() });
        Ok(info)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Let go of the device (so other programs can use it).
#[tauri::command]
pub fn mtp_close(key: String, app: AppHandle) {
    let state = app.state::<AppState>();
    let mut m = state.mtp.lock().unwrap();
    if let Some(o) = m.open.as_ref().filter(|o| o.key == key) {
        let lib = o.info.library.clone();
        m.open = None;
        state.libraries.lock().unwrap().list.retain(|l| l.id != lib);
    }
}

/// The device's details while it's open.
#[tauri::command]
pub fn mtp_info(key: String, app: AppHandle) -> Option<MtpInfo> {
    let state = app.state::<AppState>();
    let m = state.mtp.lock().unwrap();
    m.open.as_ref().filter(|o| o.key == key).map(|o| o.info.clone())
}

/// A song on the open device as a file on the computer (copied into a cache
/// folder the first time). `path` is the library track's path:
/// "mtp:<bus>-<dev>/<id>/<file name>".
pub fn fetch(app: &AppHandle, path: &str) -> Result<PathBuf, String> {
    let mut parts = path.splitn(3, '/');
    let (key, id, name) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or("song"));
    let id: u32 = id.parse().map_err(|_| "bad id")?;
    let dir = std::env::temp_dir().join("chinotto-mtp").join(key.replace(':', "_"));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let ext = Path::new(name).extension().and_then(|e| e.to_str()).unwrap_or("mp3");
    let file = dir.join(format!("{id}.{ext}"));
    if file.is_file() {
        return Ok(file);
    }
    let state = app.state::<AppState>();
    let mut m = state.mtp.lock().unwrap();
    let o = m.open.as_mut().filter(|o| o.key == key).ok_or("That device isn't open any more")?;
    let part = dir.join(format!(".{id}.part"));
    o.dev.download(id, &part).map_err(|e| e.to_string())?;
    std::fs::rename(&part, &file).map_err(|e| e.to_string())?;
    Ok(file)
}

/// For MutexGuard type in AppState.
pub type Shared = Mutex<MtpState>;
