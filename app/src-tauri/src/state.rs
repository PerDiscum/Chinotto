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
}

/// Saved between runs in the app's config folder (settings.json).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// iPods (by FireWire GUID or serial) the user has allowed Chinotto to change.
    pub writable_ipods: Vec<String>,
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
        if let Some(path) = self.settings_path.lock().unwrap().as_ref() {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            // Write a temporary file, then rename over the old one.
            let tmp = path.with_extension("json.tmp");
            std::fs::write(&tmp, serde_json::to_vec_pretty(&*s).unwrap()).map_err(|e| e.to_string())?;
            std::fs::rename(&tmp, path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn open(&self, root: &Path) -> Result<Arc<Session>, String> {
        let layout = Layout::new(root);
        let bytes = std::fs::read(layout.itunesdb()).map_err(|e| format!("Couldn't read the iPod's database: {e}"))?;
        let db = ITunesDb::parse(&bytes).map_err(|e| format!("The iPod's database couldn't be read: {e}"))?;
        // Artwork is optional: a missing or unreadable ArtworkDB just means no covers.
        let art = std::fs::read(layout.artworkdb()).ok().and_then(|b| ArtworkDb::parse(&b).ok());
        let art_index = art.as_ref().map(|a| a.images.iter().enumerate().map(|(i, im)| (im.id, i)).collect()).unwrap_or_default();
        let session = Arc::new(Session {
            root: root.to_path_buf(),
            generation: self.generation.fetch_add(1, Ordering::Relaxed) + 1,
            db,
            sysinfo: SysInfo::read(root).unwrap_or_default(),
            art,
            art_index,
        });
        *self.session.write().unwrap() = Some(session.clone());
        Ok(session)
    }
}
