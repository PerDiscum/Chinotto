//! chinotto-mtp: MTP players (Creative Zen, SanDisk Sansa in MTP mode, Sony
//! Walkman, Android phones…) through a bundled libmtp (and libusb).
//!
//! libmtp isn't thread-safe per device: a [`Device`] must be used by one
//! thread at a time (the app keeps it behind a mutex).
//!
//! Finding devices: [`usb_fingerprint`] is cheap (it only lists USB devices
//! without opening them), so the app polls it and calls [`detect`] (which
//! probes devices) only when it changes.

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::Path;
use std::ptr;
use std::sync::Once;

#[allow(non_camel_case_types, dead_code)]
mod ffi {
    use std::ffi::{c_char, c_int, c_void};

    #[repr(C)]
    pub struct LIBMTP_device_entry_t {
        pub vendor: *mut c_char,
        pub vendor_id: u16,
        pub product: *mut c_char,
        pub product_id: u16,
        pub device_flags: u32,
    }

    #[repr(C)]
    pub struct LIBMTP_raw_device_t {
        pub device_entry: LIBMTP_device_entry_t,
        pub bus_location: u32,
        pub devnum: u8,
    }

    #[repr(C)]
    pub struct LIBMTP_devicestorage_t {
        pub id: u32,
        pub storage_type: u16,
        pub filesystem_type: u16,
        pub access_capability: u16,
        pub max_capacity: u64,
        pub free_space_in_bytes: u64,
        pub free_space_in_objects: u64,
        pub storage_description: *mut c_char,
        pub volume_identifier: *mut c_char,
        pub next: *mut LIBMTP_devicestorage_t,
        pub prev: *mut LIBMTP_devicestorage_t,
    }

    #[repr(C)]
    pub struct LIBMTP_error_t {
        pub errornumber: c_int,
        pub error_text: *mut c_char,
        pub next: *mut LIBMTP_error_t,
    }

    #[repr(C)]
    pub struct LIBMTP_mtpdevice_t {
        pub object_bitsize: u8,
        pub params: *mut c_void,
        pub usbinfo: *mut c_void,
        pub storage: *mut LIBMTP_devicestorage_t,
        pub errorstack: *mut LIBMTP_error_t,
        pub maximum_battery_level: u8,
        pub default_music_folder: u32,
        pub default_playlist_folder: u32,
        pub default_picture_folder: u32,
        pub default_video_folder: u32,
        pub default_organizer_folder: u32,
        pub default_zencast_folder: u32,
        pub default_album_folder: u32,
        pub default_text_folder: u32,
        pub cd: *mut c_void,
        pub extensions: *mut c_void,
        pub cached: c_int,
        pub next: *mut LIBMTP_mtpdevice_t,
    }

    #[repr(C)]
    pub struct LIBMTP_track_t {
        pub item_id: u32,
        pub parent_id: u32,
        pub storage_id: u32,
        pub title: *mut c_char,
        pub artist: *mut c_char,
        pub composer: *mut c_char,
        pub genre: *mut c_char,
        pub album: *mut c_char,
        pub date: *mut c_char,
        pub filename: *mut c_char,
        pub tracknumber: u16,
        pub duration: u32,
        pub samplerate: u32,
        pub nochannels: u16,
        pub wavecodec: u32,
        pub bitrate: u32,
        pub bitratetype: u16,
        pub rating: u16,
        pub usecount: u32,
        pub filesize: u64,
        pub modificationdate: i64,
        pub filetype: c_int,
        pub next: *mut LIBMTP_track_t,
    }

    #[repr(C)]
    pub struct LIBMTP_playlist_t {
        pub playlist_id: u32,
        pub parent_id: u32,
        pub storage_id: u32,
        pub name: *mut c_char,
        pub tracks: *mut u32,
        pub no_tracks: u32,
        pub next: *mut LIBMTP_playlist_t,
    }

    #[repr(C)]
    pub struct LIBMTP_album_t {
        pub album_id: u32,
        pub parent_id: u32,
        pub storage_id: u32,
        pub name: *mut c_char,
        pub artist: *mut c_char,
        pub composer: *mut c_char,
        pub genre: *mut c_char,
        pub tracks: *mut u32,
        pub no_tracks: u32,
        pub next: *mut LIBMTP_album_t,
    }

    #[repr(C)]
    pub struct LIBMTP_filesampledata_t {
        pub width: u32,
        pub height: u32,
        pub duration: u32,
        pub filetype: c_int,
        pub size: u64,
        pub data: *mut c_char,
    }

