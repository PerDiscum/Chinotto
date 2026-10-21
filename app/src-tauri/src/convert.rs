//! Converting songs for a device that can't play them (FLAC, Ogg) or, if the
//! user wants, to save space (Apple Lossless). The converted file is written
//! to a temporary folder, copied to the device and deleted: it never joins a
//! library, and the original isn't touched.
//!
//! Decoding: Symphonia (chinotto-player). Encoding:
//! - MP3: LAME, built into Chinotto.
//! - AAC: macOS's own encoder (`afconvert`, part of macOS); on Linux, `ffmpeg`
//!   if installed.
//!
//! Tags and the cover are copied from the original into the converted file.

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use chinotto_player::decode::Source;
use serde::{Deserialize, Serialize};

/// What a device gets, set on its page.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Conversion {
    /// "aac256", "aac128", "mp3_320" or "mp3_192".
    pub format: String,
    /// Convert Apple Lossless too (it plays on iPods, but takes much space).
    pub convert_alac: bool,
}

impl Default for Conversion {
    fn default() -> Self {
        Conversion { format: "aac256".into(), convert_alac: true }
    }
}

impl Conversion {
    pub(crate) fn target(&self) -> (bool, u32) {
        match self.format.as_str() {
            "aac128" => (true, 128),
            "mp3_320" => (false, 320),
            "mp3_192" => (false, 192),
            _ => (true, 256),
        }
    }
}

/// Why a file needs converting for an iPod, if it does.
pub fn needs_converting(path: &Path, conv: &Conversion) -> Option<&'static str> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "flac" => Some("FLAC"),
        "ogg" | "oga" => Some("Ogg Vorbis"),
        "opus" => Some("Opus"),
        "m4a" if conv.convert_alac && is_alac(path) => Some("Apple Lossless"),
        _ => None,
    }
}

/// An .m4a holding Apple Lossless (it has a bit depth; AAC doesn't).
pub(crate) fn is_alac(path: &Path) -> bool {
    use lofty::prelude::*;
    lofty::read_from_path(path).is_ok_and(|f| f.properties().bit_depth().is_some())
}

/// A temporary folder deleted (with everything in it) when dropped.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new() -> Result<Self, String> {
        let p = std::env::temp_dir().join(format!(
            "chinotto-convert-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
        ));
        std::fs::create_dir_all(&p).map_err(|e| format!("Couldn't make a temporary folder: {e}"))?;
        Ok(TempDir(p))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Only ever our own temporary folder.
        if self.0.file_name().is_some_and(|n| n.to_string_lossy().starts_with("chinotto-convert-")) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

/// Output sample rate: the source's if a device takes it, else the nearest
/// standard one (44.1 kHz family or 48 kHz family).
fn out_rate(rate: u32) -> u32 {
    match rate {
        r if r <= 48_000 => r,
        r if r % 44_100 == 0 => 44_100,
        _ => 48_000,
    }
}

/// Convert `src` into `dir`; returns the new file (with the original's tags and cover).
pub fn convert(src: &Path, dir: &Path, conv: &Conversion, n: usize) -> Result<PathBuf, String> {
    let name = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let (aac, kbps) = conv.target();
    let out = dir.join(format!("{n:05}.{}", if aac { "m4a" } else { "mp3" }));
    let mut source = Source::open(src).map_err(|e| format!("{name}: {e}"))?;
    if aac {
        encode_aac(src, &mut source, &out, kbps, dir, n)?;
    } else {
        encode_mp3(&mut source, &out, kbps)?;
    }
    copy_tags(src, &out).map_err(|e| format!("{name}: {e}"))?;
    Ok(out)
}

/// Decoded stereo samples, chunk by chunk.
fn stereo_chunks(source: &mut Source, mut f: impl FnMut(&[f32]) -> Result<(), String>) -> Result<(), String> {
    while let Some(chunk) = source.next_chunk()? {
        let ch = source.channels.max(1);
        if ch == 2 {
            f(&chunk)?;
        } else {
            f(&chinotto_player::convert::remap(&chunk, ch, 2))?;
        }
    }
    Ok(())
}

