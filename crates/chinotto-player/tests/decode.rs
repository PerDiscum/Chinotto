//! Decoding without a sound card. The WAV test always runs; the others decode
//! every file in CHINOTTO_TEST_AUDIO (if set): name them after what they hold,
//! e.g. "sine-440-3s.mp3", "x.m4a", "x.flac", "x.ogg".

use std::path::{Path, PathBuf};

use chinotto_player::decode::Source;

fn write_wav(path: &Path, rate: u32, channels: u16, secs: u32) {
    let frames = rate * secs;
    let data_len = frames * channels as u32 * 2;
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&channels.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
    b.extend_from_slice(&(channels * 2).to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..frames {
        let v = ((i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 12_000.0) as i16;
        for _ in 0..channels {
            b.extend_from_slice(&v.to_le_bytes());
        }
    }
    std::fs::write(path, b).unwrap();
}

/// Decode everything; returns (frames, rate, channels, peak).
fn decode_all(path: &Path) -> (usize, u32, usize, f32) {
    let mut s = Source::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut samples = 0usize;
    let mut peak = 0f32;
    while let Some(chunk) = s.next_chunk().unwrap() {
        samples += chunk.len();
        peak = chunk.iter().fold(peak, |m, v| m.max(v.abs()));
    }
    (samples / s.channels, s.rate, s.channels, peak)
}

#[test]
fn decodes_and_seeks_a_wav() {
    let path = std::env::temp_dir().join(format!("chinotto-player-{}.wav", std::process::id()));
    write_wav(&path, 48_000, 2, 3);
    let (frames, rate, ch, peak) = decode_all(&path);
    assert_eq!((frames, rate, ch), (144_000, 48_000, 2));
    assert!((peak - 12_000.0 / 32_768.0).abs() < 0.01, "{peak}");

    let mut s = Source::open(&path).unwrap();
    assert_eq!(s.duration_ms, Some(3000));
    let at = s.seek(2000).unwrap();
    assert!((1990..=2010).contains(&at), "{at}");
    let mut left = 0;
    while let Some(c) = s.next_chunk().unwrap() {
        left += c.len() / 2;
    }
    assert!((47_000..=49_000).contains(&left), "{left} frames after seeking to 2 s");
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn decodes_sample_files() {
    let Some(dir) = std::env::var_os("CHINOTTO_TEST_AUDIO").map(PathBuf::from) else { return };
    let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
    files.sort();
    assert!(!files.is_empty());
    for f in files {
        let (frames, rate, ch, peak) = decode_all(&f);
        let s = Source::open(&f).unwrap();
        println!("{}: {frames} frames, {rate} Hz, {ch} ch, peak {peak:.2}, duration {:?} ms", f.display(), s.duration_ms);
        assert!(frames > rate as usize, "{}: less than a second decoded", f.display());
        assert!(peak > 0.05, "{}: silent", f.display());
    }
}
