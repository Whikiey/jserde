//! Error types shared by decoding, encoding and schema conversion.

/// Errors returned by `jser`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The stream did not begin with the Java serialization magic (`0xACED`).
    #[error("bad stream magic: expected 0xACED, found 0x{found:04X}")]
    BadMagic { found: u16 },

    /// The stream version is not supported.
    #[error("unsupported stream version 0x{found:04X}")]
    UnsupportedVersion { found: u16 },

    /// The reader reached the end of the input before a value was complete.
    #[error("unexpected end of stream: need {needed} more byte(s), {remaining} remaining")]
    Truncated { needed: usize, remaining: usize },

    /// A length prefix cannot be represented on this platform.
    #[error("invalid {kind} length: {value}")]
    InvalidLength { kind: &'static str, value: u64 },

    /// A stream tag is unknown or not valid in the current context.
    #[error("unexpected tag 0x{0:02X}")]
    BadTag(u8),

    /// A `TC_REFERENCE` points at a handle that was never assigned.
    #[error("invalid wire handle 0x{0:06X}")]
    BadHandle(u32),

    /// A "modified UTF-8" string is malformed at the given byte offset.
    #[error("malformed modified UTF-8 at byte offset {offset}")]
    MalformedString { offset: usize },

    /// A class cannot be represented by the requested typed conversion.
    #[error("unsupported class: {0}")]
    UnsupportedClass(String),

    /// The stream shape does not match the requested Rust schema.
    #[error("field mismatch: {message}")]
    FieldMismatch { message: String },

    /// A configured depth, element count or size limit was exceeded.
    #[error("limit exceeded: {kind} = {actual} (limit {limit})")]
    LimitExceeded {
        kind: &'static str,
        limit: usize,
        actual: usize,
    },

    /// A `JavaSerializable` schema is internally inconsistent.
    #[error("invalid Java schema: {0}")]
    InvalidSchema(String),
}

/// Convenient result alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;