fn encode_mp3(source: &mut Source, out: &Path, kbps: u32) -> Result<(), String> {
    use mp3lame_encoder::{Bitrate, Builder, FlushNoGap, InterleavedPcm, Quality};
    let e = |e: mp3lame_encoder::BuildError| format!("MP3 encoder: {e:?}");
    let mut b = Builder::new().ok_or("MP3 encoder couldn't start")?;
    b.set_num_channels(2).map_err(e)?;
    b.set_sample_rate(source.rate).map_err(e)?;
    b.set_output_sample_rate(std::num::NonZeroU32::new(out_rate(source.rate))).map_err(e)?;
    b.set_brate(if kbps >= 320 { Bitrate::Kbps320 } else { Bitrate::Kbps192 }).map_err(e)?;
    b.set_quality(Quality::Best).map_err(e)?;
    let mut enc = b.build().map_err(e)?;
    let mut file = BufWriter::new(File::create(out).map_err(|e| e.to_string())?);
    let mut buf = Vec::new();
    stereo_chunks(source, |pcm| {
        buf.clear();
        buf.reserve(mp3lame_encoder::max_required_buffer_size(pcm.len() / 2));
        enc.encode_to_vec(InterleavedPcm(pcm), &mut buf).map_err(|e| format!("MP3 encoder: {e:?}"))?;
        file.write_all(&buf).map_err(|e| e.to_string())
    })?;
    buf.clear();
    buf.reserve(7200);
    enc.flush_to_vec::<FlushNoGap>(&mut buf).map_err(|e| format!("MP3 encoder: {e:?}"))?;
    file.write_all(&buf).map_err(|e| e.to_string())?;
    file.flush().map_err(|e| e.to_string())
}

/// 24-bit PCM WAV of the decoded audio (afconvert's input).
fn write_wav(source: &mut Source, out: &Path) -> Result<u32, String> {
    let rate = source.rate;
    let mut f = BufWriter::new(File::create(out).map_err(|e| e.to_string())?);
    let header = |data: u32| {
        let mut h = Vec::with_capacity(44);
        h.extend_from_slice(b"RIFF");
        h.extend_from_slice(&(36 + data).to_le_bytes());
        h.extend_from_slice(b"WAVEfmt ");
        h.extend_from_slice(&16u32.to_le_bytes());
        h.extend_from_slice(&1u16.to_le_bytes()); // PCM
        h.extend_from_slice(&2u16.to_le_bytes());
        h.extend_from_slice(&rate.to_le_bytes());
        h.extend_from_slice(&(rate * 6).to_le_bytes());
        h.extend_from_slice(&6u16.to_le_bytes());
        h.extend_from_slice(&24u16.to_le_bytes());
        h.extend_from_slice(b"data");
        h.extend_from_slice(&data.to_le_bytes());
        h
    };
    f.write_all(&header(0)).map_err(|e| e.to_string())?;
    let mut bytes: u64 = 0;
    stereo_chunks(source, |pcm| {
        let mut b = Vec::with_capacity(pcm.len() * 3);
        for s in pcm {
            let v = (s.clamp(-1.0, 1.0) * 8_388_607.0).round() as i32;
            b.extend_from_slice(&v.to_le_bytes()[..3]);
        }
        bytes += b.len() as u64;
        f.write_all(&b).map_err(|e| e.to_string())
    })?;
    let data = u32::try_from(bytes).map_err(|_| "song too long to convert".to_string())?;
    let mut f = f.into_inner().map_err(|e| e.to_string())?;
    f.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    f.write_all(&header(data)).map_err(|e| e.to_string())?;
    Ok(rate)
}

