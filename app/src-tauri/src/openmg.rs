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
                track: info.track_nr.max(0) as u32,
                disc: info.disc_nr.max(0) as u32,
            };
            let mut song = Song {
                title: chinotto_omg::fit(&tags.title),
                artist: chinotto_omg::fit(&tags.artist),
                album: chinotto_omg::fit(&tags.album),
                genre: chinotto_omg::fit(&tags.genre),
                year: tags.year,
                track: tags.track,
                disc: tags.disc,
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
            song.file = Some(oma::path_for(id));
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
            // Songs already there that have no track number: from the libraries.
            let numbered = number_from_libraries(&app, &mut db);
            if let Err(e) = install(&root, &db) {
                // The database is as it was: the new files would only be clutter.
                for f in &written {
                    let _ = std::fs::remove_file(f);
                }
                return Err(e);
            }
            res.skipped.extend(save_numbers(&omg, &db, &numbered));
            attach(&app, &root);
        }
        Ok(res)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Give songs without a track number the number of the same song (title,
/// artist and album) in the computer's libraries; the indexes of those changed.
fn number_from_libraries(app: &AppHandle, db: &mut Database) -> Vec<usize> {
    let key = |t: &str, a: &str, b: &str| {
        (chinotto_omg::fit(t).to_lowercase(), chinotto_omg::fit(a).to_lowercase(), chinotto_omg::fit(b).to_lowercase())
    };
    let known: HashMap<(String, String, String), (u32, u32)> = {
        let state = app.state::<AppState>();
        let libs = state.libraries.lock().unwrap();
        libs.list
            .iter()
            .filter(|l| l.device.is_none())
            .flat_map(|l| l.tracks.iter())
            .filter(|t| t.tags.track_nr > 0)
            .map(|t| {
                let g = &t.tags;
                let or = |v: &Option<String>, d: &str| v.as_deref().map(str::trim).filter(|v| !v.is_empty()).unwrap_or(d).to_string();
                (key(&or(&g.title, ""), &or(&g.artist, "Unknown Artist"), &or(&g.album, "Unknown Album")), (g.track_nr, g.disc_nr))
            })
            .collect()
    };
    let mut changed = Vec::new();
    for (i, s) in db.songs.iter_mut().enumerate() {
        if s.track == 0 {
            if let Some(&(track, disc)) = known.get(&key(&s.title, &s.artist, &s.album)) {
                (s.track, s.disc) = (track, disc);
                changed.push(i);
            }
        }
    }
    changed
}

/// Keep track numbers found for songs already on the Walkman in their files'
/// tags (the database has no place for them); problems, as report lines.
fn save_numbers(omg: &Path, db: &Database, which: &[usize]) -> Vec<String> {
    let mut problems = Vec::new();
    for &i in which {
        let s = &db.songs[i];
        let Some(file) = &s.file else { continue };
        if let Err(e) = oma::rewrite_tag(&omg.join(file), &tags_of(s), s.length_ms) {
            problems.push(format!("{}: couldn't keep its track number ({e})", s.title));
        }
    }
    problems
}

/// Put the albums on the Walkman in track order: numbers for songs without
/// one come from the libraries; the database is rebuilt. How many songs got a number.
#[tauri::command]
pub async fn openmg_reorder(path: String, app: AppHandle) -> Result<usize, String> {
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
        let numbered = number_from_libraries(&app, &mut db);
        install(&root, &db)?;
        let problems = save_numbers(&omg, &db, &numbered);
        attach(&app, &root);
        if let Some(p) = problems.first() {
            return Err(p.clone());
        }
        Ok(numbered.len())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Put the rebuilt database (tables and pictures) in place: back up, stage,
/// swap; on failure the backup goes back.
fn install(root: &Path, db: &Database) -> Result<(), String> {
    let omg = root.join("OMGAUDIO");
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let backup = root.join(BACKUPS).join(format!("OpenMG {stamp:012}"));
    backup_omg(&omg, &backup).map_err(|e| format!("Couldn't back up the Walkman's database, so nothing was changed: {e}"))?;
    // Keep the newest ten backups (names sort by time).
    if let Ok(rd) = std::fs::read_dir(root.join(BACKUPS)) {
        let mut old: Vec<PathBuf> =
            rd.flatten().map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("OpenMG "))).collect();
        old.sort();
        while old.len() > 10 {
            let _ = std::fs::remove_dir_all(old.remove(0));
        }
    }
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

// ---- The Walkman's songs as a library; deleting and editing ----

/// The id of the song list of the Walkman at `root` ("o" + a hash of its path).
pub fn lib_id(root: &Path) -> String {
    let h = root.to_string_lossy().bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
    format!("o{h:016x}")
}

fn tags_of(s: &Song) -> oma::Tags {
    oma::Tags {
        title: s.title.clone(),
        artist: s.artist.clone(),
        album: s.album.clone(),
        genre: s.genre.clone(),
        year: s.year,
        track: s.track,
        disc: s.disc,
    }
}

/// Read the Walkman's database into its song list (a library that's never
/// saved), replacing the one there was.
pub fn attach(app: &AppHandle, root: &Path) {
    let omg = root.join("OMGAUDIO");
    let Ok(db) = Database::read(&omg) else { return };
    let id = lib_id(root);
    let name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "WALKMAN".into());
    let mut l = crate::library::Library::new(&name, Vec::new());
    l.id = id.clone();
    l.device = Some(format!("openmg:{}", root.display()));
    l.ephemeral = true;
    l.tracks = db
        .songs
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let path = s.file.as_ref().map(|f| omg.join(f)).unwrap_or_default();
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            crate::library::LibTrack {
                id: i as u32 + 1,
                path: path.to_string_lossy().into_owned(),
                size,
                mtime: 0,
                added: 0,
                play_count: 0,
                rating: 0,
                played: None,
                cover: false,
                tags: crate::library::Tags {
                    title: Some(s.title.clone()),
                    artist: Some(s.artist.clone()),
                    album: Some(s.album.clone()),
                    genre: Some(s.genre.clone()),
                    year: s.year.unwrap_or(0),
                    track_nr: s.track,
                    disc_nr: s.disc,
                    length_ms: s.length_ms,
                    bitrate: if s.length_ms > 0 { (size.saturating_sub(0xC60) * 8 / s.length_ms as u64) as u32 } else { 0 },
                    sample_rate: 44_100,
                    kind: "MP3 audio file".into(),
                    ..Default::default()
                },
            }
        })
        .collect();
    l.next_id = l.tracks.len() as u32 + 1;
    {
        let state = app.state::<AppState>();
        let mut libs = state.libraries.lock().unwrap();
        libs.list.retain(|x| x.id != id);
        libs.list.push(l);
    }
    crate::libcommands::announce(app, &id);
}

