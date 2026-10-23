//! Read-only access to the small, tagged track table used by Sony OpenMG Walkmans.

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
    parse_track_table(&bytes)
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
        tracks.push(OpenMgTrack { number: i + 1, title, artist, album, genre, length_ms });
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
