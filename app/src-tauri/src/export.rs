//! Saving songs from the iPod (or a library) to the computer: one song as a
//! file, several as a zip (stored, not compressed: music doesn't shrink).

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use tauri::AppHandle;

/// A song's file and the name to give it.
struct Song {
    file: PathBuf,
    /// "Artist - Title.ext" for a single file; "Artist/Album/01 Title.ext" in a zip.
    single: String,
    in_zip: String,
}

fn clean(s: &str) -> String {
    let c: String = s.chars().map(|c| if "/\\:*?\"<>|".contains(c) || c.is_control() { '_' } else { c }).collect();
    let c = c.trim().trim_matches('.').to_string();
    if c.is_empty() {
        "Unknown".into()
    } else {
        c
    }
}

/// Names come from the window (the song's tags as shown), so they match
/// what the user sees.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SongRef {
    dbid: String,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    track_nr: Option<u32>,
}

fn resolve(app: &AppHandle, s: &SongRef) -> Result<Song, String> {
    let file = if s.dbid.starts_with("lib:") {
        crate::libcommands::track_file(app, &s.dbid)?
    } else {
        crate::playback::track_file(app, s.dbid.parse().map_err(|_| "bad id")?)?
    };
    let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("mp3").to_ascii_lowercase();
    let title = clean(s.title.as_deref().unwrap_or("Untitled"));
    let artist = clean(s.artist.as_deref().unwrap_or("Unknown Artist"));
    let album = clean(s.album.as_deref().unwrap_or("Unknown Album"));
    let nr = s.track_nr.filter(|n| *n > 0).map(|n| format!("{n:02} ")).unwrap_or_default();
    Ok(Song { single: format!("{artist} - {title}.{ext}"), in_zip: format!("{artist}/{album}/{nr}{title}.{ext}"), file })
}

/// The name a single song would be saved as (for the save dialog).
#[tauri::command]
pub fn export_name(song: SongRef, app: AppHandle) -> Result<String, String> {
    Ok(resolve(&app, &song)?.single)
}

/// Copy `songs` to `dest`: the file itself for one song, else a zip. Never
/// replaces a file that's already there (the save dialog asks first; if it
/// still exists, a new name is used).
#[tauri::command]
pub async fn export_songs(songs: Vec<SongRef>, dest: String, app: AppHandle) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let list: Vec<Song> = songs.iter().map(|s| resolve(&app, s)).collect::<Result<_, _>>()?;
        let dest = PathBuf::from(dest);
        // Written beside the destination, then renamed, so a half-written file never has its name.
        let tmp = dest.with_file_name(format!(
            ".{}.chinotto-{}",
            dest.file_name().and_then(|n| n.to_str()).unwrap_or("export"),
            std::process::id()
        ));
        let result = if list.len() == 1 {
            std::fs::copy(&list[0].file, &tmp).map(|_| ()).map_err(|e| format!("Couldn't save the song: {e}"))
        } else {
            write_zip(&list, &tmp)
        };
        if let Err(e) = result {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
        std::fs::rename(&tmp, &dest).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("Couldn't save: {e}")
        })?;
        Ok(dest.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| e.to_string())?
}

fn write_zip(list: &[Song], path: &Path) -> Result<(), String> {
    let f = std::fs::File::create(path).map_err(|e| format!("Couldn't create the zip: {e}"))?;
    let mut z = zip::ZipWriter::new(std::io::BufWriter::new(f));
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored).large_file(true);
    let mut used = HashSet::new();
    for s in list {
        // Two songs with the same name get "(2)", "(3)"…
        let mut name = s.in_zip.clone();
        let mut n = 2;
        while !used.insert(name.clone()) {
            let (stem, ext) = s.in_zip.rsplit_once('.').unwrap_or((&s.in_zip, ""));
            name = format!("{stem} ({n}).{ext}");
            n += 1;
        }
        z.start_file(name, opts).map_err(|e| e.to_string())?;
        let mut src = std::fs::File::open(&s.file).map_err(|e| format!("{}: {e}", s.file.display()))?;
        std::io::copy(&mut src, &mut z).map_err(|e| format!("Couldn't write the zip: {e}"))?;
    }
    z.finish().map_err(|e| e.to_string())?.flush().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zips_songs_with_unique_names() {
        let dir = std::env::temp_dir().join(format!("chinotto-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.mp3");
        std::fs::write(&a, b"first").unwrap();
        let b = dir.join("b.mp3");
        std::fs::write(&b, b"second").unwrap();
        let songs = [
            Song { file: a, single: String::new(), in_zip: "Air/Moon Safari/01 Sexy Boy.mp3".into() },
            Song { file: b, single: String::new(), in_zip: "Air/Moon Safari/01 Sexy Boy.mp3".into() },
        ];
        let zip = dir.join("out.zip");
        write_zip(&songs, &zip).unwrap();
        let mut z = zip::ZipArchive::new(std::fs::File::open(&zip).unwrap()).unwrap();
        let names: Vec<String> = (0..z.len()).map(|i| z.by_index(i).unwrap().name().unwrap().to_string()).collect();
        assert_eq!(names, ["Air/Moon Safari/01 Sexy Boy.mp3", "Air/Moon Safari/01 Sexy Boy (2).mp3"]);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(clean("AC/DC: Live?"), "AC_DC_ Live_");
    }
}