/// Forget the song lists of Walkmans no longer plugged in.
pub fn detach_missing(app: &AppHandle, present: &[String]) {
    let state = app.state::<AppState>();
    let mut libs = state.libraries.lock().unwrap();
    libs.list.retain(|l| {
        !(l.ephemeral && l.device.as_deref().and_then(|d| d.strip_prefix("openmg:")).is_some_and(|r| !present.iter().any(|p| p == r)))
    });
}

/// A song on the Walkman as an MP3 the computer can play (its audio, after
/// the tag and header, copied into a cache folder the first time).
pub fn fetch(path: &Path) -> Result<PathBuf, String> {
    let meta = std::fs::metadata(path).map_err(|_| "That song isn't on the Walkman any more".to_string())?;
    let mut head = vec![0u8; oma::TAG_SIZE + oma::HEADER_SIZE];
    std::fs::File::open(path).and_then(|mut f| f.read_exact(&mut head)).map_err(|e| e.to_string())?;
    let cid = oma::content_id(&head).ok_or("That isn't an OpenMG song file")?;
    let dir = std::env::temp_dir().join("chinotto-openmg");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let name: String = cid.iter().map(|b| format!("{b:02x}")).collect();
    let out = dir.join(format!("{name}-{}.mp3", meta.len()));
    if !out.is_file() {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let part = out.with_extension("part");
        std::fs::write(&part, &bytes[oma::TAG_SIZE + oma::HEADER_SIZE..]).map_err(|e| e.to_string())?;
        std::fs::rename(&part, &out).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

/// The Walkman whose song list is `lib`.
pub fn root_of_library(app: &AppHandle, lib: &str) -> Option<PathBuf> {
    let state = app.state::<AppState>();
    let libs = state.libraries.lock().unwrap();
    libs.list.iter().find(|l| l.id == lib).and_then(|l| l.device.as_deref()?.strip_prefix("openmg:").map(PathBuf::from))
}

fn writable_db(app: &AppHandle, root: &Path) -> Result<Database, String> {
    if !msc::is_openmg_root(root) {
        return Err("That Walkman isn't plugged in".into());
    }
    if !app.state::<AppState>().is_writable(&identity(root)) {
        return Err(SWITCHED_OFF.into());
    }
    Database::read(&root.join("OMGAUDIO")).map_err(|e| format!("Couldn't read the Walkman's database: {e}"))
}

/// Delete songs (rows of the song list, from 1). The other songs' files are
/// renumbered so file N stays song N; the deleted files are moved aside and
/// only removed once the new database is in place (any failure undoes it all).
pub fn delete(root: &Path, ids: &[u32], app: &AppHandle) -> Result<Vec<String>, String> {
    let db = writable_db(app, root)?;
    let r = remove(root, db, ids);
    attach(app, root);
    r.map(|_| Vec::new())
}

/// [`delete`] without the app: the files and database of the Walkman at `root`.
fn remove(root: &Path, mut db: Database, ids: &[u32]) -> Result<(), String> {
    let omg = root.join("OMGAUDIO");
    let gone: std::collections::HashSet<usize> = ids.iter().filter(|&&i| i >= 1).map(|&i| i as usize - 1).collect();
    if gone.is_empty() {
        return Ok(());
    }
    let aside = root.join(".chinotto-omg-deleted");
    let _ = std::fs::remove_dir_all(&aside);
    std::fs::create_dir_all(aside.join("moving")).map_err(|e| e.to_string())?;
    // Every rename done, to undo in reverse.
    let mut done: Vec<(PathBuf, PathBuf)> = Vec::new();
    let undo = |done: &mut Vec<(PathBuf, PathBuf)>| {
        while let Some((from, to)) = done.pop() {
            let _ = std::fs::rename(&to, &from);
        }
    };
    let step = |from: PathBuf, to: PathBuf, done: &mut Vec<(PathBuf, PathBuf)>| -> std::io::Result<()> {
        if let Some(dir) = to.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::rename(&from, &to)?;
        done.push((from, to));
        Ok(())
    };
    let songs = std::mem::take(&mut db.songs);
    let mut kept = Vec::with_capacity(songs.len());
    let mut moves: Vec<(PathBuf, String)> = Vec::new();
    let mut result = Ok(());
    for (i, mut s) in songs.into_iter().enumerate() {
        if gone.contains(&i) {
            if let Some(f) = &s.file {
                let to = aside.join(f.replace('/', "_"));
                if let Err(e) = step(omg.join(f), to, &mut done) {
                    result = Err(format!("Couldn't move “{}” aside: {e}", s.title));
                    break;
                }
            }
            continue;
        }
        let want = oma::path_for(kept.len() as u32 + 1);
        if let Some(f) = s.file.as_ref().filter(|f| !f.eq_ignore_ascii_case(&want)) {
            moves.push((omg.join(f), want.clone()));
        }
        if s.file.is_some() {
            s.file = Some(want);
        }
        kept.push(s);
    }
    // Two steps (via "moving"), so no file is renamed onto one still in use.
    if result.is_ok() {
        let mut parked = Vec::new();
        for (n, (from, want)) in moves.iter().enumerate() {
            let tmp = aside.join("moving").join(format!("{n}.OMA"));
            if let Err(e) = step(from.clone(), tmp.clone(), &mut done) {
                result = Err(format!("Couldn't renumber the Walkman's files: {e}"));
                break;
            }
            parked.push((tmp, want.clone()));
        }
        if result.is_ok() {
            for (tmp, want) in parked {
                if let Err(e) = step(tmp, omg.join(&want), &mut done) {
                    result = Err(format!("Couldn't renumber the Walkman's files: {e}"));
                    break;
                }
            }
        }
    }
    if let Err(e) = result {
        undo(&mut done);
        let _ = std::fs::remove_dir_all(&aside);
        return Err(e);
    }
    db.songs = kept;
    if let Err(e) = install(root, &db) {
        undo(&mut done);
        let _ = std::fs::remove_dir_all(&aside);
        return Err(e);
    }
    let _ = std::fs::remove_dir_all(&aside);
    Ok(())
}

/// Change songs on the Walkman: title, artist, album, genre, year, track and
/// disc number (the rest has no place on it), and the album's cover.
pub fn edit(
    app: &AppHandle,
    root: &Path,
    ids: &[u32],
    fields: &[chinotto_gpod::Field],
    cover: Option<&crate::tagwrite::CoverChange>,
) -> Result<Vec<String>, String> {
    let db = writable_db(app, root)?;
    let r = change(root, db, ids, fields, cover);
    attach(app, root);
    r
}

/// [`edit`] without the app.
fn change(
    root: &Path,
    mut db: Database,
    ids: &[u32],
    fields: &[chinotto_gpod::Field],
    cover: Option<&crate::tagwrite::CoverChange>,
) -> Result<Vec<String>, String> {
    use chinotto_gpod::Field as F;
    let omg = root.join("OMGAUDIO");
    let mut problems = Vec::new();
    let mut ignored = false;
    let text =
        |v: &Option<String>, unknown: &str| chinotto_omg::fit(v.as_deref().map(str::trim).filter(|v| !v.is_empty()).unwrap_or(unknown));
    let mut changed = Vec::new();
    for &id in ids {
        let Some(s) = db.songs.get_mut(id.saturating_sub(1) as usize).filter(|_| id >= 1) else { continue };
        let old_album = s.album.clone();
        for f in fields {
            match f {
                F::Title(v) => s.title = text(v, &s.title.clone()),
                F::Artist(v) => s.artist = text(v, "Unknown Artist"),
                F::Album(v) => s.album = text(v, "Unknown Album"),
                F::Genre(v) => s.genre = text(v, "Unknown Genre"),
                F::Year(y) => s.year = (*y > 0).then_some(*y as u32),
                F::TrackNr(n) => s.track = (*n).max(0) as u32,
                F::DiscNr(n) => s.disc = (*n).max(0) as u32,
                _ => ignored = true,
            }
        }
        // A renamed album keeps its pictures.
        if s.album != old_album && !db.art.contains_key(&s.album) {
            if let Some(a) = db.art.get(&old_album).cloned() {
                db.art.insert(s.album.clone(), a);
            }
        }
        changed.push(id as usize - 1);
    }
    match cover {
        Some(crate::tagwrite::CoverChange::Set(bytes, _)) => match art_from(bytes) {
            Some(art) => {
                for &i in &changed {
                    db.art.insert(db.songs[i].album.clone(), art.clone());
                }
            }
            None => problems.push("That picture couldn't be read.".to_string()),
        },
        Some(crate::tagwrite::CoverChange::Remove) => {
            for &i in &changed {
                db.art.remove(&db.songs[i].album.clone());
            }
        }
        None => {}
    }
    if ignored {
        problems.push(
            "Only title, artist, album, genre, year, track and disc number are kept on this Walkman; the other changes were left out."
                .into(),
        );
    }
    install(root, &db)?;
    for &i in &changed {
        let s = &db.songs[i];
        if let Some(f) = &s.file {
            if let Err(e) = oma::rewrite_tag(&omg.join(f), &tags_of(s), s.length_ms) {
                problems.push(format!("{}: {e}", s.title));
            }
        }
    }
    Ok(problems)
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

    /// A copy of a real Walkman's OMGAUDIO with an .OMA file for each song
    /// (made up audio, the songs' content IDs, files in row order).
    fn walkman_copy(tag: &str) -> Option<(PathBuf, Database)> {
        let src = std::env::var("CHINOTTO_TEST_OMG").ok()?;
        let root = std::env::temp_dir().join(format!("chinotto-omg-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        copy_dir(Path::new(&src), &root.join("OMGAUDIO"));
        let omg = root.join("OMGAUDIO");
        let _ = std::fs::remove_dir_all(omg.join("10F00"));
        let db = Database::read(&omg).unwrap();
        let frame = {
            let mut f = vec![0xFF, 0xFB, 0x90, 0x00];
            f.resize(417, 0);
            f
        };
        for (i, s) in db.songs.iter().enumerate() {
            let mp3 = chinotto_omg::mp3::prepare(&frame.repeat(i + 2)).unwrap();
            let p = omg.join(oma::path_for(i as u32 + 1));
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, oma::build(&mp3, &tags_of(s), &s.cid)).unwrap();
        }
        let db = Database::read(&omg).unwrap();
        Some((root, db))
    }

    #[test]
    fn deletes_on_a_copy() {
        let Some((root, db)) = walkman_copy("delete") else { return };
        let omg = root.join("OMGAUDIO");
        let n = db.songs.len();
        assert!(n >= 4);
        let keep: Vec<Song> = db.songs.iter().enumerate().filter(|(i, _)| *i != 0 && *i != 2).map(|(_, s)| s.clone()).collect();
        remove(&root, db, &[1, 3]).unwrap();
        let back = Database::read(&omg).unwrap();
        assert_eq!(back.songs.len(), n - 2);
        for (i, (b, k)) in back.songs.iter().zip(&keep).enumerate() {
            assert_eq!((&b.title, b.cid), (&k.title, k.cid));
            // Renumbered: song N is file N.
            assert_eq!(b.file.as_deref(), Some(oma::path_for(i as u32 + 1).as_str()));
        }
        let files = std::fs::read_dir(omg.join("10F00")).unwrap().count();
        assert_eq!(files, n - 2);
        for (name, bytes) in back.tables() {
            assert!(std::fs::read(omg.join(&name)).unwrap() == bytes, "{name}");
        }
        assert!(!root.join(".chinotto-omg-deleted").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn edits_on_a_copy() {
        use chinotto_gpod::Field as F;
        let Some((root, db)) = walkman_copy("edit") else { return };
        let omg = root.join("OMGAUDIO");
        let album = db.songs[0].album.clone();
        let had_art = db.art.contains_key(&album);
        let problems = change(
            &root,
            db,
            &[1],
            &[F::Title(Some("New title".into())), F::Album(Some("Renamed".into())), F::TrackNr(7), F::Comment(Some("x".into()))],
            None,
        )
        .unwrap();
        assert_eq!(problems.len(), 1); // the comment
        let back = Database::read(&omg).unwrap();
        let s = &back.songs[0];
        assert_eq!((s.title.as_str(), s.album.as_str(), s.track), ("New title", "Renamed", 7));
        assert_eq!(back.art.contains_key("Renamed"), had_art);
        let _ = std::fs::remove_dir_all(&root);
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
            ..Default::default()
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
