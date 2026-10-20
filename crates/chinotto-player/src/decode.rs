//! Opening a music file and decoding it to interleaved f32 samples (Symphonia).

use std::fs::File;
use std::path::Path;

use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::Time;

pub struct Source {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    /// Sample rate and channel count of the decoded audio (known after the first packet
    /// for some formats; the codec parameters are used until then).
    pub rate: u32,
    pub channels: usize,
    pub duration_ms: Option<u64>,
}

impl Source {
    pub fn open(path: &Path) -> Result<Self, String> {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let file = File::open(path).map_err(|e| format!("Couldn't open {name}: {e}"))?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
        let format = symphonia::default::get_probe()
            .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
            .map_err(|e| format!("{name}: not a format Chinotto can play ({e})"))?;
        let track = format.default_track(TrackType::Audio).ok_or_else(|| format!("{name}: no audio in this file"))?;
        let params =
            track.codec_params.as_ref().and_then(|p| p.audio()).ok_or_else(|| format!("{name}: no audio codec information"))?.clone();
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(&params, &AudioDecoderOptions::default())
            .map_err(|e| format!("{name}: can't decode this kind of audio ({e})"))?;
        let rate = params.sample_rate.unwrap_or(44_100);
        let channels = params.channels.as_ref().map(|c| c.count()).unwrap_or(2).max(1);
        let duration_ms = match (track.num_frames, track.time_base) {
            (Some(n), Some(tb)) => {
                let t = tb.calc_time_saturating(symphonia::core::units::Timestamp::new(n as i64));
                Some((t.as_secs_f64() * 1000.0) as u64)
            }
            (Some(n), None) if rate > 0 => Some(n * 1000 / rate as u64),
            _ => None,
        };
        let track_id = track.id;
        Ok(Source { format, decoder, track_id, rate, channels, duration_ms })
    }

    /// The next chunk of interleaved samples, or `None` at the end of the file.
    /// Updates `rate`/`channels` if the decoded audio differs from what the header said.
    pub fn next_chunk(&mut self) -> Result<Option<Vec<f32>>, String> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(p)) => p,
                Ok(None) => return Ok(None),
                Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
                Err(SymError::ResetRequired) => {
                    self.decoder.reset();
                    continue;
                }
                Err(e) => return Err(e.to_string()),
            };
            if packet.track_id != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(buf) => {
                    let spec = buf.spec();
                    self.rate = spec.rate();
                    self.channels = spec.channels().count().max(1);
                    let mut out = vec![0f32; buf.samples_interleaved()];
                    buf.copy_to_slice_interleaved(&mut out);
                    if out.is_empty() {
                        continue;
                    }
                    return Ok(Some(out));
                }
                // A damaged packet: skip it, like other players do.
                Err(SymError::DecodeError(_)) | Err(SymError::IoError(_)) => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
    }

    /// Jump to `ms`; returns where playback actually resumes (ms).
    pub fn seek(&mut self, ms: u64) -> Result<u64, String> {
        let time = Time::try_from_secs_f64(ms as f64 / 1000.0).unwrap_or(Time::ZERO);
        let seeked = self
            .format
            .seek(SeekMode::Accurate, SeekTo::Time { time, track_id: Some(self.track_id) })
            .map_err(|e| format!("Couldn't jump to that point: {e}"))?;
        self.decoder.reset();
        let tb = self.format.tracks().iter().find(|t| t.id == self.track_id).and_then(|t| t.time_base);
        Ok(match tb {
            Some(tb) => (tb.calc_time_saturating(seeked.actual_ts).as_secs_f64() * 1000.0).max(0.0) as u64,
            None => ms,
        })
    }
}
