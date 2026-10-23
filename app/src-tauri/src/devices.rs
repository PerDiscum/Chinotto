//! Finding mounted iPods, watching for plug-in/unplug, and ejecting.

use std::path::{Path, PathBuf};
use std::time::Duration;

use chinotto_ipod::device::{is_ipod_root, SysInfo};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    /// Mount point (iPod root).
    pub path: String,
    /// Volume name as the OS shows it, e.g. "CAESAR".
    pub volume: String,
    pub model: Option<String>,
    pub model_number: Option<String>,
    /// Picture key for the model, e.g. "ipod-nano-3g"; "generic" when unknown.
    pub icon: String,
    /// FireWire GUID or serial, for remembering settings per device.
    pub identity: Option<String>,
    pub capacity: u64,
    pub free: u64,
    /// Added by hand with "Open iPod folder…" rather than found mounted.
    pub manual: bool,
    /// An iPod without a database (iPod_Control but no iTunesDB/iTunesCDB): it needs
    /// setting up before it can be used.
    pub needs_setup: bool,
    /// "ipod", or "msc" for a player set up as a drive (see msc.rs).
    pub kind: &'static str,
    /// MSC player: the library listing its songs.
    pub library: Option<String>,
}

/// Folders whose children are mount points on this OS.
fn mount_parents() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if cfg!(target_os = "macos") {
        out.push(PathBuf::from("/Volumes"));
    } else {
        // udisks: /media/<user>/<label> (Debian/Ubuntu), /run/media/<user>/<label> (Fedora/Arch)
        for base in ["/media", "/run/media"] {
            out.push(PathBuf::from(base));
            if let Ok(rd) = std::fs::read_dir(base) {
                out.extend(rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
            }
        }
        out.push(PathBuf::from("/mnt"));
    }
    out
}

/// True when `path` is the top of a mounted disk (not just a folder on one).
#[cfg(unix)]
pub fn is_mount_point(path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let (Ok(me), Some(parent)) = (std::fs::metadata(path), path.parent()) else { return false };
    std::fs::metadata(parent).is_ok_and(|p| p.dev() != me.dev())
}

#[cfg(not(unix))]
pub fn is_mount_point(_path: &Path) -> bool {
    false
}

pub fn describe(root: &Path, manual: bool) -> DeviceInfo {
    // A folder that isn't its own disk (e.g. a copy of an iPod) can't be ejected.
    let manual = manual || !is_mount_point(root);
    if crate::msc::is_openmg_root(root) {
        let (capacity, free) = disk_space(root);
        return DeviceInfo {
            path: root.to_string_lossy().into_owned(),
            volume: root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "WALKMAN".into()),
            model: Some("Sony Walkman S-series (OpenMG)".into()),
            model_number: None,
            icon: "sony-nwz".into(),
            identity: None,
            capacity,
            free,
            manual,
            needs_setup: false,
            kind: "openmg",
            library: None,
        };
    }
    if !is_ipod_root(root) && !needs_setup(root) {
        if let Some(cfg) = crate::msc::read_config(root) {
            let (capacity, free) = disk_space(root);
            return DeviceInfo {
                path: root.to_string_lossy().into_owned(),
                volume: cfg.name.clone(),
                model: crate::msc::profiles().into_iter().find(|p| p.key == cfg.model).map(|p| p.name.to_string()),
                model_number: None,
                icon: if cfg.icon.is_empty() { "generic".into() } else { cfg.icon.clone() },
                identity: Some(format!("msc:{}", cfg.id)),
                capacity,
                free,
                manual,
                needs_setup: false,
                kind: "msc",
                library: Some(cfg.library_id()),
            };
        }
    }
    let sys = SysInfo::read(root).unwrap_or_default();
    let generation = chinotto_ipod::device::generation_at_root(root);
    let (capacity, free) = disk_space(root);
    DeviceInfo {
        path: root.to_string_lossy().into_owned(),
        volume: root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "iPod".into()),
        model: sys.model_name().map(str::to_string).or_else(|| generation.map(|g| g.name.to_string())),
        model_number: sys.model_number.clone(),
        icon: generation.map(|g| g.icon).unwrap_or("generic").to_string(),
        identity: sys.identity().or_else(|| chinotto_ipod::device::is_nano6_root(root).then(|| crate::setup::usb_serial(root)).flatten()),
        capacity,
        free,
        manual,
        needs_setup: !is_ipod_root(root),
        kind: "ipod",
        library: None,
    }
}

/// An iPod's folder without a database: the iPod_Control folder is there,
/// the iTunesDB isn't (formatted or restored without iTunes, or lost).
pub fn needs_setup(root: &Path) -> bool {
    root.join("iPod_Control").is_dir() && !is_ipod_root(root)
}

/// Mounted volumes (just their paths: listing the mount folder doesn't touch
/// the volumes themselves).
fn mounted() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for parent in mount_parents() {
        let Ok(rd) = std::fs::read_dir(&parent) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            // macOS lists the startup disk in /Volumes as a link to "/".
            if e.file_type().is_ok_and(|t| t.is_symlink()) || out.contains(&p) {
                continue;
            }
            out.push(p);
        }
    }
    out
}

