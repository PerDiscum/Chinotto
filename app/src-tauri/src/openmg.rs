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

/// Prepare one track-table record from a known-good device record.
///
/// OpenMG records contain opaque fields that are not understood yet, so this
/// deliberately starts from a template captured from the same device family.
/// It is only a building block for an off-device database generator: callers
/// must not append this record to a mounted Walkman until all related index and
/// tree tables can be updated together.
#[allow(dead_code)]
pub(crate) fn prepare_track_record(
    template: &[u8],
    title: &str,
    artist: &str,
    album: &str,
    genre: &str,
    length_ms: u32,
) -> Result<[u8; 656], String> {
    const TAG_SIZE: usize = 128;
    if template.len() != 656 {
        return Err("OpenMG track template must be exactly 656 bytes.".into());
    }
    let mut record = [0u8; 656];
    record.copy_from_slice(template);
    record[8..12].copy_from_slice(&length_ms.to_be_bytes());
    write_tag(&mut record, 16, b"TIT2", title, TAG_SIZE)?;
    write_tag(&mut record, 16 + TAG_SIZE, b"TPE1", artist, TAG_SIZE)?;
    write_tag(&mut record, 16 + TAG_SIZE * 2, b"TALB", album, TAG_SIZE)?;
    write_tag(&mut record, 16 + TAG_SIZE * 3, b"TCON", genre, TAG_SIZE)?;
    Ok(record)
}

/// Prepare a one-value artist, album, or genre row from a matching table row.
#[allow(dead_code)]
pub(crate) fn prepare_group_value_record(template: &[u8], value: &str, representative_length_ms: u32) -> Result<Vec<u8>, String> {
    if template.len() != 144 {
        return Err("OpenMG group-value template must be exactly 144 bytes.".into());
    }
    let mut record = template.to_vec();
    record[8..12].copy_from_slice(&representative_length_ms.to_be_bytes());
    write_tag(&mut record, 16, b"TIT2", value, 128)?;
    Ok(record)
}

/// Prepare the combined album, artist, and genre row from a matching row.
#[allow(dead_code)]
pub(crate) fn prepare_album_summary_record(
    template: &[u8],
    album: &str,
    artist: &str,
    genre: &str,
    representative_length_ms: u32,
) -> Result<Vec<u8>, String> {
    if template.len() != 0x310 {
        return Err("OpenMG album-summary template must be exactly 784 bytes.".into());
    }
    let mut record = template.to_vec();
    record[8..12].copy_from_slice(&representative_length_ms.to_be_bytes());
    write_tag(&mut record, 16, b"TIT2", album, 128)?;
    write_tag(&mut record, 144, b"TPE1", artist, 128)?;
    write_tag(&mut record, 272, b"TCON", genre, 128)?;
    Ok(record)
}

fn write_tag(record: &mut [u8], offset: usize, id: &[u8; 4], value: &str, size: usize) -> Result<(), String> {
    let block = record.get_mut(offset..offset + size).ok_or_else(|| "OpenMG tag block is outside the track record.".to_string())?;
    let encoded: Vec<u16> = value.encode_utf16().collect();
    let capacity = (size - 6) / 2;
    if encoded.len() > capacity {
        return Err(format!("OpenMG tag value exceeds the {capacity}-character device limit."));
    }
    block.fill(0);
    block[..4].copy_from_slice(id);
    block[4..6].copy_from_slice(&[0, 2]);
    for (index, unit) in encoded.into_iter().enumerate() {
        let start = 6 + index * 2;
        block[start..start + 2].copy_from_slice(&unit.to_be_bytes());
    }
    Ok(())
}

/// Prepare a `05CIDLST.DAT` row from an existing row, replacing only its key.
#[allow(dead_code)]
pub(crate) fn prepare_track_key_record(template: &[u8], key: [u8; 11]) -> Result<[u8; 48], String> {
    if template.len() != 48 {
        return Err("OpenMG track-key template must be exactly 48 bytes.".into());
    }
    let mut record = [0u8; 48];
    record.copy_from_slice(template);
    record[13..24].copy_from_slice(&key);
    Ok(record)
}

/// Append one fixed-size row to an OpenMG table whose count and payload length
/// are stored in the common 48-byte header. This only prepares table bytes in
/// memory; it must be combined with all related indexes before any device write.
#[allow(dead_code)]
pub(crate) fn append_counted_record(table: &[u8], record: &[u8], magic: &[u8; 4]) -> Result<Vec<u8>, String> {
    const HEADER_SIZE: usize = 48;
    const COUNT_OFFSET: usize = 0x24;
    const RECORD_SIZE_OFFSET: usize = 0x26;
    const PAYLOAD_LENGTH_OFFSET: usize = 0x1a;

    if table.len() < HEADER_SIZE || &table[..4] != magic {
        return Err("OpenMG table header is missing or has an unexpected type.".into());
    }
    let declared_record_size = u16::from_be_bytes([table[RECORD_SIZE_OFFSET], table[RECORD_SIZE_OFFSET + 1]]) as usize;
    if declared_record_size == 0 || record.len() != declared_record_size {
        return Err("OpenMG row size does not match the table's declared record size.".into());
    }
    let count = u16::from_be_bytes([table[COUNT_OFFSET], table[COUNT_OFFSET + 1]]) as usize;
    let expected_len = HEADER_SIZE
        .checked_add(count.checked_mul(declared_record_size).ok_or("OpenMG table is too large.")?)
        .ok_or("OpenMG table is too large.")?;
    if table.len() != expected_len {
        return Err("OpenMG table length does not match its header count.".into());
    }
    let new_count = count.checked_add(1).filter(|count| *count <= u16::MAX as usize).ok_or("OpenMG table has reached its record limit.")?;
    let payload_len = 16usize
        .checked_add(new_count.checked_mul(declared_record_size).ok_or("OpenMG table is too large.")?)
        .ok_or("OpenMG table is too large.")?;
    let payload_len = u32::try_from(payload_len).map_err(|_| "OpenMG table is too large.")?;

    let mut result = Vec::with_capacity(expected_len + declared_record_size);
    result.extend_from_slice(table);
    result.extend_from_slice(record);
    result[COUNT_OFFSET..COUNT_OFFSET + 2].copy_from_slice(&(new_count as u16).to_be_bytes());
    result[PAYLOAD_LENGTH_OFFSET..PAYLOAD_LENGTH_OFFSET + 4].copy_from_slice(&payload_len.to_be_bytes());
    Ok(result)
}
