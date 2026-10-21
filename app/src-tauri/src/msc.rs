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
use tauri::{AppHandle, Manager};

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
}

/// Known players and where they keep things. "Other" fits most players.
pub fn profiles() -> Vec<Profile> {
    let p = |key, name, icon, music_folder, playlist_folder, playlist_format| Profile {
        key,
        name,
        icon,
        music_folder,
        playlist_folder,
        playlist_format,
    };
    vec![
        p("sony-nwz", "Sony Walkman NWZ (E, S, A series…)", "sony-nwz", "MUSIC", "MUSIC", "m3u8"),
        p("sony-nwa", "Sony Walkman NW-A (A10, A20…)", "sony-a10", "MUSIC", "MUSIC", "m3u8"),
        p("sansa-e200", "SanDisk Sansa e200", "sansa-e200", "Music", "Music", "m3u"),
        p("sansa-clip", "SanDisk Sansa Clip / Fuze", "sansa-clip", "Music", "Music", "m3u"),
        p("other", "Other player", "generic", "Music", "Playlists", "m3u8"),
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
    fn folder_names_stay_inside() {
        assert_eq!(clean_folder("/MUSIC/").unwrap(), "MUSIC");
        assert_eq!(clean_folder("Music\\Pop").unwrap(), "Music/Pop");
        assert!(clean_folder("../etc").is_err());
        assert_eq!(clean_folder("").unwrap(), "");
    }
}