    pub type Progress = Option<unsafe extern "C" fn(sent: u64, total: u64, data: *const c_void) -> c_int>;

    extern "C" {
        pub fn LIBMTP_Init();
        pub fn LIBMTP_Detect_Raw_Devices(list: *mut *mut LIBMTP_raw_device_t, n: *mut c_int) -> c_int;
        pub fn LIBMTP_Open_Raw_Device(raw: *mut LIBMTP_raw_device_t) -> *mut LIBMTP_mtpdevice_t;
        pub fn LIBMTP_Release_Device(d: *mut LIBMTP_mtpdevice_t);
        pub fn LIBMTP_Get_Manufacturername(d: *mut LIBMTP_mtpdevice_t) -> *mut c_char;
        pub fn LIBMTP_Get_Modelname(d: *mut LIBMTP_mtpdevice_t) -> *mut c_char;
        pub fn LIBMTP_Get_Serialnumber(d: *mut LIBMTP_mtpdevice_t) -> *mut c_char;
        pub fn LIBMTP_Get_Friendlyname(d: *mut LIBMTP_mtpdevice_t) -> *mut c_char;
        pub fn LIBMTP_Get_Batterylevel(d: *mut LIBMTP_mtpdevice_t, max: *mut u8, cur: *mut u8) -> c_int;
        pub fn LIBMTP_Get_Storage(d: *mut LIBMTP_mtpdevice_t, sortby: c_int) -> c_int;
        pub fn LIBMTP_Clear_Errorstack(d: *mut LIBMTP_mtpdevice_t);
        pub fn LIBMTP_Get_Tracklisting_With_Callback(d: *mut LIBMTP_mtpdevice_t, cb: Progress, data: *const c_void) -> *mut LIBMTP_track_t;
        pub fn LIBMTP_destroy_track_t(t: *mut LIBMTP_track_t);
        pub fn LIBMTP_Get_Playlist_List(d: *mut LIBMTP_mtpdevice_t) -> *mut LIBMTP_playlist_t;
        pub fn LIBMTP_destroy_playlist_t(p: *mut LIBMTP_playlist_t);
        pub fn LIBMTP_Get_Track_To_File(
            d: *mut LIBMTP_mtpdevice_t,
            id: u32,
            path: *const c_char,
            cb: Progress,
            data: *const c_void,
        ) -> c_int;
        pub fn LIBMTP_new_track_t() -> *mut LIBMTP_track_t;
        pub fn LIBMTP_Send_Track_From_File(
            d: *mut LIBMTP_mtpdevice_t,
            path: *const c_char,
            t: *mut LIBMTP_track_t,
            cb: Progress,
            data: *const c_void,
        ) -> c_int;
        pub fn LIBMTP_Update_Track_Metadata(d: *mut LIBMTP_mtpdevice_t, t: *const LIBMTP_track_t) -> c_int;
        pub fn LIBMTP_Delete_Object(d: *mut LIBMTP_mtpdevice_t, id: u32) -> c_int;
        pub fn LIBMTP_Get_Supported_Filetypes(d: *mut LIBMTP_mtpdevice_t, types: *mut *mut u16, len: *mut u16) -> c_int;
        pub fn LIBMTP_Get_Errorstack(d: *mut LIBMTP_mtpdevice_t) -> *mut LIBMTP_error_t;
        pub fn LIBMTP_new_album_t() -> *mut LIBMTP_album_t;
        pub fn LIBMTP_destroy_album_t(a: *mut LIBMTP_album_t);
        pub fn LIBMTP_Get_Album_List(d: *mut LIBMTP_mtpdevice_t) -> *mut LIBMTP_album_t;
        pub fn LIBMTP_Create_New_Album(d: *mut LIBMTP_mtpdevice_t, a: *mut LIBMTP_album_t) -> c_int;
        pub fn LIBMTP_Update_Album(d: *mut LIBMTP_mtpdevice_t, a: *const LIBMTP_album_t) -> c_int;
        pub fn LIBMTP_new_filesampledata_t() -> *mut LIBMTP_filesampledata_t;
        pub fn LIBMTP_destroy_filesampledata_t(s: *mut LIBMTP_filesampledata_t);
        pub fn LIBMTP_Get_Representative_Sample_Format(
            d: *mut LIBMTP_mtpdevice_t,
            filetype: c_int,
            s: *mut *mut LIBMTP_filesampledata_t,
        ) -> c_int;
        pub fn LIBMTP_Send_Representative_Sample(d: *mut LIBMTP_mtpdevice_t, id: u32, s: *mut LIBMTP_filesampledata_t) -> c_int;
        pub fn LIBMTP_Get_Representative_Sample(d: *mut LIBMTP_mtpdevice_t, id: u32, s: *mut LIBMTP_filesampledata_t) -> c_int;
        pub fn strdup(s: *const c_char) -> *mut c_char;
        pub fn chinotto_usb_fingerprint() -> u64;
        pub fn chinotto_has_mtp_interface(bus: u32, address: u8) -> c_int;
        pub fn chinotto_mtp_layout(which: c_int) -> usize;
    }
    extern "C" {
        pub fn free(p: *mut c_void);
        pub fn malloc(n: usize) -> *mut c_void;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

fn init() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe { ffi::LIBMTP_Init() });
}

