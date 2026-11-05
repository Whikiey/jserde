//! Write-side block buffer.
//!
//! The JDK flushes primitive writes in 1024-byte chunks; a chunk of at most 255
//! bytes uses `TC_BLOCKDATA`, larger chunks use `TC_BLOCKDATALONG`.

use crate::value::BlockData;
use crate::wire::writer::StreamWriter;

/// Buffers custom-data primitives and turns them into wire blocks.
#[derive(Debug, Default)]
pub struct BlockWriter {
    out: StreamWriter,
}

impl BlockWriter {
    /// Create an empty block buffer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Current buffered byte count.
    pub fn len(&self) -> usize {
        self.out.len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.out.is_empty()
    }

    pub fn write_u8(&mut self, value: u8) {
        self.out.write_u8(value);
    }

    pub fn write_i16(&mut self, value: i16) {
        self.out.write_i16(value);
    }

    pub fn write_i32(&mut self, value: i32) {
        self.out.write_i32(value);
    }

    pub fn write_i64(&mut self, value: i64) {
        self.out.write_i64(value);
    }

    pub fn write_f32(&mut self, value: f32) {
        self.out.write_f32(value);
    }

    pub fn write_f64(&mut self, value: f64) {
        self.out.write_f64(value);
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.out.write_bytes(bytes);
    }

    /// Split the buffered bytes into JDK-style blocks (chunks of at most 1024).
    pub fn into_blocks(self) -> Vec<BlockData> {
        let bytes = self.out.into_bytes();
        bytes
            .chunks(1024)
            .map(|chunk| BlockData {
                bytes: chunk.to_vec(),
                long: chunk.len() > 255,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::BlockWriter;

    #[test]
    fn splits_and_selects_header_width() {
        let mut block = BlockWriter::new();
        block.write_i32(1);
        block.write_bytes(&[0u8; 1021]);
        let blocks = block.into_blocks();

        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].long);
        assert_eq!(blocks[0].bytes.len(), 1024);
        assert!(!blocks[1].long);
        assert_eq!(blocks[1].bytes.len(), 1);
    }

    #[test]
    fn short_block_uses_short_header() {
        let mut block = BlockWriter::new();
        block.write_i32(7);
        let blocks = block.into_blocks();

        assert_eq!(blocks.len(), 1);
        assert!(!blocks[0].long);
        assert_eq!(blocks[0].bytes.len(), 4);
    }
}
