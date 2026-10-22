//! chinotto-gpod: change a classic iPod's database through libgpod.
//!
//! libgpod (bundled, built without its artwork code) parses `iTunesDB`, applies
//! our edits in memory and writes it back with the checksum newer iPods need.
//! The ArtworkDB is left alone: each track's cover link (`mhii_link` etc.) is
//! kept as read, and Chinotto's own ArtworkDB writer adds new covers.
//!
//! Tracks are identified by their 64-bit database id (`dbid`), which stays the
//! same across saves (the 32-bit `id` is renumbered by every write).
//!
//! Safety on the iPod:
//! - [`Database::save`] copies the current `iTunesDB` into
//!   `iPod_Control/iTunes/Chinotto Backups/` first (the newest
//!   [`BACKUPS_KEPT`] are kept), and libgpod writes through a temporary file
//!   that replaces the old one only once it's complete.
//! - Music files of removed tracks are deleted only after a successful save.

pub mod ffi;

use std::ffi::{c_char, c_void, CStr, CString};
use std::fmt;
use std::path::{Path, PathBuf};
use std::ptr;

use ffi::*;

extern "C" {
    fn g_free(p: *mut c_void);
    fn g_strdup(s: *const c_char) -> *mut c_char;
    fn g_error_free(e: *mut GError);
}

/// How many database backups [`Database::save`] keeps on the iPod.
pub const BACKUPS_KEPT: usize = 10;
pub const BACKUP_DIR: &str = "iPod_Control/iTunes/Chinotto Backups";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error(e.to_string())
    }
}

type Result<T> = std::result::Result<T, Error>;

/// Takes ownership of a GError and turns it into our error.
unsafe fn take_gerror(e: *mut GError, what: &str) -> Error {
    if e.is_null() {
        return Error(format!("{what} failed"));
    }
    let msg = CStr::from_ptr((*e).message).to_string_lossy().into_owned();
    g_error_free(e);
    Error(format!("{what}: {msg}"))
}

unsafe fn get_str(p: *const gchar) -> Option<String> {
    if p.is_null() {
        None
    } else {
        Some(CStr::from_ptr(p).to_string_lossy().into_owned()).filter(|s| !s.is_empty())
    }
}

/// Replace a libgpod-owned string field (`None` or "" clears it).
unsafe fn set_str(slot: *mut *mut gchar, v: Option<&str>) {
    g_free(*slot as *mut c_void);
    *slot = match v.filter(|s| !s.is_empty()) {
        Some(s) => {
            let c = CString::new(s.replace('\0', "")).unwrap();
            g_strdup(c.as_ptr())
        }
        None => ptr::null_mut(),
    };
}

unsafe fn list<T>(mut l: *mut GList) -> Vec<*mut T> {
    let mut out = Vec::new();
    while !l.is_null() {
        out.push((*l).data as *mut T);
        l = (*l).next;
    }
    out
}

/// One editable field. Strings set to `None` (or "") are cleared.
#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    Title(Option<String>),
    Artist(Option<String>),
    AlbumArtist(Option<String>),
    Album(Option<String>),
    Genre(Option<String>),
    Composer(Option<String>),
    Comment(Option<String>),
    Grouping(Option<String>),
    SortTitle(Option<String>),
    SortArtist(Option<String>),
    SortAlbum(Option<String>),
    SortAlbumArtist(Option<String>),
    SortComposer(Option<String>),
    Year(i32),
    TrackNr(i32),
    TrackCount(i32),
    DiscNr(i32),
    DiscCount(i32),
    /// 0–100 in steps of 20 (stars × 20).
    Rating(u32),
    Compilation(bool),
    Bpm(i16),
    /// Link to a cover in the ArtworkDB: `Some((image id, source width × height + 1))`,
    /// or `None` for no cover. (The ArtworkDB itself is written by Chinotto.)
    Artwork(Option<(u32, u32)>),
}

/// Everything about a track that Chinotto edits or needs when adding one.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackInfo {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub composer: Option<String>,
    pub comment: Option<String>,
    pub grouping: Option<String>,
    pub sort_title: Option<String>,
    pub sort_artist: Option<String>,
    pub sort_album: Option<String>,
    pub sort_album_artist: Option<String>,
    pub sort_composer: Option<String>,
    pub year: i32,
    pub track_nr: i32,
    pub track_count: i32,
    pub disc_nr: i32,
    pub disc_count: i32,
    pub rating: u32,
    pub compilation: bool,
    pub bpm: i16,
    // Technical details (filled from the file when adding).
    pub length_ms: i32,
    pub bitrate_kbps: i32,
    pub sample_rate: u32,
    /// Description shown as "Kind", e.g. "MPEG audio file", "AAC audio file".
    pub filetype: Option<String>,
    pub play_count: u32,
    pub dbid: u64,
    pub ipod_path: Option<String>,
}