unsafe fn text(p: *const c_char) -> Option<String> {
    if p.is_null() {
        None
    } else {
        Some(CStr::from_ptr(p).to_string_lossy().into_owned()).filter(|s| !s.trim().is_empty())
    }
}

/// A string libmtp allocated for us: copied, then freed.
unsafe fn owned_text(p: *mut c_char) -> Option<String> {
    let s = text(p);
    if !p.is_null() {
        ffi::free(p as *mut c_void);
    }
    s
}

/// A number that changes whenever USB devices are plugged in or out (it
/// lists them without opening any).
pub fn usb_fingerprint() -> u64 {
    unsafe { ffi::chinotto_usb_fingerprint() }
}

/// An MTP device seen on USB (not yet opened).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawDevice {
    pub bus: u32,
    pub devnum: u8,
    pub vendor_id: u16,
    pub product_id: u16,
    /// From libmtp's list of known devices, e.g. "SanDisk" / "Sansa e200".
    pub vendor: Option<String>,
    pub product: Option<String>,
}

impl RawDevice {
    /// Stable within a plug-in: "mtp:<bus>-<devnum>".
    pub fn key(&self) -> String {
        format!("mtp:{}-{}", self.bus, self.devnum)
    }
}

/// MTP devices connected now (on macOS, not players that are only a drive). Probing may open USB devices briefly.
pub fn detect() -> Vec<RawDevice> {
    init();
    let mut list: *mut ffi::LIBMTP_raw_device_t = ptr::null_mut();
    let mut n: c_int = 0;
    let rc = unsafe { ffi::LIBMTP_Detect_Raw_Devices(&mut list, &mut n) };
    let mut out = Vec::new();
    if rc == 0 && !list.is_null() {
        for i in 0..n.max(0) as usize {
            let r = unsafe { &*list.add(i) };
            // A player whose only interface is mass storage: some (Sony
            // Walkmans) answer MTP on it too, once the disk driver lets go.
            // Linux lets libmtp unload that driver; macOS doesn't, so there
            // such a player is only a drive.
            if cfg!(target_os = "macos") && unsafe { ffi::chinotto_has_mtp_interface(r.bus_location, r.devnum) } == 0 {
                continue;
            }
            out.push(RawDevice {
                bus: r.bus_location,
                devnum: r.devnum,
                vendor_id: r.device_entry.vendor_id,
                product_id: r.device_entry.product_id,
                vendor: unsafe { text(r.device_entry.vendor) },
                product: unsafe { text(r.device_entry.product) },
            });
        }
    }
    if !list.is_null() {
        unsafe { ffi::free(list as *mut c_void) };
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
pub struct Storage {
    pub id: u32,
    pub description: Option<String>,
    pub capacity: u64,
    pub free: u64,
}

/// A track (song) on the device, as its database describes it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Track {
    pub id: u32,
    pub parent: u32,
    pub storage: u32,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub composer: Option<String>,
    pub genre: Option<String>,
    pub album: Option<String>,
    /// "YYYYMMDDThhmmss.s" or a year, as the device has it.
    pub date: Option<String>,
    pub filename: Option<String>,
    pub track_nr: u16,
    pub duration_ms: u32,
    pub sample_rate: u32,
    pub channels: u16,
    pub wave_codec: u32,
    pub bitrate: u32,
    pub bitrate_type: u16,
    /// 0–100.
    pub rating: u16,
    pub play_count: u32,
    pub size: u64,
    pub modified: i64,
    /// libmtp's file type number (see [`filetype_name`]).
    pub filetype: i32,
}

impl Track {
    /// The year, from the device's date ("2003", "20030101T000000.0"…).
    pub fn year(&self) -> i32 {
        self.date.as_deref().and_then(|d| d.get(..4)).and_then(|y| y.parse().ok()).unwrap_or(0)
    }
}

/// A short name for libmtp's file type numbers.
pub fn filetype_name(t: i32) -> &'static str {
    match t {
        1 => "WAV",
        2 => "MP3",
        3 => "WMA",
        4 => "Ogg Vorbis",
        5 => "Audible",
        6 => "MP4",
        30 => "AAC",
        32 => "FLAC",
        33 => "MP2",
        34 => "M4A",
        _ => "Audio",
    }
}

