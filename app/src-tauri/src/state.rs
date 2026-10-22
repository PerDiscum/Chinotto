//! Application state: folders added by hand, the currently open iPod (read
//! with chinotto-ipod), the libgpod writer for it, and saved settings.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use std::sync::OnceLock;

use chinotto_gpod::Database;
use chinotto_ipod::artworkdb::ArtworkDb;
use chinotto_ipod::device::{Layout, SysInfo};
use chinotto_ipod::itunesdb::ITunesDb;
use chinotto_player::Player;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct AppState {
    manual: Mutex<Vec<PathBuf>>,
    session: RwLock<Option<Arc<Session>>>,
    generation: AtomicU32,
    /// libgpod's copy of the open iPod's database, opened on the first change.
    pub writer: Mutex<Option<Database>>,
    settings: Mutex<Settings>,
    settings_path: Mutex<Option<PathBuf>>,
    player: OnceLock<Player>,
    /// Remembers which volumes are iPods (see devices::Scanner).
    pub scanner: Mutex<crate::devices::Scanner>,
    /// Music libraries on the computer.
    pub libraries: Mutex<crate::libcommands::Libraries>,
    /// Songs opened from outside the libraries.
    pub scratch: Mutex<crate::scratch::Scratch>,
    /// MTP devices found, and the one open.
    pub mtp: crate::mtp::Shared,
}

/// Saved between runs in the app's config folder (settings.json).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// iPods (by FireWire GUID or serial) the user has allowed Chinotto to change.
    pub writable_ipods: Vec<String>,
    /// Picture chosen by the user per device (identity → picture key).
    pub device_icons: HashMap<String, String>,
    /// How songs are converted for each device (identity → setting).
    pub conversions: HashMap<String, crate::convert::Conversion>,
}

/// An iPod's databases, read into memory. Read-only for now.
pub struct Session {
    pub root: PathBuf,
    /// Changes every time an iPod is opened, so cached artwork URLs never go stale.
    pub generation: u32,
    pub db: ITunesDb,
    pub sysinfo: SysInfo,
    pub art: Option<ArtworkDb>,
    /// ArtworkDB image id → index in `art.images`.
    pub art_index: HashMap<u32, usize>,
}

impl AppState {
    pub fn set_player(&self, p: Player) {
        let _ = self.player.set(p);
    }

    pub fn player(&self) -> &Player {
        self.player.get().expect("player not started")
    }

    pub fn manual_roots(&self) -> Vec<PathBuf> {
        self.manual.lock().unwrap().clone()
    }

    pub fn add_manual_root(&self, p: PathBuf) {
        let mut m = self.manual.lock().unwrap();
        if !m.contains(&p) {
            m.push(p);
        }
    }

    pub fn session(&self) -> Option<Arc<Session>> {
        self.session.read().unwrap().clone()
    }

    pub fn close(&self) {
        *self.session.write().unwrap() = None;
        *self.writer.lock().unwrap() = None;
    }

    pub fn close_if(&self, root: &Path) {
        let mut s = self.session.write().unwrap();
        if s.as_ref().is_some_and(|s| s.root == root) {
            *s = None;
        }
        let mut w = self.writer.lock().unwrap();
        if w.as_ref().is_some_and(|w| w.root() == root) {
            *w = None;
        }
    }

    pub fn load_settings(&self, path: PathBuf) {
        let s = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        *self.settings.lock().unwrap() = s;
        *self.settings_path.lock().unwrap() = Some(path);
    }

    pub fn is_writable(&self, identity: &str) -> bool {
        self.settings.lock().unwrap().writable_ipods.iter().any(|i| i == identity)
    }

    pub fn set_writable(&self, identity: &str, allow: bool) -> Result<(), String> {
        let mut s = self.settings.lock().unwrap();
        s.writable_ipods.retain(|i| i != identity);
        if allow {
            s.writable_ipods.push(identity.to_string());
        }
        self.save_settings(&s)
    }

    pub fn device_icons(&self) -> HashMap<String, String> {
        self.settings.lock().unwrap().device_icons.clone()
    }

    /// `None` goes back to the automatic picture.
    pub fn set_device_icon(&self, identity: &str, icon: Option<String>) -> Result<(), String> {
        let mut s = self.settings.lock().unwrap();
        match icon {
            Some(k) => s.device_icons.insert(identity.to_string(), k),
            None => s.device_icons.remove(identity),
        };
        self.save_settings(&s)
    }

    pub fn conversion(&self, identity: &str) -> crate::convert::Conversion {
        self.settings.lock().unwrap().conversions.get(identity).cloned().unwrap_or_default()
    }

