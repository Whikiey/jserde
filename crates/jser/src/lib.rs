//! Rust implementation of the Java Object Serialization stream protocol.
//!
//! P0 provided the workspace and error model. P1 added the bounded wire reader,
//! modified UTF-8 codec, wire handle table and the descriptor-free codec subset
//! (`String`, `null`, block data and `TC_RESET`). P2 adds class descriptors,
//! objects (inheritance and custom data), arrays, enums, the common collection
//! layouts, shared references/cycles and configurable decoding limits. The
//! writer/byte-replay side is P3.

#![forbid(unsafe_code)]

pub mod builtin;
pub mod desc;
pub mod error;
pub mod reader;
pub mod value;
pub mod wire;

pub use error::{Error, Result};
pub use reader::{
    decode, decode_stream, decode_stream_with_limits, decode_with_limits, Limits, Reader,
};
pub use value::{
    BlockData, ClassData, ClassDesc, FieldDesc, FieldType, Graph, Handle, JObject, JValue, MapView,
    Scalar,
};
