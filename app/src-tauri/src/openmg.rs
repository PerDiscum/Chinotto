//! Read-only access to the small, tagged track table used by Sony OpenMG Walkmans.

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
