//! MSC players: music players that show up as an ordinary drive (most Sony
//! Walkmans, SanDisk Sansas…), rather than with a database like an iPod.
//!
//! A player is set up once in the "Add Drive" window; its settings are kept on
//! the player itself, in `.chinotto-device.json` at the top of the drive, so
//! it's recognised whenever (and wherever) it's plugged in. Its songs are
//! listed like a library's (a `Library` with `device` set, kept with the
//! others so later scans only read new or changed files); its playlists are
//! read from the playlist files in its playlist folder.
//!
//! This step reads players. Copying songs to them, playlists and the
//! per-player rules for tags and covers come next.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::library::{LibPlaylist, Library};
use crate::state::AppState;

pub const CONFIG: &str = ".chinotto-device.json";

/// A player's settings, as kept on it.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct MscConfig {
    /// Made when the player is first set up; its library is "d<id>".
    pub id: String,
    pub name: String,
    /// One of `profiles()`, e.g. "sony-nwz".
    pub model: String,
    /// Picture key (deviceImages.ts).
    pub icon: String,
    /// Folders inside the drive, e.g. "MUSIC".
    pub music_folder: String,
    pub playlist_folder: String,
    /// "m3u8", "m3u" or "pls".
    pub playlist_format: String,
}

impl MscConfig {
    pub fn library_id(&self) -> String {
        format!("d{}", self.id)
    }
}

/// The settings on the drive at `root`, if it has been set up.
pub fn read_config(root: &Path) -> Option<MscConfig> {
    let text = std::fs::read_to_string(root.join(CONFIG)).ok()?;
    serde_json::from_str::<MscConfig>(&text).ok().filter(|c| !c.id.is_empty())
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub key: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub music_folder: &'static str,
    pub playlist_folder: &'static str,
    pub playlist_format: &'static str,
    /// What it plays: file extensions, plus "aac" and "alac" for the two kinds of .m4a.
    pub plays: &'static [&'static str],
    /// Its tags must be ID3v2.3 for covers to show.
    pub id3v23: bool,
    /// It only shows small embedded covers: at most this many pixels a side
    /// and bytes (they're re-encoded as plain, "baseline" JPEGs).
    pub small_covers: Option<(u32, usize)>,
    /// It shows the album folder's `folder.jpg` (tested on a Sansa e260 v2,
    /// firmware 03.01.16A: its own firmware shows folder.jpg only, Rockbox
    /// either; when songs are copied as files,
    /// SanDisk's firmware ignores covers embedded in them).
    pub folder_jpg: bool,
}

/// Known players and where they keep things. "Other" fits most players.
pub fn profiles() -> Vec<Profile> {
    let p = |key, name, icon, music_folder, playlist_folder, playlist_format, plays, id3v23| Profile {
        key,
        name,
        icon,
        music_folder,
        playlist_folder,
        playlist_format,
        plays,
        id3v23,
        small_covers: None,
        folder_jpg: false,
    };
    vec![
        p("sony-nwz", "Sony Walkman NWZ (E, S, A series…)", "sony-nwz", "MUSIC", "MUSIC", "m3u8", &["mp3", "aac", "wma", "wav"], true),
        p(
            "sony-nwa",
            "Sony Walkman NW-A (A10, A20…)",
            "sony-a10",
            "MUSIC",
            "MUSIC",
            "m3u8",
            &["mp3", "aac", "alac", "flac", "wma", "wav", "aiff", "aif"],
            true,
        ),
        // The e200 shows embedded covers up to about 32 KB (a 300×300 one did, a
        // 455×455 one of 42 KB didn't, as reported on SanDisk's and JRiver's forums).
        // Copied as files (not through MTP), SanDisk players show the album
        // folder's folder.jpg rather than covers in the songs.
        Profile {
            small_covers: Some((300, 30_000)),
            folder_jpg: true,
            ..p("sansa-e200", "SanDisk Sansa e200", "sansa-e200", "Music", "Music", "m3u", &["mp3", "wma", "wav"], true)
        },
        Profile {
            small_covers: Some((300, 30_000)),
            folder_jpg: true,
            ..p(
                "sansa-clip",
                "SanDisk Sansa Clip / Fuze",
                "sansa-clip",
                "Music",
                "Music",
                "m3u",
                &["mp3", "wma", "wav", "flac", "ogg"],
                true,
            )
        },
        p("other", "Other player", "generic", "Music", "Playlists", "m3u8", &["mp3", "aac", "wav"], false),
    ]
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveInfo {
    path: String,
    volume: String,
    is_disk: bool,
    /// An iPod (has a database), or one that needs setting up.
    is_ipod: bool,
    /// Settings already on the drive.
    config: Option<MscConfig>,
    /// A first guess at the model, from the drive's folders.
    suggested: &'static str,
    /// Folders at the top of the drive.
    folders: Vec<String>,
    profiles: Vec<Profile>,
}

/// What's on a drive the user picked with Add Drive.
#[tauri::command]
pub fn msc_inspect(path: String) -> Result<DriveInfo, String> {
    let root = PathBuf::from(&path);
    if !root.is_dir() {
        return Err(format!("{path} isn't there"));
    }
    let mut folders: Vec<String> = std::fs::read_dir(&root)
        .map_err(|e| format!("Couldn't look inside {path}: {e}"))?
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !n.starts_with('.'))
        .collect();
    folders.sort_by_key(|f| f.to_lowercase());
    let has = |name: &str| folders.iter().any(|f| f == name);
    // Walkmans have an upper-case MUSIC folder; Sansas a "Music" one and a "SYSTEM" one.
    let suggested = if has("MUSIC") {
        "sony-nwz"
    } else if has("Music") && (has("SYSTEM") || has("Service")) {
        "sansa-e200"
    } else {
        "other"
    };
    Ok(DriveInfo {
        volume: root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone()),
        is_disk: crate::devices::is_mount_point(&root),
        is_ipod: chinotto_ipod::device::is_ipod_root(&root) || crate::devices::needs_setup(&root),
        config: read_config(&root),
        suggested,
        folders,
        profiles: profiles(),
        path,
    })
}

