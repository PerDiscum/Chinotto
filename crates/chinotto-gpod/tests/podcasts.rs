//! Podcast episodes on a fresh, empty iPod that libgpod sets up in a
//! temporary folder (no real iPod or private data needed).

use chinotto_gpod::{init_ipod, Database, PodcastInfo, TrackInfo};
use chinotto_ipod::itunesdb::ITunesDb;

#[test]
fn episodes_go_under_podcasts() {
    let root = std::env::temp_dir().join(format!("chinotto-gpod-podcast-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    // An iPod nano 2G: no database checksum, so no FireWire ID needed.
    init_ipod(&root, Some("xA477"), "Test").unwrap();
    let file = root.join("episode.mp3");
    std::fs::write(&file, vec![0u8; 4096]).unwrap();

    let mut db = Database::open(&root).unwrap();
    let info = TrackInfo {
        title: Some("Ep. 1".into()),
        album: Some("The Show".into()),
        artist: Some("Host".into()),
        length_ms: 60_000,
        ..Default::default()
    };
    let pod = PodcastInfo {
        feed_url: "https://example.com/feed.xml".into(),
        episode_url: "https://cdn.example.com/ep1.mp3".into(),
        description: "First line.\nMore notes.".into(),
        released: 1_728_394_200,
    };
    let id = db.add_podcast(&file, &info, &pod).unwrap();
    db.save().unwrap();
    drop(db);

    // libgpod reads it back as a podcast, unplayed, in the Podcasts playlist.
    let db = Database::open(&root).unwrap();
    let eps = db.podcasts();
    assert_eq!(eps.len(), 1);
    assert_eq!((eps[0].dbid, eps[0].album.as_str(), eps[0].played), (id, "The Show", false));
    assert_eq!(eps[0].episode_url, pod.episode_url);
    assert_eq!(eps[0].feed_url, pod.feed_url);
    assert!(db.playlists().iter().any(|p| p.name == "Podcasts" && p.tracks == [id]));

    // Chinotto's own reader sees media type 4 (podcast).
    let itdb = ITunesDb::parse(&std::fs::read(root.join("iPod_Control/iTunes/iTunesDB")).unwrap()).unwrap();
    assert_eq!(itdb.tracks.iter().find(|t| t.dbid == id).unwrap().media_type, 4);
    let _ = std::fs::remove_dir_all(&root);
}
