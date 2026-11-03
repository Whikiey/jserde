//! Bounded, big-endian cursor over a byte slice.

use crate::error::{Error, Result};

/// A cursor that never panics on truncated input.
#[derive(Debug)]
pub struct StreamReader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> StreamReader<'a> {
    /// Wrap a byte slice. The stream header is checked by `crate::Reader`.
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    /// Current byte offset from the start of the slice.
    pub fn position(&self) -> usize {
        self.position
    }

    /// Bytes left after the cursor.
    pub fn remaining(&self) -> usize {
        self.bytes.len() - self.position
    }

    /// `true` when the cursor reached the end of the slice.
    pub fn is_empty(&self) -> bool {
        self.position == self.bytes.len()
    }

    /// Read one byte without advancing.
    pub fn peek_u8(&self) -> Result<u8> {
        self.bytes
            .get(self.position)
            .copied()
            .ok_or(Error::Truncated {
                needed: 1,
                remaining: 0,
            })
    }

    /// Read exactly `len` bytes and advance.
    pub fn read_exact(&mut self, len: usize) -> Result<&'a [u8]> {
        let remaining = self.remaining();
        if len > remaining {
            return Err(Error::Truncated {
                needed: len,
                remaining,
            });
        }

        let start = self.position;
        self.position += len;
        Ok(&self.bytes[start..self.position])
    }

    /// Read an unsigned byte.
    pub fn read_u8(&mut self) -> Result<u8> {
        let byte = self.read_exact(1)?;
        Ok(byte[0])
    }

    /// Read a signed byte.
    pub fn read_i8(&mut self) -> Result<i8> {
        Ok(self.read_u8()? as i8)
    }

    /// Read a big-endian `u16`.
    pub fn read_u16(&mut self) -> Result<u16> {
        let bytes = self.read_exact(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    /// Read a big-endian `i16`.
    pub fn read_i16(&mut self) -> Result<i16> {
        Ok(self.read_u16()? as i16)
    }

    /// Read a big-endian `u32`.
    pub fn read_u32(&mut self) -> Result<u32> {
        let bytes = self.read_exact(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Read a big-endian `i32`.
    pub fn read_i32(&mut self) -> Result<i32> {
        Ok(self.read_u32()? as i32)
    }

    /// Read a big-endian `u64`.
    pub fn read_u64(&mut self) -> Result<u64> {
        let bytes = self.read_exact(8)?;
        Ok(u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    /// Read a big-endian `i64`.
    pub fn read_i64(&mut self) -> Result<i64> {
        Ok(self.read_u64()? as i64)
    }

    /// Read a big-endian IEEE-754 `f32`.
    pub fn read_f32(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.read_u32()?))
    }

    /// Read a big-endian IEEE-754 `f64`.
    pub fn read_f64(&mut self) -> Result<f64> {
        Ok(f64::from_bits(self.read_u64()?))
    }

    /// Read a Java boolean (`0` or `1`).
    pub fn read_bool(&mut self) -> Result<bool> {
        match self.read_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(Error::BadTag(other)),
        }
    }
}
