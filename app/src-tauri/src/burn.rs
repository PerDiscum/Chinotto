//! Burning disc images (.iso, and .bin + .cue) to blank CDs and DVDs, at the
//! slowest speed the drive and disc allow unless asked otherwise, in one pass
//! (disc-at-once), then checked against the image where the tools can.
//!
//! - macOS: `hdiutil burn` (part of macOS) for ISO images, which verifies the
//!   disc afterwards; `cdrdao` (Homebrew) for .cue images.
//! - Linux: `wodim`/`cdrecord` for ISO on CD, `growisofs` for ISO on DVD,
//!   `cdrdao` for .cue; ISO burns are verified by reading the disc back.
//! - A .cue with one data track (MODE1, 2048 or 2352-byte sectors) needs no
//!   cdrdao: its data is taken out as an ISO, which is the same disc.

use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// Sectors on a blank 80-minute CD-R and a single-layer DVD±R, for when the
/// system doesn't say.
const CD_SECTORS: u64 = 360_000;
const DVD_SECTORS: u64 = 2_295_104;

#[derive(Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Media {
    /// "CD-R", "CD-RW", "DVD-R", "DVD+R", "DVD-RW"…
    pub kind: String,
    pub blank: bool,
    /// Free 2048-byte sectors.
    pub free_sectors: u64,
    pub dvd: bool,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BurnStatus {
    /// The burner's name; None when there's no burner.
    pub drive: Option<String>,
    pub media: Option<Media>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    pub path: String,
    /// "iso", or "cue" (a .bin with its .cue).
    pub kind: String,
    /// "Data", or the tracks' modes for a .cue ("Data + 12 audio").
    pub description: String,
    /// Size in 2048-byte sectors on the disc (for checking it fits).
    pub sectors: u64,
    pub bytes: u64,
    /// Whether it can be burned here without cdrdao.
    pub needs_cdrdao: bool,
}

/// A program: next to Chinotto in the app, else the system's, also where
/// Homebrew puts them (apps don't get the shell's PATH).
fn tool(name: &str) -> Option<PathBuf> {
    if let Some(own) = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join(name))).filter(|p| p.is_file()) {
        return Some(own);
    }
    let dirs = ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin", "/usr/sbin", "/opt/local/bin"];
    if let Some(p) = std::env::var_os("PATH") {
        for d in std::env::split_paths(&p) {
            if d.join(name).is_file() {
                return Some(d.join(name));
            }
        }
    }
    dirs.iter().map(|d| Path::new(d).join(name)).find(|p| p.is_file())
}

fn linux_device() -> Option<&'static str> {
    ["/dev/sr0", "/dev/sr1", "/dev/cdrom"].into_iter().find(|d| Path::new(d).exists())
}

fn run(prog: &str, args: &[&str]) -> String {
    Command::new(prog)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
        .unwrap_or_default()
}

/// macOS: `drutil status` →  "Type: CD-R  Name: /dev/disk4", "Space Free: … blocks: 359849 / …",
/// "Writability: appendable, blank, overwritable".
fn parse_drutil(text: &str) -> Option<Media> {
    let mut m = Media::default();
    for l in text.lines() {
        let l = l.trim();
        if let Some(rest) = l.strip_prefix("Type:") {
            m.kind = rest.trim().split("  ").next().unwrap_or("").trim().to_string();
        } else if l.starts_with("Space Free:") {
            m.free_sectors = l.split("blocks:").nth(1).and_then(|b| b.split('/').next()).and_then(|n| n.trim().parse().ok()).unwrap_or(0);
        } else if let Some(rest) = l.strip_prefix("Writability:") {
            m.blank = rest.split(',').any(|w| w.trim() == "blank");
        }
    }
    if m.kind.is_empty() || m.kind.starts_with("No Media") {
        return None;
    }
    m.dvd = m.kind.contains("DVD") || m.kind.contains("BD");
    Some(m)
}

/// Linux: udev's cdrom_id properties ("ID_CDROM_MEDIA_STATE=blank", "ID_CDROM_MEDIA_DVD_R=1"…).
fn parse_udev(text: &str) -> Option<Media> {
    let get = |k: &str| text.lines().find_map(|l| l.strip_prefix(k).and_then(|v| v.strip_prefix('='))).map(str::trim);
    get("ID_CDROM_MEDIA")?;
    let kinds = [
        ("ID_CDROM_MEDIA_CD_R", "CD-R"),
        ("ID_CDROM_MEDIA_CD_RW", "CD-RW"),
        ("ID_CDROM_MEDIA_DVD_R", "DVD-R"),
        ("ID_CDROM_MEDIA_DVD_RW", "DVD-RW"),
        ("ID_CDROM_MEDIA_DVD_PLUS_R", "DVD+R"),
        ("ID_CDROM_MEDIA_DVD_PLUS_RW", "DVD+RW"),
        ("ID_CDROM_MEDIA_DVD_PLUS_R_DL", "DVD+R DL"),
        ("ID_CDROM_MEDIA_BD_R", "BD-R"),
        ("ID_CDROM_MEDIA_CD", "CD"),
        ("ID_CDROM_MEDIA_DVD", "DVD"),
    ];
    let kind = kinds.iter().find(|(k, _)| get(k) == Some("1")).map(|(_, n)| n.to_string()).unwrap_or_else(|| "Disc".into());
    let dvd = kind.contains("DVD") || kind.contains("BD");
    let blank = get("ID_CDROM_MEDIA_STATE") == Some("blank");
    let dl = kind.contains("DL");
    Some(Media {
        free_sectors: if !blank {
            0
        } else if dl {
            DVD_SECTORS * 181 / 100
        } else if dvd {
            DVD_SECTORS
        } else {
            CD_SECTORS
        },
        kind,
        blank,
        dvd,
    })
}

