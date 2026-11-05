//! Big-endian byte sink used by the encoder.

use crate::error::{Error, Result};
use crate::wire::mutf8;

/// Growable output buffer with Java `DataOutput`-style writes.
#[derive(Debug, Default)]
pub struct StreamWriter {
    bytes: Vec<u8>,
}

impl StreamWriter {
    /// Create an empty writer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of bytes written so far.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether nothing has been written.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Consume the writer and return the bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Append raw bytes.
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    pub fn write_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub fn write_i8(&mut self, value: i8) {
        self.bytes.push(value as u8);
    }

    pub fn write_u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    pub fn write_i16(&mut self, value: i16) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    pub fn write_u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    pub fn write_i32(&mut self, value: i32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    pub fn write_u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    pub fn write_i64(&mut self, value: i64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    pub fn write_f32(&mut self, value: f32) {
        self.write_u32(value.to_bits());
    }

    pub fn write_f64(&mut self, value: f64) {
        self.write_u64(value.to_bits());
    }

    /// Write a `u16`-length-prefixed modified-UTF-8 string (class/field names).
    pub fn write_utf(&mut self, value: &str) -> Result<()> {
        let bytes = mutf8::encode(value);
        let length = u16::try_from(bytes.len()).map_err(|_| Error::InvalidLength {
            kind: "UTF",
            value: bytes.len() as u64,
        })?;

        self.write_u16(length);
        self.write_bytes(&bytes);
        Ok(())
    }
}