/// A folder name inside the drive: no going outside it.
fn clean_folder(f: &str) -> Result<String, String> {
    let f = f.trim().trim_matches(['/', '\\']).replace('\\', "/");
    if f.split('/').any(|c| c == ".." || c.is_empty()) && !f.is_empty() {
        return Err(format!("“{f}” isn't a folder on the player"));
    }
    Ok(f)
}

/// Save the player's settings on it, and start listing its songs.
#[tauri::command]
pub fn msc_save(path: String, config: MscConfig, app: AppHandle) -> Result<crate::devices::DeviceInfo, String> {
    let root = PathBuf::from(&path);
    let mut cfg = config;
    if cfg.id.is_empty() {
        // Keep the id of settings already there (a re-setup keeps its songs list).
        cfg.id = read_config(&root).map(|c| c.id).unwrap_or_else(|| {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
            format!("{:x}", now ^ (std::process::id() as u128) << 64)
        });
    }
    cfg.name = cfg.name.trim().to_string();
    if cfg.name.is_empty() {
        cfg.name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Player".into());
    }
    cfg.music_folder = clean_folder(&cfg.music_folder)?;
    cfg.playlist_folder = clean_folder(&cfg.playlist_folder)?;
    if !matches!(cfg.playlist_format.as_str(), "m3u8" | "m3u" | "pls") {
        cfg.playlist_format = "m3u8".into();
    }
    if !root.join(&cfg.music_folder).is_dir() {
        std::fs::create_dir_all(root.join(&cfg.music_folder)).map_err(|e| format!("Couldn't create the music folder: {e}"))?;
    }
    let json = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    std::fs::write(root.join(CONFIG), json).map_err(|e| format!("Couldn't save the settings on the player: {e}"))?;

    let state = app.state::<AppState>();
    // A folder that isn't a mounted drive (a copy, for testing) is watched by hand.
    if !crate::devices::is_mount_point(&root) {
        state.add_manual_root(root.clone());
    }
    state.scanner.lock().unwrap().forget(&root);
    attach(&app, &root, &cfg);
    Ok(crate::devices::describe(&root, !crate::devices::is_mount_point(&root)))
}

