//! MP3 audio as an OpenMG Walkman takes it: the bare MPEG-1 Layer III
//! frames (no ID3 tags, no Xing/Info frame), 44.1 kHz, one bitrate
//! throughout.

/// Bitrates (kbps) of MPEG-1 Layer III, by header index.
const KBPS: [u32; 16] = [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0];

/// Bitrate indexes whose header value is confirmed on a device: 128 and
/// 320 kbps (see [`crate::oma::codec_params`]). Others are converted.
pub const CONFIRMED_BITRATES: [u8; 2] = [9, 14];

#[derive(Debug, Clone, PartialEq)]
pub struct Mp3 {
    /// The frames, back to back.
    pub audio: Vec<u8>,
    pub frames: u32,
    pub length_ms: u32,
    /// The header's bitrate index (9 = 128 kbps, 14 = 320 kbps…).
    pub bitrate_index: u8,
}

/// Why a file can't go to the Walkman as it is (it's converted instead).
#[derive(Debug, Clone, PartialEq)]
pub enum NotUsable {
    NoFrames,
    /// MPEG-2/2.5, another layer, or not 44.1 kHz.
    Format,
    /// The bitrate changes (VBR).
    Variable,
    /// A bitrate not confirmed on a device.
    Bitrate(u32),
}

impl std::fmt::Display for NotUsable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotUsable::NoFrames => f.write_str("no MP3 audio found"),
            NotUsable::Format => f.write_str("not MPEG-1 Layer III at 44.1 kHz"),
            NotUsable::Variable => f.write_str("variable bitrate"),
            NotUsable::Bitrate(k) => write!(f, "{k} kbps"),
        }
    }
}

struct Header {
    len: usize,
    bitrate_index: u8,
    mono: bool,
}

/// An MPEG-1 Layer III, 44.1 kHz frame header at the start of `b`.
fn header(b: &[u8]) -> Option<Header> {
    if b.len() < 4 || b[0] != 0xFF || b[1] & 0xFE != 0xFA {
        // 0xFA/0xFB: sync, MPEG-1, Layer III (with or without CRC).
        return None;
    }
    let bitrate_index = b[2] >> 4;
    let rate_index = (b[2] >> 2) & 3;
    let kbps = KBPS[bitrate_index as usize];
    if kbps == 0 || rate_index != 0 {
        return None;
    }
    let padding = ((b[2] >> 1) & 1) as usize;
    Some(Header { len: (144 * kbps * 1000 / 44_100) as usize + padding, bitrate_index, mono: b[3] >> 6 == 3 })
}

/// Any MPEG audio frame sync, for telling "other format" from "no audio".
fn any_sync(b: &[u8]) -> bool {
    b.len() >= 2 && b[0] == 0xFF && b[1] & 0xE0 == 0xE0
}

/// Size of an ID3v2 tag at the start, if there is one.
fn id3v2_len(b: &[u8]) -> usize {
    if b.len() >= 10 && &b[..3] == b"ID3" {
        let size = b[6..10].iter().fold(0usize, |a, &x| (a << 7) | (x & 0x7F) as usize);
        let footer = if b[5] & 0x10 != 0 { 10 } else { 0 };
        10 + size + footer
    } else {
        0
    }
}

/// The frames of `data` (an .mp3 file), checked and stripped of tags and of
/// a leading Xing/Info frame.
pub fn prepare(data: &[u8]) -> Result<Mp3, NotUsable> {
    let mut p = id3v2_len(data).min(data.len());
    // The first frame: one whose successor is a frame too (skips junk that happens to look like a header).
    let mut other_format = false;
    loop {
        if p + 4 > data.len() {
            return Err(if other_format { NotUsable::Format } else { NotUsable::NoFrames });
        }
        if let Some(h) = header(&data[p..]) {
            let next = p + h.len;
            if next == data.len() || data.get(next..).and_then(header).is_some() {
                break;
            }
        } else if any_sync(&data[p..]) {
            other_format = true;
        }
        p += 1;
    }
    let mut audio = Vec::with_capacity(data.len() - p);
    let mut frames = 0u32;
    let mut first = true;
    let mut rate: Option<u8> = None;
    while let Some(h) = data.get(p..).and_then(header) {
        let Some(frame) = data.get(p..p + h.len) else { break };
        // The encoder's Xing/Info frame (bitrate and seek table, no audio).
        let side = if h.mono { 17 } else { 32 };
        let info = first && matches!(frame.get(4 + side..8 + side), Some(b"Xing") | Some(b"Info"));
        first = false;
        p += h.len;
        if info {
            continue;
        }
        match rate {
            None => rate = Some(h.bitrate_index),
            Some(r) if r != h.bitrate_index => return Err(NotUsable::Variable),
            _ => {}
        }
        audio.extend_from_slice(frame);
        frames += 1;
    }
    let bitrate_index = rate.ok_or(NotUsable::NoFrames)?;
    if !CONFIRMED_BITRATES.contains(&bitrate_index) {
        return Err(NotUsable::Bitrate(KBPS[bitrate_index as usize]));
    }
    // What's left must be tags (ID3v1, APE, Lyrics3) or nothing; MPEG audio
    // of another kind after the frames means it isn't one plain stream.
    if data.get(p..).is_some_and(|rest| rest.len() >= 4 && any_sync(rest) && header(rest).is_none()) {
        return Err(NotUsable::Format);
    }
    let length_ms = (frames as u64 * 1152 * 1000 / 44_100) as u32;
    Ok(Mp3 { audio, frames, length_ms, bitrate_index })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A 44.1 kHz MPEG-1 Layer III frame of `kbps` (silent; contents don't matter here).
    pub fn frame(kbps_index: u8, pad: bool) -> Vec<u8> {
        let h = [0xFF, 0xFB, (kbps_index << 4) | if pad { 2 } else { 0 }, 0x00];
        let len = header(&h).unwrap().len;
        let mut f = h.to_vec();
        f.resize(len, 0);
        f
    }

    #[test]
    fn strips_tags_and_info_frame() {
        let mut data = b"ID3\x03\0\0\0\0\0\x05hello".to_vec();
        let mut info = frame(14, false);
        info[36..40].copy_from_slice(b"Info");
        data.extend(&info);
        for i in 0..10 {
            data.extend(frame(14, i % 3 == 0));
        }
        data.extend(b"TAG");
        data.resize(data.len() + 125, 0);
        let m = prepare(&data).unwrap();
        assert_eq!(m.frames, 10);
        assert_eq!(m.bitrate_index, 14);
        assert_eq!(m.length_ms, 261); // 10 × 1152 / 44.1
        assert_eq!(&m.audio[..4], &[0xFF, 0xFB, 0xE2, 0x00]);
    }

    #[test]
    fn refuses_what_needs_converting() {
        let vbr: Vec<u8> = [frame(9, false), frame(14, false), frame(9, false)].concat();
        assert_eq!(prepare(&vbr), Err(NotUsable::Variable));
        let k192: Vec<u8> = [frame(11, false), frame(11, false)].concat();
        assert_eq!(prepare(&k192), Err(NotUsable::Bitrate(192)));
        // 48 kHz.
        let mut f = frame(14, false);
        f[2] |= 0x04;
        assert_eq!(prepare(&[f.clone(), f].concat()), Err(NotUsable::Format));
        assert_eq!(prepare(b"not audio at all"), Err(NotUsable::NoFrames));
    }
}
