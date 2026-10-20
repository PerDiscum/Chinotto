//! chinotto-inspect: summarise and sanity-check a classic iPod.
//! Usage: chinotto-inspect <iPod root folder>   (read-only)

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::ExitCode;

use chinotto_ipod::artworkdb::ArtworkDb;
use chinotto_ipod::itunesdb::ITunesDb;

fn main() -> ExitCode {
    let Some(root) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: chinotto-inspect <iPod root folder>");
        return ExitCode::from(2);
    };
    match run(&root) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(root: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let ctl = root.join("iPod_Control");
    if let Ok(sysinfo) = std::fs::read_to_string(ctl.join("Device/SysInfo")) {
        for line in sysinfo.lines().filter(|l| l.starts_with("ModelNumStr") || l.starts_with("FirewireGuid")) {
            println!("{line}");
        }
    }

    let db = ITunesDb::parse(&std::fs::read(ctl.join("iTunes/iTunesDB"))?)?;
    let total_ms: u64 = db.tracks.iter().map(|t| t.length_ms as u64).sum();
    let total_bytes: u64 = db.tracks.iter().map(|t| t.size as u64).sum();
    println!(
        "Tracks: {}  ({:.1} days, {:.1} GB)",
        db.tracks.len(),
        total_ms as f64 / 86_400_000.0,
        total_bytes as f64 / 1e9
    );
    println!("Database version: 0x{:x}; sections: {:?}", db.version, db.sections);

    println!("Playlists: {}", db.playlists.len());
    for p in &db.playlists {
        let kind = if p.is_master { "library" } else if p.is_smart { "smart" } else { "" };
        println!("  {:5} tracks  {}{}", p.track_ids.len(), p.name, if kind.is_empty() { String::new() } else { format!("  [{kind}]") });
    }

    let mut kinds: HashMap<&str, usize> = HashMap::new();
    for t in &db.tracks {
        *kinds.entry(t.strings.filetype.as_deref().unwrap_or("(none)")).or_default() += 1;
    }
    println!("File kinds: {kinds:?}");

    let no_album_artist: Vec<_> = db.tracks.iter().filter(|t| t.strings.album_artist.is_none()).collect();
    println!("Tracks without album artist: {}", no_album_artist.len());
    for t in no_album_artist.iter().take(10) {
        println!(
            "    {} — {} ({})",
            t.strings.artist.as_deref().unwrap_or("?"),
            t.strings.title.as_deref().unwrap_or("?"),
            t.strings.album.as_deref().unwrap_or("?")
        );
    }

    // Playlist entries that point at missing tracks
    let ids: HashSet<u32> = db.tracks.iter().map(|t| t.id).collect();
    let dangling: usize = db.playlists.iter().flat_map(|p| &p.track_ids).filter(|id| !ids.contains(id)).count();
    println!("Playlist entries pointing at missing tracks: {dangling}");

    match std::fs::read(ctl.join("Artwork/ArtworkDB")) {
        Err(_) => println!("Artwork: no ArtworkDB"),
        Ok(bytes) => {
            let art = ArtworkDb::parse(&bytes)?;
            let image_ids: HashSet<u32> = art.images.iter().map(|i| i.id).collect();
            let with_art = db.tracks.iter().filter(|t| t.has_artwork == 1).count();
            let broken: Vec<_> = db.tracks.iter().filter(|t| t.has_artwork == 1 && !image_ids.contains(&t.mhii_link)).collect();
            let dbids: HashSet<u64> = db.tracks.iter().map(|t| t.dbid).collect();
            let orphans = art.images.iter().filter(|i| !dbids.contains(&i.song_dbid)).count();
            let formats = art.common_formats();
            println!("Artwork: {} entries; tracks with covers: {with_art}", art.images.len());
            println!(
                "  formats: {}",
                formats.iter().map(|f| format!("{} ({}×{}, {} B)", f.format, f.width, f.height, f.size)).collect::<Vec<_>>().join(", ")
            );
            let mut files: Vec<_> = art.last_file_index().into_iter().collect();
            files.sort();
            println!("  highest .ithmb file per format: {files:?}");
            println!("  tracks whose cover link points at no entry: {}", broken.len());
            for t in broken.iter().take(10) {
                println!("    {} — {} (link {})", t.strings.artist.as_deref().unwrap_or("?"), t.strings.title.as_deref().unwrap_or("?"), t.mhii_link);
            }
            println!("  cover entries for tracks no longer on the iPod: {orphans}");
        }
    }
    Ok(())
}
