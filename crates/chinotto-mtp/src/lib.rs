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

    pub type Progress = Option<unsafe extern "C" fn(sent: u64, total: u64, data: *const c_void) -> c_int>;

    extern "C" {
        pub fn LIBMTP_Init();
        pub fn LIBMTP_Detect_Raw_Devices(list: *mut *mut LIBMTP_raw_device_t, n: *mut c_int) -> c_int;
        pub fn LIBMTP_Open_Raw_Device_Uncached(raw: *mut LIBMTP_raw_device_t) -> *mut LIBMTP_mtpdevice_t;
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
        pub fn chinotto_usb_fingerprint() -> u64;
        pub fn chinotto_has_mtp_interface(bus: u32, address: u8) -> c_int;
        pub fn chinotto_mtp_layout(which: c_int) -> usize;
    }
    extern "C" {
        pub fn free(p: *mut c_void);
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

/// MTP devices connected now (only those offering an MTP interface: a
/// player set to mass storage is left out). Probing may open USB devices briefly.
pub fn detect() -> Vec<RawDevice> {
    init();
    let mut list: *mut ffi::LIBMTP_raw_device_t = ptr::null_mut();
    let mut n: c_int = 0;
    let rc = unsafe { ffi::LIBMTP_Detect_Raw_Devices(&mut list, &mut n) };
    let mut out = Vec::new();
    if rc == 0 && !list.is_null() {
        for i in 0..n.max(0) as usize {
            let r = unsafe { &*list.add(i) };
            // libmtp knows the USB ID, but a player set to mass storage has no MTP interface.
            if unsafe { ffi::chinotto_has_mtp_interface(r.bus_location, r.devnum) } == 0 {
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
    pub bitrate: u32,
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

#[derive(Debug, Clone, PartialEq)]
pub struct Playlist {
    pub id: u32,
    pub name: String,
    pub tracks: Vec<u32>,
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
                    dev = unsafe { ffi::LIBMTP_Open_Raw_Device_Uncached(r) };
                    break;
                }
            }
            unsafe { ffi::free(list as *mut c_void) };
        }
        if dev.is_null() {
            return Err(Error(
                "Couldn't open the device. Another program may be using it: on macOS, Image Capture or Android File Transfer; on Linux, the file manager (unmount it there first)."
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
                    bitrate: r.bitrate,
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
    }

    /// Runs on any machine: detection works with nothing plugged in.
    #[test]
    fn detects_without_devices() {
        let _ = usb_fingerprint();
        let found = detect();
        assert!(found.iter().all(|d| d.key().starts_with("mtp:")));
    }
}
