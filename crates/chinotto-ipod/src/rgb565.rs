//! RGB565 little-endian pixels, as stored in iPod `.ithmb` thumbnail files.
//! Row-major, no row padding. A slot may hold fewer rows than the nominal
//! height (the 56×56 format on iPod Classic stores 56×55 = 6,160 bytes).

/// RGBA8 (width × height × 4) → RGB565LE filling exactly `slot_bytes`.
pub fn encode(rgba: &[u8], width: usize, height: usize, slot_bytes: usize) -> Vec<u8> {
    let mut out = vec![0u8; slot_bytes];
    let rows = height.min(slot_bytes / (width * 2));
    let mut o = 0;
    for y in 0..rows {
        for x in 0..width {
            let i = (y * width + x) * 4;
            let v: u16 = ((rgba[i] as u16 >> 3) << 11) | ((rgba[i + 1] as u16 >> 2) << 5) | (rgba[i + 2] as u16 >> 3);
            out[o..o + 2].copy_from_slice(&v.to_le_bytes());
            o += 2;
        }
    }
    out
}

/// RGB565LE → RGBA8 (rows beyond the stored data are left black).
pub fn decode(raw: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut out = vec![0u8; width * height * 4];
    let rows = height.min(raw.len() / (width * 2));
    for i in 0..width * rows {
        let v = u16::from_le_bytes([raw[2 * i], raw[2 * i + 1]]);
        out[4 * i] = (((v >> 11) & 31) * 255 / 31) as u8;
        out[4 * i + 1] = (((v >> 5) & 63) * 255 / 63) as u8;
        out[4 * i + 2] = ((v & 31) * 255 / 31) as u8;
        out[4 * i + 3] = 255;
    }
    for px in out[width * rows * 4..].as_chunks_mut::<4>().0 {
        px[3] = 255;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_is_stable_after_first_quantisation() {
        let rgba: Vec<u8> = (0..56 * 56).flat_map(|i| [(i % 256) as u8, (i / 7 % 256) as u8, (255 - i % 256) as u8, 255]).collect();
        let a = encode(&rgba, 56, 56, 6160);
        assert_eq!(a.len(), 6160);
        let back = decode(&a, 56, 56);
        assert_eq!(encode(&back, 56, 56, 6160), a);
    }
}
