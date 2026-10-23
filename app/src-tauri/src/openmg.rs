//! Sony OpenMG Walkmans (NW-S700 series…): the songs on the device page, and
//! adding songs. The formats are in the chinotto-omg crate; this module
//! converts what the Walkman can't play, and changes the device safely:
//! every table and picture folder is backed up first (on the Walkman, in
//! "Chinotto Backups"), the new ones are staged beside OMGAUDIO and swapped
//! in, and a failure puts the backup back.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use serde::Serialize;

use crate::msc;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenMgTrack {
    pub number: usize,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub length_ms: u32,
    pub audio_file: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenMgView {
    pub tracks: Vec<OpenMgTrack>,
}

/// Read the metadata table only. This command deliberately does not write to the device.
#[tauri::command]
pub fn openmg_view(path: String) -> Result<OpenMgView, String> {
    let root = Path::new(&path);
    if !msc::is_openmg_root(root) {
        return Err("This drive is not a recognized Sony OpenMG Walkman.".into());
    }
    let bytes = std::fs::read(root.join("OMGAUDIO/04CNTINF.DAT")).map_err(|e| format!("Could not read the Walkman's track table: {e}"))?;
    let mut view = parse_track_table(&bytes)?;
    let linked_audio = read_audio_links(root, view.tracks.len());
    for (track, audio_file) in view.tracks.iter_mut().zip(linked_audio) {
        track.audio_file = audio_file;
    }
    Ok(view)
}

/// Match the track table to the OMA files through the per-track keys in
/// 05CIDLST.DAT. Filename ordering alone is not a reliable link.
fn read_audio_links(root: &Path, track_count: usize) -> Vec<Option<String>> {
    const HEADER_SIZE: usize = 48;
    const RECORD_SIZE: usize = 48;
    const KEY_OFFSET: usize = 13;
    const OMA_EA3_OFFSET: usize = 0xC00;
    const OMA_KEY_OFFSET: usize = OMA_EA3_OFFSET + 0x15;
    const OMA_HEADER_BYTES: usize = OMA_KEY_OFFSET + 11;

    let mut result = vec![None; track_count];
    let Ok(ids) = std::fs::read(root.join("OMGAUDIO/05CIDLST.DAT")) else { return result };
    if ids.len() < HEADER_SIZE || &ids[..4] != b"CIDL" {
        return result;
    }
    let count = u16::from_be_bytes([ids[0x24], ids[0x25]]) as usize;
    let Some(records_size) = count.checked_mul(RECORD_SIZE) else { return result };
    let Some(required) = HEADER_SIZE.checked_add(records_size) else { return result };
    if count != track_count || ids.len() < required {
        return result;
    }

    let mut by_key = HashMap::<[u8; 11], String>::new();
    let Ok(folders) = std::fs::read_dir(root.join("OMGAUDIO")) else { return result };
    for folder in folders.flatten() {
        let name = folder.file_name();
        if !name.to_string_lossy().to_ascii_uppercase().starts_with("10F") || !folder.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let Ok(files) = std::fs::read_dir(folder.path()) else { continue };
        for file in files.flatten() {
            if !file.file_type().is_ok_and(|kind| kind.is_file())
                || !file.path().extension().is_some_and(|ext| ext.eq_ignore_ascii_case("oma"))
            {
                continue;
            }
            let Ok(mut oma) = std::fs::File::open(file.path()) else { continue };
            if oma.metadata().map(|metadata| metadata.len() < OMA_HEADER_BYTES as u64).unwrap_or(true) {
                continue;
            }
            let mut header = [0; OMA_HEADER_BYTES];
            if oma.read_exact(&mut header).is_err() || &header[OMA_EA3_OFFSET..OMA_EA3_OFFSET + 3] != b"EA3" {
                continue;
            }
            let key_bytes = &header[OMA_KEY_OFFSET..OMA_KEY_OFFSET + 11];
            let mut key = [0; 11];
            key.copy_from_slice(key_bytes);
            by_key.insert(key, file.file_name().to_string_lossy().into_owned());
        }
    }

    for (index, record) in (0..count).enumerate() {
        let start = HEADER_SIZE + record * RECORD_SIZE + KEY_OFFSET;
        let Some(key_bytes) = ids.get(start..start + 11) else { continue };
        let mut key = [0; 11];
        key.copy_from_slice(key_bytes);
        result[index] = by_key.get(&key).cloned();
    }
    result
}

fn parse_track_table(bytes: &[u8]) -> Result<OpenMgView, String> {
    const HEADER_AND_FIRST_RECORD: usize = 48;
    const RECORD_SIZE: usize = 656;
    const TAG_SIZE: usize = 128;
    if bytes.len() < HEADER_AND_FIRST_RECORD || &bytes[..4] != b"CNIF" {
        return Err("The Walkman's track table is missing or has an unknown format.".into());
    }
    let count = u16::from_be_bytes([bytes[0x24], bytes[0x25]]) as usize;
    let required = HEADER_AND_FIRST_RECORD
        .checked_add(count.checked_mul(RECORD_SIZE).ok_or("Track table is too large.")?)
        .ok_or("Track table is too large.")?;
    if bytes.len() < required {
        return Err("The Walkman's track table is incomplete.".into());
    }

    let mut tracks = Vec::with_capacity(count);
    for i in 0..count {
        let record = &bytes[HEADER_AND_FIRST_RECORD + i * RECORD_SIZE..][..RECORD_SIZE];
        let length_ms = u32::from_be_bytes([record[8], record[9], record[10], record[11]]);
        let title = tag_text(record, 16, b"TIT2", TAG_SIZE);
        let artist = tag_text(record, 16 + TAG_SIZE, b"TPE1", TAG_SIZE);
        let album = tag_text(record, 16 + TAG_SIZE * 2, b"TALB", TAG_SIZE);
        let genre = tag_text(record, 16 + TAG_SIZE * 3, b"TCON", TAG_SIZE);
        tracks.push(OpenMgTrack { number: i + 1, title, artist, album, genre, length_ms, audio_file: None });
    }
    Ok(OpenMgView { tracks })
}

fn tag_text(record: &[u8], offset: usize, expected: &[u8; 4], size: usize) -> Option<String> {
    let block = record.get(offset..offset + size)?;
    if &block[..4] != expected || block[4..6] != [0, 2] {
        return None;
    }
    let mut units = Vec::with_capacity((size - 6) / 2);
    for pair in block[6..].chunks_exact(2) {
        let unit = u16::from_be_bytes([pair[0], pair[1]]);
        if unit == 0 {
            break;
        }
        units.push(unit);
    }
    let text = String::from_utf16_lossy(&units).trim().to_string();
    (!text.is_empty()).then_some(text)
}

// ---- Adding songs ----

use std::path::PathBuf;

use chinotto_omg::db::{Art, Database, Song};
use chinotto_omg::oma;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

const SWITCHED_OFF: &str = "Changes to this Walkman are switched off. Turn on “Allow changes” on its page first.";
const BACKUPS: &str = "Chinotto Backups";
const STAGING: &str = ".chinotto-omg-staging";
/// Songs live in 10F00…; only that first folder (255 songs) is confirmed.
const MAX_SONGS: usize = 0xFF;

/// The settings identity of the Walkman at `root` (its volume name).
pub fn identity(root: &Path) -> String {
    format!("openmg:{}", root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
}

/// Its page settings: Allow changes (songs always go as MP3).
pub fn state(root: &Path, app: &AppHandle) -> msc::PlayerState {
    msc::PlayerState {
        writable: app.state::<AppState>().is_writable(&identity(root)),
        conversion: crate::convert::Conversion { format: "mp3_320".into(), ..Default::default() },
        plays: vec!["mp3"],
        aac: false,
    }
}

/// An album's three pictures (200, 96 and 32 pixels, baseline JPEG) from a cover.
fn art_from(cover: &[u8]) -> Option<Art> {
    let img = crate::cover::decode(cover).ok()?;
    let jpeg = |px: u32| -> Option<Vec<u8>> {
        let small = img.resize_to_fill(px, px, image::imageops::FilterType::Lanczos3).to_rgb8();
        let mut out = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85).encode_image(&small).ok()?;
        Some(out)
    };
    Some(Art { large: jpeg(200)?, medium: jpeg(96)?, small: jpeg(32)? })
}

fn same_song(s: &Song) -> (String, String, String) {
    (s.title.to_lowercase(), s.artist.to_lowercase(), s.album.to_lowercase())
}

/// Copy songs (dbids) and files to the Walkman at `path`: converted to MP3
/// when needed, written as .OMA files, then the database rebuilt.
pub async fn add(path: String, dbids: Vec<String>, files: Vec<String>, app: AppHandle) -> Result<msc::CopyResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = PathBuf::from(&path);
        if !msc::is_openmg_root(&root) {
            return Err("This drive is not a recognized Sony OpenMG Walkman.".into());
        }
        if !app.state::<AppState>().is_writable(&identity(&root)) {
            return Err(SWITCHED_OFF.into());
        }
        let omg = root.join("OMGAUDIO");
        let mut db = Database::read(&omg).map_err(|e| format!("Couldn't read the Walkman's database: {e}"))?;
        let mut res = msc::CopyResult::default();
        let sources = msc::sources(&app, &dbids, &files, &mut res.skipped);
        // Tags first, so an album's songs go in order (the Walkman lists
        // an album's songs in the order they were added).
        let mut todo: Vec<(PathBuf, chinotto_gpod::TrackInfo)> =
            sources.into_iter().map(|src| (src.clone(), crate::tags::read(&src).unwrap_or_default())).collect();
        todo.sort_by_key(|(_, i)| (i.album.clone().unwrap_or_default().to_lowercase(), i.disc_nr, i.track_nr));
        let mut on_device: std::collections::HashSet<_> = db.songs.iter().map(same_song).collect();
        let before = db.songs.len();
        let mut written: Vec<PathBuf> = Vec::new();
        let tmp = crate::convert::TempDir::new()?;
        let total = todo.len();
        for (i, (src, info)) in todo.iter().enumerate() {
            let name = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let _ = app.emit("add-progress", crate::write::AddProgress { done: i, total, current: crate::tags::label(src) });
            let text =
                |v: &Option<String>, unknown: &str| v.as_deref().map(str::trim).filter(|v| !v.is_empty()).unwrap_or(unknown).to_string();
            let stem = src.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let tags = oma::Tags {
                title: text(&info.title, &stem),
                artist: text(&info.artist, "Unknown Artist"),
                album: text(&info.album, "Unknown Album"),
                genre: text(&info.genre, "Unknown Genre"),
                year: (info.year > 0).then_some(info.year as u32),
            };
            let mut song = Song {
                title: chinotto_omg::fit(&tags.title),
                artist: chinotto_omg::fit(&tags.artist),
                album: chinotto_omg::fit(&tags.album),
                genre: chinotto_omg::fit(&tags.genre),
                year: tags.year,
                ..Default::default()
            };
            if !on_device.insert(same_song(&song)) {
                res.already += 1;
                continue;
            }
            if db.songs.len() >= MAX_SONGS {
                res.skipped.push(format!("{name}: this version of Chinotto adds up to {MAX_SONGS} songs to this Walkman"));
                continue;
            }
            // The MP3 as it is when the Walkman takes it, else converted.
            let is_mp3 = src.extension().is_some_and(|e| e.eq_ignore_ascii_case("mp3"));
            let as_is = if is_mp3 { std::fs::read(src).ok().and_then(|b| chinotto_omg::mp3::prepare(&b).ok()) } else { None };
            let mp3 = match as_is {
                Some(m) => m,
                None => match crate::convert::mp3_44k(src, &tmp.0, i)
                    .and_then(|f| std::fs::read(&f).map_err(|e| e.to_string()))
                    .and_then(|b| chinotto_omg::mp3::prepare(&b).map_err(|e| e.to_string()))
                {
                    Ok(m) => m,
                    Err(e) => {
                        res.skipped.push(format!("{name}: couldn't convert it ({e})"));
                        continue;
                    }
                },
            };
            song.length_ms = mp3.length_ms;
            song.codec = oma::codec_params(mp3.bitrate_index);
            song.cid = oma::new_content_id(&db.songs.iter().map(|s| s.cid).collect::<Vec<_>>());
            let id = db.songs.len() as u32 + 1;
            let dest = omg.join(oma::path_for(id));
            let part = dest.with_extension("part");
            let write = std::fs::create_dir_all(dest.parent().unwrap())
                .and_then(|_| std::fs::write(&part, oma::build(&mp3, &tags, &song.cid)))
                .and_then(|_| std::fs::rename(&part, &dest));
            if let Err(e) = write {
                let _ = std::fs::remove_file(&part);
                res.skipped.push(format!("{name}: couldn't write it to the Walkman ({e})"));
                if e.raw_os_error() == Some(28) {
                    break; // full
                }
                continue;
            }
            written.push(dest);
            if !db.art.contains_key(&song.album) {
                let cover = crate::tags::embedded_cover(src)
                    .or_else(|| crate::library::folder_cover(src.parent()?).and_then(|p| std::fs::read(p).ok()));
                if let Some(art) = cover.as_deref().and_then(art_from) {
                    db.art.insert(song.album.clone(), art);
                }
            }
            db.songs.push(song);
            res.added += 1;
        }
        let _ = app.emit("add-progress", crate::write::AddProgress { done: total, total, current: String::new() });
        drop(tmp);
        if db.songs.len() > before {
            if let Err(e) = install(&root, &db) {
                // The database is as it was: the new files would only be clutter.
                for f in &written {
                    let _ = std::fs::remove_file(f);
                }
                return Err(e);
            }
        }
        Ok(res)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Put the rebuilt database (tables and pictures) in place: back up, stage,
/// swap; on failure the backup goes back.
fn install(root: &Path, db: &Database) -> Result<(), String> {
    let omg = root.join("OMGAUDIO");
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let backup = root.join(BACKUPS).join(format!("OpenMG {stamp}"));
    backup_omg(&omg, &backup).map_err(|e| format!("Couldn't back up the Walkman's database, so nothing was changed: {e}"))?;
    let stage = root.join(STAGING);
    let _ = std::fs::remove_dir_all(&stage);
    let staged = (|| -> std::io::Result<()> {
        std::fs::create_dir_all(stage.join("tables"))?;
        for (name, bytes) in db.tables() {
            std::fs::write(stage.join("tables").join(name), bytes)?;
        }
        for (rel, bytes) in db.pictures() {
            let p = stage.join("pictures").join(rel);
            std::fs::create_dir_all(p.parent().unwrap())?;
            std::fs::write(p, bytes)?;
        }
        Ok(())
    })();
    if let Err(e) = staged {
        let _ = std::fs::remove_dir_all(&stage);
        return Err(format!("Couldn't prepare the Walkman's new database, so nothing was changed: {e}"));
    }
    let swapped = (|| -> std::io::Result<()> {
        std::fs::create_dir_all(stage.join("old"))?;
        for e in std::fs::read_dir(stage.join("tables"))?.flatten() {
            let live = omg.join(e.file_name());
            if live.exists() {
                std::fs::rename(&live, stage.join("old").join(e.file_name()))?;
            }
            std::fs::rename(e.path(), &live)?;
        }
        for d in picture_dirs(&omg)? {
            std::fs::rename(&d, stage.join("old").join(d.file_name().unwrap()))?;
        }
        if let Ok(rd) = std::fs::read_dir(stage.join("pictures")) {
            for e in rd.flatten() {
                std::fs::rename(e.path(), omg.join(e.file_name()))?;
            }
        }
        Ok(())
    })();
    if let Err(e) = swapped {
        let restored = restore_omg(&omg, &backup);
        let _ = std::fs::remove_dir_all(&stage);
        return Err(match restored {
            Ok(()) => format!("Couldn't update the Walkman's database ({e}); it's back as it was."),
            Err(r) => format!(
                "Couldn't update the Walkman's database ({e}) and couldn't put the backup back ({r}). Copy the files in “{}” back into OMGAUDIO by hand.",
                backup.display()
            ),
        });
    }
    let _ = std::fs::remove_dir_all(&stage);
    Ok(())
}

/// OMGAUDIO's picture folders (20P01…).
fn picture_dirs(omg: &Path) -> std::io::Result<Vec<PathBuf>> {
    Ok(std::fs::read_dir(omg)?
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()) && e.file_name().to_string_lossy().to_ascii_uppercase().starts_with("20P"))
        .map(|e| e.path())
        .collect())
}

