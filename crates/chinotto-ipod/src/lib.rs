//! chinotto-ipod: read and write the databases on a classic iPod.
//!
//! - [`itunesdb`]: tracks and playlists (`iPod_Control/iTunes/iTunesDB`)
//! - [`artworkdb`]: cover art index (`iPod_Control/Artwork/ArtworkDB`)
//! - [`rgb565`]: the raw thumbnail pixel format stored in `.ithmb` files
//! - [`device`]: recognising an iPod folder and reading `SysInfo`
//!
//! Record layouts were checked against a real iPod Classic (7th gen) database
//! written by Swinsian and by libgpod.

pub mod artworkdb;
pub mod bytes;
pub mod device;
pub mod itunesdb;
pub mod models;
pub mod rgb565;

use std::fmt;

#[derive(Debug)]
pub enum Error {
    Truncated { at: usize, len: usize },
    UnexpectedTag { at: usize, expected: [u8; 4], found: [u8; 4] },
    Invalid(String),
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Truncated { at, len } => write!(f, "file ends early (needed {len} bytes at offset {at})"),
            Error::UnexpectedTag { at, expected, found } => {
                write!(f, "expected '{}' at offset {at}, found '{}'", String::from_utf8_lossy(expected), String::from_utf8_lossy(found))
            }
            Error::Invalid(msg) => write!(f, "{msg}"),
            Error::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

/// Seconds between 1904-01-01 (Mac epoch, used by the iPod) and 1970-01-01.
pub const MAC_EPOCH_OFFSET: i64 = 2_082_844_800;

/// Convert an iPod timestamp to Unix seconds (None when unset).
pub fn mac_to_unix(t: u32) -> Option<i64> {
    if t == 0 {
        None
    } else {
        Some(t as i64 - MAC_EPOCH_OFFSET)
    }
}