/// libmtp's file type for a file extension, among the types the device
/// takes (an .m4a goes as M4A, MP4 or AAC, whichever it lists).
pub fn filetype_for(ext: &str, supported: &[i32]) -> i32 {
    match ext.to_ascii_lowercase().as_str() {
        "wav" => 1,
        "mp3" => 2,
        "wma" => 3,
        "ogg" | "oga" => 4,
        "flac" => 32,
        "aac" => 30,
        "m4a" | "m4b" | "mp4" => [34, 6, 30].into_iter().find(|t| supported.contains(t)).unwrap_or(34),
        _ => 7, // undefined audio
    }
}

/// What to send with a song: its tags as the device's database keeps them.
#[derive(Debug, Clone, Default)]
pub struct NewTrack {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub composer: Option<String>,
    pub year: u32,
    pub track_nr: u16,
    pub duration_ms: u32,
    /// The name it gets on the device.
    pub filename: String,
    pub filetype: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Playlist {
    pub id: u32,
    pub name: String,
    pub tracks: Vec<u32>,
}

/// An album object in the device's database: what SanDisk, Creative and
/// other Windows-era players show album art from (the "representative
/// sample" image Windows Media Player sends with it), not covers embedded in
/// the song files.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Album {
    pub id: u32,
    pub storage: u32,
    pub name: String,
    pub artist: Option<String>,
    pub genre: Option<String>,
    pub tracks: Vec<u32>,
}

/// libmtp's JPEG and album file types.
const FILETYPE_JPEG: c_int = 14;
const FILETYPE_ALBUM: c_int = 42;

/// A string libmtp frees itself later (its own copy).
fn c_dup(s: Option<&str>) -> *mut c_char {
    match s.and_then(|s| CString::new(s.replace('\0', "")).ok()) {
        Some(c) => unsafe { ffi::strdup(c.as_ptr()) },
        None => ptr::null_mut(),
    }
}

/// A track list libmtp frees itself later.
fn c_tracks(ids: &[u32]) -> *mut u32 {
    if ids.is_empty() {
        return ptr::null_mut();
    }
    unsafe {
        let p = ffi::malloc(std::mem::size_of_val(ids)) as *mut u32;
        if !p.is_null() {
            ptr::copy_nonoverlapping(ids.as_ptr(), p, ids.len());
        }
        p
    }
}

/// An opened MTP device. Other programs can't use it while it's open.
pub struct Device {
    d: *mut ffi::LIBMTP_mtpdevice_t,
}

// Used by one thread at a time (behind the app's mutex).
unsafe impl Send for Device {}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe { ffi::LIBMTP_Release_Device(self.d) }
    }
}

impl Device {
    /// Open the device found by [`detect`] at `raw` (it must still be plugged in).
    pub fn open(raw: &RawDevice) -> Result<Self> {
        init();
        let mut list: *mut ffi::LIBMTP_raw_device_t = ptr::null_mut();
        let mut n: c_int = 0;
        unsafe { ffi::LIBMTP_Detect_Raw_Devices(&mut list, &mut n) };
        let mut dev = ptr::null_mut();
        if !list.is_null() {
            for i in 0..n.max(0) as usize {
                let r = unsafe { list.add(i) };
                if unsafe { (*r).bus_location == raw.bus && (*r).devnum == raw.devnum } {
                    // Cached: libmtp's track and playlist lists need the object
                    // list it caches (uncached, they come back empty).
                    dev = unsafe { ffi::LIBMTP_Open_Raw_Device(r) };
                    break;
                }
            }
            unsafe { ffi::free(list as *mut c_void) };
        }
        if dev.is_null() {
            return Err(Error(
                "Couldn't open the device. Another program may be using it (on macOS, Image Capture or Android File Transfer; on Linux, the file manager), or the device didn't answer (some older players' MTP doesn't work with every computer: use its USB drive mode instead)."
                    .into(),
            ));
        }
        Ok(Device { d: dev })
    }

