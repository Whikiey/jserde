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
//! use jser::{builtin::new_hash_map, decode_stream, encode_stream, Graph, JValue};
//!
//! let mut graph = Graph::new();
//! let map = new_hash_map(&mut graph, &[("a", "one")])?;
//! graph.add_item(JValue::Obj(map));
//! let bytes = encode_stream(&graph)?;
//!
//! let decoded = decode_stream(&bytes)?;
//! let view = decoded.as_map(&decoded.items()[0])?.unwrap();
//! assert_eq!(view.entries.len(), 1);
//! # Ok::<(), jser::Error>(())
//! ```
//!
//! # Typed Map
//!
//! ```
//! use std::collections::HashMap;
//! use jser::{decode_typed, encode_typed};
//!
//! let map: HashMap<String, i32> = [("a".to_owned(), 1)].into_iter().collect();
//! let bytes = encode_typed(map.clone())?;
//! let back: HashMap<String, i32> = decode_typed(&bytes)?;
//! assert_eq!(back["a"], 1);
//! # Ok::<(), jser::Error>(())
//! ```
//!
//! # Reading a POJO
//!
//! ```no_run
//! use jser::{decode_typed, FromJValue, Graph, JValue, Result};
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
//! # Ok::<(), jser::Error>(())
//! ```
//!
//! Unknown Java classes remain [`JValue::Obj`] and can be re-encoded from the
//! preserved descriptor/custom-data metadata; see `JValue` and `Graph`.
//!
//! # Security
//!
//! Decoding has depth/element/stream-size limits, but the class allowlist is not
//! implemented yet (planned for P6). Do not decode untrusted streams until that
//! hook exists; Java deserialization gadgets are a real risk.

#![forbid(unsafe_code)]

pub mod builtin;
pub mod desc;
pub mod error;
pub mod reader;
pub mod typed;
pub mod value;
pub mod wire;
pub mod writer;

pub use error::{Error, Result};
pub use reader::{
    decode, decode_stream, decode_stream_with_limits, decode_with_limits, Limits, Reader,
};
pub use typed::{decode_typed, encode_typed, FromJValue, IntoJValue};
pub use value::{
    BlockData, ClassData, ClassDesc, FieldDesc, FieldType, Graph, Handle, JObject, JValue, MapView,
    Ref, Scalar,
};
pub use writer::{encode, encode_stream};
