//! Tests against a real iPod's database files (never committed).
//! Set CHINOTTO_TEST_IPOD to a folder laid out like an iPod root; skipped otherwise.
//! The expected values below come from the developer's own iPod Classic 7G.

use std::path::PathBuf;

use chinotto_ipod::artworkdb::{ArtworkDb, NewImage, NewThumb};
use chinotto_ipod::itunesdb::ITunesDb;

fn ipod() -> Option<PathBuf> {
    std::env::var_os("CHINOTTO_TEST_IPOD").map(PathBuf::from)
}

#[test]
fn reads_tracks_and_playlists() {
    let Some(root) = ipod() else { return };
    let db = ITunesDb::parse(&std::fs::read(root.join("iPod_Control/iTunes/iTunesDB")).unwrap()).unwrap();
    assert_eq!(db.tracks.len(), 23516);
    assert_eq!(db.playlists.len(), 18);

    let t = &db.tracks[0];
    assert_eq!(t.strings.title.as_deref(), Some("Dieci"));
    assert_eq!(t.strings.artist.as_deref(), Some("Annalisa"));
    assert_eq!(t.strings.album.as_deref(), Some("Nuda10"));
    assert_eq!(t.strings.album_artist.as_deref(), Some("Annalisa"));
    assert_eq!(t.strings.location.as_deref(), Some(":iPod_Control:Music:F40:libgpod412390.m4a"));
    assert_eq!(
        (t.track_nr, t.disc_nr, t.year, t.length_ms, t.bitrate_kbps, t.sample_rate, t.size),
        (1, 1, 2021, 200000, 257, 44100, 7290839)
    );
    assert_eq!((t.has_artwork, t.artwork_count, t.mhii_link), (1, 1, 100));

    let master = db.master_playlist().unwrap();
    assert_eq!(master.name, "CAESAR VERIDIS");
    assert_eq!(master.track_ids.len(), 23516);
    let runner = db.playlists.iter().find(|p| p.name == "0 - R U N N E R").unwrap();
    assert_eq!(runner.track_ids.len(), 129);
    assert!(runner.track_ids.iter().all(|id| db.track_by_id(*id).is_some()));
    assert_eq!(db.playlists.iter().filter(|p| p.is_smart).count(), 3);

    // Non-Latin text survives
    assert!(db.tracks.iter().any(|t| t.strings.artist.as_deref() == Some("Φοίβος Δεληβοριάς")));
}

#[test]
fn artwork_append_matches_the_proven_javascript_writer() {
    let Some(root) = ipod() else { return };
    let art_path = root.join("iPod_Control/Artwork/ArtworkDB");
    let Ok(bytes) = std::fs::read(&art_path) else { return };
    let art = ArtworkDb::parse(&bytes).unwrap();
    assert_eq!(art.images.len(), 23093);
    assert_eq!(art.max_id(), 23192);
    // iTunes' own covers on this nano carry five formats, so those win.
    let common: Vec<u32> = art.common_formats().iter().map(|f| f.format).collect();
    assert_eq!(common, vec![1055, 1060, 1061, 1068, 1069]);
    let formats = vec![1055, 1060, 1061];

    // Same two entries the JavaScript test appended; compare byte-for-byte when that output is present.
    let tpl = art.find_template(&formats).unwrap();
    let entries = vec![
        NewImage {
            id: 23193,
            song_dbid: 10208284158074208777,
            source_pixels: 360 * 360 + 1,
            thumbs: vec![
                NewThumb { format: 1055, file: "F1055_2.ithmb".into(), offset: 0, width: 128, height: 128 },
                NewThumb { format: 1060, file: "F1060_2.ithmb".into(), offset: 11059200, width: 320, height: 320 },
                NewThumb { format: 1061, file: "F1061_1.ithmb".into(), offset: 16699760, width: 56, height: 56 },
            ],
        },
        NewImage {
            id: 23194,
            song_dbid: 1,
            source_pixels: 1000 * 1000 + 1,
            thumbs: vec![
                NewThumb { format: 1055, file: "F1055_2.ithmb".into(), offset: 32768, width: 128, height: 128 },
                NewThumb { format: 1060, file: "F1060_2.ithmb".into(), offset: 11264000, width: 320, height: 320 },
                NewThumb { format: 1061, file: "F1061_1.ithmb".into(), offset: 16705920, width: 56, height: 56 },
            ],
        },
    ];
    let out = art.append(tpl, &entries).unwrap();
    let reparsed = ArtworkDb::parse(&out).unwrap();
    assert_eq!(reparsed.images.len(), 23095);
    assert_eq!(reparsed.images[..23093], art.images[..]);
    if let Ok(js) = std::fs::read(root.join("ArtworkDB.appended-by-js")) {
        assert_eq!(out, js, "Rust and JavaScript writers disagree");
    }
}