    pub fn friendly_name(&self) -> Option<String> {
        unsafe { owned_text(ffi::LIBMTP_Get_Friendlyname(self.d)) }
    }
    pub fn model(&self) -> Option<String> {
        unsafe { owned_text(ffi::LIBMTP_Get_Modelname(self.d)) }
    }
    pub fn manufacturer(&self) -> Option<String> {
        unsafe { owned_text(ffi::LIBMTP_Get_Manufacturername(self.d)) }
    }
    pub fn serial(&self) -> Option<String> {
        unsafe { owned_text(ffi::LIBMTP_Get_Serialnumber(self.d)) }
    }

    /// Battery level in percent, when the device says.
    pub fn battery(&self) -> Option<u8> {
        let (mut max, mut cur) = (0u8, 0u8);
        let rc = unsafe { ffi::LIBMTP_Get_Batterylevel(self.d, &mut max, &mut cur) };
        unsafe { ffi::LIBMTP_Clear_Errorstack(self.d) };
        (rc == 0 && max > 0).then(|| ((cur as u32 * 100) / max as u32) as u8)
    }

    pub fn storages(&self) -> Vec<Storage> {
        let mut out = Vec::new();
        unsafe {
            if ffi::LIBMTP_Get_Storage(self.d, 0) != 0 {
                ffi::LIBMTP_Clear_Errorstack(self.d);
                return out;
            }
            let mut s = (*self.d).storage;
            while !s.is_null() {
                out.push(Storage {
                    id: (*s).id,
                    description: text((*s).storage_description),
                    capacity: (*s).max_capacity,
                    free: (*s).free_space_in_bytes,
                });
                s = (*s).next;
            }
        }
        out
    }

    /// Every track on the device. `progress(done, total)` while reading.
    pub fn tracks(&mut self, mut progress: impl FnMut(u64, u64)) -> Vec<Track> {
        unsafe extern "C" fn cb(sent: u64, total: u64, data: *const c_void) -> c_int {
            let f = &mut *(data as *mut &mut dyn FnMut(u64, u64));
            f(sent, total);
            0
        }
        let mut f: &mut dyn FnMut(u64, u64) = &mut progress;
        let data = &mut f as *mut &mut dyn FnMut(u64, u64) as *const c_void;
        let mut out = Vec::new();
        unsafe {
            let head = ffi::LIBMTP_Get_Tracklisting_With_Callback(self.d, Some(cb), data);
            let mut t = head;
            while !t.is_null() {
                let r = &*t;
                out.push(Track {
                    id: r.item_id,
                    parent: r.parent_id,
                    storage: r.storage_id,
                    title: text(r.title),
                    artist: text(r.artist),
                    composer: text(r.composer),
                    genre: text(r.genre),
                    album: text(r.album),
                    date: text(r.date),
                    filename: text(r.filename),
                    track_nr: r.tracknumber,
                    duration_ms: r.duration,
                    sample_rate: r.samplerate,
                    channels: r.nochannels,
                    wave_codec: r.wavecodec,
                    bitrate: r.bitrate,
                    bitrate_type: r.bitratetype,
                    rating: r.rating,
                    play_count: r.usecount,
                    size: r.filesize,
                    modified: r.modificationdate,
                    filetype: r.filetype,
                });
                let next = r.next;
                ffi::LIBMTP_destroy_track_t(t);
                t = next;
            }
            ffi::LIBMTP_Clear_Errorstack(self.d);
        }
        out
    }

    pub fn playlists(&mut self) -> Vec<Playlist> {
        let mut out = Vec::new();
        unsafe {
            let mut p = ffi::LIBMTP_Get_Playlist_List(self.d);
            while !p.is_null() {
                let r = &*p;
                let tracks =
                    if r.tracks.is_null() { Vec::new() } else { std::slice::from_raw_parts(r.tracks, r.no_tracks as usize).to_vec() };
                out.push(Playlist { id: r.playlist_id, name: text(r.name).unwrap_or_default(), tracks });
                let next = r.next;
                ffi::LIBMTP_destroy_playlist_t(p);
                p = next;
            }
            ffi::LIBMTP_Clear_Errorstack(self.d);
        }
        out
    }

