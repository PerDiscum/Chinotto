//! MTP players in the app: found on USB (chinotto-mtp), opened when the user
//! clicks one, and shown like a library (a `Library` with `device` =
//! "mtp:<key>" that's never saved or scanned: its songs come from the
//! device's own database each time it's opened).
//!
//! Songs are played by copying them from the device into a cache folder
//! first. Songs are copied to the device with the same rules as to MSC
//! players (conversion, Greek letters, ID3v2.3 and small covers by model),
//! and can be deleted from it. Playlists on the device come next.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chinotto_mtp::{Device, RawDevice};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

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
    /// "mtp:<vid>:<pid>": its Allow changes and conversion settings.
    identity: String,
    /// What it plays (from the file types it lists).
    plays: Vec<&'static str>,
    /// libmtp's file types it lists.
    filetypes: Vec<i32>,
    /// Rules for what's copied to it (by model).
    profile: crate::msc::Profile,
    /// Its songs as its database has them (edits send every field back).
    tracks: std::collections::HashMap<u32, chinotto_mtp::Track>,
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

/// What a device plays, from libmtp's file types ("aac" for .m4a).
fn plays_of(types: &[i32]) -> Vec<&'static str> {
    let mut out = Vec::new();
    for (t, name) in [(2, "mp3"), (34, "aac"), (6, "aac"), (30, "aac"), (3, "wma"), (1, "wav"), (32, "flac"), (4, "ogg")] {
        if types.contains(&t) && !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

/// The rules for songs copied to the device, by model (the MSC player
/// profiles: Walkmans want ID3v2.3 and plain Greek, Sansas small covers).
/// No folder.jpg: over MTP there are no folders of our own.
fn profile_for(r: &RawDevice) -> crate::msc::Profile {
    // By name first; then by maker, as product strings vary with firmware and
    // libmtp doesn't know every model (USB vendors: SanDisk 0781, Sony 054c).
    let key = match icon_for(r) {
        "sony-nwz" => "sony-nwz",
        "sansa-clip" | "sansa-fuze" => "sansa-clip",
        i if i.starts_with("sansa-") => "sansa-e200",
        _ if r.vendor_id == 0x0781 => "sansa-e200",
        _ if r.vendor_id == 0x054c => "sony-nwz",
        _ => "other",
    };
    let mut p = crate::msc::profiles().into_iter().find(|p| p.key == key).unwrap_or_else(|| crate::msc::profiles().pop().unwrap());
    p.folder_jpg = false;
    p
}

fn identity(r: &RawDevice) -> String {
    format!("mtp:{:04x}:{:04x}", r.vendor_id, r.product_id)
}

/// A song on the device as a library song.
fn lib_track(key: &str, t: &chinotto_mtp::Track) -> LibTrack {
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
        identity: Some(identity(r)),
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
        let mut dev = match Device::open(&raw) {
            Ok(d) => d,
            // On Linux, the desktop's own MTP service (GNOME's gvfs, KDE's
            // kiod) usually has it: ask it to let go, then try again.
            Err(e) if release_from_desktop() => {
                std::thread::sleep(std::time::Duration::from_millis(1500));
                Device::open(&raw).map_err(|_| e.to_string())?
            }
            Err(e) => return Err(e.to_string()),
        };
        let storages = dev.storages();
        let tracks = dev.tracks(|_, _| {});
        let playlists = dev.playlists();
        let filetypes = dev.supported_filetypes();
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
        l.tracks = tracks.iter().map(|t| lib_track(&key, t)).collect();
        let known: std::collections::HashSet<u32> = tracks.iter().map(|t| t.id).collect();
        l.playlists = playlists
            .into_iter()
            .map(|p| LibPlaylist {
                id: p.id,
                name: p.name,
                tracks: p.tracks.into_iter().filter(|t| known.contains(t)).collect(),
                file_name: None,
            })
            .collect();
        l.next_playlist_id = l.playlists.iter().map(|p| p.id).max().unwrap_or(0) + 1;
        {
            let mut libs = state.libraries.lock().unwrap();
            libs.list.retain(|x| x.id != lib);
            libs.list.push(l);
        }
        let profile = profile_for(&raw);
        let mut plays = plays_of(&filetypes);
        if plays.is_empty() {
            plays = profile.plays.to_vec();
        }
        let tracks = tracks.into_iter().map(|t| (t.id, t)).collect();
        state.mtp.lock().unwrap().open =
            Some(Opened { key, dev, info: info.clone(), identity: identity(&raw), plays, filetypes, profile, tracks });
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

/// Ask the desktop's MTP service to let go of MTP devices (Linux: unmount
/// the mtp:// mounts that `gio` lists). Whether there were any.
#[cfg(target_os = "linux")]
fn release_from_desktop() -> bool {
    use std::process::Command;
    let Ok(out) = Command::new("gio").args(["mount", "-l"]).output() else { return false };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut uris: Vec<String> = text
        .lines()
        .filter_map(|l| l.find("mtp://").map(|i| l[i..].split_whitespace().next().unwrap_or("").to_string()))
        .filter(|u| u.len() > "mtp://".len())
        .collect();
    uris.sort();
    uris.dedup();
    for u in &uris {
        let _ = Command::new("gio").args(["mount", "-u", u]).status();
    }
    !uris.is_empty()
}

#[cfg(not(target_os = "linux"))]
fn release_from_desktop() -> bool {
    false
}

/// The settings identity ("mtp:<vid>:<pid>") of a connected device.
pub fn identity_of(key: &str, app: &AppHandle) -> Result<String, String> {
    let state = app.state::<AppState>();
    let m = state.mtp.lock().unwrap();
    m.found.iter().find(|r| r.key() == key).map(identity).ok_or_else(|| "That device isn't connected any more".into())
}

const SWITCHED_OFF: &str = "Changes to this player are switched off. Turn on “Allow changes” on its page first.";

/// The conversion for the device: AAC only if it plays AAC (else MP3 320).
fn conversion(app: &AppHandle, identity: &str, plays: &[&str]) -> crate::convert::Conversion {
    let mut c = app.state::<AppState>().conversion(identity);
    if c.format.starts_with("aac") && !plays.contains(&"aac") {
        c.format = "mp3_320".into();
    }
    c
}

/// Its page settings (the device must be open).
pub fn player_state(key: &str, app: &AppHandle) -> Result<crate::msc::PlayerState, String> {
    let state = app.state::<AppState>();
    let (identity, plays) = {
        let m = state.mtp.lock().unwrap();
        let o = m.open.as_ref().filter(|o| o.key == key).ok_or("Open the device first")?;
        (o.identity.clone(), o.plays.clone())
    };
    Ok(crate::msc::PlayerState {
        writable: state.is_writable(&identity),
        conversion: conversion(app, &identity, &plays),
        aac: plays.contains(&"aac"),
        plays,
    })
}

/// Free space again, after copying or deleting.
fn refresh_space(o: &mut Opened) {
    let storages = o.dev.storages();
    if !storages.is_empty() {
        o.info.capacity = storages.iter().map(|s| s.capacity).sum();
        o.info.free = storages.iter().map(|s| s.free).sum();
    }
}

fn same_song(title: &Option<String>, artist: &Option<String>, album: &Option<String>) -> (String, String, String) {
    let n = |s: &Option<String>| s.as_deref().unwrap_or("").trim().to_lowercase();
    (n(title), n(artist), n(album))
}

/// Copy songs (dbids) and files onto the open device, converting what it
/// can't play and following its model's rules for tags and covers.
pub async fn add(key: String, dbids: Vec<String>, files: Vec<String>, app: AppHandle) -> Result<crate::msc::CopyResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let (identity, plays, filetypes, profile, lib) = {
            let m = state.mtp.lock().unwrap();
            let o = m.open.as_ref().filter(|o| o.key == key).ok_or("That device isn't open any more")?;
            (o.identity.clone(), o.plays.clone(), o.filetypes.clone(), o.profile.clone(), o.info.library.clone())
        };
        if !state.is_writable(&identity) {
            return Err(SWITCHED_OFF.to_string());
        }
        let conv = conversion(&app, &identity, &plays);
        let mut res = crate::msc::CopyResult::default();
        // Before the device is locked: songs on it are fetched through it.
        let sources = crate::msc::sources(&app, &dbids, &files, &mut res.skipped);
        let mut on_device: std::collections::HashSet<(String, String, String)> = {
            let libs = state.libraries.lock().unwrap();
            libs.list
                .iter()
                .find(|l| l.id == lib)
                .map(|l| l.tracks.iter().map(|t| same_song(&t.tags.title, &t.tags.artist, &t.tags.album)).collect())
                .unwrap_or_default()
        };
        let tmp = crate::convert::TempDir::new()?;
        let total = sources.len();
        // Songs sent, for their album objects afterwards.
        let mut sent_songs: Vec<SentSong> = Vec::new();
        for (i, src) in sources.iter().enumerate() {
            let name = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let _ = app
                .emit("add-progress", crate::write::AddProgress { target: key.clone(), done: i, total, current: crate::tags::label(src) });
            let converted;
            let f = if let Some(target) = conv.plan(src, &plays) {
                match crate::convert::convert(src, &tmp.0, &target, i) {
                    Ok(c) => {
                        converted = c;
                        &converted
                    }
                    Err(e) => {
                        res.skipped.push(format!("{name}: couldn't convert it ({e})"));
                        continue;
                    }
                }
            } else {
                src
            };
            let mut info = crate::tags::read(f).unwrap_or_default();
            if profile.plain_greek {
                crate::msc::plain_greek_info(&mut info);
            }
            let id3 = same_song(&info.title, &info.artist, &info.album);
            if !id3.0.is_empty() && on_device.contains(&id3) {
                res.already += 1;
                continue;
            }
            // A copy with the model's rules (tags, cover), then sent.
            let ext = f.extension().and_then(|e| e.to_str()).unwrap_or("mp3").to_ascii_lowercase();
            let ready = tmp.0.join(format!("send-{i}.{ext}"));
            if let Err(e) = crate::msc::copy_in(f, &ready, &profile) {
                res.skipped.push(format!("{name}: {e}"));
                continue;
            }
            let title = info
                .title
                .clone()
                .unwrap_or_else(|| Path::new(&name).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
            let file_name = if info.track_nr > 0 {
                format!("{:02} {}.{ext}", info.track_nr, crate::msc::fat_name(&title, 80))
            } else {
                format!("{}.{ext}", crate::msc::fat_name(&title, 80))
            };
            let new = chinotto_mtp::NewTrack {
                title: Some(title),
                artist: info.artist.clone(),
                album: info.album.clone(),
                genre: info.genre.clone(),
                composer: info.composer.clone(),
                year: info.year.max(0) as u32,
                track_nr: info.track_nr.clamp(0, u16::MAX as i32) as u16,
                duration_ms: info.length_ms.max(0) as u32,
                filename: file_name.clone(),
                filetype: chinotto_mtp::filetype_for(&ext, &filetypes),
            };
            let size = std::fs::metadata(&ready).map(|m| m.len()).unwrap_or(0);
            let sent = {
                let mut m = state.mtp.lock().unwrap();
                let Some(o) = m.open.as_mut().filter(|o| o.key == key) else {
                    return Err("The device was closed while copying".into());
                };
                o.dev.send_track(&ready, &new, |_, _| {})
            };
            let _ = std::fs::remove_file(&ready);
            match sent {
                Ok(id) => {
                    res.added += 1;
                    on_device.insert(id3);
                    if let Some(album) = info.album.clone().filter(|a| !a.trim().is_empty()) {
                        let artist = if info.compilation {
                            Some("Various Artists".to_string())
                        } else {
                            info.album_artist.clone().filter(|a| !a.trim().is_empty()).or_else(|| info.artist.clone())
                        };
                        sent_songs.push(SentSong { id, album, artist, genre: info.genre.clone(), src: src.clone() });
                    }
                    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
                    let t = chinotto_mtp::Track {
                        id,
                        title: new.title.clone(),
                        artist: new.artist.clone(),
                        album: new.album.clone(),
                        genre: new.genre.clone(),
                        composer: new.composer.clone(),
                        date: (new.year > 0).then(|| new.year.to_string()),
                        filename: Some(file_name),
                        track_nr: new.track_nr,
                        duration_ms: new.duration_ms,
                        sample_rate: info.sample_rate,
                        bitrate: info.bitrate_kbps.max(0) as u32 * 1000,
                        size,
                        modified: now,
                        filetype: new.filetype,
                        ..Default::default()
                    };
                    if let Some(o) = state.mtp.lock().unwrap().open.as_mut().filter(|o| o.key == key) {
                        o.tracks.insert(id, t.clone());
                    }
                    let mut libs = state.libraries.lock().unwrap();
                    if let Some(l) = libs.list.iter_mut().find(|l| l.id == lib) {
                        l.tracks.push(lib_track(&key, &t));
                    }
                }
                Err(e) => res.skipped.push(format!("{name}: {e}")),
            }
        }
        let _ = app.emit("add-progress", crate::write::AddProgress { target: key.clone(), done: total, total, current: String::new() });
        drop(tmp);
        if let Some(o) = state.mtp.lock().unwrap().open.as_mut().filter(|o| o.key == key) {
            if !sent_songs.is_empty() {
                res.skipped.extend(file_albums(o, &sent_songs));
            }
            refresh_space(o);
        }
        Ok(res)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A song just sent to the device, for its album object.
struct SentSong {
    id: u32,
    album: String,
    /// The album's artist (or "Various Artists" for a compilation).
    artist: Option<String>,
    genre: Option<String>,
    /// The original file, for the cover.
    src: std::path::PathBuf,
}

/// Put songs just sent into the device's album objects, making the albums
/// that don't exist yet and giving them a picture: SanDisk, Creative and
/// other players built around Windows Media Player show album art from the
/// album object, not from covers embedded in the songs (the approach of
/// XNJB). Problems come back as report lines; the songs are on the device
/// either way.
fn file_albums(o: &mut Opened, songs: &[SentSong]) -> Vec<String> {
    let mut problems = Vec::new();
    let key = |name: &str, artist: Option<&str>| (name.trim().to_lowercase(), artist.unwrap_or("").trim().to_lowercase());
    let mut albums = o.dev.albums();
    let mut groups: Vec<(&SentSong, Vec<u32>)> = Vec::new();
    for s in songs {
        match groups.iter_mut().find(|(first, _)| key(&first.album, first.artist.as_deref()) == key(&s.album, s.artist.as_deref())) {
            Some((_, ids)) => ids.push(s.id),
            None => groups.push((s, vec![s.id])),
        }
    }
    for (first, ids) in groups {
        let k = key(&first.album, first.artist.as_deref());
        // The same album, also matched by name alone when the device kept no artist.
        let existing = albums
            .iter()
            .position(|a| key(&a.name, a.artist.as_deref()) == k)
            .or_else(|| albums.iter().position(|a| a.artist.is_none() && a.name.trim().eq_ignore_ascii_case(first.album.trim())));
        let id = match existing {
            Some(i) => {
                let a = &mut albums[i];
                for id in &ids {
                    if !a.tracks.contains(id) {
                        a.tracks.push(*id);
                    }
                }
                if let Err(e) = o.dev.update_album(a) {
                    problems.push(format!("Album “{}”: {e}", first.album));
                    continue;
                }
                a.id
            }
            None => {
                let a = chinotto_mtp::Album {
                    name: first.album.clone(),
                    artist: first.artist.clone(),
                    genre: first.genre.clone(),
                    tracks: ids.clone(),
                    ..Default::default()
                };
                match o.dev.create_album(&a) {
                    Ok(id) => {
                        albums.push(chinotto_mtp::Album { id, ..a });
                        id
                    }
                    Err(e) => {
                        problems.push(format!("Album “{}”: {e}", first.album));
                        continue;
                    }
                }
            }
        };
        if existing.is_some() && o.dev.album_has_art(id) {
            continue;
        }
        let cover = crate::tags::embedded_cover(&first.src)
            .or_else(|| crate::library::folder_cover(first.src.parent()?).and_then(|p| std::fs::read(p).ok()));
        let Some(cover) = cover else { continue };
        if let Err(e) = send_album_art(o, id, &cover) {
            problems.push(format!("Album “{}”: {e}", first.album));
        }
    }
    problems
}

/// Give album object `id` a picture made from `cover` (any image): the size
/// the device asks for, else the model's small-cover rule, else 300 pixels.
fn send_album_art(o: &mut Opened, id: u32, cover: &[u8]) -> Result<(), String> {
    let (max_px, max_bytes) = match (o.dev.album_art_size(), o.profile.small_covers) {
        (Some((w, h)), small) => (w.min(h).min(small.map_or(u32::MAX, |s| s.0)), small.map_or(200_000, |s| s.1)),
        (None, Some((px, bytes))) => (px, bytes),
        (None, None) => (300, 200_000),
    };
    let jpeg = crate::msc::small_jpeg(cover, max_px, max_bytes).map_err(|e| format!("its cover couldn't be prepared ({e})"))?;
    let (w, h) = image::load_from_memory(&jpeg).map(|i| (i.width(), i.height())).unwrap_or((max_px, max_px));
    o.dev.set_album_art(id, &jpeg, w, h).map_err(|e| e.to_string())
}

/// A new cover for songs on the device: it goes to their album objects
/// (made for them if there are none), which is where these players show
/// covers from.
fn set_cover(o: &mut Opened, ids: &[u32], cover: &[u8]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut albums = o.dev.albums();
    let mut done: Vec<u32> = Vec::new();
    for id in ids {
        if albums.iter().any(|a| done.contains(&a.id) && a.tracks.contains(id)) {
            continue;
        }
        let album_id = match albums.iter().find(|a| a.tracks.contains(id)) {
            Some(a) => a.id,
            None => {
                let Some(t) = o.tracks.get(id).cloned() else { continue };
                let Some(name) = t.album.clone().filter(|a| !a.trim().is_empty()) else {
                    problems.push(format!("{}: a song needs an album name for its cover to show", t.title.as_deref().unwrap_or("?")));
                    continue;
                };
                // Every song of that album on the device.
                let same = |x: &chinotto_mtp::Track| x.album.as_deref().is_some_and(|a| a.trim().eq_ignore_ascii_case(name.trim()));
                let mut tracks: Vec<u32> = o.tracks.values().filter(|x| same(x)).map(|x| x.id).collect();
                tracks.sort();
                let a = chinotto_mtp::Album {
                    name: name.clone(),
                    artist: t.artist.clone(),
                    genre: t.genre.clone(),
                    tracks,
                    ..Default::default()
                };
                match o.dev.create_album(&a) {
                    Ok(new_id) => {
                        albums.push(chinotto_mtp::Album { id: new_id, ..a });
                        new_id
                    }
                    Err(e) => {
                        problems.push(format!("Album “{name}”: {e}"));
                        continue;
                    }
                }
            }
        };
        done.push(album_id);
        if let Err(e) = send_album_art(o, album_id, cover) {
            let name = albums.iter().find(|a| a.id == album_id).map(|a| a.name.clone()).unwrap_or_default();
            problems.push(format!("Album “{name}”: {e}"));
        }
    }
    problems
}

/// Delete songs (the device library's song ids, which are the device's own
/// ids) from the open device.
pub fn delete(key: &str, ids: &[u32], app: &AppHandle) -> Result<Vec<String>, String> {
    let state = app.state::<AppState>();
    let mut m = state.mtp.lock().unwrap();
    let o = m.open.as_mut().filter(|o| o.key == key).ok_or("That device isn't open any more")?;
    if !state.is_writable(&o.identity) {
        return Err(SWITCHED_OFF.into());
    }
    let lib = o.info.library.clone();
    let mut problems = Vec::new();
    let mut gone = Vec::new();
    for &id in ids {
        match o.dev.delete(id) {
            Ok(()) => gone.push(id),
            Err(e) => problems.push(format!("{id}: {e}")),
        }
    }
    for id in &gone {
        o.tracks.remove(id);
    }
    // Album objects left with none of their songs (only ones that had some of these).
    if !gone.is_empty() {
        for a in o.dev.albums() {
            if a.tracks.iter().any(|t| gone.contains(t)) && !a.tracks.iter().any(|t| o.tracks.contains_key(t)) {
                let _ = o.dev.delete(a.id);
            }
        }
    }
    refresh_space(o);
    drop(m);
    let mut libs = state.libraries.lock().unwrap();
    if let Some(l) = libs.list.iter_mut().find(|l| l.id == lib) {
        l.tracks.retain(|t| !gone.contains(&t.id));
        l.prune_playlists();
    }
    Ok(problems)
}

/// The library lists the songs of the open MTP device.
pub fn is_mtp_library(app: &AppHandle, lib: &str) -> bool {
    let state = app.state::<AppState>();
    let m = state.mtp.lock().unwrap();
    m.open.as_ref().is_some_and(|o| o.info.library == lib)
}

/// Change songs' tags (and ratings) in the open device's database. The
/// device keeps title, artist, album, genre, composer, year, track number
/// and rating; other fields aren't kept there and are left out (said once).
pub fn edit(
    app: &AppHandle,
    lib: &str,
    ids: &[u32],
    fields: &[chinotto_gpod::Field],
    rating: Option<u8>,
    cover: Option<&crate::tagwrite::CoverChange>,
) -> Result<Vec<String>, String> {
    use chinotto_gpod::Field as F;
    let state = app.state::<AppState>();
    let mut problems = Vec::new();
    let mut changed: Vec<(String, chinotto_mtp::Track)> = Vec::new();
    {
        let mut m = state.mtp.lock().unwrap();
        let o = m.open.as_mut().filter(|o| o.info.library == lib).ok_or("That device isn't open any more")?;
        if !state.is_writable(&o.identity) {
            return Err(SWITCHED_OFF.into());
        }
        let greek = o.profile.plain_greek;
        let text = |v: &Option<String>| v.as_ref().map(|s| if greek { crate::msc::plain_greek(s) } else { s.clone() });
        let mut ignored = false;
        for id in ids {
            let Some(mut t) = o.tracks.get(id).cloned() else { continue };
            for f in fields {
                match f {
                    F::Title(v) => t.title = text(v),
                    F::Artist(v) => t.artist = text(v),
                    F::Album(v) => t.album = text(v),
                    F::Genre(v) => t.genre = text(v),
                    F::Composer(v) => t.composer = text(v),
                    F::Year(y) => t.date = (*y > 0).then(|| format!("{y:04}0101T000000.0")),
                    F::TrackNr(n) => t.track_nr = (*n).clamp(0, u16::MAX as i32) as u16,
                    _ => ignored = true,
                }
            }
            if let Some(r) = rating {
                t.rating = r as u16 * 20;
            }
            match o.dev.update_track(&t) {
                Ok(()) => {
                    o.tracks.insert(*id, t.clone());
                    changed.push((o.key.clone(), t));
                }
                Err(e) => problems.push(format!("{}: {e}", t.title.as_deref().unwrap_or("?"))),
            }
        }
        if ignored {
            problems.push("Only title, artist, album, genre, composer, year, track number and rating are kept on an MTP device; the other changes were left out.".into());
        }
        match cover {
            Some(crate::tagwrite::CoverChange::Set(bytes, _)) => problems.extend(set_cover(o, ids, bytes)),
            Some(crate::tagwrite::CoverChange::Remove) => {
                problems.push("Covers can't be taken off songs on an MTP device yet; choose another image instead.".into())
            }
            None => {}
        }
    }
    crate::libcommands::change(app, lib, |l| {
        for (key, t) in &changed {
            if let Some(lt) = l.tracks.iter_mut().find(|x| x.id == t.id) {
                let added = lt.added;
                *lt = lib_track(key, t);
                lt.added = added;
            }
        }
        Ok(())
    })?;
    Ok(problems)
}
