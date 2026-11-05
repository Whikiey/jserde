//! Rust implementation of the Java Object Serialization stream protocol.
//!
//! P0 provided the workspace and error model. P1 added the bounded wire reader,
//! modified UTF-8 codec, wire handle table and the descriptor-free codec subset
//! (`String`, `null`, block data and `TC_RESET`). P2 adds class descriptors,
//! objects (inheritance and custom data), arrays, enums, the common collection
//! layouts, shared references/cycles and configurable decoding limits. P3 adds
//! the graph encoder and byte-exact replay; P4 adds typed conversions.
//!
//! # Reading a Map
//!
//! ```
//! use jserde::{builtin::new_hash_map, decode_stream, encode_stream, Graph, JValue};
//!
//! let mut graph = Graph::new();
//! let map = new_hash_map(&mut graph, &[("a", "one")])?;
//! graph.add_item(JValue::Obj(map));
//! let bytes = encode_stream(&graph)?;
//!
//! let decoded = decode_stream(&bytes)?;
//! let view = decoded.as_map(&decoded.items()[0])?.unwrap();
//! assert_eq!(view.entries.len(), 1);
//! # Ok::<(), jserde::Error>(())
//! ```
//!
//! # Typed Map
//!
//! ```
//! use std::collections::HashMap;
//! use jserde::{decode_typed, encode_typed};
//!
//! let map: HashMap<String, i32> = [("a".to_owned(), 1)].into_iter().collect();
//! let bytes = encode_typed(map.clone())?;
//! let back: HashMap<String, i32> = decode_typed(&bytes)?;
//! assert_eq!(back["a"], 1);
//! # Ok::<(), jserde::Error>(())
//! ```
//!
//! # Allowlist
//!
//! ```
//! use jserde::{decode_stream_with_allowlist, Allowlist};
//!
//! # let bytes: &[u8] = &[0xAC, 0xED, 0x00, 0x05];
//! let allowlist = Allowlist::strict().allow("com.example.*")?;
//! let _ = decode_stream_with_allowlist(bytes, &allowlist);
//! # Ok::<(), jserde::Error>(())
//! ```
//!
//! # Reading a POJO
//!
//! ```no_run
//! use jserde::{decode_typed, FromJValue, Graph, JValue, Result};
//!
//! struct Point { x: i32, y: i32 }
//!
//! impl FromJValue for Point {
//!     fn from_jvalue(value: &JValue, graph: &Graph) -> Result<Self> {
//!         Ok(Self {
//!             x: i32::from_jvalue(graph.require_field(value, "x")?, graph)?,
//!             y: i32::from_jvalue(graph.require_field(value, "y")?, graph)?,
//!         })
//!     }
//! }
//!
//! let bytes: &[u8] = &[];
//! let point: Point = decode_typed(bytes)?;
//! # Ok::<(), jserde::Error>(())
//! ```
//!
//! Unknown Java classes remain [`JValue::Obj`] and can be re-encoded from the
//! preserved descriptor/custom-data metadata; see `JValue` and `Graph`.
//!
//! # Security
//!
//! Decoding uses a strict class [`Allowlist`] by default (`java.lang.*` and
//! `java.util.*`). Use [`decode_stream_with_allowlist`] / [`Reader::with_allowlist`]
//! for trusted extra classes. An allowlist reduces exposure but is **not** a
//! gadget sandbox: allowed classes can still run dangerous `readObject` code.
//! Keep depth/element/stream-size limits enabled for untrusted data.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

pub mod allowlist;
pub mod builtin;
pub mod desc;
pub mod error;
pub mod java;
pub mod reader;
pub mod suid;
pub mod typed;
pub mod value;
pub mod wire;
pub mod writer;

pub use allowlist::Allowlist;
pub use builtin::HashMapLayout;
pub use error::{Error, JavaException, JavaExceptionField, Result};
pub use java::{JavaField, JavaSerializable};
pub use reader::{
    decode, decode_from, decode_from_with_allowlist, decode_from_with_options, decode_stream,
    decode_stream_with_allowlist, decode_stream_with_limits, decode_stream_with_options,
    decode_with_allowlist, decode_with_limits, Limits, Reader,
};
pub use suid::{
    array_class_suid, compute_default_suid, parse_class_file, ClassSignature, FieldSignature,
    MemberSignature,
};
pub use typed::{decode_typed, decode_typed_with_allowlist, encode_typed, FromJValue, IntoJValue};
pub use value::{
    BlockData, ClassData, ClassDesc, FieldDesc, FieldType, Graph, Handle, JObject, JValue, MapView,
    PrimitiveArray, Ref, Scalar,
};
pub use writer::{encode, encode_stream, encode_to};
