//! Rust implementation of the Java Object Serialization stream protocol.
//!
//! P0 provided the workspace and error model. P1 adds the bounded wire reader,
//! modified UTF-8 codec, wire handle table and the top-level codec subset
//! (`String`, `null`, block data and `TC_RESET`). The dynamic object graph is
//! extended in P2.

#![forbid(unsafe_code)]

pub mod builtin;
pub mod desc;
pub mod error;
pub mod reader;
pub mod value;
pub mod wire;

pub use error::{Error, Result};
pub use reader::{decode, decode_stream, Reader};
pub use value::{BlockData, Graph, Handle, JValue};