/// Copy the tables (not RESERVED.DAT, which nothing changes) and picture folders.
fn backup_omg(omg: &Path, backup: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(backup)?;
    for e in std::fs::read_dir(omg)?.flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        if e.file_type()?.is_file() && n.to_ascii_uppercase().ends_with(".DAT") && !n.starts_with("._") && n != "RESERVED.DAT" {
            std::fs::copy(e.path(), backup.join(&n))?;
        }
    }
    for d in picture_dirs(omg)? {
        let to = backup.join(d.file_name().unwrap());
        std::fs::create_dir_all(&to)?;
        for f in std::fs::read_dir(&d)?.flatten() {
            if f.file_type()?.is_file() && !f.file_name().to_string_lossy().starts_with("._") {
                std::fs::copy(f.path(), to.join(f.file_name()))?;
            }
        }
    }
    Ok(())
}

fn restore_omg(omg: &Path, backup: &Path) -> std::io::Result<()> {
    for d in picture_dirs(omg)? {
        std::fs::remove_dir_all(d)?;
    }
    for e in std::fs::read_dir(backup)?.flatten() {
        if e.file_type()?.is_dir() {
            let to = omg.join(e.file_name());
            std::fs::create_dir_all(&to)?;
            for f in std::fs::read_dir(e.path())?.flatten() {
                std::fs::copy(f.path(), to.join(f.file_name()))?;
            }
        } else {
            std::fs::copy(e.path(), omg.join(e.file_name()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            if e.file_type().unwrap().is_dir() {
                copy_dir(&e.path(), &to.join(e.file_name()));
            } else {
                std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
            }
        }
    }

    /// Adding a song in a new album (with a cover) to a copy of a real
    /// Walkman's OMGAUDIO: backup made, tables and pictures swapped in and
    /// consistent, staging gone. Set CHINOTTO_TEST_OMG (never committed).
    #[test]
    fn installs_on_a_copy() {
        let Ok(src) = std::env::var("CHINOTTO_TEST_OMG") else { return };
        let root = std::env::temp_dir().join(format!("chinotto-omg-install-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        copy_dir(Path::new(&src), &root.join("OMGAUDIO"));
        let omg = root.join("OMGAUDIO");
        let mut db = Database::read(&omg).unwrap();
        let n = db.songs.len();
        // "Abba" sorts first: every album and 2D number shifts by one.
        let cover = {
            let mut png = Vec::new();
            image::DynamicImage::new_rgb8(500, 400).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
            png
        };
        db.art.insert("Abba Gold".into(), art_from(&cover).unwrap());
        db.songs.push(Song {
            title: "Waterloo".into(),
            artist: "ABBA".into(),
            album: "Abba Gold".into(),
            genre: "Pop".into(),
            year: Some(1992),
            length_ms: 1000,
            codec: oma::codec_params(14),
            cid: oma::new_content_id(&[]),
        });
        install(&root, &db).unwrap();
        let back = Database::read(&omg).unwrap();
        assert_eq!(back.songs.len(), n + 1);
        assert_eq!(back.songs, db.songs);
        assert_eq!(back.art.len(), db.art.len());
        for (album, art) in &db.art {
            assert_eq!(back.art.get(album), Some(art), "{album}");
        }
        for (name, bytes) in back.tables() {
            assert!(std::fs::read(omg.join(&name)).unwrap() == bytes, "{name}");
        }
        assert!(!root.join(STAGING).exists());
        let backups: Vec<_> = std::fs::read_dir(root.join(BACKUPS)).unwrap().flatten().collect();
        assert_eq!(backups.len(), 1);
        assert!(backups[0].path().join("04CNTINF.DAT").is_file());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The app's MP3 encoder makes what the Walkman takes (44.1 kHz, 320 kbps CBR).
    #[test]
    fn encoder_output_is_usable() {
        let dir = std::env::temp_dir().join(format!("chinotto-omg-enc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let wav = dir.join("tone.wav");
        let rate = 48_000u32;
        let samples: Vec<i16> = (0..rate * 2)
            .flat_map(|i| {
                let v = ((i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 8000.0) as i16;
                [v, v]
            })
            .collect();
        let mut w = b"RIFF".to_vec();
        w.extend((36 + samples.len() as u32 * 2).to_le_bytes());
        w.extend(b"WAVEfmt ");
        w.extend(16u32.to_le_bytes());
        w.extend(1u16.to_le_bytes());
        w.extend(2u16.to_le_bytes());
        w.extend(rate.to_le_bytes());
        w.extend((rate * 4).to_le_bytes());
        w.extend(4u16.to_le_bytes());
        w.extend(16u16.to_le_bytes());
        w.extend(b"data");
        w.extend((samples.len() as u32 * 2).to_le_bytes());
        w.extend(samples.iter().flat_map(|s| s.to_le_bytes()));
        std::fs::write(&wav, w).unwrap();
        let mp3 = crate::convert::mp3_44k(&wav, &dir, 0).unwrap();
        let m = chinotto_omg::mp3::prepare(&std::fs::read(mp3).unwrap()).unwrap();
        assert_eq!(m.bitrate_index, 14);
        assert!((1900..2200).contains(&m.length_ms), "{}", m.length_ms);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