/// Make sure the player's library exists and points at where it's plugged in
/// now; then scan it.
pub fn attach(app: &AppHandle, root: &Path, cfg: &MscConfig) {
    let id = cfg.library_id();
    let music = root.join(&cfg.music_folder).to_string_lossy().into_owned();
    let playlists = root.join(&cfg.playlist_folder).to_string_lossy().into_owned();
    {
        let state = app.state::<AppState>();
        let mut libs = state.libraries.lock().unwrap();
        match libs.list.iter_mut().find(|l| l.id == id) {
            Some(l) => {
                l.name = cfg.name.clone();
                l.folders = vec![music];
                l.playlist_dir = Some(playlists);
            }
            None => {
                let mut l = Library::new(&cfg.name, vec![music]);
                l.id = id.clone();
                l.device = Some(cfg.id.clone());
                l.playlist_dir = Some(playlists);
                libs.list.push(l);
            }
        }
        let _ = libs.save(&id);
    }
    crate::libcommands::start_scan(app, &id);
}

/// Replace a player library's playlists with those in its playlist folder
/// (.m3u, .m3u8, .pls). Songs are matched by path, ignoring case (players
/// are usually FAT drives, and playlists often come from Windows).
pub fn import_playlists(lib: &mut Library) {
    let Some(dir) = lib.playlist_dir.clone() else { return };
    let dir = PathBuf::from(dir);
    let Ok(rd) = std::fs::read_dir(&dir) else { return };
    // The drive's top: the music folder's parent chain up to the mount point.
    let root = lib.folders.first().map(PathBuf::from).and_then(|m| m.ancestors().find(|a| a.join(CONFIG).exists()).map(Path::to_path_buf));
    let by_path: std::collections::HashMap<String, u32> = lib.tracks.iter().map(|t| (t.path.to_lowercase(), t.id)).collect();
    let mut files: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'))
                && p.extension().and_then(|e| e.to_str()).is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "m3u" | "m3u8" | "pls"))
        })
        .collect();
    files.sort();
    let mut playlists = Vec::new();
    for (i, f) in files.iter().enumerate() {
        let Ok(bytes) = std::fs::read(f) else { continue };
        let text = String::from_utf8(bytes.clone()).unwrap_or_else(|_| bytes.iter().map(|&b| b as char).collect());
        let pls = f.extension().is_some_and(|e| e.eq_ignore_ascii_case("pls"));
        let tracks: Vec<u32> = entries(&text, pls)
            .into_iter()
            .filter_map(|e| resolve(&e, f.parent().unwrap_or(&dir), root.as_deref()))
            .filter_map(|p| by_path.get(&p.to_string_lossy().to_lowercase()).copied())
            .collect();
        let name = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        playlists.push(LibPlaylist { id: i as u32 + 1, name, tracks });
    }
    lib.next_playlist_id = playlists.len() as u32 + 1;
    lib.playlists = playlists;
}

/// The song entries of a playlist file, as written.
fn entries(text: &str, pls: bool) -> Vec<String> {
    text.lines()
        .map(|l| l.trim_start_matches('\u{feff}').trim())
        .filter_map(|l| {
            if pls {
                let (k, v) = l.split_once('=')?;
                k.trim().to_ascii_lowercase().starts_with("file").then(|| v.trim().to_string())
            } else {
                (!l.is_empty() && !l.starts_with('#')).then(|| l.to_string())
            }
        })
        .collect()
}

/// Where an entry points: relative to the playlist's folder, or (starting
/// with / or \) to the top of the drive. Windows-style separators are fine.
fn resolve(entry: &str, base: &Path, root: Option<&Path>) -> Option<PathBuf> {
    let e = entry.replace('\\', "/");
    if e.contains("://") {
        return None;
    }
    // "C:/…" or "E:/…": a drive letter means the top of the player.
    let e = if e.len() > 2 && e.as_bytes()[1] == b':' { e[2..].to_string() } else { e };
    let p = if let Some(rest) = e.strip_prefix('/') { root?.join(rest) } else { base.join(&e) };
    // Tidy "a/../b" without touching the disk.
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            c => out.push(c),
        }
    }
    Some(out)
}

// ---- Changing a player: allow changes, conversion, copying, deleting ----

/// The player at `path`, its settings and its model.
fn player(path: &str) -> Result<(PathBuf, MscConfig, Profile), String> {
    let root = PathBuf::from(path);
    let cfg = read_config(&root).ok_or("That player isn't set up (or isn't plugged in)")?;
    let profile = profiles().into_iter().find(|p| p.key == cfg.model).unwrap_or_else(|| profiles().pop().unwrap());
    Ok((root, cfg, profile))
}

fn identity(cfg: &MscConfig) -> String {
    format!("msc:{}", cfg.id)
}

