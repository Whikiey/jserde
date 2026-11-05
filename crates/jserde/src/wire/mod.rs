//! Wire-level stream primitives: reader, modified UTF-8 and the wire handle
//! table. Block buffers and class descriptors build on these in P1/P2.

pub mod block;
pub mod handle;
pub mod mutf8;
pub mod reader;
pub mod writer;

pub use block::BlockWriter;
pub use handle::WireHandles;
pub use reader::{Input, ReadStream, StreamReader};

/// Stream magic (`AC ED`).
pub const STREAM_MAGIC: u16 = 0xACED;
/// Stream version (`00 05`).
pub const STREAM_VERSION: u16 = 0x0005;

pub const TC_NULL: u8 = 0x70;
pub const TC_REFERENCE: u8 = 0x71;
pub const TC_CLASSDESC: u8 = 0x72;
pub const TC_OBJECT: u8 = 0x73;
pub const TC_STRING: u8 = 0x74;
pub const TC_ARRAY: u8 = 0x75;
pub const TC_CLASS: u8 = 0x76;
pub const TC_BLOCKDATA: u8 = 0x77;
pub const TC_ENDBLOCKDATA: u8 = 0x78;
pub const TC_RESET: u8 = 0x79;
pub const TC_BLOCKDATALONG: u8 = 0x7A;
pub const TC_EXCEPTION: u8 = 0x7B;
pub const TC_LONGSTRING: u8 = 0x7C;
pub const TC_PROXYCLASSDESC: u8 = 0x7D;
pub const TC_ENUM: u8 = 0x7E;

pub const SC_WRITE_METHOD: u8 = 0x01;
pub const SC_SERIALIZABLE: u8 = 0x02;
pub const SC_EXTERNALIZABLE: u8 = 0x04;
pub const SC_BLOCK_DATA: u8 = 0x08;
pub const SC_ENUM: u8 = 0x10;
