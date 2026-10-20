//! Application state: folders added by hand and the currently open iPod.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use chinotto_ipod::artworkdb::ArtworkDb;
use chinotto_ipod::device::{Layout, SysInfo};
use chinotto_ipod::itunesdb::ITunesDb;

#[derive(Default)]
pub struct AppState {
    manual: Mutex<Vec<PathBuf>>,
    session: RwLock<Option<Arc<Session>>>,
    generation: AtomicU32,
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
    }

    pub fn close_if(&self, root: &Path) {
        let mut s = self.session.write().unwrap();
        if s.as_ref().is_some_and(|s| s.root == root) {
            *s = None;
        }
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