impl TrackInfo {
    /// The fields of `self` that [`Database::edit`] can set, as a list.
    pub fn fields(&self) -> Vec<Field> {
        vec![
            Field::Title(self.title.clone()),
            Field::Artist(self.artist.clone()),
            Field::AlbumArtist(self.album_artist.clone()),
            Field::Album(self.album.clone()),
            Field::Genre(self.genre.clone()),
            Field::Composer(self.composer.clone()),
            Field::Comment(self.comment.clone()),
            Field::Grouping(self.grouping.clone()),
            Field::SortTitle(self.sort_title.clone()),
            Field::SortArtist(self.sort_artist.clone()),
            Field::SortAlbum(self.sort_album.clone()),
            Field::SortAlbumArtist(self.sort_album_artist.clone()),
            Field::SortComposer(self.sort_composer.clone()),
            Field::Year(self.year),
            Field::TrackNr(self.track_nr),
            Field::TrackCount(self.track_count),
            Field::DiscNr(self.disc_nr),
            Field::DiscCount(self.disc_count),
            Field::Rating(self.rating),
            Field::Compilation(self.compilation),
            Field::Bpm(self.bpm),
        ]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaylistInfo {
    pub id: u64,
    pub name: String,
    pub master: bool,
    pub smart: bool,
    /// Track dbids in order.
    pub tracks: Vec<u64>,
}

/// An iPod database opened for changes.
pub struct Database {
    itdb: *mut Itdb_iTunesDB,
    root: PathBuf,
    /// Music files to delete once the next save succeeds.
    pending_deletes: Vec<PathBuf>,
}

// libgpod has no global state; a Database is only ever used by one thread at a time.
unsafe impl Send for Database {}

impl Drop for Database {
    fn drop(&mut self) {
        unsafe { itdb_free(self.itdb) }
    }
}

impl Database {
    /// Parse the iPod mounted at `root` (its top folder, containing iPod_Control).
    pub fn open(root: &Path) -> Result<Self> {
        let c = CString::new(root.to_string_lossy().as_bytes()).map_err(|_| Error("bad path".into()))?;
        let mut err: *mut GError = ptr::null_mut();
        let itdb = unsafe { itdb_parse(c.as_ptr(), &mut err) };
        if itdb.is_null() {
            return Err(unsafe { take_gerror(err, "Couldn't read the iPod's database") });
        }
        if !err.is_null() {
            unsafe { g_error_free(err) };
        }
        Ok(Database { itdb, root: root.to_path_buf(), pending_deletes: Vec::new() })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn tracks_raw(&self) -> Vec<*mut Itdb_Track> {
        unsafe { list((*self.itdb).tracks) }
    }

    fn playlists_raw(&self) -> Vec<*mut Itdb_Playlist> {
        unsafe { list((*self.itdb).playlists) }
    }

    fn find(&self, dbid: u64) -> Option<*mut Itdb_Track> {
        self.tracks_raw().into_iter().find(|t| unsafe { (**t).dbid } == dbid)
    }

    fn find_playlist(&self, id: u64) -> Option<*mut Itdb_Playlist> {
        self.playlists_raw().into_iter().find(|p| unsafe { (**p).id } == id)
    }

    pub fn track_count(&self) -> usize {
        self.tracks_raw().len()
    }

    pub fn track_ids(&self) -> Vec<u64> {
        self.tracks_raw().iter().map(|t| unsafe { (**t).dbid }).collect()
    }

    pub fn track(&self, dbid: u64) -> Option<TrackInfo> {
        let t = self.find(dbid)?;
        Some(unsafe { read_track(t) })
    }

    /// Change fields on several tracks at once. Unknown dbids are an error and nothing changes.
    pub fn edit(&mut self, dbids: &[u64], fields: &[Field]) -> Result<()> {
        let tracks: Vec<_> =
            dbids.iter().map(|id| self.find(*id).ok_or_else(|| Error(format!("no track with id {id}")))).collect::<Result<_>>()?;
        let now = unix_now();
        for t in tracks {
            unsafe {
                for f in fields {
                    apply(t, f);
                }
                (*t).time_modified = now;
            }
        }
        Ok(())
    }

    /// Copy a music file onto the iPod and add it to the library.
    /// Returns the new track's dbid. Nothing is saved until [`save`](Self::save).
    pub fn add_track(&mut self, file: &Path, info: &TrackInfo) -> Result<u64> {
        let size = std::fs::metadata(file)?.len();
        let size = u32::try_from(size).map_err(|_| Error(format!("{} is larger than 4 GB", file.display())))?;
        let cfile = CString::new(file.to_string_lossy().as_bytes()).map_err(|_| Error("bad path".into()))?;
        unsafe {
            let t = itdb_track_new();
            for f in info.fields() {
                apply(t, &f);
            }
            set_str(&mut (*t).filetype, info.filetype.as_deref());
            (*t).size = size;
            (*t).tracklen = info.length_ms;
            (*t).bitrate = info.bitrate_kbps;
            (*t).samplerate = info.sample_rate.min(u16::MAX as u32) as u16;
            (*t).mediatype = 1; // audio
            let now = unix_now();
            (*t).time_added = now;
            (*t).time_modified = now;
            (*t).has_artwork = 2; // none, until Chinotto adds a cover
            (*t).visible = 1;
            itdb_track_add(self.itdb, t, -1);
            let mpl = itdb_playlist_mpl(self.itdb);
            if !mpl.is_null() {
                itdb_playlist_add_track(mpl, t, -1);
            }
            let mut err: *mut GError = ptr::null_mut();
            if itdb_cp_track_to_ipod(t, cfile.as_ptr(), &mut err) == 0 {
                let e = take_gerror(err, &format!("Couldn't copy {}", file.display()));
                self.unlink(t);
                itdb_track_remove(t);
                return Err(e);
            }
            Ok((*t).dbid)
        }
    }

    /// Add a music file that's already on the iPod (under `iPod_Control/Music`)
    /// to the library, without copying it: for rebuilding a lost database.
    pub fn add_existing(&mut self, file: &Path, info: &TrackInfo) -> Result<u64> {
        let rel = file.strip_prefix(&self.root).map_err(|_| Error(format!("{} isn't on this iPod", file.display())))?;
        // libgpod's form: ":iPod_Control:Music:F00:ABCD.mp3".
        let ipod_path: String = rel.components().map(|c| format!(":{}", c.as_os_str().to_string_lossy())).collect();
        let size = std::fs::metadata(file)?.len();
        let size = u32::try_from(size).map_err(|_| Error(format!("{} is larger than 4 GB", file.display())))?;
        unsafe {
            let t = itdb_track_new();
            for f in info.fields() {
                apply(t, &f);
            }
            set_str(&mut (*t).filetype, info.filetype.as_deref());
            set_str(&mut (*t).ipod_path, Some(&ipod_path));
            (*t).transferred = 1;
            (*t).size = size;
            (*t).tracklen = info.length_ms;
            (*t).bitrate = info.bitrate_kbps;
            (*t).samplerate = info.sample_rate.min(u16::MAX as u32) as u16;
            (*t).mediatype = 1;
            let now = unix_now();
            (*t).time_added = now;
            (*t).time_modified = now;
            (*t).has_artwork = 2;
            (*t).visible = 1;
            itdb_track_add(self.itdb, t, -1);
            let mpl = itdb_playlist_mpl(self.itdb);
            if !mpl.is_null() {
                itdb_playlist_add_track(mpl, t, -1);
            }
            Ok((*t).dbid)
        }
    }

    /// Remove tracks from the library and every playlist. Their music files are
    /// deleted from the iPod after the next successful save.
    pub fn remove_tracks(&mut self, dbids: &[u64]) -> Result<()> {
        for id in dbids {
            let t = self.find(*id).ok_or_else(|| Error(format!("no track with id {id}")))?;
            unsafe {
                let name = itdb_filename_on_ipod(t);
                if !name.is_null() {
                    self.pending_deletes.push(PathBuf::from(CStr::from_ptr(name).to_string_lossy().into_owned()));
                    g_free(name as *mut c_void);
                }
                self.unlink(t);
                itdb_track_remove(t);
            }
        }
        Ok(())
    }

    /// Take a track out of every playlist (including the library playlist).
    unsafe fn unlink(&self, t: *mut Itdb_Track) {
        for pl in self.playlists_raw() {
            if itdb_playlist_contains_track(pl, t) != 0 {
                itdb_playlist_remove_track(pl, t);
            }
        }
    }

    pub fn playlists(&self) -> Vec<PlaylistInfo> {
        self.playlists_raw()
            .into_iter()
            .map(|p| unsafe {
                PlaylistInfo {
                    id: (*p).id,
                    name: get_str((*p).name).unwrap_or_default(),
                    master: itdb_playlist_is_mpl(p) != 0,
                    smart: (*p).is_spl != 0,
                    tracks: list::<Itdb_Track>((*p).members).iter().map(|t| (**t).dbid).collect(),
                }
            })
            .collect()
    }

    /// New, empty, regular playlist at the end of the list. Returns its id.
    pub fn create_playlist(&mut self, name: &str) -> Result<u64> {
        let c = CString::new(name).map_err(|_| Error("bad name".into()))?;
        unsafe {
            let p = itdb_playlist_new(c.as_ptr(), 0);
            itdb_playlist_add(self.itdb, p, -1);
            Ok((*p).id)
        }
    }

    pub fn rename_playlist(&mut self, id: u64, name: &str) -> Result<()> {
        let p = self.any_playlist(id)?;
        unsafe { set_str(&mut (*p).name, Some(name)) };
        Ok(())
    }

    /// Delete a playlist (its songs stay in the library).
    pub fn delete_playlist(&mut self, id: u64) -> Result<()> {
        let p = self.any_playlist(id)?;
        unsafe { itdb_playlist_remove(p) };
        Ok(())
    }

    /// Replace a playlist's contents with `dbids`, in that order.
    pub fn set_playlist_tracks(&mut self, id: u64, dbids: &[u64]) -> Result<()> {
        let p = self.editable_playlist(id)?;
        let tracks: Vec<_> =
            dbids.iter().map(|id| self.find(*id).ok_or_else(|| Error(format!("no track with id {id}")))).collect::<Result<_>>()?;
        unsafe {
            for t in list::<Itdb_Track>((*p).members) {
                itdb_playlist_remove_track(p, t);
            }
            for t in tracks {
                itdb_playlist_add_track(p, t, -1);
            }
        }
        Ok(())
    }

    /// Any playlist but the library itself (smart ones too: for renaming and deleting).
    fn any_playlist(&self, id: u64) -> Result<*mut Itdb_Playlist> {
        let p = self.find_playlist(id).ok_or_else(|| Error(format!("no playlist with id {id}")))?;
        if unsafe { itdb_playlist_is_mpl(p) } != 0 {
            return Err(Error("The library playlist can't be changed directly".into()));
        }
        Ok(p)
    }

    fn editable_playlist(&self, id: u64) -> Result<*mut Itdb_Playlist> {
        let p = self.find_playlist(id).ok_or_else(|| Error(format!("no playlist with id {id}")))?;
        unsafe {
            if itdb_playlist_is_mpl(p) != 0 {
                return Err(Error("The library playlist can't be changed directly".into()));
            }
            if (*p).is_spl != 0 {
                return Err(Error("Smart playlists can't be edited yet".into()));
            }
        }
        Ok(p)
    }

    /// Back up the current iTunesDB, then write the new one.
    ///
    /// If the old database carried a checksum (iPod Classic, nano 3G and
    /// later) and the new one doesn't, the iPod would show an empty library:
    /// the backup is put back and an error returned.
    pub fn save(&mut self) -> Result<PathBuf> {
        let path = self.root.join("iPod_Control/iTunes/iTunesDB");
        let before = checksum_scheme(&path);
        let backup = backup_itunesdb(&self.root)?;
        let mut err: *mut GError = ptr::null_mut();
        if unsafe { itdb_write(self.itdb, &mut err) } == 0 {
            return Err(unsafe { take_gerror(err, "Couldn't save the iPod's database") });
        }
        if !err.is_null() {
            unsafe { g_error_free(err) };
        }
        let after = checksum_scheme(&path);
        if before.is_some_and(|b| b != 0) && after != before {
            std::fs::copy(&backup, &path).map_err(|e| {
                Error(format!(
                    "The new database lacks the iPod's checksum and the backup couldn't be restored ({e}). Restore it by hand from {}",
                    backup.display()
                ))
            })?;
            return Err(Error(format!(
                "Not saved: the new database would have lacked the checksum this iPod needs (type {} before, {} after). The previous database is back in place.",
                before.unwrap_or(0),
                after.map(|a| a.to_string()).unwrap_or_else(|| "none".into())
            )));
        }
        for f in self.pending_deletes.drain(..) {
            // A file that's already gone is fine.
            let _ = std::fs::remove_file(f);
        }
        Ok(backup)
    }
}

/// Set up an iPod that has no database: libgpod creates the folders
/// (`iPod_Control/Music/F00…`, `iTunes`, `Device`), records `model` (e.g.
/// "xC297") in SysInfo, and writes an empty database named `name`.
///
/// Refuses when a database is already there (see [`set_aside_database`]).
/// For models that need a checksum, SysInfo must already hold the
/// FirewireGuid, or the iPod will show no music.
pub fn init_ipod(root: &Path, model: Option<&str>, name: &str) -> Result<()> {
    if root.join("iPod_Control/iTunes/iTunesDB").exists() || root.join("iPod_Control/iTunes/iTunesCDB").exists() {
        return Err(Error("This iPod already has a database".into()));
    }
    let c_root = CString::new(root.to_string_lossy().as_bytes()).map_err(|_| Error("bad path".into()))?;
    let c_model = model.map(|m| CString::new(m).map_err(|_| Error("bad model".into()))).transpose()?;
    let c_name = CString::new(name).map_err(|_| Error("bad name".into()))?;
    let mut err: *mut GError = ptr::null_mut();
    let ok = unsafe { itdb_init_ipod(c_root.as_ptr(), c_model.as_ref().map_or(ptr::null(), |m| m.as_ptr()), c_name.as_ptr(), &mut err) };
    if ok == 0 {
        return Err(unsafe { take_gerror(err, "Couldn't set up the iPod") });
    }
    if !err.is_null() {
        unsafe { g_error_free(err) };
    }
    Ok(())
}

/// Move the iPod's database (and its artwork database) into
/// `Chinotto Backups/Before rebuild <time>/`, so a new one can be set up.
/// Nothing is deleted. Returns the folder they went to.
pub fn set_aside_database(root: &Path) -> Result<PathBuf> {
    let dest = root.join(BACKUP_DIR).join(format!("Before rebuild {}", utc_stamp(unix_now() as i64)));
    std::fs::create_dir_all(&dest)?;
    let itunes = root.join("iPod_Control/iTunes/iTunesDB");
    if itunes.exists() {
        std::fs::rename(&itunes, dest.join("iTunesDB"))?;
    }
    // Other files the iPod keeps beside the database describe the old one.
    for extra in ["iTunesSD", "iTunesPState", "iTunesStats", "Play Counts", "OTGPlaylistInfo"] {
        let p = root.join("iPod_Control/iTunes").join(extra);
        if p.exists() {
            std::fs::rename(&p, dest.join(extra))?;
        }
    }
    let art = root.join("iPod_Control/Artwork");
    if art.exists() {
        std::fs::rename(&art, dest.join("Artwork"))?;
    }
    Ok(dest)
}

/// The checksum scheme an iTunesDB header declares (0 none, 1 hash58,
/// 2 hash72, 4 hashAB), and for hash58 whether the hash is actually filled in.
/// `None` when the file can't be read or isn't an iTunesDB.
fn checksum_scheme(path: &Path) -> Option<u16> {
    use std::io::Read;
    let mut head = [0u8; 0x6C];
    std::fs::File::open(path).ok()?.read_exact(&mut head).ok()?;
    if &head[..4] != b"mhbd" {
        return None;
    }
    let scheme = u16::from_le_bytes([head[0x30], head[0x31]]);
    // A hash58 header with an all-zero hash is as good as none.
    if scheme == 1 && head[0x58..0x6C].iter().all(|b| *b == 0) {
        return Some(0);
    }
    Some(scheme)
}

unsafe fn read_track(t: *mut Itdb_Track) -> TrackInfo {
    let t = &*t;
    TrackInfo {
        title: get_str(t.title),
        artist: get_str(t.artist),
        album_artist: get_str(t.albumartist),
        album: get_str(t.album),
        genre: get_str(t.genre),
        composer: get_str(t.composer),
        comment: get_str(t.comment),
        grouping: get_str(t.grouping),
        sort_title: get_str(t.sort_title),
        sort_artist: get_str(t.sort_artist),
        sort_album: get_str(t.sort_album),
        sort_album_artist: get_str(t.sort_albumartist),
        sort_composer: get_str(t.sort_composer),
        year: t.year,
        track_nr: t.track_nr,
        track_count: t.tracks,
        disc_nr: t.cd_nr,
        disc_count: t.cds,
        rating: t.rating,
        compilation: t.compilation != 0,
        bpm: t.BPM,
        length_ms: t.tracklen,
        bitrate_kbps: t.bitrate,
        sample_rate: t.samplerate as u32,
        filetype: get_str(t.filetype),
        play_count: t.playcount,
        dbid: t.dbid,
        ipod_path: get_str(t.ipod_path),
    }
}

unsafe fn apply(t: *mut Itdb_Track, f: &Field) {
    let t = &mut *t;
    match f {
        Field::Title(v) => set_str(&mut t.title, v.as_deref()),
        Field::Artist(v) => set_str(&mut t.artist, v.as_deref()),
        Field::AlbumArtist(v) => set_str(&mut t.albumartist, v.as_deref()),
        Field::Album(v) => set_str(&mut t.album, v.as_deref()),
        Field::Genre(v) => set_str(&mut t.genre, v.as_deref()),
        Field::Composer(v) => set_str(&mut t.composer, v.as_deref()),
        Field::Comment(v) => set_str(&mut t.comment, v.as_deref()),
        Field::Grouping(v) => set_str(&mut t.grouping, v.as_deref()),
        Field::SortTitle(v) => set_str(&mut t.sort_title, v.as_deref()),
        Field::SortArtist(v) => set_str(&mut t.sort_artist, v.as_deref()),
        Field::SortAlbum(v) => set_str(&mut t.sort_album, v.as_deref()),
        Field::SortAlbumArtist(v) => set_str(&mut t.sort_albumartist, v.as_deref()),
        Field::SortComposer(v) => set_str(&mut t.sort_composer, v.as_deref()),
        Field::Year(v) => t.year = *v,
        Field::TrackNr(v) => t.track_nr = *v,
        Field::TrackCount(v) => t.tracks = *v,
        Field::DiscNr(v) => t.cd_nr = *v,
        Field::DiscCount(v) => t.cds = *v,
        Field::Rating(v) => t.rating = (*v).min(100) / 20 * 20,
        Field::Compilation(v) => t.compilation = *v as u8,
        Field::Bpm(v) => t.BPM = *v,
        Field::Artwork(Some((id, size))) => {
            t.mhii_link = *id;
            t.artwork_count = 1;
            t.artwork_size = *size;
            t.has_artwork = 1;
        }
        Field::Artwork(None) => {
            t.mhii_link = 0;
            t.artwork_count = 0;
            t.artwork_size = 0;
            t.has_artwork = 2;
        }
    }
}

fn unix_now() -> time_t {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as time_t).unwrap_or(0)
}

/// Copy iTunesDB to the backup folder as `iTunesDB <UTC timestamp>`, keeping the newest few.
fn backup_itunesdb(root: &Path) -> Result<PathBuf> {
    let src = root.join("iPod_Control/iTunes/iTunesDB");
    let dir = root.join(BACKUP_DIR);
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(format!("iTunesDB {}", utc_stamp(unix_now() as i64)));
    std::fs::copy(&src, &dest).map_err(|e| Error(format!("Couldn't back up the database before saving: {e}")))?;
    // Prune only our own backups, oldest first (names sort by time).
    let mut ours: Vec<PathBuf> = std::fs::read_dir(&dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("iTunesDB 20")))
        .collect();
    ours.sort();
    while ours.len() > BACKUPS_KEPT {
        let _ = std::fs::remove_file(ours.remove(0));
    }
    Ok(dest)
}

/// `2026-10-20 143005` (UTC; sorts by time; no characters FAT32 rejects).
fn utc_stamp(unix: i64) -> String {
    let days = unix.div_euclid(86_400);
    let secs = unix.rem_euclid(86_400);
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02} {:02}{:02}{:02}", secs / 3600, secs % 3600 / 60, secs % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps() {
        assert_eq!(utc_stamp(0), "1970-01-01 000000");
        assert_eq!(utc_stamp(1_792_503_005), "2026-10-20 133005");
    }
}