    /// The device's album objects.
    pub fn albums(&mut self) -> Vec<Album> {
        let mut out = Vec::new();
        unsafe {
            let mut a = ffi::LIBMTP_Get_Album_List(self.d);
            while !a.is_null() {
                let r = &*a;
                let tracks =
                    if r.tracks.is_null() { Vec::new() } else { std::slice::from_raw_parts(r.tracks, r.no_tracks as usize).to_vec() };
                out.push(Album {
                    id: r.album_id,
                    storage: r.storage_id,
                    name: text(r.name).unwrap_or_default(),
                    artist: text(r.artist),
                    genre: text(r.genre),
                    tracks,
                });
                let next = r.next;
                (*a).next = ptr::null_mut();
                ffi::LIBMTP_destroy_album_t(a);
                a = next;
            }
            ffi::LIBMTP_Clear_Errorstack(self.d);
        }
        out
    }

    /// Make a new album object (in the device's album folder); its id.
    pub fn create_album(&mut self, album: &Album) -> Result<u32> {
        unsafe {
            let a = ffi::LIBMTP_new_album_t();
            if a.is_null() {
                return Err(Error("out of memory".into()));
            }
            let r = &mut *a;
            r.parent_id = 0;
            r.storage_id = album.storage;
            r.name = c_dup(Some(&album.name));
            r.artist = c_dup(album.artist.as_deref());
            r.genre = c_dup(album.genre.as_deref());
            r.tracks = c_tracks(&album.tracks);
            r.no_tracks = album.tracks.len() as u32;
            let rc = ffi::LIBMTP_Create_New_Album(self.d, a);
            let id = r.album_id;
            ffi::LIBMTP_destroy_album_t(a);
            if rc != 0 {
                return Err(self.take_error("The device didn't make the album"));
            }
            ffi::LIBMTP_Clear_Errorstack(self.d);
            Ok(id)
        }
    }

    /// Write an album object's songs (and name) back to the device.
    pub fn update_album(&mut self, album: &Album) -> Result<()> {
        unsafe {
            let a = ffi::LIBMTP_new_album_t();
            if a.is_null() {
                return Err(Error("out of memory".into()));
            }
            let r = &mut *a;
            r.album_id = album.id;
            r.storage_id = album.storage;
            r.name = c_dup(Some(&album.name));
            r.artist = c_dup(album.artist.as_deref());
            r.genre = c_dup(album.genre.as_deref());
            r.tracks = c_tracks(&album.tracks);
            r.no_tracks = album.tracks.len() as u32;
            let rc = ffi::LIBMTP_Update_Album(self.d, a);
            ffi::LIBMTP_destroy_album_t(a);
            if rc != 0 {
                return Err(self.take_error("The device didn't change the album"));
            }
            ffi::LIBMTP_Clear_Errorstack(self.d);
        }
        Ok(())
    }

    /// The largest album picture the device asks for (width, height), if it says.
    pub fn album_art_size(&mut self) -> Option<(u32, u32)> {
        unsafe {
            let mut s: *mut ffi::LIBMTP_filesampledata_t = ptr::null_mut();
            let rc = ffi::LIBMTP_Get_Representative_Sample_Format(self.d, FILETYPE_ALBUM, &mut s);
            ffi::LIBMTP_Clear_Errorstack(self.d);
            if s.is_null() {
                return None;
            }
            let size = ((*s).width, (*s).height);
            ffi::LIBMTP_destroy_filesampledata_t(s);
            (rc == 0 && size.0 > 0 && size.1 > 0).then_some(size)
        }
    }

    /// Whether an album object already has a picture.
    pub fn album_has_art(&mut self, id: u32) -> bool {
        unsafe {
            let s = ffi::LIBMTP_new_filesampledata_t();
            if s.is_null() {
                return false;
            }
            let rc = ffi::LIBMTP_Get_Representative_Sample(self.d, id, s);
            let has = rc == 0 && (*s).size > 0 && !(*s).data.is_null();
            ffi::LIBMTP_destroy_filesampledata_t(s);
            ffi::LIBMTP_Clear_Errorstack(self.d);
            has
        }
    }

    /// Give an album object its picture: a baseline JPEG, `width`×`height`.
    pub fn set_album_art(&mut self, id: u32, jpeg: &[u8], width: u32, height: u32) -> Result<()> {
        unsafe {
            let s = ffi::LIBMTP_new_filesampledata_t();
            if s.is_null() {
                return Err(Error("out of memory".into()));
            }
            let data = ffi::malloc(jpeg.len().max(1)) as *mut c_char;
            if data.is_null() {
                ffi::LIBMTP_destroy_filesampledata_t(s);
                return Err(Error("out of memory".into()));
            }
            ptr::copy_nonoverlapping(jpeg.as_ptr() as *const c_char, data, jpeg.len());
            (*s).data = data;
            (*s).size = jpeg.len() as u64;
            (*s).width = width;
            (*s).height = height;
            (*s).filetype = FILETYPE_JPEG;
            let rc = ffi::LIBMTP_Send_Representative_Sample(self.d, id, s);
            ffi::LIBMTP_destroy_filesampledata_t(s);
            if rc != 0 {
                return Err(self.take_error("The device didn't take the album picture"));
            }
            ffi::LIBMTP_Clear_Errorstack(self.d);
        }
        Ok(())
    }

