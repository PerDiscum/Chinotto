//! Ripping audio CDs into a folder the user picks (not into a library: a
//! library picks the songs up when the folder is inside one of its folders).
//!
//! Reading the disc:
//! - macOS mounts an audio CD as a volume of AIFF files ("1 Audio Track.aiff"
//!   …, with a hidden `.TOC.plist`); they're read like any other file.
//! - Linux: `cdparanoia` (or `cd-paranoia`, from libcdio), if installed,
//!   reads each track into a WAV first.
//!
//! Files go to `<folder>/ARTIST/ARTIST - YEAR - ALBUM/ARTIST - YEAR - ALBUM -
//! NN - TITLE.<ext>` (no " - YEAR" without a year), never over an existing
//! file.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DiscTrack {
    pub number: u32,
    pub length_ms: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Disc {
    /// The volume's path (macOS) or the drive's device (Linux).
    pub id: String,
    /// The volume name (macOS; CD-Text title or "Audio CD"), else "Audio CD".
    pub name: String,
    pub tracks: Vec<DiscTrack>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RipTrack {
    pub number: u32,
    pub title: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RipRequest {
    pub disc: String,
    pub folder: String,
    /// "alac", "flac", "aac256", "aac128", "mp3_320", "mp3_256", "mp3_192" or "mp3_128".
    pub format: String,
    pub artist: String,
    pub album: String,
    pub year: i32,
    pub genre: String,
    pub tracks: Vec<RipTrack>,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RipResult {
    pub ripped: usize,
    /// The album's folder.
    pub folder: String,
    pub problems: Vec<String>,
}

/// macOS: a track file's number ("12 Audio Track.aiff" → 12).
fn track_number(p: &Path) -> Option<u32> {
    let name = p.file_name()?.to_str()?;
    if !name.to_ascii_lowercase().ends_with(".aiff") {
        return None;
    }
    name.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}

fn mac_discs() -> Vec<Disc> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir("/Volumes") else { return out };
    for v in rd.flatten() {
        let root = v.path();
        if !root.join(".TOC.plist").is_file() {
            continue;
        }
        let mut tracks: Vec<DiscTrack> = std::fs::read_dir(&root)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|f| {
                let n = track_number(&f.path())?;
                // 44.1 kHz, 16-bit stereo: 176.4 bytes a millisecond.
                let size = f.metadata().ok()?.len();
                Some(DiscTrack { number: n, length_ms: size * 10 / 1764 })
            })
            .collect();
        if tracks.is_empty() {
            continue;
        }
        tracks.sort_by_key(|t| t.number);
        out.push(Disc { id: root.to_string_lossy().into_owned(), name: v.file_name().to_string_lossy().into_owned(), tracks });
    }
    out
}

/// Linux: the CD reader program, if installed.
fn paranoia() -> Option<&'static str> {
    ["cdparanoia", "cd-paranoia"].into_iter().find(|p| Command::new(p).arg("--version").output().is_ok())
}

fn linux_discs() -> Vec<Disc> {
    let Some(prog) = paranoia() else { return Vec::new() };
    let mut out = Vec::new();
    for dev in ["/dev/sr0", "/dev/sr1", "/dev/cdrom"] {
        if !Path::new(dev).exists() || out.iter().any(|d: &Disc| d.id == dev) {
            continue;
        }
        // Its table of contents: "  1.    16503 [03:40.03]    0 [00:00.00]  no …"
        let Ok(o) = Command::new(prog).args(["-Q", "-d", dev]).output() else { continue };
        let text = String::from_utf8_lossy(&o.stderr).into_owned() + &String::from_utf8_lossy(&o.stdout);
        let tracks: Vec<DiscTrack> = text
            .lines()
            .filter_map(|l| {
                let mut w = l.split_whitespace();
                let n = w.next()?.strip_suffix('.')?.parse().ok()?;
                let sectors: u64 = w.next()?.parse().ok()?;
                Some(DiscTrack { number: n, length_ms: sectors * 1000 / 75 })
            })
            .collect();
        if !tracks.is_empty() {
            out.push(Disc { id: dev.into(), name: "Audio CD".into(), tracks });
        }
    }
    out
}

/// Audio CDs in the computer's drives now.
#[tauri::command]
pub async fn cd_list() -> Vec<Disc> {
    tauri::async_runtime::spawn_blocking(|| if cfg!(target_os = "macos") { mac_discs() } else { linux_discs() }).await.unwrap_or_default()
}

/// A name that's safe as a file or folder name anywhere.
fn clean(s: &str, fallback: &str) -> String {
    if s.trim().is_empty() {
        fallback.into()
    } else {
        crate::msc::fat_name(s.trim(), 120)
    }
}

/// `path`, or "path (2)", "path (3)"… if it's taken.
fn free_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = path.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
    (2..).map(|n| path.with_file_name(format!("{stem} ({n}).{ext}"))).find(|p| !p.exists()).unwrap()
}

/// Rip the chosen tracks, encoding and tagging each.
#[tauri::command]
pub async fn cd_rip(req: RipRequest, app: AppHandle) -> Result<RipResult, String> {
    tauri::async_runtime::spawn_blocking(move || rip(req, &app)).await.map_err(|e| e.to_string())?
}

