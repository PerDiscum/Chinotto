//! chinotto-player: plays one music file at a time.
//!
//! A player thread decodes (Symphonia) into a short buffer; the sound card's
//! callback (cpal) drains it. The output runs at the file's own sample rate
//! when the device allows, otherwise audio is resampled. Commands go in through
//! [`Player`]; what happens comes back through the event callback.
//!
//! Formats: MP3, AAC (LC), Apple Lossless, WAV, AIFF, FLAC, Ogg Vorbis.
//! (WMA and HE-AAC need FFmpeg; not handled here.)

pub mod convert;
pub mod decode;
pub mod spectrum;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};

use convert::{remap, Resampler};
use decode::Source;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Status {
    pub state: State,
    pub path: Option<PathBuf>,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// State or position changed (sent a few times a second while playing).
    Status(Status),
    /// The file played to its end.
    Ended,
    /// Something went wrong; playback has stopped.
    Error(String),
    /// Levels (0 to 1) of what's playing: [`spectrum::BANDS`] for the left
    /// channel, then as many for the right. Sent about 25 times a second
    /// while playing.
    Spectrum(Vec<f32>),
}

enum Cmd {
    Load { path: PathBuf, start_ms: u64, play: bool },
    Play,
    Pause,
    Stop(Option<Sender<()>>),
    Seek(u64),
    Volume(f32),
}

/// Handle to the player thread. Cheap to share behind an `Arc`.
pub struct Player {
    tx: Mutex<Sender<Cmd>>,
}

impl Player {
    pub fn new(on_event: impl Fn(Event) + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("chinotto-player".into())
            .spawn(move || Engine::new(Box::new(on_event)).run(rx))
            .expect("couldn't start the player thread");
        Player { tx: Mutex::new(tx) }
    }

    fn send(&self, c: Cmd) {
        let _ = self.tx.lock().unwrap().send(c);
    }

    /// Open a file and (if `play`) start playing it from `start_ms`.
    pub fn load(&self, path: PathBuf, start_ms: u64, play: bool) {
        self.send(Cmd::Load { path, start_ms, play });
    }
    pub fn play(&self) {
        self.send(Cmd::Play);
    }
    pub fn pause(&self) {
        self.send(Cmd::Pause);
    }
    /// Stop and close the file.
    pub fn stop(&self) {
        self.send(Cmd::Stop(None));
    }
    /// Stop, and return only once the file is closed (e.g. before ejecting an iPod).
    pub fn stop_and_wait(&self) {
        let (tx, rx) = mpsc::channel();
        self.send(Cmd::Stop(Some(tx)));
        let _ = rx.recv_timeout(Duration::from_secs(2));
    }
    pub fn seek(&self, ms: u64) {
        self.send(Cmd::Seek(ms));
    }
    /// 0.0 (silent) to 1.0 (full).
    pub fn set_volume(&self, v: f32) {
        self.send(Cmd::Volume(v.clamp(0.0, 1.0)));
    }
}

/// Shared between the player thread and the audio callback.
struct Shared {
    buf: Mutex<VecDeque<f32>>,
    volume: AtomicU32,
    paused: AtomicBool,
    /// Output frames played since `base_ms`.
    played: AtomicU64,
    /// What was last sent to the sound card, for the spectrum.
    scope: Mutex<spectrum::Scope>,
}

/// An open output stream and its format.
struct Output {
    stream: cpal::Stream,
    rate: u32,
    channels: usize,
}

struct Engine {
    on_event: Box<dyn Fn(Event) + Send>,
    shared: Arc<Shared>,
    output: Option<Output>,
    source: Option<Source>,
    path: Option<PathBuf>,
    resampler: Option<Resampler>,
    /// Format of the decoded audio the resampler/remapper was set up for.
    src_format: (u32, usize),
    base_ms: u64,
    state: State,
    eof: bool,
    ended_sent: bool,
}

impl Engine {
    fn new(on_event: Box<dyn Fn(Event) + Send>) -> Self {
        Engine {
            on_event,
            shared: Arc::new(Shared {
                buf: Mutex::new(VecDeque::new()),
                volume: AtomicU32::new(0.8f32.to_bits()),
                paused: AtomicBool::new(true),
                played: AtomicU64::new(0),
                scope: Mutex::new(spectrum::Scope::new()),
            }),
            output: None,
            source: None,
            path: None,
            resampler: None,
            src_format: (0, 0),
            base_ms: 0,
            state: State::Stopped,
            eof: false,
            ended_sent: false,
        }
    }

    fn position_ms(&self) -> u64 {
        let rate = self.output.as_ref().map(|o| o.rate).unwrap_or(44_100).max(1) as u64;
        self.base_ms + self.shared.played.load(Ordering::Relaxed) * 1000 / rate
    }

    fn status(&self) -> Status {
        Status {
            state: self.state,
            path: self.path.clone(),
            position_ms: self.position_ms(),
            duration_ms: self.source.as_ref().and_then(|s| s.duration_ms),
        }
    }