/// The conversion for this player: AAC only if it plays AAC (else MP3 320).
fn conversion_for(app: &AppHandle, cfg: &MscConfig, profile: &Profile) -> crate::convert::Conversion {
    let mut c = app.state::<AppState>().conversion(&identity(cfg));
    if c.target().0 && !profile.plays.contains(&"aac") {
        c.format = "mp3_320".into();
    }
    c
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerState {
    writable: bool,
    conversion: crate::convert::Conversion,
    /// The formats it plays, for the page ("MP3, AAC, WMA, WAV").
    plays: Vec<&'static str>,
    aac: bool,
}

#[tauri::command]
pub fn msc_state(path: String, app: AppHandle) -> Result<PlayerState, String> {
    let (_, cfg, profile) = player(&path)?;
    let state = app.state::<AppState>();
    Ok(PlayerState {
        writable: state.is_writable(&identity(&cfg)),
        conversion: conversion_for(&app, &cfg, &profile),
        plays: profile.plays.to_vec(),
        aac: profile.plays.contains(&"aac"),
    })
}

#[tauri::command]
pub fn msc_allow(path: String, allow: bool, app: AppHandle) -> Result<PlayerState, String> {
    let (_, cfg, _) = player(&path)?;
    app.state::<AppState>().set_writable(&identity(&cfg), allow)?;
    msc_state(path, app)
}

#[tauri::command]
pub fn msc_set_conversion(path: String, conversion: crate::convert::Conversion, app: AppHandle) -> Result<PlayerState, String> {
    let (_, cfg, _) = player(&path)?;
    app.state::<AppState>().set_conversion(&identity(&cfg), conversion)?;
    msc_state(path, app)
}

/// Why `file` must be converted for this player, if it must.
fn needs_converting(file: &Path, profile: &Profile, conv: &crate::convert::Conversion) -> bool {
    let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let kind = match ext.as_str() {
        "m4a" | "mp4" | "m4b" => {
            if crate::convert::is_alac(file) {
                "alac"
            } else {
                "aac"
            }
        }
        "oga" => "ogg",
        e => e,
    };
    !profile.plays.contains(&kind) || (kind == "alac" && conv.convert_alac)
}

/// Characters FAT drives (and Windows) don't allow, and names they choke on.
fn fat_name(s: &str, max: usize) -> String {
    let c: String = s.chars().map(|c| if "/\\:*?\"<>|".contains(c) || c.is_control() { '_' } else { c }).collect();
    let c: String = c.trim().trim_end_matches('.').chars().take(max).collect();
    let c = c.trim().to_string();
    if c.is_empty() {
        "Unknown".into()
    } else {
        c
    }
}

/// Where a song goes on the player: <music>/<Artist>/<Album>/<NN Title>.<ext>.
fn place(music: &Path, info: &chinotto_gpod::TrackInfo, file: &Path) -> PathBuf {
    let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("mp3").to_ascii_lowercase();
    let artist = if info.compilation {
        "Compilations".to_string()
    } else {
        info.album_artist.clone().or_else(|| info.artist.clone()).unwrap_or_else(|| "Unknown Artist".into())
    };
    let album = info.album.clone().unwrap_or_else(|| "Unknown Album".into());
    let title = info.title.clone().unwrap_or_else(|| file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
    let nr = match (info.disc_nr, info.track_nr) {
        (d, t) if d > 1 && t > 0 => format!("{d}-{t:02} "),
        (_, t) if t > 0 => format!("{t:02} "),
        _ => String::new(),
    };
    music.join(fat_name(&artist, 64)).join(fat_name(&album, 64)).join(format!("{}.{ext}", fat_name(&format!("{nr}{title}"), 80)))
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CopyResult {
    added: usize,
    /// Already on the player (same place, same size).
    already: usize,
    skipped: Vec<String>,
}

/// Copy songs (library or Scratch songs by dbid, and/or files and folders)
/// onto the player, converting what it can't play.
#[tauri::command]
pub async fn msc_add(path: String, dbids: Vec<String>, files: Vec<String>, app: AppHandle) -> Result<CopyResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (root, cfg, profile) = player(&path)?;
        let state = app.state::<AppState>();
        if !state.is_writable(&identity(&cfg)) {
            return Err("Changes to this player are switched off. Turn on “Allow changes” on its page first.".into());
        }
        let conv = conversion_for(&app, &cfg, &profile);
        let music = root.join(&cfg.music_folder);
        let mut res = CopyResult::default();
        let mut sources: Vec<PathBuf> = Vec::new();
        for d in &dbids {
            let f = if d.starts_with("scratch:") {
                crate::scratch::track_file(&app, d)
            } else if d.starts_with("lib:") {
                crate::libcommands::track_file(&app, d)
            } else {
                // A song on the open iPod.
                d.parse::<u64>().map_err(|_| "bad id".to_string()).and_then(|id| crate::playback::track_file(&app, id))
            };
            match f {
                Ok(f) => sources.push(f),
                Err(e) => res.skipped.push(e),
            }
        }
        sources.extend(crate::write::expand(&files).into_iter().filter(|f| crate::tags::library_kind(f).is_some()));
        let tmp = crate::convert::TempDir::new()?;
        let total = sources.len();
        for (i, src) in sources.iter().enumerate() {
            let name = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let _ = app.emit("add-progress", crate::write::AddProgress { done: i, total, current: name.clone() });
            let converted;
            let f = if needs_converting(src, &profile, &conv) {
                match crate::convert::convert(src, &tmp.0, &conv, i) {
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
            let info = crate::tags::read(f).unwrap_or_default();
            let mut dest = place(&music, &info, f);
            let size = std::fs::metadata(f).map(|m| m.len()).unwrap_or(0);
            if dest.exists() {
                // The same song copied before (the place comes from its tags).
                if std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0) == size || f != src {
                    res.already += 1;
                    continue;
                }
                let stem = dest.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let ext = dest.extension().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let mut n = 2;
                while dest.exists() {
                    dest = dest.with_file_name(format!("{stem} ({n}).{ext}"));
                    n += 1;
                }
            }
            if let Err(e) = copy_in(f, &dest, &profile) {
                res.skipped.push(format!("{name}: {e}"));
                continue;
            }
            res.added += 1;
        }
        let _ = app.emit("add-progress", crate::write::AddProgress { done: total, total, current: String::new() });
        drop(tmp);
        crate::libcommands::start_scan(&app, &cfg.library_id());
        Ok(res)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Copy `from` to `to` through a hidden part file, so a half-copied song
/// never has its real name.
fn copy_in(from: &Path, to: &Path, profile: &Profile) -> Result<(), String> {
    let dir = to.parent().ok_or("bad place")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("couldn't create {} ({e})", dir.display()))?;
    // Hidden, and keeping its extension (the tag code goes by it).
    let stem = to.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = to.extension().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let part = dir.join(format!(".{stem}.part.{ext}"));
    std::fs::copy(from, &part).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        if e.raw_os_error() == Some(28) {
            "the player is full".to_string()
        } else {
            format!("couldn't copy it ({e})")
        }
    })?;
    let mp3 = to.extension().is_some_and(|e| e.eq_ignore_ascii_case("mp3"));
    if let Some((px, max)) = profile.small_covers {
        // The song's cover (or its folder's), made small enough for the player.
        // Not fatal: without it, only the cover is missing.
        let cover = crate::tags::embedded_cover(from)
            .or_else(|| crate::library::folder_cover(from.parent()?).and_then(|p| std::fs::read(p).ok()))
            .and_then(|b| small_jpeg(&b, px, max).ok());
        if let Some(jpeg) = &cover {
            if mp3 {
                let _ = crate::tagwrite::write(
                    &part,
                    &[],
                    Some(&crate::tagwrite::CoverChange::Set(jpeg.clone(), lofty::picture::MimeType::Jpeg)),
                );
            }
            if profile.folder_jpg {
                let folder = dir.join("folder.jpg");
                if !folder.exists() {
                    let _ = std::fs::write(&folder, jpeg);
                }
            }
        }
    }
    if profile.id3v23 && mp3 {
        // Not fatal: the song plays either way.
        let _ = crate::tagwrite::resave_id3v23(&part);
    }
    std::fs::rename(&part, to).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        format!("couldn't finish copying it ({e})")
    })
}

