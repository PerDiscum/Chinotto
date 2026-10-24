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

/// A program, also where Homebrew puts them (apps don't get the shell's PATH).
fn tool(name: &str) -> Option<PathBuf> {
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
fn run_with_progress(app: &AppHandle, mut cmd: Command, percent: fn(&str) -> Option<f64>) -> Result<(), String> {
    let name = cmd.get_program().to_string_lossy().into_owned();
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| format!("Couldn't start {name}: {e}"))?;
    let out = child.stdout.take().unwrap();
    let err = child.stderr.take().unwrap();
    let tail = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let watch = |r: Box<dyn Read + Send>, app: AppHandle, tail: std::sync::Arc<std::sync::Mutex<Vec<String>>>| {
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
    let status = child.wait().map_err(|e| e.to_string())?;
    let _ = (a.join(), b.join());
    if status.success() {
        Ok(())
    } else {
        Err(format!("{name} stopped: {}", tail.lock().unwrap().join(" / ")))
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
    if info.kind == "cue" && !info.needs_cdrdao {
        let c = parse_cue(Path::new(&info.path))?;
        let out = tmp.0.join("image.iso");
        bin_to_iso(&c.files[0], single_data_track(&c).unwrap(), &out, &|p| progress(app, "prepare", p))?;
        iso = Some(out);
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
        None => {
            // A .cue with audio or several tracks: cdrdao, disc-at-once, raw.
            let c = tool("cdrdao").ok_or(if cfg!(target_os = "macos") {
                "This image has audio or several tracks, which needs cdrdao: install it with Homebrew (brew install cdrdao)."
            } else {
                "This image has audio or several tracks, which needs cdrdao (e.g. sudo apt install cdrdao)."
            })?;
            if media.dvd {
                return Err("A .cue with audio or several tracks goes on a CD, not a DVD.".into());
            }
            let dev = if cfg!(target_os = "macos") { "IODVDServices" } else { linux_device().ok_or("No disc burner found.")? };
            let mut cmd = Command::new(c);
            cmd.args(["write", "--device", dev, "--speed", &x, "--eject", "-n"]).arg(&info.path);
            run_with_progress(app, cmd, pct_of_mb)?;
            report.push("cdrdao can't check the disc against the image; play or mount it to make sure.".into());
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
        let iso = tmp.0.join("out.iso");
        bin_to_iso(&bin, 2352, &iso, &|_| {}).unwrap();
        let out = std::fs::read(&iso).unwrap();
        assert_eq!(out.len(), 4096);
        assert!(out[..2048].iter().all(|b| *b == 1) && out[2048..].iter().all(|b| *b == 2));
    }
}