    fn emit_status(&self) {
        (self.on_event)(Event::Status(self.status()));
    }

    fn emit_spectrum(&self) {
        let rate = self.output.as_ref().map(|o| o.rate).unwrap_or(44_100);
        let (l, r) = self.shared.scope.lock().unwrap().snapshot();
        let mut v = spectrum::levels(&l, rate).to_vec();
        v.extend(spectrum::levels(&r, rate));
        (self.on_event)(Event::Spectrum(v));
    }

    fn fail(&mut self, msg: String) {
        self.stop();
        (self.on_event)(Event::Error(msg));
    }

    fn run(mut self, rx: Receiver<Cmd>) {
        let mut last_tick = Instant::now();
        let mut last_spectrum = Instant::now();
        loop {
            // Wait briefly for a command when there's nothing to decode.
            let wait = if self.needs_audio() { Duration::from_millis(0) } else { Duration::from_millis(40) };
            match rx.recv_timeout(wait) {
                Ok(cmd) => self.handle(cmd),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            while let Ok(cmd) = rx.try_recv() {
                self.handle(cmd);
            }
            if self.needs_audio() {
                self.decode_some();
            }
            if self.state == State::Playing && self.eof && !self.ended_sent && self.shared.buf.lock().unwrap().is_empty() {
                self.ended_sent = true;
                self.stop();
                (self.on_event)(Event::Ended);
            }
            if self.state == State::Playing && last_spectrum.elapsed() >= Duration::from_millis(40) {
                last_spectrum = Instant::now();
                self.emit_spectrum();
            }
            if self.state == State::Playing && last_tick.elapsed() >= Duration::from_millis(250) {
                last_tick = Instant::now();
                self.emit_status();
            }
        }
    }

    /// Keep about half a second buffered.
    fn needs_audio(&self) -> bool {
        let Some(out) = &self.output else { return false };
        self.source.is_some() && !self.eof && self.shared.buf.lock().unwrap().len() < (out.rate as usize * out.channels) / 2
    }

    fn handle(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Load { path, start_ms, play } => {
                if let Err(e) = self.load(path, start_ms) {
                    self.fail(e);
                    return;
                }
                self.set_state(if play { State::Playing } else { State::Paused });
            }
            Cmd::Play => {
                if self.source.is_some() {
                    self.set_state(State::Playing);
                }
            }
            Cmd::Pause => {
                if self.source.is_some() {
                    self.set_state(State::Paused);
                }
            }
            Cmd::Stop(done) => {
                self.stop();
                self.emit_status();
                if let Some(d) = done {
                    let _ = d.send(());
                }
            }
            Cmd::Seek(ms) => {
                if let Some(src) = self.source.as_mut() {
                    match src.seek(ms) {
                        Ok(actual) => {
                            self.shared.buf.lock().unwrap().clear();
                            self.shared.played.store(0, Ordering::Relaxed);
                            self.base_ms = actual;
                            self.eof = false;
                            self.ended_sent = false;
                            self.emit_status();
                        }
                        Err(e) => (self.on_event)(Event::Error(e)),
                    }
                }
            }
            Cmd::Volume(v) => self.shared.volume.store(v.to_bits(), Ordering::Relaxed),
        }
    }

    fn set_state(&mut self, s: State) {
        self.state = s;
        self.shared.paused.store(s != State::Playing, Ordering::Relaxed);
        self.run_output(s == State::Playing);
        self.emit_status();
    }

    /// Keep the sound output running only while playing: an open, running
    /// stream (even a silent one) can stop the computer from sleeping.
    fn run_output(&self, on: bool) {
        if let Some(o) = &self.output {
            let r = if on { o.stream.play() } else { o.stream.pause() };
            if let Err(e) = r {
                eprintln!("sound output: {e}");
            }
        }
    }

    fn stop(&mut self) {
        self.shared.paused.store(true, Ordering::Relaxed);
        self.run_output(false);
        self.shared.buf.lock().unwrap().clear();
        self.shared.played.store(0, Ordering::Relaxed);
        self.shared.scope.lock().unwrap().clear();
        self.source = None;
        self.path = None;
        self.resampler = None;
        self.base_ms = 0;
        self.eof = false;
        self.state = State::Stopped;
    }

    fn load(&mut self, path: PathBuf, start_ms: u64) -> Result<(), String> {
        self.stop();
        let mut src = Source::open(&path)?;
        self.ensure_output(src.rate, src.channels)?;
        if start_ms > 0 {
            self.base_ms = src.seek(start_ms).unwrap_or(0);
        }
        self.src_format = (0, 0);
        self.source = Some(src);
        self.path = Some(path);
        self.eof = false;
        self.ended_sent = false;
        Ok(())
    }