/// A cover as a baseline JPEG of at most `px` a side and `max` bytes (lower
/// quality until it fits).
fn small_jpeg(bytes: &[u8], px: u32, max: usize) -> Result<Vec<u8>, String> {
    if bytes.len() <= max && bytes.starts_with(&[0xFF, 0xD8]) && !is_progressive(bytes) && img_fits(bytes, px) {
        return Ok(bytes.to_vec()); // already fine
    }
    let img = crate::cover::decode(bytes)?;
    let small = if img.width() > px || img.height() > px { img.resize(px, px, image::imageops::FilterType::Lanczos3) } else { img };
    let rgb = small.to_rgb8();
    let mut jpeg = Vec::new();
    for q in [90u8, 80, 70, 60, 50, 40] {
        jpeg.clear();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, q).encode_image(&rgb).map_err(|e| e.to_string())?;
        if jpeg.len() <= max {
            break;
        }
    }
    Ok(jpeg)
}

/// A progressive JPEG (SOF2 marker), which many old players can't show.
fn is_progressive(jpeg: &[u8]) -> bool {
    jpeg.windows(2).any(|w| w == [0xFF, 0xC2])
}

fn img_fits(bytes: &[u8], px: u32) -> bool {
    crate::cover::decode(bytes).is_ok_and(|i| i.width() <= px && i.height() <= px)
}