    /// The file types the device says it takes (libmtp's numbers).
    pub fn supported_filetypes(&mut self) -> Vec<i32> {
        let mut types: *mut u16 = ptr::null_mut();
        let mut len: u16 = 0;
        let mut out = Vec::new();
        unsafe {
            if ffi::LIBMTP_Get_Supported_Filetypes(self.d, &mut types, &mut len) == 0 && !types.is_null() {
                out = std::slice::from_raw_parts(types, len as usize).iter().map(|&t| t as i32).collect();
            }
            if !types.is_null() {
                ffi::free(types as *mut c_void);
            }
            ffi::LIBMTP_Clear_Errorstack(self.d);
        }
        out
    }

    /// The device's last error message (and the error list is cleared).
    fn take_error(&mut self, fallback: &str) -> Error {
        let msg = unsafe {
            let e = ffi::LIBMTP_Get_Errorstack(self.d);
            let m = if e.is_null() { None } else { text((*e).error_text) };
            ffi::LIBMTP_Clear_Errorstack(self.d);
            m
        };
        Error(match msg {
            Some(m) => format!("{fallback} ({m})"),
            None => fallback.into(),
        })
    }

    /// Send a song file to the device (into its music folder); its new id.
    /// `progress(sent, total)` while sending.
    pub fn send_track(&mut self, file: &Path, info: &NewTrack, mut progress: impl FnMut(u64, u64)) -> Result<u32> {
        unsafe extern "C" fn cb(sent: u64, total: u64, data: *const c_void) -> c_int {
            let f = &mut *(data as *mut &mut dyn FnMut(u64, u64));
            f(sent, total);
            0
        }
        let size = std::fs::metadata(file).map_err(|e| Error(e.to_string()))?.len();
        let c_path = CString::new(file.to_string_lossy().as_bytes()).map_err(|_| Error("bad path".into()))?;
        // Strings libmtp frees with the track (so they're its own copies).
        let dup = |s: &Option<String>| -> *mut c_char {
            match s.as_deref().and_then(|s| CString::new(s.replace('\0', "")).ok()) {
                Some(c) => unsafe { ffi::strdup(c.as_ptr()) },
                None => ptr::null_mut(),
            }
        };
        let mut f: &mut dyn FnMut(u64, u64) = &mut progress;
        let data = &mut f as *mut &mut dyn FnMut(u64, u64) as *const c_void;
        unsafe {
            let t = ffi::LIBMTP_new_track_t();
            if t.is_null() {
                return Err(Error("out of memory".into()));
            }
            let r = &mut *t;
            r.title = dup(&info.title);
            r.artist = dup(&info.artist);
            r.album = dup(&info.album);
            r.genre = dup(&info.genre);
            r.composer = dup(&info.composer);
            r.date = if info.year > 0 { dup(&Some(format!("{:04}0101T0000.0", info.year))) } else { ptr::null_mut() };
            r.filename = dup(&Some(info.filename.clone()));
            r.tracknumber = info.track_nr;
            r.duration = info.duration_ms;
            r.filesize = size;
            r.filetype = info.filetype;
            // 0: libmtp picks the default storage and music folder.
            r.parent_id = 0;
            r.storage_id = 0;
            let rc = ffi::LIBMTP_Send_Track_From_File(self.d, c_path.as_ptr(), t, Some(cb), data);
            let id = r.item_id;
            ffi::LIBMTP_destroy_track_t(t);
            if rc != 0 {
                return Err(self.take_error("The device didn't take the song"));
            }
            ffi::LIBMTP_Clear_Errorstack(self.d);
            Ok(id)
        }
    }

