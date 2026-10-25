//! Setting up an iPod that has no database (freshly formatted or restored),
//! and rebuilding one whose database is lost or broken — so a Mac or Linux
//! user never needs iTunes for it.
//!
//! What a database needs from the iPod:
//! - its model, recorded in `iPod_Control/Device/SysInfo` (chosen here, or
//!   already there);
//! - for the iPod classic and nano 3G, its FireWire ID: their database carries
//!   a checksum made from it, and without one they show no music. The ID is
//!   the iPod's USB serial number, which Chinotto reads from the system
//!   (sysfs on Linux, system_profiler on macOS) and writes into SysInfo.
//!
//! Rebuilding first moves the old database and artwork into
//! `iPod_Control/iTunes/Chinotto Backups/Before rebuild <time>/` (nothing is
//! deleted); the songs already on the iPod can then be added back from their
//! files.

use std::path::{Path, PathBuf};

use chinotto_gpod::Database;
use chinotto_ipod::device::{Layout, SysInfo};
use chinotto_ipod::models;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;
use crate::write::AddProgress;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelChoice {
    /// e.g. "xC297"
    number: &'static str,
    name: &'static str,
    icon: &'static str,
    needs_id: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupInfo {
    path: String,
    volume: String,
    capacity: u64,
    /// The folder is the top of a disk (not just a folder on one).
    is_disk: bool,
    has_database: bool,
    /// Model number the iPod already reports, if any (e.g. "xC297").
    model: Option<String>,
    /// FireWire ID: from SysInfo, or else the USB serial number.
    firewire_id: Option<String>,
    /// Music files already in iPod_Control/Music.
    music_files: usize,
    models: Vec<ModelChoice>,
}

/// Audio files under iPod_Control/Music (the F00…F49 folders).
fn music_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![Layout::new(root).control().join("Music")];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')) {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
            } else if crate::tags::ipod_kind(&p).is_some() {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// 16 hex digits, as the iPod reports its FireWire ID.
fn clean_id(s: &str) -> Option<String> {
    let s = s.trim().trim_start_matches("0x").trim_start_matches("0X").to_ascii_uppercase();
    (s.len() == 16 && s.chars().all(|c| c.is_ascii_hexdigit())).then_some(s)
}

/// The iPod's USB serial number (= its FireWire ID) for the disk mounted at `root`.
#[cfg(target_os = "linux")]
pub(crate) fn usb_serial(root: &Path) -> Option<String> {
    let mounts = std::fs::read_to_string("/proc/self/mounts").ok()?;
    let want = root.to_string_lossy();
    let dev = mounts.lines().find_map(|l| {
        let mut f = l.split(' ');
        let dev = f.next()?;
        let mnt = f.next()?.replace("\\040", " ");
        (mnt == want).then(|| dev.trim_start_matches("/dev/").to_string())
    })?;
    // /sys/class/block/sdb2 → …/usbN/N-M/…/block/sdb/sdb2: walk up to the USB device.
    let mut dir = std::fs::canonicalize(Path::new("/sys/class/block").join(dev)).ok()?;
    while dir.pop() {
        if let Ok(vendor) = std::fs::read_to_string(dir.join("idVendor")) {
            if vendor.trim() != "05ac" {
                return None;
            }
            return clean_id(&std::fs::read_to_string(dir.join("serial")).ok()?);
        }
    }
    None
}

#[cfg(target_os = "macos")]
pub(crate) fn usb_serial(root: &Path) -> Option<String> {
    use serde_json::Value;
    let want = root.to_string_lossy().to_string();
    // Apple devices' serial numbers, and whether each has the volume mounted at `root`.
    fn walk(v: &Value, want: &str, out: &mut Vec<(String, bool)>) {
        match v {
            Value::Array(a) => a.iter().for_each(|x| walk(x, want, out)),
            Value::Object(o) => {
                let serial = o.get("serial_num").or_else(|| o.get("USBDeviceKeySerialNumber")).and_then(|s| s.as_str());
                let vendor = o.get("vendor_id").or_else(|| o.get("USBDeviceKeyVendorID")).and_then(|s| s.as_str()).unwrap_or("");
                if let Some(s) = serial {
                    if vendor.contains("05ac") || vendor.to_ascii_lowercase().contains("apple") {
                        let text = v.to_string();
                        out.push((s.to_string(), text.contains(&format!("\"mount_point\":{}", Value::from(want)))));
                    }
                }
                o.values().for_each(|x| walk(x, want, out));
            }
            _ => {}
        }
    }
    let mut found = Vec::new();
    for kind in ["SPUSBDataType", "SPUSBHostDataType"] {
        let out = std::process::Command::new("/usr/sbin/system_profiler").args([kind, "-json"]).output().ok()?;
        if let Ok(v) = serde_json::from_slice::<Value>(&out.stdout) {
            walk(&v, &want, &mut found);
        }
    }
    let ids: Vec<(String, bool)> = found.into_iter().filter_map(|(s, here)| clean_id(&s).map(|s| (s, here))).collect();
    // The one with this volume; else the only Apple device with a FireWire-style serial.
    ids.iter().find(|(_, here)| *here).or_else(|| ids.first().filter(|_| ids.len() == 1)).map(|(s, _)| s.clone())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(crate) fn usb_serial(_root: &Path) -> Option<String> {
    None
}

#[tauri::command]
pub async fn setup_info(path: String) -> Result<SetupInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = PathBuf::from(&path);
        if !root.is_dir() {
            return Err(format!("{path} isn't there"));
        }
        let sys = SysInfo::read(&root).unwrap_or_default();
        let firewire_id = sys.firewire_guid.as_deref().and_then(clean_id).or_else(|| usb_serial(&root));
        let models = models::setup_models()
            .into_iter()
            .map(|(number, g)| ModelChoice { number, name: g.name, icon: g.icon, needs_id: models::needs_firewire_id(&g) })
            .collect();
        // The model the iPod reports; else, from its serial number.
        let model = sys.model_number.clone().or_else(|| {
            let g = sys.generation()?;
            models::setup_models().into_iter().find(|(_, x)| x.icon == g.icon && x.name == g.name).map(|(n, _)| n.to_string())
        });
        Ok(SetupInfo {
            volume: root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone()),
            capacity: crate::devices::describe(&root, false).capacity,
            is_disk: crate::devices::is_mount_point(&root),
            has_database: Layout::new(&root).has_database(),
            model,
            firewire_id,
            music_files: music_files(&root).len(),
            models,
            path,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Record the FireWire ID in SysInfo (keeping what's there), so libgpod can
/// make the database checksum.
fn write_firewire_id(root: &Path, id: &str) -> Result<(), String> {
    let path = Layout::new(root).sysinfo();
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| format!("Couldn't create the Device folder: {e}"))?;
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<String> = old.lines().filter(|l| !l.trim_start().starts_with("FirewireGuid:")).map(str::to_string).collect();
    lines.push(format!("FirewireGuid: 0x{id}"));
    std::fs::write(&path, lines.join("\n") + "\n").map_err(|e| format!("Couldn't write SysInfo: {e}"))
}

/// Playlists and ratings of the database being replaced, by the songs' places
/// on the iPod (the music files stay where they are, so they find their songs again).
struct Kept {
    ratings: std::collections::HashMap<String, u32>,
    playlists: Vec<(String, Vec<String>)>,
}

impl Kept {
    fn read(root: &Path) -> Result<Kept, String> {
        let db = Database::open(root).map_err(|e| e.to_string())?;
        let mut path_of = std::collections::HashMap::new();
        let mut ratings = std::collections::HashMap::new();
        for id in db.track_ids() {
            if let Some(t) = db.track(id) {
                if let Some(p) = t.ipod_path {
                    let p = p.to_lowercase();
                    if t.rating > 0 {
                        ratings.insert(p.clone(), t.rating);
                    }
                    path_of.insert(id, p);
                }
            }
        }
        let playlists = db
            .playlists()
            .into_iter()
            .filter(|p| !p.master && !p.smart)
            .map(|p| (p.name, p.tracks.iter().filter_map(|id| path_of.get(id).cloned()).collect()))
            .collect();
        Ok(Kept { ratings, playlists })
    }

    fn apply(&self, db: &mut Database, problems: &mut Vec<String>) -> Result<(), String> {
        let mut by_path = std::collections::HashMap::new();
        for id in db.track_ids() {
            if let Some(p) = db.track(id).and_then(|t| t.ipod_path) {
                by_path.insert(p.to_lowercase(), id);
            }
        }
        for (path, rating) in &self.ratings {
            if let Some(id) = by_path.get(path) {
                db.edit(&[*id], &[chinotto_gpod::Field::Rating(*rating)]).map_err(|e| e.to_string())?;
            }
        }
        for (name, paths) in &self.playlists {
            let ids: Vec<u64> = paths.iter().filter_map(|p| by_path.get(p).copied()).collect();
            if ids.len() < paths.len() {
                problems.push(format!("Playlist “{name}”: {} of its songs weren't found on the iPod.", paths.len() - ids.len()));
            }
            let pl = db.create_playlist(name).map_err(|e| e.to_string())?;
            db.set_playlist_tracks(pl, &ids).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupResult {
    added: usize,
    problems: Vec<String>,
    /// Where the old database went (rebuilds).
    set_aside: Option<String>,
}

/// The setup itself (see [`setup_ipod`]).
fn run(
    root: &Path,
    model: &str,
    name: &str,
    id: Option<&str>,
    add_existing: bool,
    rebuild: bool,
    progress: impl Fn(AddProgress),
) -> Result<SetupResult, String> {
    if chinotto_ipod::device::is_nano6_root(root) {
        return Err("Nano 6 setup and rebuild are not supported yet; its compressed and SQLite databases must be handled together.".into());
    }
    let has_db = Layout::new(root).has_database();
    // What can be carried over from the old database, while it can still be read.
    let old = if rebuild && has_db && add_existing { Some(Kept::read(root)) } else { None };
    let set_aside = if rebuild && has_db {
        Some(chinotto_gpod::set_aside_database(root).map_err(|e| e.to_string())?)
    } else if has_db {
        return Err("This iPod already has a database. Use Rebuild Database to replace it.".into());
    } else {
        None
    };
    if let Some(id) = &id {
        write_firewire_id(root, id)?;
    }
    let name = if name.trim().is_empty() { "iPod".to_string() } else { name.trim().to_string() };
    chinotto_gpod::init_ipod(root, Some(model), &name).map_err(|e| e.to_string())?;

    let mut problems = Vec::new();
    let mut added = 0;
    if add_existing {
        let files = music_files(root);
        let mut db = Database::open(root).map_err(|e| e.to_string())?;
        let mut added_files = Vec::new();
        let total = files.len();
        for (i, f) in files.iter().enumerate() {
            let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if i % 25 == 0 {
                progress(AddProgress { target: root.to_string_lossy().into_owned(), done: i, total, current: crate::tags::label(f) });
            }
            match crate::tags::read(f).and_then(|info| db.add_existing(f, &info).map_err(|e| e.to_string())) {
                Ok(dbid) => added_files.push((dbid, f.clone())),
                Err(e) => problems.push(format!("{name}: {e}")),
            }
        }
        progress(AddProgress { target: root.to_string_lossy().into_owned(), done: total, total, current: String::new() });
        crate::write::add_embedded_covers(&mut db, &added_files, &mut problems)?;
        match &old {
            Some(Ok(kept)) => kept.apply(&mut db, &mut problems)?,
            Some(Err(e)) => problems.push(format!("Playlists and ratings couldn't be kept: the old database couldn't be read ({e}).")),
            None => {}
        }
        db.save().map_err(|e| e.to_string())?;
        added = added_files.len();
    }
    Ok(SetupResult { added, problems, set_aside: set_aside.map(|p| p.to_string_lossy().into_owned()) })
}

/// Set up (or, with `rebuild`, set up again) the iPod at `path`.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn setup_ipod(
    path: String,
    model: String,
    name: String,
    firewire_id: Option<String>,
    add_existing: bool,
    rebuild: bool,
    app: AppHandle,
) -> Result<SetupResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = PathBuf::from(&path);
        let gen = SysInfo { model_number: Some(model.clone()), ..Default::default() }.generation().ok_or("Unknown iPod model")?;
        let id = match firewire_id.as_deref().filter(|s| !s.trim().is_empty()) {
            Some(s) => Some(clean_id(s).ok_or("The FireWire ID should be 16 hexadecimal digits, like 000A27001B2C3D4E")?),
            None => None,
        };
        if models::needs_firewire_id(&gen) && id.is_none() {
            return Err(format!("The {} needs its FireWire ID, or it will show no music.", gen.name));
        }
        let state = app.state::<AppState>();
        // Nothing may hold the old database open.
        state.close_if(&root);
        *state.writer.lock().unwrap() = None;
        let emitter = app.clone();
        let result = run(&root, &model, &name, id.as_deref(), add_existing, rebuild, move |p| {
            let _ = emitter.emit("add-progress", p);
        })?;
        // Setting it up was the user's go-ahead to change it.
        if let Some(identity) = SysInfo::read(&root).and_then(|s| s.identity()) {
            state.set_writable(&identity, true)?;
        }
        state.scanner.lock().unwrap().forget(&root);
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn firewire_ids() {
        assert_eq!(clean_id("0x000a27001b2c3d4e").as_deref(), Some("000A27001B2C3D4E"));
        assert_eq!(clean_id(" 000A27001B2C3D4E\n").as_deref(), Some("000A27001B2C3D4E"));
        assert_eq!(clean_id("000A27"), None);
        assert_eq!(clean_id("XX000XXXYXV"), None);
    }

    /// A "formatted" iPod (folders only) with songs left in Music: set up,
    /// songs added back with covers; then rebuilt, with the old database set aside.
    #[test]
    fn sets_up_and_rebuilds() {
        let Some(src) = std::env::var_os("CHINOTTO_TEST_AUDIO").map(PathBuf::from) else { return };
        let root = std::env::temp_dir().join(format!("chinotto-setup-run-{}", std::process::id()));
        let music = root.join("iPod_Control/Music/F03");
        std::fs::create_dir_all(&music).unwrap();
        let mut copied = 0;
        for (i, ext) in ["mp3", "m4a"].iter().enumerate() {
            let from = src.join(format!("sine-440-3s.{ext}"));
            if from.is_file() {
                let to = music.join(format!("ABC{i}.{ext}"));
                std::fs::copy(&from, &to).unwrap();
                let png = {
                    let img = image::RgbImage::from_pixel(700, 700, image::Rgb([200, 30, 30]));
                    let mut v = Vec::new();
                    image::DynamicImage::ImageRgb8(img).write_to(&mut std::io::Cursor::new(&mut v), image::ImageFormat::Png).unwrap();
                    v
                };
                crate::tagwrite::write(
                    &to,
                    &[chinotto_gpod::Field::Title(Some(format!("Song {i}")))],
                    Some(&crate::tagwrite::CoverChange::Set(png, lofty::picture::MimeType::Png)),
                )
                .unwrap();
                copied += 1;
            }
        }
        assert!(crate::devices::needs_setup(&root));
        // nano 3G: needs the FireWire ID for its checksum.
        let r = run(&root, "xA978", "Test Nano", Some("000A27001B2C3D4E"), true, false, |_| {}).unwrap();
        assert_eq!(r.added, copied, "{:?}", r.problems);
        let db = Database::open(&root).unwrap();
        assert_eq!(db.track_count(), copied);
        assert!(root.join("iPod_Control/Artwork/ArtworkDB").is_file());
        assert!(root.join("iPod_Control/Artwork/F1061_1.ithmb").is_file());
        let art = chinotto_ipod::artworkdb::ArtworkDb::parse(&std::fs::read(root.join("iPod_Control/Artwork/ArtworkDB")).unwrap()).unwrap();
        assert_eq!(art.images.len(), copied);
        // The checksum is there (hash58).
        let head = std::fs::read(root.join("iPod_Control/iTunes/iTunesDB")).unwrap();
        assert_eq!(u16::from_le_bytes([head[0x30], head[0x31]]), 1);
        assert!(head[0x58..0x6C].iter().any(|b| *b != 0));
        drop(db);
        // A playlist and a rating, to be kept by the rebuild.
        let mut db = Database::open(&root).unwrap();
        let ids = db.track_ids();
        db.edit(&ids[..1], &[chinotto_gpod::Field::Rating(80)]).unwrap();
        let pl = db.create_playlist("Kept").unwrap();
        db.set_playlist_tracks(pl, &ids).unwrap();
        db.save().unwrap();
        drop(db);
        // A second setup is refused; a rebuild sets the old one aside.
        assert!(run(&root, "xA978", "Again", None, false, false, |_| {}).is_err());
        let r = run(&root, "xA978", "Again", Some("000A27001B2C3D4E"), true, true, |_| {}).unwrap();
        let aside = PathBuf::from(r.set_aside.unwrap());
        assert!(aside.join("iTunesDB").is_file() && aside.join("Artwork/ArtworkDB").is_file());
        let db = Database::open(&root).unwrap();
        assert_eq!(db.track_count(), copied);
        let kept = db.playlists().into_iter().find(|p| p.name == "Kept").expect("playlist kept");
        assert_eq!(kept.tracks.len(), copied);
        assert_eq!(db.track_ids().iter().filter(|id| db.track(**id).unwrap().rating == 80).count(), 1);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn keeps_sysinfo_when_adding_the_id() {
        let dir = std::env::temp_dir().join(format!("chinotto-setup-{}", std::process::id()));
        let dev = dir.join("iPod_Control/Device");
        std::fs::create_dir_all(&dev).unwrap();
        std::fs::write(dev.join("SysInfo"), "BoardHwName: iPod Q98\nFirewireGuid: 0x1111\nModelNumStr: xC297\n").unwrap();
        write_firewire_id(&dir, "000A27001B2C3D4E").unwrap();
        let s = SysInfo::read(&dir).unwrap();
        assert_eq!(s.firewire_guid.as_deref(), Some("0x000A27001B2C3D4E"));
        assert_eq!(s.model_number.as_deref(), Some("xC297"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