    fn decode_some(&mut self) {
        let Some(src) = self.source.as_mut() else { return };
        let chunk = match src.next_chunk() {
            Ok(Some(c)) => c,
            Ok(None) => {
                self.eof = true;
                return;
            }
            Err(e) => {
                self.fail(format!("Playback stopped: {e}"));
                return;
            }
        };
        let (rate, ch) = (src.rate, src.channels);
        let out = self.output.as_ref().unwrap();
        let (out_rate, out_ch) = (out.rate, out.channels);
        if (rate, ch) != self.src_format {
            self.src_format = (rate, ch);
            self.resampler = (rate != out_rate).then(|| Resampler::new(rate, out_rate, out_ch));
        }
        let mut samples = remap(&chunk, ch, out_ch);
        if let Some(r) = self.resampler.as_mut() {
            samples = r.process(&samples);
        }
        self.shared.buf.lock().unwrap().extend(samples);
    }

    /// Open (or keep) an output stream, at `rate` if the device supports it.
    fn ensure_output(&mut self, rate: u32, channels: usize) -> Result<(), String> {
        let want_ch = channels.clamp(1, 2);
        if let Some(o) = &self.output {
            if o.rate == rate && o.channels == want_ch {
                return Ok(());
            }
        }
        self.output = None;
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or("No sound output device found")?;
        let supported: Vec<_> = device.supported_output_configs().map_err(|e| format!("Sound output: {e}"))?.collect();
        // Prefer the file's rate with matching channels and float samples, then any format at that rate.
        let pick = supported
            .iter()
            .filter(|c| c.min_sample_rate() <= rate && rate <= c.max_sample_rate() && c.channels() as usize >= want_ch)
            .min_by_key(|c| (c.channels() as usize != want_ch, c.sample_format() != SampleFormat::F32))
            .map(|c| c.with_sample_rate(rate));
        let config = match pick {
            Some(c) => c,
            None => device.default_output_config().map_err(|e| format!("Sound output: {e}"))?,
        };
        let format = config.sample_format();
        // Ask for small chunks (~45 ms). Left to itself, PulseAudio asks for seconds
        // at a time, which makes pause and seek lag and the position jump.
        let buffer = match config.buffer_size() {
            cpal::SupportedBufferSize::Range { min, max } => cpal::BufferSize::Fixed(2048u32.clamp(*min, *max)),
            cpal::SupportedBufferSize::Unknown => cpal::BufferSize::Default,
        };
        let mut stream_config: cpal::StreamConfig = config.into();
        stream_config.buffer_size = buffer;
        let out_rate = stream_config.sample_rate;
        let out_ch = stream_config.channels as usize;
        let shared = self.shared.clone();
        let stream = match format {
            SampleFormat::F32 => build::<f32>(&device, &stream_config, shared),
            SampleFormat::I16 => build::<i16>(&device, &stream_config, shared),
            SampleFormat::I32 => build::<i32>(&device, &stream_config, shared),
            SampleFormat::U16 => build::<u16>(&device, &stream_config, shared),
            other => return Err(format!("Sound output uses an unsupported sample format ({other})")),
        }?;
        stream.play().map_err(|e| format!("Sound output: {e}"))?;
        self.output = Some(Output { stream, rate: out_rate, channels: out_ch });
        Ok(())
    }
}

fn build<T>(device: &cpal::Device, config: &cpal::StreamConfig, shared: Arc<Shared>) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    device
        .build_output_stream(
            *config,
            move |data: &mut [T], _| {
                let silence = T::from_sample(0.0f32);
                if shared.paused.load(Ordering::Relaxed) {
                    data.fill(silence);
                    return;
                }
                let vol = f32::from_bits(shared.volume.load(Ordering::Relaxed));
                // Never block the audio thread: if the decoder holds the lock, play silence this once.
                let Ok(mut buf) = shared.buf.try_lock() else {
                    data.fill(silence);
                    return;
                };
                // The spectrum is a nicety: skip it rather than wait for its lock.
                let mut scope = shared.scope.try_lock().ok();
                let mut frames = 0u64;
                for frame in data.chunks_mut(channels) {
                    if buf.len() < channels {
                        frame.fill(silence);
                        continue;
                    }
                    let (mut l, mut r) = (0.0, 0.0);
                    for (i, s) in frame.iter_mut().enumerate() {
                        let v = buf.pop_front().unwrap_or(0.0);
                        match i {
                            0 => (l, r) = (v, v),
                            1 => r = v,
                            _ => {}
                        }
                        // Perceived loudness: square the slider position.
                        *s = T::from_sample(v * vol * vol);
                    }
                    if let Some(sc) = scope.as_mut() {
                        sc.push(l, r);
                    }
                    frames += 1;
                }
                shared.played.fetch_add(frames, Ordering::Relaxed);
            },
            |e| eprintln!("sound output error: {e}"),
            None,
        )
        .map_err(|e| format!("Sound output: {e}"))
}
