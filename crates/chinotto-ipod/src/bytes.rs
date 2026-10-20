//! Little-endian reading/writing helpers with bounds checking.

use crate::Error;

#[derive(Clone, Copy)]
pub struct Bytes<'a> {
    data: &'a [u8],
}

impl<'a> Bytes<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn slice(&self, at: usize, len: usize) -> Result<&'a [u8], Error> {
        at.checked_add(len)
            .and_then(|end| self.data.get(at..end))
            .ok_or(Error::Truncated { at, len })
    }

    pub fn u8(&self, at: usize) -> Result<u8, Error> {
        Ok(self.slice(at, 1)?[0])
    }

    pub fn u16(&self, at: usize) -> Result<u16, Error> {
        let b = self.slice(at, 2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn u32(&self, at: usize) -> Result<u32, Error> {
        let b = self.slice(at, 4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn u64(&self, at: usize) -> Result<u64, Error> {
        let b = self.slice(at, 8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(b);
        Ok(u64::from_le_bytes(a))
    }

    pub fn tag(&self, at: usize) -> Result<[u8; 4], Error> {
        let b = self.slice(at, 4)?;
        Ok([b[0], b[1], b[2], b[3]])
    }

    /// Check a 4-byte record tag such as `b"mhit"`.
    pub fn expect(&self, at: usize, tag: &[u8; 4]) -> Result<(), Error> {
        let got = self.tag(at)?;
        if &got == tag {
            Ok(())
        } else {
            Err(Error::UnexpectedTag { at, expected: *tag, found: got })
        }
    }
}

pub fn put_u16(buf: &mut [u8], at: usize, v: u16) {
    buf[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

pub fn put_u32(buf: &mut [u8], at: usize, v: u32) {
    buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

pub fn put_u64(buf: &mut [u8], at: usize, v: u64) {
    buf[at..at + 8].copy_from_slice(&v.to_le_bytes());
}

pub fn decode_utf16le(b: &[u8]) -> String {
    let units: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    String::from_utf16_lossy(&units)
}

pub fn encode_utf16le(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect()
}