fn run(cmd: &mut Command, what: &str) -> Result<(), String> {
    let out = cmd.output().map_err(|e| format!("{what}: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("{what} failed: {}", String::from_utf8_lossy(&out.stderr).trim()))
    }
}

fn encode_aac(src: &Path, source: &mut Source, out: &Path, kbps: u32, dir: &Path, n: usize) -> Result<(), String> {
    if cfg!(target_os = "macos") {
        // macOS's own AAC encoder (iTunes' encoder), from a temporary WAV.
        let wav = dir.join(format!("{n:05}.wav"));
        let rate = write_wav(source, &wav)?;
        let r = run(
            Command::new("/usr/bin/afconvert")
                .arg("-f")
                .arg("m4af")
                .arg("-d")
                .arg(format!("aac@{}", out_rate(rate)))
                .arg("-b")
                .arg((kbps * 1000).to_string())
                .arg("-s")
                .arg("2") // constrained VBR, as iTunes Plus
                .arg("-q")
                .arg("127")
                .arg(&wav)
                .arg(out),
            "AAC encoder (afconvert)",
        );
        let _ = std::fs::remove_file(&wav);
        r
    } else {
        // Linux: ffmpeg, if installed (reads the original directly).
        let rate = source.rate;
        run(
            Command::new("ffmpeg")
                .args(["-nostdin", "-loglevel", "error", "-y", "-i"])
                .arg(src)
                .args(["-vn", "-map_metadata", "-1", "-c:a", "aac", "-b:a"])
                .arg(format!("{kbps}k"))
                .arg("-ar")
                .arg(out_rate(rate).to_string())
                .arg("-ac")
                .arg("2")
                .arg(out),
            "AAC encoder (ffmpeg)",
        )
        .map_err(|e| {
            if e.contains("No such file") || e.contains("not found") {
                "Converting to AAC on Linux needs ffmpeg: install it (e.g. sudo apt install ffmpeg), or choose MP3 on the device's page"
                    .to_string()
            } else {
                e
            }
        })
    }
}

/// The original's tags and cover, written into the converted file.
fn copy_tags(src: &Path, out: &Path) -> Result<(), String> {
    let kind = crate::tags::library_kind(src).unwrap_or("Audio file");
    let (info, _) = crate::tags::read_tags(src, kind)?;
    let cover = crate::tags::embedded_cover(src)
        .or_else(|| crate::library::folder_cover(src.parent()?).and_then(|p| std::fs::read(p).ok()))
        .map(|b| {
            let mime =
                if b.starts_with(&[0x89, b'P', b'N', b'G']) { lofty::picture::MimeType::Png } else { lofty::picture::MimeType::Jpeg };
            crate::tagwrite::CoverChange::Set(b, mime)
        });
    crate::tagwrite::write(out, &info.fields(), cover.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decides_what_to_convert() {
        let c = Conversion::default();
        assert_eq!(needs_converting(Path::new("a.flac"), &c), Some("FLAC"));
        assert_eq!(needs_converting(Path::new("a.mp3"), &c), None);
        assert_eq!(out_rate(96_000), 48_000);
        assert_eq!(out_rate(88_200), 44_100);
        assert_eq!(out_rate(44_100), 44_100);
    }

    /// MP3 from a FLAC in CHINOTTO_TEST_AUDIO, with tags carried over.
    #[test]
    fn converts_flac_to_mp3_with_tags() {
        let Some(src) = std::env::var_os("CHINOTTO_TEST_AUDIO").map(PathBuf::from) else { return };
        let dir = TempDir::new().unwrap();
        let flac = dir.0.join("in.flac");
        std::fs::copy(src.join("sine-440-3s.flac"), &flac).unwrap();
        crate::tagwrite::write(&flac, &[chinotto_gpod::Field::Title(Some("Converted".into()))], None).unwrap();
        // AAC needs afconvert (macOS) or ffmpeg (Linux).
        let aac = cfg!(target_os = "macos") || Command::new("ffmpeg").arg("-version").output().is_ok();
        for (n, format) in ["mp3_320", "aac256"].into_iter().enumerate() {
            if format.starts_with("aac") && !aac {
                continue;
            }
            let conv = Conversion { format: format.into(), convert_alac: true };
            let out = convert(&flac, &dir.0, &conv, n).unwrap_or_else(|e| panic!("{format}: {e}"));
            let (info, _) = crate::tags::read_tags(&out, "audio").unwrap();
            assert_eq!(info.title.as_deref(), Some("Converted"), "{format}");
            assert!((2_800..3_300).contains(&info.length_ms), "{format}: {}", info.length_ms);
            assert!(crate::tags::ipod_kind(&out).is_some(), "{format}");
        }
        let kept = dir.0.clone();
        drop(dir);
        assert!(!kept.exists(), "temporary folder removed");
    }
}