#[tauri::command]
pub async fn burn_status() -> BurnStatus {
    tauri::async_runtime::spawn_blocking(status_now).await.unwrap_or_default()
}

fn status_now() -> BurnStatus {
    {
        if cfg!(target_os = "macos") {
            let info = run("drutil", &["info"]);
            // "Vendor   Product           Rev\n MATSHITA DVD-R   UJ-85J    FM0S"
            let name =
                info.lines().skip_while(|l| !l.contains("Vendor")).nth(1).map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "));
            let Some(name) = name.filter(|n| !n.is_empty()) else { return BurnStatus::default() };
            BurnStatus { drive: Some(name), media: parse_drutil(&run("drutil", &["status"])) }
        } else {
            let Some(dev) = linux_device() else { return BurnStatus::default() };
            let props = run("udevadm", &["info", "--query=property", &format!("--name={dev}")]);
            let name =
                props.lines().find_map(|l| l.strip_prefix("ID_MODEL=")).map(|m| m.replace('_', " ")).unwrap_or_else(|| dev.to_string());
            BurnStatus { drive: Some(name), media: parse_udev(&props) }
        }
    }
}

/// A .cue's tracks: (data file, mode) for each TRACK line, in order.
struct Cue {
    files: Vec<PathBuf>,
    /// ("MODE1/2352", "AUDIO"…), one per track.
    modes: Vec<String>,
}

fn parse_cue(cue: &Path) -> Result<Cue, String> {
    let text = std::fs::read(cue).map_err(|e| format!("Couldn't read the .cue: {e}"))?;
    let text = String::from_utf8_lossy(&text);
    let dir = cue.parent().unwrap_or(Path::new("."));
    let mut c = Cue { files: Vec::new(), modes: Vec::new() };
    for l in text.lines() {
        let l = l.trim();
        let upper = l.to_ascii_uppercase();
        if upper.starts_with("FILE ") {
            // FILE "name.bin" BINARY  (quotes optional)
            let rest = &l[5..];
            let name = if let Some(r) = rest.strip_prefix('"') {
                r.split('"').next().unwrap_or("")
            } else {
                rest.split_whitespace().next().unwrap_or("")
            };
            let mut p = dir.join(name);
            if !p.is_file() {
                // Often the .cue names a file that was renamed with it: the .bin beside the .cue.
                let alt = cue.with_extension("bin");
                if alt.is_file() {
                    p = alt;
                } else {
                    return Err(format!("The .cue names “{name}”, which isn't next to it."));
                }
            }
            c.files.push(p);
        } else if upper.starts_with("TRACK ") {
            c.modes.push(upper.split_whitespace().nth(2).unwrap_or("").to_string());
        }
    }
    if c.files.is_empty() || c.modes.is_empty() {
        return Err("That .cue doesn't list any tracks.".into());
    }
    Ok(c)
}

/// A copy of `cue` at `out` whose FILE lines name their files by full path.
fn absolute_cue(cue: &Path, out: &Path) -> Result<(), String> {
    let c = parse_cue(cue)?;
    let text = String::from_utf8_lossy(&std::fs::read(cue).map_err(|e| e.to_string())?).into_owned();
    let mut files = c.files.iter();
    let mut lines = Vec::new();
    for l in text.lines() {
        let t = l.trim_start();
        if t.len() >= 5 && t[..5].eq_ignore_ascii_case("FILE ") {
            let path = files.next().ok_or("The .cue changed while reading it")?;
            // The file type (BINARY, WAVE…) stays as it was.
            let kind = t.rsplit(|ch: char| ch == '"' || ch.is_whitespace()).next().filter(|k| !k.is_empty()).unwrap_or("BINARY");
            lines.push(format!("FILE \"{}\" {kind}", path.display()));
        } else {
            lines.push(l.to_string());
        }
    }
    std::fs::write(out, lines.join("\n") + "\n").map_err(|e| format!("Couldn't prepare the .cue: {e}"))
}

fn sector_size(mode: &str) -> u64 {
    match mode {
        "MODE1/2048" | "MODE2/2048" => 2048,
        "MODE2/2336" => 2336,
        _ => 2352,
    }
}