fn rip(req: RipRequest, app: &AppHandle) -> Result<RipResult, String> {
    let artist = clean(&req.artist, "Unknown Artist");
    let album = clean(&req.album, "Unknown Album");
    let head = if req.year > 0 { format!("{artist} - {} - {album}", req.year) } else { format!("{artist} - {album}") };
    let dir = Path::new(&req.folder).join(&artist).join(&head);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't make the folder {}: {e}", dir.display()))?;
    let tmp = crate::convert::TempDir::new()?;
    let ext = crate::convert::extension(&req.format);
    let count = req.tracks.len();
    let mut res = RipResult { folder: dir.to_string_lossy().into_owned(), ..Default::default() };
    let mac = cfg!(target_os = "macos");
    let prog = if mac {
        None
    } else {
        Some(paranoia().ok_or("Ripping CDs on Linux needs cdparanoia: install it (e.g. sudo apt install cdparanoia)")?)
    };
    for (i, t) in req.tracks.iter().enumerate() {
        let title = if t.title.trim().is_empty() { format!("Track {}", t.number) } else { t.title.trim().to_string() };
        let _ = app.emit("add-progress", crate::write::AddProgress { done: i, total: count, current: title.clone() });
        // The track as audio the decoder reads.
        let src = if mac {
            let found =
                std::fs::read_dir(&req.disc).into_iter().flatten().flatten().map(|f| f.path()).find(|p| track_number(p) == Some(t.number));
            match found {
                Some(p) => p,
                None => {
                    res.problems.push(format!("{title}: not found on the disc"));
                    continue;
                }
            }
        } else {
            let wav = tmp.0.join(format!("track{:02}.wav", t.number));
            let ok = Command::new(prog.unwrap())
                .args(["-q", "-d", &req.disc, &t.number.to_string()])
                .arg(&wav)
                .status()
                .is_ok_and(|s| s.success());
            if !ok || !wav.is_file() {
                res.problems.push(format!("{title}: couldn't read it from the disc"));
                continue;
            }
            wav
        };
        let encoded = match crate::convert::encode(&src, &tmp.0, &req.format, i) {
            Ok(f) => f,
            Err(e) => {
                res.problems.push(format!("{title}: {e}"));
                continue;
            }
        };
        if !mac {
            let _ = std::fs::remove_file(&src);
        }
        use chinotto_gpod::Field;
        let some = |s: &str| (!s.trim().is_empty()).then(|| s.trim().to_string());
        let fields = [
            Field::Title(Some(title.clone())),
            Field::Artist(some(&req.artist)),
            Field::AlbumArtist(some(&req.artist)),
            Field::Album(some(&req.album)),
            Field::Genre(some(&req.genre)),
            Field::Year(req.year.max(0)),
            Field::TrackNr(t.number as i32),
            Field::TrackCount(count as i32),
        ];
        if let Err(e) = crate::tagwrite::write(&encoded, &fields, None) {
            res.problems.push(format!("{title}: its tags couldn't be written ({e})"));
        }
        let name = format!("{head} - {:02} - {}.{ext}", t.number, clean(&title, "Track"));
        let dest = free_path(dir.join(name));
        // A rename can't cross drives: copy, then the temporary file goes with the folder.
        if std::fs::rename(&encoded, &dest).is_err() {
            if let Err(e) = std::fs::copy(&encoded, &dest) {
                res.problems.push(format!("{title}: couldn't save it ({e})"));
                continue;
            }
        }
        res.ripped += 1;
    }
    let _ = app.emit("add-progress", crate::write::AddProgress { done: count, total: count, current: String::new() });
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(track_number(Path::new("/Volumes/Audio CD/12 Audio Track.aiff")), Some(12));
        assert_eq!(track_number(Path::new("/Volumes/Audio CD/.TOC.plist")), None);
        assert_eq!(clean("AC/DC", "x"), "AC_DC");
        assert_eq!(clean("  ", "Unknown Artist"), "Unknown Artist");
    }
}

#[cfg(test)]
mod encode_tests {
    /// A second of 16-bit stereo sine as a WAV, encoded to FLAC and read back.
    #[test]
    fn flac_round_trip() {
        let tmp = crate::convert::TempDir::new().unwrap();
        let n = 44_100u32;
        let mut wav = Vec::new();
        let data = n * 4;
        wav.extend(b"RIFF");
        wav.extend((36 + data).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(44_100u32.to_le_bytes());
        wav.extend((44_100u32 * 4).to_le_bytes());
        wav.extend(4u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend(data.to_le_bytes());
        for i in 0..n {
            let v = ((i as f32 * 440.0 * std::f32::consts::TAU / 44_100.0).sin() * 12_000.0) as i16;
            wav.extend(v.to_le_bytes());
            wav.extend(v.to_le_bytes());
        }
        let src = tmp.0.join("in.wav");
        std::fs::write(&src, wav).unwrap();
        let out = crate::convert::encode(&src, &tmp.0, "flac", 0).unwrap();
        assert!(out.to_string_lossy().ends_with(".flac"));
        let mut s = chinotto_player::decode::Source::open(&out).unwrap();
        let mut frames = 0;
        while let Some(c) = s.next_chunk().unwrap() {
            frames += c.len() / s.channels;
        }
        assert_eq!(frames, n as usize);
        crate::tagwrite::write(&out, &[chinotto_gpod::Field::Title(Some("Sine".into()))], None).unwrap();
    }
}