/// What's known about each volume and added folder: its iPod details, or
/// `None` when it isn't one. Each is looked inside only once, when it first
/// appears, because on macOS every look inside a removable volume can bring
/// up a permission prompt.
#[derive(Default)]
pub struct Scanner {
    known: std::collections::HashMap<PathBuf, (std::time::Instant, Option<DeviceInfo>)>,
}

impl Scanner {
    /// All iPods currently reachable: mounted ones plus folders added by hand.
    pub fn scan(&mut self, manual: &[PathBuf]) -> Vec<DeviceInfo> {
        let mut now: Vec<(PathBuf, bool)> = mounted().into_iter().map(|p| (p, false)).collect();
        for m in manual {
            if !now.iter().any(|(p, _)| p == m) {
                now.push((m.clone(), true));
            }
        }
        self.known.retain(|p, _| now.iter().any(|(q, _)| q == p));
        let mut out = Vec::new();
        for (p, manual) in now {
            let probe = |p: &Path| {
                (is_ipod_root(p) || needs_setup(p) || crate::msc::is_openmg_root(p) || p.join(crate::msc::CONFIG).is_file())
                    .then(|| describe(p, manual))
            };
            let entry = self.known.entry(p.clone()).or_insert_with(|| (std::time::Instant::now(), probe(&p)));
            // A volume can show up a moment before it's readable: keep looking for a few seconds.
            if entry.1.is_none() && entry.0.elapsed() < Duration::from_secs(8) {
                entry.1 = probe(&p);
            }
            if let Some(d) = &entry.1 {
                out.push(d.clone());
            }
        }
        out
    }
}

impl Scanner {
    /// Look inside `root` again on the next scan (it was just set up).
    pub fn forget(&mut self, root: &Path) {
        self.known.remove(root);
    }
}

/// Poll every two seconds and tell the window when the set of iPods changes.
pub fn start_watcher(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last: Vec<(String, bool)> = Vec::new();
        loop {
            let manual = app.state::<AppState>().manual_roots();
            let mut list = app.state::<AppState>().scanner.lock().unwrap().scan(&manual);
            list.extend(crate::mtp::poll(&app));
            let paths: Vec<(String, bool)> = list.iter().map(|d| (d.path.clone(), d.needs_setup)).collect();
            if paths != last {
                // Players set up as drives: point their song lists at where they are now.
                for d in list.iter().filter(|d| d.kind == "msc" && !last.iter().any(|(p, _)| p == &d.path)) {
                    if let Some(cfg) = crate::msc::read_config(Path::new(&d.path)) {
                        crate::msc::attach(&app, Path::new(&d.path), &cfg);
                    }
                }
                last = paths;
                let _ = app.emit("devices-changed", &list);
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    });
}

#[cfg(unix)]
fn disk_space(path: &Path) -> (u64, u64) {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let Ok(c) = CString::new(path.as_os_str().as_bytes()) else { return (0, 0) };
    // SAFETY: statvfs fills the zeroed struct; we only read it on success.
    unsafe {
        let mut s: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c.as_ptr(), &mut s) != 0 {
            return (0, 0);
        }
        let frsize = s.f_frsize as u64;
        (s.f_blocks as u64 * frsize, s.f_bavail as u64 * frsize)
    }
}

#[cfg(not(unix))]
fn disk_space(_path: &Path) -> (u64, u64) {
    (0, 0)
}

/// Unmount (and on Linux power off) the disk mounted at `path`.
pub fn eject(path: &Path) -> Result<(), String> {
    // Never unmount the disk that merely contains a folder.
    if !is_mount_point(path) {
        return Err(format!("{} is a folder, not a disk, so there's nothing to eject", path.display()));
    }
    #[cfg(target_os = "macos")]
    {
        return run("diskutil", &["eject", &path.to_string_lossy()]);
    }
    #[cfg(target_os = "linux")]
    {
        let Some(dev) = linux_device_for(path) else {
            return Err(format!("{} is not a mounted disk", path.display()));
        };
        if run("udisksctl", &["unmount", "-b", &dev]).is_err() {
            run("umount", &[&path.to_string_lossy()])?;
        }
        // Spin down so the iPod shows "OK to disconnect"; not fatal if it fails.
        let _ = run("udisksctl", &["power-off", "-b", &dev]);
        return Ok(());
    }
    #[allow(unreachable_code)]
    Err("Eject is not supported on this system yet".into())
}

#[cfg(target_os = "linux")]
fn linux_device_for(path: &Path) -> Option<String> {
    let mounts = std::fs::read_to_string("/proc/self/mounts").ok()?;
    let want = path.to_string_lossy();
    mounts.lines().find_map(|l| {
        let mut f = l.split(' ');
        let dev = f.next()?;
        let mnt = f.next()?.replace("\\040", " ").replace("\\011", "\t");
        (mnt == want && dev.starts_with("/dev/")).then(|| dev.to_string())
    })
}

#[allow(dead_code)]
fn run(cmd: &str, args: &[&str]) -> Result<(), String> {
    let out = std::process::Command::new(cmd).args(args).output().map_err(|e| format!("{cmd}: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if msg.is_empty() { format!("{cmd} failed") } else { msg })
    }
}