    pub fn set_conversion(&self, identity: &str, c: crate::convert::Conversion) -> Result<(), String> {
        let mut s = self.settings.lock().unwrap();
        s.conversions.insert(identity.to_string(), c);
        self.save_settings(&s)
    }

    fn save_settings(&self, s: &Settings) -> Result<(), String> {
        if let Some(path) = self.settings_path.lock().unwrap().as_ref() {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            // Write a temporary file, then rename over the old one.
            let tmp = path.with_extension("json.tmp");
            std::fs::write(&tmp, serde_json::to_vec_pretty(s).unwrap()).map_err(|e| e.to_string())?;
            std::fs::rename(&tmp, path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Cover URLs contain this number, and the window may cache them. It must
    /// never repeat across launches (or two iPods' covers get mixed up), so it
    /// starts from the clock and counts up.
    fn next_generation(&self) -> u32 {
        let _ = self.generation.compare_exchange(
            0,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| (d.as_secs() as u32) & 0x3fff_ffff)
                .unwrap_or(1)
                .max(1),
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
        self.generation.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn open(&self, root: &Path) -> Result<Arc<Session>, String> {
        let layout = Layout::new(root);
        let db = if layout.itunesdb().is_file() {
            let bytes = std::fs::read(layout.itunesdb()).map_err(|e| format!("Couldn't read the iPod's database: {e}"))?;
            ITunesDb::parse(&bytes).map_err(|e| format!("The iPod's database couldn't be read: {e}"))?
        } else if layout.itunescdb().is_file() {
            // Nano 6 stores its index as a compressed iTunesCDB. libgpod can
            // read that container; expose a read-only view until Chinotto can
            // update the companion SQLite databases and their signatures.
            let gpod = Database::open(root).map_err(|e| format!("Couldn't read the iPod's compressed database: {e}"))?;
            let track_ids = gpod.track_ids();
            let indexes: HashMap<u64, u32> = track_ids.iter().enumerate().map(|(i, id)| (*id, i as u32 + 1)).collect();
            let tracks = track_ids
                .iter()
                .filter_map(|id| {
                    let t = gpod.track(*id)?;
                    Some(chinotto_ipod::itunesdb::Track {
                        id: indexes[id],
                        dbid: t.dbid,
                        strings: chinotto_ipod::itunesdb::TrackStrings {
                            title: t.title,
                            artist: t.artist,
                            album_artist: t.album_artist,
                            album: t.album,
                            genre: t.genre,
                            composer: t.composer,
                            comment: t.comment,
                            grouping: t.grouping,
                            sort_artist: t.sort_artist,
                            sort_title: t.sort_title,
                            sort_album: t.sort_album,
                            sort_album_artist: t.sort_album_artist,
                            sort_composer: t.sort_composer,
                            filetype: t.filetype,
                            location: t.ipod_path,
                            ..Default::default()
                        },
                        year: t.year.max(0) as u32,
                        track_nr: t.track_nr.max(0) as u32,
                        track_count: t.track_count.max(0) as u32,
                        disc_nr: t.disc_nr.max(0) as u32,
                        disc_count: t.disc_count.max(0) as u32,
                        length_ms: t.length_ms.max(0) as u32,
                        bitrate_kbps: t.bitrate_kbps.max(0) as u32,
                        sample_rate: t.sample_rate,
                        rating: t.rating.min(100) as u8,
                        compilation: t.compilation,
                        play_count: t.play_count,
                        ..Default::default()
                    })
                })
                .collect();
            let playlists = gpod
                .playlists()
                .into_iter()
                .map(|p| chinotto_ipod::itunesdb::Playlist {
                    id: p.id,
                    name: p.name,
                    is_master: p.master,
                    is_smart: p.smart,
                    track_ids: p.tracks.iter().filter_map(|id| indexes.get(id).copied()).collect(),
                })
                .collect();
            ITunesDb { tracks, playlists, ..Default::default() }
        } else {
            return Err("This iPod has no readable iTunes database".into());
        };
        // Artwork is optional: a missing or unreadable ArtworkDB just means no covers.
        let art = std::fs::read(layout.artworkdb()).ok().and_then(|b| ArtworkDb::parse(&b).ok());
        let art_index = art.as_ref().map(|a| a.images.iter().enumerate().map(|(i, im)| (im.id, i)).collect()).unwrap_or_default();
        let session = Arc::new(Session {
            root: root.to_path_buf(),
            generation: self.next_generation(),
            db,
            sysinfo: SysInfo::read(root).unwrap_or_default(),
            art,
            art_index,
        });
        *self.session.write().unwrap() = Some(session.clone());
        Ok(session)
    }
}
