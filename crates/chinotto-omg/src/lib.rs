//! Sony OpenMG Walkmans (the NW-S700 series and relatives that keep their
//! music in `OMGAUDIO` rather than as plain files).
//!
//! Worked out from the user's NW-S716F (see docs/HANDOFF.md, 0089):
//!
//! - Songs are `OMGAUDIO/10Fxx/1xxxxxxx.OMA`: a 0xC00-byte `ea3` tag (an
//!   ID3v2.3 tag with another magic), a 0x60-byte `EA3` header (content ID,
//!   codec, length, frame count) and the MP3 frames, unencrypted.
//! - The database is a set of tables that all share one layout (see
//!   [`db`]): the songs (`04CNTINF`), their content IDs (`05CIDLST`), and
//!   browse trees (`01TREExx`) with their group names (`03GINFxx`), all of
//!   which can be rebuilt from the song list alone.

pub mod db;
pub mod mp3;
pub mod oma;

/// The longest text a table slot holds (UTF-16 units).
pub const MAX_TEXT: usize = 61;

/// `text`, cut to what the Walkman's tables hold.
pub fn fit(text: &str) -> String {
    let units: Vec<u16> = text.encode_utf16().take(MAX_TEXT).collect();
    let mut s = String::from_utf16_lossy(&units);
    // Don't end on half a surrogate pair.
    if s.ends_with('\u{fffd}') && !text.encode_utf16().nth(MAX_TEXT - 1).is_some_and(|u| u == 0xfffd) {
        s.pop();
    }
    s
}
