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
    pub icon: &'static str,
    pub capacity: u64,
    pub free: u64,
    /// Added by hand with "Open iPod folder…" rather than found mounted.
    pub manual: bool,
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
    let sys = SysInfo::read(root).unwrap_or_default();
    let (capacity, free) = disk_space(root);
    DeviceInfo {
        path: root.to_string_lossy().into_owned(),
        volume: root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "iPod".into()),
        model: sys.model_name().map(str::to_string),
        model_number: sys.model_number.clone(),
        icon: sys.generation().map(|g| g.icon).unwrap_or("generic"),
        capacity,
        free,
        manual,
    }
}

/// All iPods currently reachable: mounted ones plus folders added by hand.
pub fn scan(manual: &[PathBuf]) -> Vec<DeviceInfo> {
    let mut roots: Vec<(PathBuf, bool)> = Vec::new();
    for parent in mount_parents() {
        let Ok(rd) = std::fs::read_dir(&parent) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if is_ipod_root(&p) && !roots.iter().any(|(r, _)| r == &p) {
                roots.push((p, false));
            }
        }
    }
    for m in manual {
        if is_ipod_root(m) && !roots.iter().any(|(r, _)| r == m) {
            roots.push((m.clone(), true));
        }
    }
    roots.iter().map(|(p, manual)| describe(p, *manual)).collect()
}

/// Poll every two seconds and tell the window when the set of iPods changes.
pub fn start_watcher(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last: Vec<String> = Vec::new();
        loop {
            let manual = app.state::<AppState>().manual_roots();
            let list = scan(&manual);
            let paths: Vec<String> = list.iter().map(|d| d.path.clone()).collect();
            if paths != last {
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
