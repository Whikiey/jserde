//! Rust implementation of the Java Object Serialization stream protocol.
//!
//! This crate is currently the P0 skeleton: the wire reader/writer, class
//! descriptor model and dynamic value graph are added in later phases. The
//! public error type is already stable enough for the decoder work to build on.

#![forbid(unsafe_code)]

pub mod builtin;
pub mod desc;
pub mod error;
pub mod value;
pub mod wire;

pub use error::{Error, Result};
