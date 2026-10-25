//! How loud each part of the sound is right now, for the display's bars.
//!
//! The sound card's callback copies what it's playing into a [`Scope`]; a
//! few times a second the player thread takes the newest [`N`] frames of each
//! channel, runs an FFT over them, and sums them into [`BANDS`] bands spaced
//! evenly in pitch (about one octave each), from 60 Hz to 16 kHz.

/// Frames looked at each time (about 23 ms at 44.1 kHz).
pub const N: usize = 1024;
/// Bars per channel.
pub const BANDS: usize = 8;

/// The last [`N`] frames sent to the sound card, left and right (a mono file
/// goes into both).
pub(crate) struct Scope {
    l: Vec<f32>,
    r: Vec<f32>,
    pos: usize,
}

impl Scope {
    pub(crate) fn new() -> Self {
        Scope { l: vec![0.0; N], r: vec![0.0; N], pos: 0 }
    }
    pub(crate) fn push(&mut self, l: f32, r: f32) {
        self.l[self.pos] = l;
        self.r[self.pos] = r;
        self.pos = (self.pos + 1) % N;
    }
    pub(crate) fn clear(&mut self) {
        self.l.fill(0.0);
        self.r.fill(0.0);
    }
    /// Both channels, oldest frame first.
    pub(crate) fn snapshot(&self) -> (Vec<f32>, Vec<f32>) {
        let order = |v: &Vec<f32>| v[self.pos..].iter().chain(&v[..self.pos]).copied().collect();
        (order(&self.l), order(&self.r))
    }
}

/// Levels from 0 (silent) to 1 for one channel's [`N`] samples.
pub fn levels(samples: &[f32], rate: u32) -> [f32; BANDS] {
    assert_eq!(samples.len(), N);
    // Hann window, then FFT.
    let mut re: Vec<f32> =
        samples.iter().enumerate().map(|(i, s)| s * (0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / N as f32).cos())).collect();
    let mut im = vec![0.0f32; N];
    fft(&mut re, &mut im);

    let bin_hz = rate as f32 / N as f32;
    let top = 16_000f32.min(rate as f32 / 2.0);
    let edge = |i: usize| 60.0 * (top / 60.0).powf(i as f32 / BANDS as f32);
    let mut out = [0.0f32; BANDS];
    for (b, level) in out.iter_mut().enumerate() {
        let lo = (edge(b) / bin_hz).round() as usize;
        let hi = ((edge(b + 1) / bin_hz).round() as usize).max(lo + 1).min(N / 2);
        let power: f32 = (lo..hi).map(|k| re[k] * re[k] + im[k] * im[k]).sum();
        // A full-scale sine comes out at about N/4 through a Hann window.
        let amp = power.sqrt() * 4.0 / N as f32;
        // Music has much less energy up high: tilt by 3 dB a band so the bars
        // on the right move as much as those on the left.
        let db = 20.0 * amp.max(1e-9).log10() + 3.0 * b as f32;
        *level = ((db + 54.0) / 54.0).clamp(0.0, 1.0);
    }
    out
}

/// In-place radix-2 FFT; the length must be a power of two.
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -std::f32::consts::TAU / len as f32;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (s, c) = (ang * k as f32).sin_cos();
                let (a, b) = (start + k, start + k + len / 2);
                let tr = re[b] * c - im[b] * s;
                let ti = re[b] * s + im[b] * c;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f32, amp: f32) -> Vec<f32> {
        (0..N).map(|i| amp * (std::f32::consts::TAU * hz * i as f32 / 44_100.0).sin()).collect()
    }

    #[test]
    fn silence_is_zero() {
        assert_eq!(levels(&vec![0.0; N], 44_100), [0.0; BANDS]);
    }

    #[test]
    fn a_tone_lights_its_own_band() {
        for (hz, band) in [(85.0, 0), (340.0, 2), (1000.0, 4), (5000.0, 6)] {
            let l = levels(&sine(hz, 0.5), 44_100);
            let loudest = (0..BANDS).max_by(|&a, &b| l[a].total_cmp(&l[b])).unwrap();
            assert_eq!(loudest, band, "{hz} Hz: {l:?}");
            assert!(l[band] > 0.8, "{hz} Hz: {l:?}");
        }
    }

    #[test]
    fn quieter_is_lower() {
        let loud = levels(&sine(1000.0, 0.5), 44_100)[4];
        let soft = levels(&sine(1000.0, 0.05), 44_100)[4];
        assert!(soft < loud - 0.2, "{loud} {soft}");
    }
}
