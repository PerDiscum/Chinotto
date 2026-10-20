//! Channel mixing and sample-rate conversion between a file and the sound card.

/// Interleaved `input` with `from` channels → interleaved with `to` channels.
/// Mono is copied to every channel; anything → mono averages the first two;
/// otherwise channels are matched by position and extra ones are silent.
pub fn remap(input: &[f32], from: usize, to: usize) -> Vec<f32> {
    if from == to {
        return input.to_vec();
    }
    let frames = input.len() / from;
    let mut out = Vec::with_capacity(frames * to);
    for f in input.chunks_exact(from) {
        if from == 1 {
            out.extend(std::iter::repeat_n(f[0], to));
        } else if to == 1 {
            out.push((f[0] + f[1]) * 0.5);
        } else {
            let n = from.min(to);
            out.extend_from_slice(&f[..n]);
            out.extend(std::iter::repeat_n(0.0, to - n));
        }
    }
    out
}

/// Streaming linear-interpolation resampler for interleaved audio.
/// Good enough for listening when the sound card can't run at the file's rate
/// (most can, in which case this isn't used at all).
pub struct Resampler {
    channels: usize,
    /// Input frames per output frame.
    step: f64,
    /// Position of the next output frame, in input frames, relative to `prev`.
    pos: f64,
    /// Last input frame of the previous chunk (interpolation needs one frame of history).
    prev: Vec<f32>,
}

impl Resampler {
    pub fn new(from_rate: u32, to_rate: u32, channels: usize) -> Self {
        Resampler { channels, step: from_rate as f64 / to_rate as f64, pos: 0.0, prev: vec![0.0; channels] }
    }

    pub fn process(&mut self, input: &[f32]) -> Vec<f32> {
        let ch = self.channels;
        let n = input.len() / ch;
        if n == 0 {
            return Vec::new();
        }
        // Frame i of the virtual stream [prev, input...]: 0 is prev, k+1 is input frame k.
        let frame = |i: usize, c: usize| if i == 0 { self.prev[c] } else { input[(i - 1) * ch + c] };
        let mut out = Vec::with_capacity(((n as f64 / self.step) as usize + 1) * ch);
        while self.pos < n as f64 {
            let i = self.pos.floor() as usize;
            let t = (self.pos - i as f64) as f32;
            for c in 0..ch {
                let a = frame(i, c);
                let b = frame(i + 1, c);
                out.push(a + (b - a) * t);
            }
            self.pos += self.step;
        }
        self.pos -= n as f64;
        self.prev.copy_from_slice(&input[(n - 1) * ch..n * ch]);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remaps_channels() {
        assert_eq!(remap(&[0.5, -0.5], 1, 2), vec![0.5, 0.5, -0.5, -0.5]);
        assert_eq!(remap(&[1.0, 0.0, 0.5, 0.5], 2, 1), vec![0.5, 0.5]);
        assert_eq!(remap(&[1.0, 2.0], 2, 2), vec![1.0, 2.0]);
        assert_eq!(remap(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 6, 2), vec![1.0, 2.0]);
    }

    #[test]
    fn resamples_length_and_shape() {
        // 1 s of a ramp at 44.1 kHz → ~1 s at 48 kHz, still a ramp.
        let input: Vec<f32> = (0..44_100).map(|i| i as f32 / 44_100.0).collect();
        let mut r = Resampler::new(44_100, 48_000, 1);
        let mut out = Vec::new();
        for chunk in input.chunks(1000) {
            out.extend(r.process(chunk));
        }
        assert!((out.len() as i64 - 48_000).abs() <= 2, "{}", out.len());
        assert!(out.windows(2).skip(1).all(|w| w[1] >= w[0]), "monotonic");
        assert!((out[24_000] - 0.5).abs() < 0.001);
    }
}