/// Delete songs (the player library's song ids) from the player. Only files
/// inside its music folder are touched; folders left empty go too.
#[tauri::command]
pub fn msc_delete(path: String, ids: Vec<u32>, app: AppHandle) -> Result<Vec<String>, String> {
    let (root, cfg, _) = player(&path)?;
    let state = app.state::<AppState>();
    if !state.is_writable(&identity(&cfg)) {
        return Err("Changes to this player are switched off. Turn on “Allow changes” on its page first.".into());
    }
    let music = root.join(&cfg.music_folder);
    let files: Vec<PathBuf> = {
        let libs = state.libraries.lock().unwrap();
        let lib = libs.list.iter().find(|l| l.id == cfg.library_id()).ok_or("The player's songs aren't listed yet")?;
        ids.iter().filter_map(|id| lib.track(*id)).map(|t| PathBuf::from(&t.path)).collect()
    };
    let mut problems = Vec::new();
    for f in files {
        if !f.starts_with(&music) {
            problems.push(format!("{}: not in the player's music folder, so left alone", f.display()));
            continue;
        }
        if let Err(e) = std::fs::remove_file(&f) {
            problems.push(format!("{}: {e}", f.display()));
            continue;
        }
        // Tidy up folders the song leaves empty (never the music folder itself);
        // a folder.jpg or hidden files left on their own go with it.
        let mut dir = f.parent().map(Path::to_path_buf);
        while let Some(d) = dir {
            if d == music || !d.starts_with(&music) {
                break;
            }
            let rest: Vec<PathBuf> = std::fs::read_dir(&d).map(|rd| rd.flatten().map(|e| e.path()).collect()).unwrap_or_default();
            let leftovers = rest.iter().all(|p| {
                let n = p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
                p.is_file() && (n == "folder.jpg" || n.starts_with('.'))
            });
            if leftovers {
                for p in &rest {
                    let _ = std::fs::remove_file(p);
                }
            }
            if std::fs::remove_dir(&d).is_err() {
                break;
            }
            dir = d.parent().map(Path::to_path_buf);
        }
    }
    crate::libcommands::start_scan(&app, &cfg.library_id());
    Ok(problems)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_playlist_entries() {
        let m3u = "\u{feff}#EXTM3U\n#EXTINF:123,Artist - Title\nArtist\\Album\\01 Song.mp3\n\n/MUSIC/Other.mp3\n";
        assert_eq!(entries(m3u, false), vec!["Artist\\Album\\01 Song.mp3", "/MUSIC/Other.mp3"]);
        let pls = "[playlist]\nFile1=a.mp3\nTitle1=A\nFile2=../b.mp3\nNumberOfEntries=2\n";
        assert_eq!(entries(pls, true), vec!["a.mp3", "../b.mp3"]);
        let root = Path::new("/media/W");
        let base = Path::new("/media/W/MUSIC");
        assert_eq!(resolve("Artist\\Album\\01 Song.mp3", base, Some(root)).unwrap(), Path::new("/media/W/MUSIC/Artist/Album/01 Song.mp3"));
        assert_eq!(resolve("/MUSIC/Other.mp3", base, Some(root)).unwrap(), Path::new("/media/W/MUSIC/Other.mp3"));
        assert_eq!(resolve("E:\\MUSIC\\x.mp3", base, Some(root)).unwrap(), Path::new("/media/W/MUSIC/x.mp3"));
        assert_eq!(resolve("../b.mp3", base, Some(root)).unwrap(), Path::new("/media/W/b.mp3"));
        assert_eq!(resolve("http://radio", base, Some(root)), None);
    }

    #[test]
    fn places_songs_by_artist_and_album() {
        let info = chinotto_gpod::TrackInfo {
            title: Some("Sour Times: Live?".into()),
            artist: Some("Portishead".into()),
            album: Some("Dummy".into()),
            track_nr: 2,
            ..Default::default()
        };
        let p = place(Path::new("/W/MUSIC"), &info, Path::new("/x/a.MP3"));
        assert_eq!(p, Path::new("/W/MUSIC/Portishead/Dummy/02 Sour Times_ Live_.mp3"));
        let comp = chinotto_gpod::TrackInfo { compilation: true, album: Some("Trainspotting".into()), ..Default::default() };
        assert_eq!(
            place(Path::new("/W"), &comp, Path::new("/x/Born Slippy.flac")),
            Path::new("/W/Compilations/Trainspotting/Born Slippy.flac")
        );
        let nwz = profiles().into_iter().find(|p| p.key == "sony-nwz").unwrap();
        let conv = crate::convert::Conversion::default();
        assert!(needs_converting(Path::new("a.flac"), &nwz, &conv));
        assert!(!needs_converting(Path::new("a.mp3"), &nwz, &conv));
    }

    /// An MP3 copied for a Walkman has an ID3v2.3 tag, cover kept.
    #[test]
    fn copies_with_id3v23() {
        let Some(src) = std::env::var_os("CHINOTTO_TEST_AUDIO").map(PathBuf::from) else { return };
        let dir = std::env::temp_dir().join(format!("chinotto-msc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let from = dir.join("in.mp3");
        std::fs::copy(src.join("sine-440-3s.mp3"), &from).unwrap();
        let png = {
            let img = image::RgbImage::from_pixel(8, 8, image::Rgb([1, 2, 3]));
            let mut v = Vec::new();
            image::DynamicImage::ImageRgb8(img).write_to(&mut std::io::Cursor::new(&mut v), image::ImageFormat::Png).unwrap();
            v
        };
        crate::tagwrite::write(
            &from,
            &[chinotto_gpod::Field::Title(Some("T".into()))],
            Some(&crate::tagwrite::CoverChange::Set(png, lofty::picture::MimeType::Png)),
        )
        .unwrap();
        let to = dir.join("A/B/01 T.mp3");
        let nwz = profiles().into_iter().find(|p| p.key == "sony-nwz").unwrap();
        copy_in(&from, &to, &nwz).unwrap();
        let head = std::fs::read(&to).unwrap();
        assert_eq!((&head[..3], head[3]), (&b"ID3"[..], 3));
        assert!(crate::tags::embedded_cover(&to).is_some());
        assert_eq!(std::fs::read_dir(to.parent().unwrap()).unwrap().count(), 1, "no part file left");
        // For the Sansa e200: a big cover becomes a small baseline JPEG.
        let big = {
            let img = image::RgbImage::from_fn(900, 900, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x * y) % 256) as u8]));
            let mut v = Vec::new();
            image::DynamicImage::ImageRgb8(img).write_to(&mut std::io::Cursor::new(&mut v), image::ImageFormat::Png).unwrap();
            v
        };
        crate::tagwrite::write(&from, &[], Some(&crate::tagwrite::CoverChange::Set(big, lofty::picture::MimeType::Png))).unwrap();
        let e200 = profiles().into_iter().find(|p| p.key == "sansa-e200").unwrap();
        let to2 = dir.join("C/02 T.mp3");
        copy_in(&from, &to2, &e200).unwrap();
        let cover = crate::tags::embedded_cover(&to2).unwrap();
        assert!(cover.starts_with(&[0xFF, 0xD8]) && cover.len() <= 30_000 && !is_progressive(&cover), "{} bytes", cover.len());
        assert!(img_fits(&cover, 300));
        let folder = std::fs::read(dir.join("C/folder.jpg")).expect("folder.jpg for the e200");
        assert!(folder.len() <= 30_000 && !is_progressive(&folder));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn folder_names_stay_inside() {
        assert_eq!(clean_folder("/MUSIC/").unwrap(), "MUSIC");
        assert_eq!(clean_folder("Music\\Pop").unwrap(), "Music/Pop");
        assert!(clean_folder("../etc").is_err());
        assert_eq!(clean_folder("").unwrap(), "");
    }
}
