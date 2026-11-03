//! Dynamic value graph: `JValue`, arena slots and `Graph` views.
//!
//! P1 implements the reference-bearing string slot plus the scalar/block/reset
//! items needed by the codec fixtures. Arrays, objects, enums and class
//! descriptors are added in P2.

use crate::error::{Error, Result};

/// Arena index. Monotonic over the whole stream; **not** a wire handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Handle(pub u32);

/// A block-data payload with its original header width.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockData {
    /// Raw bytes of the block (header excluded).
    pub bytes: Vec<u8>,
    /// `true` when the stream used `TC_BLOCKDATALONG`.
    pub long: bool,
}

/// A decoded stream item.
#[derive(Debug, Clone, PartialEq)]
pub enum JValue {
    Null,
    Bool(bool),
    Byte(i8),
    Char(u16),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    /// A `java.lang.String`; the value lives in a `Graph` slot.
    Str(Handle),
    /// An array; implemented in P2.
    Arr(Handle),
    /// An object; implemented in P2.
    Obj(Handle),
    /// An enum constant; implemented in P2.
    Enum(Handle),
    /// Top-level or custom block data, preserving the original block boundary.
    Block(BlockData),
    /// `TC_RESET`: valid only as a stream item or inside custom data.
    Reset,
}

#[derive(Debug)]
enum Slot {
    Str { value: String, long: bool },
}

/// Arena of reference-bearing values plus the top-level item sequence.
#[derive(Debug, Default)]
pub struct Graph {
    slots: Vec<Slot>,
    items: Vec<JValue>,
}

impl Graph {
    /// Top-level items in stream order, including `JValue::Reset`.
    pub fn items(&self) -> &[JValue] {
        &self.items
    }

    /// Resolve a string item to its UTF-8 value.
    pub fn as_str(&self, value: &JValue) -> Result<&str> {
        let JValue::Str(handle) = value else {
            return Err(Error::FieldMismatch {
                message: "expected a Java String".to_owned(),
            });
        };
        self.str(*handle)
    }

    /// Whether the original stream used `TC_LONGSTRING` for this string.
    pub fn str_is_long(&self, value: &JValue) -> Result<bool> {
        let JValue::Str(handle) = value else {
            return Err(Error::FieldMismatch {
                message: "expected a Java String".to_owned(),
            });
        };

        match self.slots.get(handle.0 as usize) {
            Some(Slot::Str { long, .. }) => Ok(*long),
            _ => Err(Error::BadHandle(handle.0)),
        }
    }

    pub(crate) fn push_item(&mut self, value: JValue) {
        self.items.push(value);
    }

    pub(crate) fn insert_str(&mut self, value: String, long: bool) -> Handle {
        let handle = Handle(self.slots.len() as u32);
        self.slots.push(Slot::Str { value, long });
        handle
    }

    fn str(&self, handle: Handle) -> Result<&str> {
        match self.slots.get(handle.0 as usize) {
            Some(Slot::Str { value, .. }) => Ok(value),
            _ => Err(Error::BadHandle(handle.0)),
        }
    }
}