/// The .cue for `path` (itself, or the .cue beside a .bin).
fn cue_for(path: &Path) -> Option<PathBuf> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "cue" => Some(path.to_path_buf()),
        "bin" | "img" => {
            let c = path.with_extension("cue");
            c.is_file().then_some(c)
        }
        _ => None,
    }
}

/// A single MODE1 data track: it can go to the disc as an ISO.
/// One track of a .cue, located in its file.
#[derive(Debug, Clone, PartialEq)]
struct PlanTrack {
    file: PathBuf,
    mode: String,
    /// Byte offset of INDEX 01 in the file.
    offset: u64,
    file_sector: u32,
    /// Sectors from INDEX 01 to the next track (or the end of the file).
    length: u32,
    /// Sectors before INDEX 01: from the file (INDEX 00), else silence (PREGAP).
    pregap: u32,
    pregap_offset: Option<u64>,
}

/// "mm:ss:ff" → frames (75 a second).
fn msf(s: &str) -> Option<u64> {
    let mut p = s.trim().split(':').map(|x| x.parse::<u64>().ok());
    let (m, sec, f) = (p.next()??, p.next()??, p.next()??);
    Some((m * 60 + sec) * 75 + f)
}

/// The tracks of a .cue: where each one's sectors are in its file.
fn cue_plan(cue: &Path) -> Result<Vec<PlanTrack>, String> {
    let c = parse_cue(cue)?;
    let text = String::from_utf8_lossy(&std::fs::read(cue).map_err(|e| e.to_string())?).into_owned();
    // (file index, mode, INDEX 00, INDEX 01, PREGAP) per track, in order.
    type Raw = (usize, String, Option<u64>, Option<u64>, u64);
    let mut raw: Vec<Raw> = Vec::new();
    let mut file: Option<usize> = None;
    for l in text.lines() {
        let w: Vec<&str> = l.split_whitespace().collect();
        match w.first().map(|x| x.to_ascii_uppercase()).as_deref() {
            Some("FILE") => file = Some(file.map_or(0, |f| f + 1)),
            Some("TRACK") => {
                raw.push((file.ok_or("The .cue has a TRACK before any FILE")?, w.get(2).unwrap_or(&"").to_ascii_uppercase(), None, None, 0))
            }
            Some("INDEX") => {
                let t = raw.last_mut().ok_or("The .cue has an INDEX before any TRACK")?;
                let at = w.get(2).and_then(|x| msf(x)).ok_or("The .cue has an INDEX it can't read")?;
                match w.get(1).and_then(|n| n.parse::<u32>().ok()) {
                    Some(0) => t.2 = Some(at),
                    Some(1) => t.3 = Some(at),
                    _ => {}
                }
            }
            Some("PREGAP") => {
                if let Some(t) = raw.last_mut() {
                    t.4 = w.get(1).and_then(|x| msf(x)).unwrap_or(0);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for (i, (f, mode, idx0, idx1, gap)) in raw.iter().enumerate() {
        let idx1 = idx1.ok_or_else(|| format!("Track {} of the .cue has no INDEX 01", i + 1))?;
        let path = c.files.get(*f).ok_or("The .cue's files don't match its tracks")?.clone();
        let sector = sector_size(mode) as u32;
        let size = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
        // Ends where the next track in the same file starts (its pregap included).
        let end = match raw.get(i + 1) {
            Some((nf, _, n0, n1, _)) if nf == f => n0.or(*n1).ok_or("A track of the .cue has no INDEX 01")?,
            _ => size / sector as u64,
        };
        let length = end.checked_sub(idx1).filter(|l| *l > 0).ok_or_else(|| format!("Track {} of the .cue has no sectors", i + 1))?;
        let (pregap, pregap_offset) = match idx0 {
            Some(z) if *z < idx1 => ((idx1 - z) as u32, Some(z * sector as u64)),
            // The first track's 2 seconds of lead-in pregap are never in the file.
            _ if i == 0 => (150, None),
            _ => (*gap as u32, None),
        };
        out.push(PlanTrack {
            file: path,
            mode: mode.clone(),
            offset: idx1 * sector as u64,
            file_sector: sector,
            length: length as u32,
            pregap,
            pregap_offset,
        });
    }
    if out.is_empty() {
        return Err("That .cue doesn't list any tracks.".into());
    }
    Ok(out)
}

#[cfg_attr(not(any(target_os = "macos", test)), allow(dead_code))]
/// The drive's block format for a track mode: (bytes skipped in each file
/// sector, block size, block type, data form, track mode).
fn dr_format(mode: &str) -> Option<(u32, u32, u32, u32, u32)> {
    Some(match mode {
        "AUDIO" => (0, 2352, 0, 0, 0),
        "MODE1/2352" => (16, 2048, 8, 16, 4),
        "MODE1/2048" => (0, 2048, 8, 16, 4),
        // Mode 2 (CD-ROM XA, PlayStation): subheader and data, 2332 bytes.
        "MODE2/2352" => (16, 2332, 13, 32, 4),
        "MODE2/2336" => (0, 2332, 13, 32, 4),
        _ => return None,
    })
}

#[cfg(target_os = "macos")]
fn dr_burn(plan: &[PlanTrack], speed: u32, verify: bool, app: &AppHandle) -> Result<(), String> {
    use std::ffi::{c_char, c_int, c_void, CString};
    #[repr(C)]
    struct DrTrack {
        path: *const c_char,
        offset: u64,
        file_sector: u32,
        skip: u32,
        block_size: u32,
        block_type: u32,
        data_form: u32,
        track_mode: u32,
        session_format: u32,
        length: u32,
        pregap: u32,
        pregap_offset: i64,
    }
    extern "C" {
        fn chinotto_dr_burn(
            tracks: *const DrTrack,
            count: c_int,
            speed_x: f64,
            verify: c_int,
            progress: extern "C" fn(*mut c_void, c_int, f64),
            ctx: *mut c_void,
            err: *mut c_char,
            errlen: usize,
        ) -> c_int;
    }
    extern "C" fn on_progress(ctx: *mut c_void, phase: c_int, fraction: f64) {
        let app = unsafe { &*(ctx as *const AppHandle) };
        progress(app, if phase == 1 { "verify" } else { "burn" }, (fraction * 1000.0) as usize);
    }
    let xa = plan.iter().any(|t| t.mode.starts_with("MODE2"));
    let paths: Vec<CString> = plan
        .iter()
        .map(|t| CString::new(t.file.to_string_lossy().as_bytes()).map_err(|_| "bad path".to_string()))
        .collect::<Result<_, _>>()?;
    let mut tracks = Vec::new();
    for (t, p) in plan.iter().zip(&paths) {
        let (skip, block_size, block_type, data_form, track_mode) =
            dr_format(&t.mode).ok_or_else(|| format!("Chinotto can't burn {} tracks yet.", t.mode))?;
        tracks.push(DrTrack {
            path: p.as_ptr(),
            offset: t.offset,
            file_sector: t.file_sector,
            skip,
            block_size,
            block_type,
            data_form,
            track_mode,
            session_format: if xa { 0x20 } else { 0 },
            length: t.length,
            pregap: t.pregap,
            pregap_offset: t.pregap_offset.map_or(-1, |o| o as i64),
        });
    }
    let mut err = vec![0 as c_char; 1024];
    let rc = unsafe {
        chinotto_dr_burn(
            tracks.as_ptr(),
            tracks.len() as c_int,
            speed as f64,
            verify as c_int,
            on_progress,
            app as *const AppHandle as *mut c_void,
            err.as_mut_ptr(),
            err.len(),
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(unsafe { std::ffi::CStr::from_ptr(err.as_ptr()) }.to_string_lossy().into_owned())
    }
}

#[cfg(not(target_os = "macos"))]
fn dr_burn(_: &[PlanTrack], _: u32, _: bool, _: &AppHandle) -> Result<(), String> {
    Err("Disc Recording is only on macOS.".into())
}

fn single_data_track(c: &Cue) -> Option<u64> {
    (c.modes.len() == 1 && c.files.len() == 1 && c.modes[0].starts_with("MODE1/")).then(|| sector_size(&c.modes[0]))
}

#[tauri::command]
pub fn burn_inspect(path: String) -> Result<ImageInfo, String> {
    let p = PathBuf::from(&path);
    if let Some(cue) = cue_for(&p) {
        let c = parse_cue(&cue)?;
        let bytes: u64 = c.files.iter().map(|f| std::fs::metadata(f).map(|m| m.len()).unwrap_or(0)).sum();
        // Sectors: the data file's size over its sector size (multi-track files: the first track's mode).
        let sectors = bytes / sector_size(&c.modes[0]).max(1);
        let audio = c.modes.iter().filter(|m| *m == "AUDIO").count();
        let data = c.modes.len() - audio;
        let description = match (data, audio) {
            (0, a) => format!("Audio CD, {a} tracks"),
            (_, 0) => "Data".to_string(),
            (_, a) => format!("Data + {a} audio tracks"),
        };
        return Ok(ImageInfo {
            path: cue.to_string_lossy().into_owned(),
            kind: "cue".into(),
            description,
            sectors,
            bytes,
            needs_cdrdao: single_data_track(&c).is_none(),
        });
    }
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if !matches!(ext.as_str(), "iso" | "cdr" | "toast" | "img") {
        return Err("Choose an .iso, or a .cue (with its .bin).".into());
    }
    let bytes = std::fs::metadata(&p).map_err(|e| e.to_string())?.len();
    if bytes == 0 || bytes % 2048 != 0 {
        return Err("That doesn't look like a disc image (its size isn't a whole number of 2048-byte sectors).".into());
    }
    Ok(ImageInfo { path, kind: "iso".into(), description: "Data".into(), sectors: bytes / 2048, bytes, needs_cdrdao: false })
}

fn progress(app: &AppHandle, phase: &str, permille: usize) {
    let _ = app.emit("add-progress", crate::write::AddProgress { done: permille.min(1000), total: 1000, current: phase.into() });
}

/// Run a burning program, turning its output into progress with `percent`
/// (a line → percent done). Both stdout and stderr are read.
/// A burning program that failed; `started`: it had begun writing (so the
/// disc is used up), else trying again another way is safe.
struct BurnFail {
    msg: String,
    started: bool,
}

impl From<BurnFail> for String {
    fn from(f: BurnFail) -> String {
        f.msg
    }
}

fn run_with_progress(app: &AppHandle, mut cmd: Command, percent: fn(&str) -> Option<f64>) -> Result<(), BurnFail> {
    let name = cmd.get_program().to_string_lossy().into_owned();
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| BurnFail { msg: format!("Couldn't start {name}: {e}"), started: false })?;
    let started = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let out = child.stdout.take().unwrap();
    let err = child.stderr.take().unwrap();
    let tail = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let watch = |r: Box<dyn Read + Send>, app: AppHandle, tail: std::sync::Arc<std::sync::Mutex<Vec<String>>>| {
        let started = started.clone();
        std::thread::spawn(move || {
            // Progress lines end in \r as often as \n.
            let mut buf = Vec::new();
            for b in BufReader::new(r).bytes().map_while(Result::ok) {
                if b == b'\n' || b == b'\r' {
                    let line = String::from_utf8_lossy(&buf).trim().to_string();
                    buf.clear();
                    if line.is_empty() {
                        continue;
                    }
                    if let Some(p) = percent(&line) {
                        if p > 0.0 {
                            started.store(true, std::sync::atomic::Ordering::Relaxed);
                        }
                        progress(&app, "burn", (p * 10.0) as usize);
                    }
                    let mut t = tail.lock().unwrap();
                    t.push(line);
                    if t.len() > 8 {
                        t.remove(0);
                    }
                } else {
                    buf.push(b);
                }
            }
        })
    };
    let a = watch(Box::new(out), app.clone(), tail.clone());
    let b = watch(Box::new(err), app.clone(), tail.clone());
    let status = child.wait().map_err(|e| BurnFail { msg: e.to_string(), started: true })?;
    let _ = (a.join(), b.join());
    if status.success() {
        Ok(())
    } else {
        Err(BurnFail {
            msg: format!("{name} stopped: {}", tail.lock().unwrap().join(" / ")),
            started: started.load(std::sync::atomic::Ordering::Relaxed),
        })
    }
}

fn pct_hdiutil(l: &str) -> Option<f64> {
    l.strip_prefix("PERCENT:")?.trim().parse::<f64>().ok().filter(|p| *p >= 0.0)
}
/// "Track 01:  123 of  650 MB written" (wodim) or "Wrote 123 of 650 MB" (cdrdao).
fn pct_of_mb(l: &str) -> Option<f64> {
    let w: Vec<&str> = l.split_whitespace().collect();
    let i = w.iter().position(|x| *x == "of")?;
    let done: f64 = w.get(i.checked_sub(1)?)?.parse().ok()?;
    let total: f64 = w.get(i + 1)?.parse().ok()?;
    (total > 0.0).then(|| done * 100.0 / total)
}
/// growisofs: " 123456789/4700000000 ( 2.6%) @1.0x, remaining 9:59".
fn pct_paren(l: &str) -> Option<f64> {
    let s = l.split('(').nth(1)?.split('%').next()?.trim();
    s.parse().ok()
}

/// The data of a single-track .bin as an ISO (2048 bytes of each sector).
fn bin_to_iso(bin: &Path, sector: u64, out: &Path, on: &dyn Fn(usize)) -> Result<(), String> {
    let mut r = BufReader::new(std::fs::File::open(bin).map_err(|e| e.to_string())?);
    let mut w = std::io::BufWriter::new(std::fs::File::create(out).map_err(|e| e.to_string())?);
    let total = std::fs::metadata(bin).map(|m| m.len()).unwrap_or(1).max(1);
    let mut buf = vec![0u8; sector as usize];
    let (mut done, offset) = (0u64, if sector == 2352 { 16 } else { 0 });
    loop {
        match r.read_exact(&mut buf) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.to_string()),
        }
        w.write_all(&buf[offset..offset + 2048]).map_err(|e| e.to_string())?;
        done += sector;
        if done % (sector * 4096) == 0 {
            on((done * 1000 / total) as usize);
        }
    }
    w.flush().map_err(|e| e.to_string())
}

/// Read the image's files through once before burning: a file kept online
/// (iCloud, Proton Drive, Dropbox…) is downloaded now rather than while the
/// drive waits for data, which it doesn't do for long; and it's known to be
/// readable before a disc is used.
fn read_through(files: &[PathBuf], app: &AppHandle) -> Result<(), String> {
    let total: u64 = files.iter().map(|f| std::fs::metadata(f).map(|m| m.len()).unwrap_or(0)).sum::<u64>().max(1);
    let mut done = 0u64;
    let mut buf = vec![0u8; 4 << 20];
    for f in files {
        let mut r = std::fs::File::open(f).map_err(|e| format!("Couldn't open {}: {e}", f.display()))?;
        loop {
            let n = r.read(&mut buf).map_err(|e| format!("Couldn't read {}: {e}", f.display()))?;
            if n == 0 {
                break;
            }
            done += n as u64;
            progress(app, "prepare", (done * 1000 / total) as usize);
        }
    }
    Ok(())
}

/// Linux: read the disc back and compare it with the image.
fn verify_disc(dev: &str, image: &Path, app: &AppHandle) -> Result<(), String> {
    let len = std::fs::metadata(image).map_err(|e| e.to_string())?.len();
    // The drive may need a moment after closing the disc.
    let mut disc = None;
    for _ in 0..60 {
        if let Ok(f) = std::fs::File::open(dev) {
            disc = Some(f);
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    let mut disc = BufReader::new(disc.ok_or("Couldn't read the disc back to check it")?);
    let mut img = BufReader::new(std::fs::File::open(image).map_err(|e| e.to_string())?);
    let (mut a, mut b) = (vec![0u8; 1 << 20], vec![0u8; 1 << 20]);
    let mut done = 0u64;
    while done < len {
        let n = ((len - done) as usize).min(a.len());
        img.read_exact(&mut a[..n]).map_err(|e| e.to_string())?;
        disc.read_exact(&mut b[..n]).map_err(|e| format!("Couldn't read the disc back at {} MB ({e})", done >> 20))?;
        if a[..n] != b[..n] {
            return Err(format!(
                "The disc doesn't match the image (from about {} MB on). Burn it again, slower or on another disc.",
                done >> 20
            ));
        }
        done += n as u64;
        progress(app, "verify", (done * 1000 / len) as usize);
    }
    Ok(())
}

/// Burn `path` (from [`burn_inspect`]). `speed`: 0 for the slowest the
/// drive and disc allow, else an x-factor. What happened, as report lines.
#[tauri::command]
pub async fn burn_image(path: String, speed: u32, verify: bool, app: AppHandle) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || burn(&path, speed, verify, &app)).await.map_err(|e| e.to_string())?
}

fn burn(path: &str, speed: u32, verify: bool, app: &AppHandle) -> Result<Vec<String>, String> {
    let info = burn_inspect(path.to_string())?;
    let status = status_now();
    status.drive.as_ref().ok_or("No disc burner found.")?;
    let media = status.media.ok_or("Put a blank CD or DVD in the burner first.")?;
    if !media.blank {
        return Err(format!("The {} in the burner isn't blank.", media.kind));
    }
    if media.free_sectors > 0 && info.sectors > media.free_sectors {
        return Err(format!(
            "The image needs {} MB, more than the {} holds ({} MB).",
            (info.sectors * 2048) >> 20,
            media.kind,
            (media.free_sectors * 2048) >> 20
        ));
    }
    let x = if speed == 0 { "1".to_string() } else { speed.to_string() };
    let mut report = vec![format!(
        "Burned at {} onto a {}.",
        if speed == 0 { "the slowest speed the drive allows".to_string() } else { format!("{speed}×") },
        media.kind
    )];
    let tmp = crate::convert::TempDir::new()?;
    // A .cue with one data track goes as the ISO inside it.
    let mut iso: Option<PathBuf> = (info.kind == "iso").then(|| PathBuf::from(&info.path));
    // A .cue goes raw with cdrdao: every sector exactly as in the image,
    // error-correction data included (some discs depend on theirs). Without
    // cdrdao, a single MODE1 data track can still go as its ISO: the drive
    // writes fresh error correction, which is the same for an ordinary disc.
    // macOS burns .cue images itself (Disc Recording); cdrdao can't open
    // a drive macOS is using.
    let mac = cfg!(target_os = "macos");
    let cdrdao = if mac { None } else { tool("cdrdao") };
    if info.kind == "cue" && !info.needs_cdrdao && cdrdao.is_none() && !mac {
        let c = parse_cue(Path::new(&info.path))?;
        let out = tmp.0.join("image.iso");
        bin_to_iso(&c.files[0], single_data_track(&c).unwrap(), &out, &|p| progress(app, "prepare", p))?;
        iso = Some(out);
        report.push("cdrdao isn't installed, so the image's data track was burned as an ISO: the data is the same, but sectors deliberately damaged in the image (some copy protections) aren't reproduced.".into());
    }
    if let Some(i) = &iso {
        read_through(std::slice::from_ref(i), app)?;
    }
    progress(app, "burn", 0);
    match iso {
        Some(iso) if cfg!(target_os = "macos") => {
            let mut cmd = Command::new("hdiutil");
            cmd.args(["burn", "-puppetstrings", "-speed", &x]);
            if !verify {
                cmd.arg("-noverifyburn");
            }
            cmd.arg(&iso);
            run_with_progress(app, cmd, pct_hdiutil)?;
            report.push(if verify { "macOS checked the disc against the image.".into() } else { "Not checked against the image.".into() });
        }
        Some(iso) => {
            let dev = linux_device().ok_or("No disc burner found.")?;
            if media.dvd {
                let g = tool("growisofs").ok_or("Burning DVDs needs growisofs: install dvd+rw-tools (e.g. sudo apt install growisofs).")?;
                let mut cmd = Command::new(g);
                cmd.arg("-dvd-compat").arg(format!("-speed={x}")).arg("-Z").arg(format!("{dev}={}", iso.display()));
                run_with_progress(app, cmd, pct_paren)?;
            } else {
                let w = tool("wodim")
                    .or_else(|| tool("cdrecord"))
                    .ok_or("Burning CDs needs wodim or cdrecord (e.g. sudo apt install wodim).")?;
                let mut cmd = Command::new(w);
                cmd.args(["-v", "-dao", "-data"]).arg(format!("speed={x}")).arg(format!("dev={dev}")).arg(&iso);
                run_with_progress(app, cmd, pct_of_mb)?;
            }
            if verify {
                verify_disc(dev, &iso, app)?;
                report.push("The disc was read back and matches the image.".into());
            } else {
                report.push("Not checked against the image.".into());
            }
            let _ = Command::new("eject").arg(dev).status();
        }
        None if mac => {
            if media.dvd {
                return Err("A .bin/.cue image goes on a CD-R, not a DVD.".into());
            }
            let plan = cue_plan(Path::new(&info.path))?;
            let mut files: Vec<PathBuf> = plan.iter().map(|t| t.file.clone()).collect();
            files.dedup();
            read_through(&files, app)?;
            dr_burn(&plan, speed, verify, app)?;
            report.push("Burned session-at-once, every track's sectors from the image; the drive writes each sector's sync, header and error correction, which is the same for any disc made the standard way.".into());
            report.push(if verify {
                "macOS read the disc back and it matches the image.".into()
            } else {
                "Not checked against the image.".into()
            });
            report.push("Subchannel data (.sub/.sbi files, used by a few protected PlayStation games) isn't written.".into());
        }
        None => {
            // A .cue: cdrdao, disc-at-once, raw (PlayStation and other mode 2
            // discs, audio, mixed mode, several tracks).
            let c = cdrdao.ok_or("Burning a .bin/.cue exactly (PlayStation and other mode 2 discs, audio, several tracks) needs cdrdao (e.g. sudo apt install cdrdao).")?;
            if media.dvd {
                return Err("A .bin/.cue image goes on a CD-R, not a DVD.".into());
            }
            let dev = linux_device().ok_or("No disc burner found.")?;
            // cdrdao opens a .cue's files relative to where it's run, not to
            // the .cue: give it a copy naming them by full path (as found
            // by parse_cue, including a .bin renamed with its .cue).
            let cue = tmp.0.join("image.cue");
            absolute_cue(Path::new(&info.path), &cue)?;
            let write = |driver: Option<&str>| -> Result<(), BurnFail> {
                let mut cmd = Command::new(&c);
                cmd.args(["write", "--device", dev, "--speed", &x, "--eject", "-n"]);
                if let Some(d) = driver {
                    cmd.args(["--driver", d]);
                }
                cmd.arg(&cue);
                if let Some(dir) = Path::new(&info.path).parent() {
                    cmd.current_dir(dir);
                }
                run_with_progress(app, cmd, pct_of_mb)
            };
            // The drive may still be busy for a moment (the desktop looking at
            // the new disc): wait and try again.
            let busy = |f: &BurnFail| !f.started && f.msg.contains("Cannot setup device");
            let attempt = |driver: &str| -> Result<(), BurnFail> {
                let mut last = None;
                for wait in [0u64, 3, 5, 8, 12] {
                    std::thread::sleep(std::time::Duration::from_secs(wait));
                    match write(Some(driver)) {
                        Err(f) if busy(&f) => last = Some(f),
                        other => return other,
                    }
                }
                let f = last.unwrap();
                Err(BurnFail { msg: f.msg, started: false })
            };
            match attempt("generic-mmc-raw") {
                Ok(()) => report.push("Written raw: every sector as it is in the image.".into()),
                Err(f) if busy(&f) => return Err(f.msg),
                // The drive can't write raw, and nothing was written yet: cooked (it makes the error correction).
                Err(f) if !f.started => {
                    attempt("generic-mmc")?;
                    report.push("This drive can't write raw, so it wrote the sectors' error correction itself: right for ordinary discs, but sectors deliberately damaged in the image aren't reproduced.".into());
                }
                Err(f) => return Err(f.msg),
            }
            report.push("Subchannel data (.sub/.sbi files, used by a few protected PlayStation games) isn't written.".into());
            report.push("cdrdao can't check the disc against the image; play it to make sure.".into());
        }
    }
    progress(app, "burn", 1000);
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_tools_output() {
        let s = "           Type: CD-R               Name: /dev/disk4\n     Space Free:   79:59:74         blocks:   359999 / 737.28MB / 703.12MiB\n    Writability: appendable, blank, overwritable\n";
        let m = parse_drutil(s).unwrap();
        assert_eq!((m.kind.as_str(), m.blank, m.free_sectors, m.dvd), ("CD-R", true, 359_999, false));
        assert!(parse_drutil("           Type: No Media Inserted\n").is_none());
        let u = parse_udev("ID_CDROM_MEDIA=1\nID_CDROM_MEDIA_DVD_PLUS_R=1\nID_CDROM_MEDIA_STATE=blank\n").unwrap();
        assert_eq!((u.kind.as_str(), u.blank, u.dvd), ("DVD+R", true, true));
        assert_eq!(pct_hdiutil("PERCENT:12.5"), Some(12.5));
        assert_eq!(pct_hdiutil("PERCENT:-1.000000"), None);
        assert_eq!(pct_of_mb("Track 01:  130 of  650 MB written (fifo 100%) [buf  99%]  16.0x."), Some(20.0));
        assert_eq!(pct_of_mb("Wrote 325 of 650 MB (Buffers 100%  99%)."), Some(50.0));
        assert_eq!(pct_paren(" 123/4700 ( 2.6%) @1.0x, remaining 9:59"), Some(2.6));
    }

    #[test]
    fn cue_images() {
        let tmp = crate::convert::TempDir::new().unwrap();
        let bin = tmp.0.join("Game.bin");
        // Two MODE1/2352 sectors with recognisable user data.
        let mut data = Vec::new();
        for s in 0..2u8 {
            let mut sec = vec![0u8; 2352];
            sec[16..16 + 2048].fill(s + 1);
            data.extend(sec);
        }
        std::fs::write(&bin, &data).unwrap();
        std::fs::write(tmp.0.join("Game.cue"), "FILE \"Other name.bin\" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n").unwrap();
        let i = burn_inspect(bin.to_string_lossy().into_owned()).unwrap();
        assert_eq!((i.kind.as_str(), i.sectors, i.needs_cdrdao), ("cue", 2, false));
        let c = parse_cue(Path::new(&i.path)).unwrap();
        assert_eq!(c.files[0], bin);
        std::fs::write(tmp.0.join("Mixed.cue"), "FILE \"Game.bin\" BINARY\n TRACK 01 MODE1/2352\n TRACK 02 AUDIO\n").unwrap();
        let m = burn_inspect(tmp.0.join("Mixed.cue").to_string_lossy().into_owned()).unwrap();
        assert!(m.needs_cdrdao);
        assert_eq!(m.description, "Data + 1 audio tracks");
        let abs = tmp.0.join("abs.cue");
        absolute_cue(Path::new(&i.path), &abs).unwrap();
        let text = std::fs::read_to_string(&abs).unwrap();
        assert!(text.starts_with(&format!("FILE \"{}\" BINARY\n", bin.display())), "{text}");
        assert!(text.contains("TRACK 01 MODE1/2352"));
        // A PlayStation-style layout: data, then audio with a 2-second pregap in the file.
        let mixed = tmp.0.join("Mixed.bin");
        std::fs::write(&mixed, vec![0u8; 2352 * (100 + 150 + 300)]).unwrap();
        std::fs::write(
            tmp.0.join("Psx.cue"),
            "FILE \"Mixed.bin\" BINARY\r\n  TRACK 01 MODE2/2352\r\n    INDEX 01 00:00:00\r\n  TRACK 02 AUDIO\r\n    INDEX 00 00:01:25\r\n    INDEX 01 00:03:25\r\n",
        )
        .unwrap();
        let plan = cue_plan(&tmp.0.join("Psx.cue")).unwrap();
        assert_eq!(plan.len(), 2);
        assert_eq!((plan[0].offset, plan[0].length, plan[0].pregap, plan[0].pregap_offset), (0, 100, 150, None));
        assert_eq!((plan[1].offset, plan[1].length, plan[1].pregap, plan[1].pregap_offset), (250 * 2352, 300, 150, Some(100 * 2352)));
        assert_eq!(dr_format("MODE2/2352"), Some((16, 2332, 13, 32, 4)));
        let iso = tmp.0.join("out.iso");
        bin_to_iso(&bin, 2352, &iso, &|_| {}).unwrap();
        let out = std::fs::read(&iso).unwrap();
        assert_eq!(out.len(), 4096);
        assert!(out[..2048].iter().all(|b| *b == 1) && out[2048..].iter().all(|b| *b == 2));
    }
}
