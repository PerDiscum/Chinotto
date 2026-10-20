//! Writes through libgpod, checked with Chinotto's own reader, on a *copy* of a
//! real iPod's database (CHINOTTO_TEST_IPOD; skipped when unset). The copy lives
//! in a temporary folder; the original is only read.

use std::path::{Path, PathBuf};

use chinotto_gpod::{Database, Field, TrackInfo};
use chinotto_ipod::itunesdb::{ITunesDb, Track};

/// A scratch iPod: the real iTunesDB, plus device files laid out like the
/// developer's nano 3G: model number in SysInfo, FireWire GUID only in
/// SysInfoExtended (made up here; the copy never goes back to an iPod).
/// The checksum these iPods need is computed from that GUID.
fn scratch_ipod(name: &str) -> Option<PathBuf> {
    scratch_ipod_with(name, Some("000A27001234ABCD"), true)
}

/// `db_version`: whether SysInfoExtended says DBVersion 3 (= hash58 checksum),
/// as real nano 3G/Classic files do. Without it libgpod writes no checksum.
fn scratch_ipod_with(name: &str, guid: Option<&str>, db_version: bool) -> Option<PathBuf> {
    let src = PathBuf::from(std::env::var_os("CHINOTTO_TEST_IPOD")?);
    let dir = std::env::temp_dir().join(format!("chinotto-gpod-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("iPod_Control/iTunes")).unwrap();
    std::fs::create_dir_all(dir.join("iPod_Control/Device")).unwrap();
    // Like a real iPod: music lives in F00…F49.
    for i in 0..50 {
        std::fs::create_dir_all(dir.join(format!("iPod_Control/Music/F{i:02}"))).unwrap();
    }
    std::fs::copy(src.join("iPod_Control/iTunes/iTunesDB"), dir.join("iPod_Control/iTunes/iTunesDB")).unwrap();
    std::fs::write(dir.join("iPod_Control/Device/SysInfo"), "ModelNumStr: xA980\n").unwrap();
    let mut keys = guid.map(|g| format!("<key>FireWireGUID</key>\n<string>{g}</string>\n")).unwrap_or_default();
    if db_version {
        keys.push_str("<key>DBVersion</key>\n<integer>3</integer>\n");
    }
    std::fs::write(
        dir.join("iPod_Control/Device/SysInfoExtended"),
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
             \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n{keys}\
             <key>FamilyID</key>\n<integer>9</integer>\n</dict>\n</plist>\n"
        ),
    )
    .unwrap();
    Some(dir)
}

fn read(root: &Path) -> ITunesDb {
    ITunesDb::parse(&std::fs::read(root.join("iPod_Control/iTunes/iTunesDB")).unwrap()).unwrap()
}

/// Everything per track that must survive a libgpod write unchanged.
fn essentials(t: &Track) -> impl PartialEq + std::fmt::Debug {
    let s = &t.strings;
    (
        t.dbid,
        (s.title.clone(), s.artist.clone(), s.album.clone(), s.album_artist.clone(), s.genre.clone(), s.location.clone()),
        (s.sort_artist.clone(), s.sort_album.clone(), s.sort_album_artist.clone(), s.composer.clone()),
        (t.track_nr, t.disc_nr, t.year, t.length_ms, t.size, t.rating, t.play_count, t.compilation),
        (t.has_artwork, t.artwork_count, t.artwork_size, t.mhii_link),
    )
}

#[test]
fn unchanged_save_keeps_every_track_and_playlist() {
    let Some(root) = scratch_ipod("unchanged") else { return };
    let before = read(&root);

    let mut db = Database::open(&root).unwrap();
    assert_eq!(db.track_count(), before.tracks.len());
    let backup = db.save().unwrap();
    assert!(backup.is_file(), "backup written");
    drop(db);

    let after = read(&root);
    assert_eq!(after.tracks.len(), before.tracks.len());
    let by_dbid: std::collections::HashMap<u64, &Track> = after.tracks.iter().map(|t| (t.dbid, t)).collect();
    for t in &before.tracks {
        let a = by_dbid.get(&t.dbid).unwrap_or_else(|| panic!("track {} lost", t.dbid));
        assert_eq!(essentials(a), essentials(t));
    }

    // Playlists: same names, same songs in the same order (by dbid).
    let dbid_of = |db: &ITunesDb, id: u32| db.track_by_id(id).map(|t| t.dbid);
    let lists = |db: &ITunesDb| -> Vec<(String, bool, Vec<Option<u64>>)> {
        db.playlists
            .iter()
            .filter(|p| !p.is_smart)
            .map(|p| (p.name.clone(), p.is_master, p.track_ids.iter().map(|id| dbid_of(db, *id)).collect()))
            .collect()
    };
    assert_eq!(lists(&after), lists(&before));
    assert_eq!(after.playlists.iter().filter(|p| p.is_smart).count(), before.playlists.iter().filter(|p| p.is_smart).count());

    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn edits_playlists_add_and_remove() {
    let Some(root) = scratch_ipod("edits") else { return };
    let before = read(&root);
    let first = before.tracks[0].dbid;
    let second = before.tracks[1].dbid;
    let gone = before.tracks[2].dbid;

    // A tiny WAV to add.
    let wav = root.join("new.wav");
    write_silent_wav(&wav);

    let mut db = Database::open(&root).unwrap();
    db.edit(
        &[first, second],
        &[Field::AlbumArtist(Some("Älbum Ärtist".into())), Field::SortAlbumArtist(Some("album artist".into())), Field::Rating(80)],
    )
    .unwrap();
    db.edit(&[first], &[Field::Title(Some("Dieci (edited)".into())), Field::DiscNr(2), Field::Compilation(true)]).unwrap();

    let pl = db.create_playlist("Chinotto test").unwrap();
    db.set_playlist_tracks(pl, &[second, first]).unwrap();

    let info = TrackInfo {
        title: Some("Silence".into()),
        artist: Some("Chinotto".into()),
        album: Some("Tests".into()),
        track_nr: 1,
        length_ms: 1000,
        bitrate_kbps: 1411,
        sample_rate: 44100,
        filetype: Some("WAV audio file".into()),
        ..Default::default()
    };
    let added = db.add_track(&wav, &info).unwrap();
    let added_path = db.track(added).unwrap().ipod_path.unwrap();
    assert!(added_path.starts_with(":iPod_Control:Music:F"), "{added_path}");

    db.remove_tracks(&[gone]).unwrap();
    db.save().unwrap();
    drop(db);

    let after = read(&root);
    assert_eq!(after.tracks.len(), before.tracks.len()); // one added, one removed
    let t = |dbid: u64| after.tracks.iter().find(|t| t.dbid == dbid);
    let a = t(first).unwrap();
    assert_eq!(a.strings.title.as_deref(), Some("Dieci (edited)"));
    assert_eq!(a.strings.album_artist.as_deref(), Some("Älbum Ärtist"));
    assert_eq!(a.strings.sort_album_artist.as_deref(), Some("album artist"));
    assert_eq!((a.rating, a.disc_nr, a.compilation), (80, 2, true));
    // Cover link untouched by the edit.
    assert_eq!(a.mhii_link, before.tracks[0].mhii_link);
    assert_eq!(t(second).unwrap().strings.album_artist.as_deref(), Some("Älbum Ärtist"));
    assert!(t(gone).is_none());

    let n = t(added).unwrap();
    assert_eq!(n.strings.title.as_deref(), Some("Silence"));
    assert_eq!(n.size as u64, std::fs::metadata(&wav).unwrap().len());
    let on_ipod = root.join(added_path.trim_start_matches(':').replace(':', "/"));
    assert!(on_ipod.is_file(), "music file copied to {}", on_ipod.display());

    let p = after.playlists.iter().find(|p| p.name == "Chinotto test").unwrap();
    let ids: Vec<u64> = p.track_ids.iter().map(|id| after.track_by_id(*id).unwrap().dbid).collect();
    assert_eq!(ids, vec![second, first]);
    let master = after.master_playlist().unwrap();
    assert!(master.track_ids.iter().any(|id| after.track_by_id(*id).unwrap().dbid == added));

    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn save_without_firewire_guid_is_refused_and_changes_nothing() {
    let Some(root) = scratch_ipod_with("noguid", None, true) else { return };
    let original = std::fs::read(root.join("iPod_Control/iTunes/iTunesDB")).unwrap();
    let mut db = Database::open(&root).unwrap();
    let first = db.track_ids()[0];
    db.edit(&[first], &[Field::Title(Some("should not be saved".into()))]).unwrap();
    let err = db.save().unwrap_err();
    assert!(err.0.to_lowercase().contains("firewire"), "{err}");
    drop(db);
    assert_eq!(std::fs::read(root.join("iPod_Control/iTunes/iTunesDB")).unwrap(), original);
    std::fs::remove_dir_all(&root).unwrap();
}

/// If the device files don't say a checksum is needed but the iPod's current
/// database has one, Chinotto must not replace it with an unchecksummed one.
#[test]
fn save_that_would_drop_the_checksum_is_rolled_back() {
    let Some(root) = scratch_ipod_with("nodbversion", Some("000A27001234ABCD"), false) else { return };
    let path = root.join("iPod_Control/iTunes/iTunesDB");
    let original = std::fs::read(&path).unwrap();
    let mut db = Database::open(&root).unwrap();
    let first = db.track_ids()[0];
    let gone = db.track_ids()[1];
    db.edit(&[first], &[Field::Title(Some("should not be saved".into()))]).unwrap();
    db.remove_tracks(&[gone]).unwrap();
    let err = db.save().unwrap_err();
    assert!(err.0.contains("checksum"), "{err}");
    drop(db);
    assert_eq!(std::fs::read(&path).unwrap(), original, "previous database restored");
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn saved_database_carries_a_checksum() {
    let Some(root) = scratch_ipod("checksum") else { return };
    let mut db = Database::open(&root).unwrap();
    db.save().unwrap();
    drop(db);
    let d = std::fs::read(root.join("iPod_Control/iTunes/iTunesDB")).unwrap();
    assert_eq!(u16::from_le_bytes([d[0x30], d[0x31]]), 1, "hash58 scheme");
    assert!(d[0x58..0x6C].iter().any(|b| *b != 0), "hash58 filled in");
    std::fs::remove_dir_all(&root).unwrap();
}

fn write_silent_wav(path: &Path) {
    let samples = 44_100u32; // 1 s, 16-bit stereo
    let data_len = samples * 4;
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&44_100u32.to_le_bytes());
    b.extend_from_slice(&(44_100u32 * 4).to_le_bytes());
    b.extend_from_slice(&4u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    b.resize(b.len() + data_len as usize, 0);
    std::fs::write(path, b).unwrap();
}