    /// Write a track's tags to the device's database. Every field is sent
    /// (libmtp sets each property the device has), so `t` should be the
    /// track as read, with the changes made to it. A rating of 0 is left as
    /// it is on the device.
    pub fn update_track(&mut self, t: &Track) -> Result<()> {
        let dup = |s: &Option<String>| -> *mut c_char {
            match CString::new(s.as_deref().unwrap_or("").replace('\0', "")) {
                Ok(c) => unsafe { ffi::strdup(c.as_ptr()) },
                Err(_) => ptr::null_mut(),
            }
        };
        unsafe {
            let p = ffi::LIBMTP_new_track_t();
            if p.is_null() {
                return Err(Error("out of memory".into()));
            }
            let r = &mut *p;
            r.item_id = t.id;
            r.parent_id = t.parent;
            r.storage_id = t.storage;
            r.title = dup(&t.title);
            r.artist = dup(&t.artist);
            r.album = dup(&t.album);
            r.genre = dup(&t.genre);
            r.composer = dup(&t.composer);
            r.date = match &t.date {
                Some(d) if !d.is_empty() => dup(&Some(d.clone())),
                _ => ptr::null_mut(),
            };
            r.filename = if t.filename.is_some() { dup(&t.filename) } else { ptr::null_mut() };
            r.tracknumber = t.track_nr;
            r.duration = t.duration_ms;
            r.samplerate = t.sample_rate;
            r.nochannels = t.channels;
            r.wavecodec = t.wave_codec;
            r.bitrate = t.bitrate;
            r.bitratetype = t.bitrate_type;
            r.rating = t.rating;
            r.usecount = t.play_count;
            r.filesize = t.size;
            r.filetype = t.filetype;
            let rc = ffi::LIBMTP_Update_Track_Metadata(self.d, p);
            ffi::LIBMTP_destroy_track_t(p);
            if rc != 0 {
                return Err(self.take_error("The device didn't take the changes"));
            }
            ffi::LIBMTP_Clear_Errorstack(self.d);
        }
        Ok(())
    }

    /// Delete a track (or a playlist, or any object) from the device.
    pub fn delete(&mut self, id: u32) -> Result<()> {
        if unsafe { ffi::LIBMTP_Delete_Object(self.d, id) } == 0 {
            unsafe { ffi::LIBMTP_Clear_Errorstack(self.d) };
            Ok(())
        } else {
            Err(self.take_error("The device didn't delete it"))
        }
    }

    /// Copy a track from the device to a file on the computer.
    pub fn download(&mut self, id: u32, to: &Path) -> Result<()> {
        let c = CString::new(to.to_string_lossy().as_bytes()).map_err(|_| Error("bad path".into()))?;
        let rc = unsafe { ffi::LIBMTP_Get_Track_To_File(self.d, id, c.as_ptr(), None, ptr::null()) };
        unsafe { ffi::LIBMTP_Clear_Errorstack(self.d) };
        if rc == 0 {
            Ok(())
        } else {
            Err(Error("Couldn't copy the song from the device".into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Rust declarations of libmtp's structs match the C ones.
    #[test]
    fn struct_layouts_match() {
        use std::mem::{offset_of, size_of};
        let c = |i| unsafe { ffi::chinotto_mtp_layout(i) };
        assert_eq!(c(0), size_of::<ffi::LIBMTP_raw_device_t>());
        assert_eq!(c(1), size_of::<ffi::LIBMTP_track_t>());
        assert_eq!(c(2), offset_of!(ffi::LIBMTP_track_t, next));
        assert_eq!(c(3), size_of::<ffi::LIBMTP_playlist_t>());
        assert_eq!(c(4), size_of::<ffi::LIBMTP_devicestorage_t>());
        assert_eq!(c(5), size_of::<ffi::LIBMTP_mtpdevice_t>());
        assert_eq!(c(6), offset_of!(ffi::LIBMTP_mtpdevice_t, storage));
        assert_eq!(c(7), offset_of!(ffi::LIBMTP_track_t, filetype));
        assert_eq!(c(8), size_of::<ffi::LIBMTP_album_t>());
        assert_eq!(c(9), offset_of!(ffi::LIBMTP_album_t, next));
        assert_eq!(c(10), size_of::<ffi::LIBMTP_filesampledata_t>());
        assert_eq!(c(11), offset_of!(ffi::LIBMTP_filesampledata_t, data));
    }

    /// Runs on any machine: detection works with nothing plugged in.
    #[test]
    fn detects_without_devices() {
        let _ = usb_fingerprint();
        let found = detect();
        assert!(found.iter().all(|d| d.key().starts_with("mtp:")));
    }

    #[test]
    fn file_types() {
        assert_eq!(filetype_for("MP3", &[]), 2);
        assert_eq!(filetype_for("m4a", &[2, 6]), 6);
        assert_eq!(filetype_for("m4a", &[2, 34, 6]), 34);
        assert_eq!(filetype_for("m4a", &[2]), 34);
    }
}
