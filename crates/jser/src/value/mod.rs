//! Dynamic value graph: `JValue`, arena slots and `Graph` views.
//!
//! Reference-bearing values (strings, arrays, objects, enums and class
//! descriptors) all live in the arena. `JValue` only carries a [`Handle`], so
//! shared references and cycles are represented without `Rc<RefCell<_>>`.
//!
//! Arrays, objects, enums and descriptors are populated by the P2 decoder; the
//! views below (`as_map`, `as_list`, `as_set`, `as_object`, ...) are the
//! read-side API used by tests and callers.

use crate::builtin;
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

/// Java field type code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    Byte,
    Char,
    Double,
    Float,
    Int,
    Long,
    Short,
    Boolean,
    Object,
    Array,
}

impl FieldType {
    /// Decode a descriptor type code (`B`, `C`, ... `L`, `[`).
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            b'B' => Some(Self::Byte),
            b'C' => Some(Self::Char),
            b'D' => Some(Self::Double),
            b'F' => Some(Self::Float),
            b'I' => Some(Self::Int),
            b'J' => Some(Self::Long),
            b'S' => Some(Self::Short),
            b'Z' => Some(Self::Boolean),
            b'L' => Some(Self::Object),
            b'[' => Some(Self::Array),
            _ => None,
        }
    }

    /// The descriptor type code for this field type.
    pub fn code(self) -> u8 {
        match self {
            Self::Byte => b'B',
            Self::Char => b'C',
            Self::Double => b'D',
            Self::Float => b'F',
            Self::Int => b'I',
            Self::Long => b'J',
            Self::Short => b'S',
            Self::Boolean => b'Z',
            Self::Object => b'L',
            Self::Array => b'[',
        }
    }

    /// Whether the field is written as bare bytes in classdata.
    pub fn is_primitive(self) -> bool {
        !matches!(self, Self::Object | Self::Array)
    }

    /// Wire width in bytes for a primitive type; `None` for object arrays.
    pub fn primitive_width(self) -> Option<usize> {
        match self {
            Self::Byte | Self::Boolean => Some(1),
            Self::Char | Self::Short => Some(2),
            Self::Int | Self::Float => Some(4),
            Self::Long | Self::Double => Some(8),
            Self::Object | Self::Array => None,
        }
    }
}

/// One field in a class descriptor, in descriptor order.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDesc {
    /// Field name.
    pub name: String,
    /// Field type code.
    pub typecode: FieldType,
    /// JVM descriptor for object/array fields (`Ljava/lang/String;`, `[I`).
    pub class_name1: Option<String>,
    /// The decoded `className1` string slot, kept for byte-exact replay.
    ///
    /// `None` for primitive fields and for descriptors constructed by the
    /// caller; the writer then emits a fresh `TC_STRING`.
    pub class_name_handle: Option<Handle>,
}

/// Parsed `TC_CLASSDESC` payload.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassDesc {
    /// Dot-separated class name (`java.util.HashMap`, `[I`).
    pub name: String,
    /// `serialVersionUID` exactly as written in the stream.
    pub suid: i64,
    /// Raw `SC_*` flags.
    pub flags: u8,
    /// Fields in descriptor order (primitives first, then object/array).
    pub fields: Vec<FieldDesc>,
    /// Superclass descriptor, if any.
    pub super_desc: Option<Handle>,
    /// `classAnnotation` contents (normally empty; kept for byte replay).
    pub annotations: Vec<JValue>,
}

impl ClassDesc {
    pub fn is_serializable(&self) -> bool {
        self.flags & crate::wire::SC_SERIALIZABLE != 0
    }

    pub fn has_write_method(&self) -> bool {
        self.flags & crate::wire::SC_WRITE_METHOD != 0
    }

    pub fn is_externalizable(&self) -> bool {
        self.flags & crate::wire::SC_EXTERNALIZABLE != 0
    }

    pub fn has_block_data(&self) -> bool {
        self.flags & crate::wire::SC_BLOCK_DATA != 0
    }

    pub fn is_enum(&self) -> bool {
        self.flags & crate::wire::SC_ENUM != 0
    }

    /// Whether this class contributes a custom data section to the stream.
    pub fn has_custom_data(&self) -> bool {
        self.has_write_method() || (self.is_externalizable() && self.has_block_data())
    }
}

/// One class layer of an object's classdata, ordered parent → child.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassData {
    /// Descriptor for this layer.
    pub desc: Handle,
    /// Fields in descriptor order.
    pub fields: Vec<(String, JValue)>,
    /// `Some` for `SC_WRITE_METHOD` / `SC_BLOCK_DATA`; `Some(vec![])` still needs
    /// the terminating `TC_ENDBLOCKDATA`.
    pub custom: Option<Vec<JValue>>,
}

/// A decoded object with one entry per class in its hierarchy (parent → child).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct JObject {
    pub classes: Vec<ClassData>,
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
    /// An array; the descriptor and elements live in a `Graph` slot.
    Arr(Handle),
    /// An object; the class layers live in a `Graph` slot.
    Obj(Handle),
    /// An enum constant; the descriptor and constant name live in a slot.
    Enum(Handle),
    /// Top-level or custom block data, preserving the original block boundary.
    Block(BlockData),
    /// `TC_RESET`: valid only as a stream item or inside custom data.
    Reset,
}

/// Borrowed view of one arena slot.
///
/// `resolve` returns this without expanding nested values, so cyclic graphs are
/// safe to inspect.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ref<'a> {
    Desc(&'a ClassDesc),
    Str(&'a str),
    Arr(&'a [JValue]),
    /// A compact primitive array; use `Graph::as_primitive_array` for typed access.
    PrimitiveArr {
        typecode: FieldType,
        bytes: &'a [u8],
    },
    Obj(&'a JObject),
    Enum(&'a str),
}

/// Scalar view of a decoded value, including boxed `java.lang.*` wrappers.
#[derive(Debug, Clone, PartialEq)]
pub enum Scalar<'a> {
    Null,
    Bool(bool),
    I64(i64),
    F64(f64),
    Char(u16),
    String(&'a str),
}

/// A collection-like map view over a decoded `HashMap`/`LinkedHashMap`.
#[derive(Debug, Clone, PartialEq)]
pub struct MapView<'a> {
    /// Entries in wire order (which is insertion order for `LinkedHashMap`).
    pub entries: Vec<(&'a JValue, &'a JValue)>,
    /// `loadFactor` from the `HashMap` classdata.
    pub load_factor: f32,
    /// Capacity from the custom block header.
    pub capacity: i32,
    /// `true` for `LinkedHashMap`.
    pub ordered: bool,
}

/// Borrowed view of a compact primitive array.
///
/// Elements are stored as their big-endian wire bytes (the same bytes Java
/// wrote), so a 25M-element `int[]` costs 100 MB rather than 32 bytes per
/// [`JValue`]. Use [`Graph::as_primitive_array`] to obtain one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrimitiveArray<'a> {
    typecode: FieldType,
    bytes: &'a [u8],
}

impl<'a> PrimitiveArray<'a> {
    /// Element type (`FieldType::Int` for `int[]`, ...).
    pub fn typecode(&self) -> FieldType {
        self.typecode
    }

    /// Number of elements.
    pub fn len(&self) -> usize {
        self.typecode
            .primitive_width()
            .map(|width| self.bytes.len() / width)
            .unwrap_or(0)
    }

    /// Whether the array is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Raw wire bytes (big-endian, header excluded).
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// Materialize one element as a [`JValue`].
    pub fn get(&self, index: usize) -> Option<JValue> {
        let width = self.typecode.primitive_width()?;
        let start = index.checked_mul(width)?;
        let end = start.checked_add(width)?;
        let bytes = self.bytes.get(start..end)?;

        Some(match self.typecode {
            FieldType::Byte => JValue::Byte(bytes[0] as i8),
            FieldType::Boolean => JValue::Bool(bytes[0] != 0),
            FieldType::Char => JValue::Char(u16::from_be_bytes([bytes[0], bytes[1]])),
            FieldType::Short => JValue::Short(i16::from_be_bytes([bytes[0], bytes[1]])),
            FieldType::Int => {
                JValue::Int(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
            }
            FieldType::Long => JValue::Long(i64::from_be_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ])),
            FieldType::Float => JValue::Float(f32::from_bits(u32::from_be_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3],
            ]))),
            FieldType::Double => JValue::Double(f64::from_bits(u64::from_be_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ]))),
            FieldType::Object | FieldType::Array => return None,
        })
    }

    /// Iterate elements, materializing one [`JValue`] at a time.
    pub fn iter(&self) -> impl Iterator<Item = JValue> + '_ {
        (0..self.len()).filter_map(|index| self.get(index))
    }
}

/// Element storage for arrays.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ArrayData {
    /// Object arrays keep one arena value per element.
    Values(Vec<JValue>),
    /// Primitive arrays keep their wire-order big-endian bytes.
    Primitive { typecode: FieldType, bytes: Vec<u8> },
}

#[derive(Debug)]
enum Slot {
    Desc(ClassDesc),
    Str {
        value: String,
        long: bool,
    },
    Arr {
        desc: Handle,
        data: ArrayData,
    },
    Obj(JObject),
    Enum {
        desc: Handle,
        constant: Option<Handle>,
    },
}

/// Arena of reference-bearing values plus the top-level item sequence.
#[derive(Debug, Default)]
pub struct Graph {
    slots: Vec<Slot>,
    items: Vec<JValue>,
}

impl Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new Java string to the arena. Header width is chosen from the
    /// modified UTF-8 byte length, not Rust `String::len()`.
    pub fn new_string(&mut self, value: impl Into<String>) -> Handle {
        let value = value.into();
        let long = crate::wire::mutf8::encoded_len(&value) > usize::from(u16::MAX);
        self.insert_str(value, long)
    }

    /// Append a top-level stream item.
    pub fn add_item(&mut self, value: JValue) {
        self.push_item(value);
    }

    /// Append a `TC_RESET` marker between top-level items.
    pub fn add_reset(&mut self) {
        self.add_item(JValue::Reset);
    }

    /// Add a caller-built class descriptor, reusing an identical one if present.
    pub fn add_class_desc(&mut self, desc: ClassDesc) -> Result<Handle> {
        for (index, slot) in self.slots.iter().enumerate() {
            if let Slot::Desc(existing) = slot {
                if existing == &desc {
                    return Ok(Handle(index as u32));
                }
            }
        }
        if let Some(super_desc) = desc.super_desc {
            self.expect_desc(super_desc)?;
        }
        Ok(self.insert_desc(desc))
    }

    /// Add a caller-built object. `classes` must be ordered parent → child.
    pub fn new_object(&mut self, classes: Vec<ClassData>) -> Result<Handle> {
        if classes.is_empty() {
            return Err(Error::FieldMismatch {
                message: "an object needs at least one class layer".to_owned(),
            });
        }
        for class in &classes {
            self.expect_desc(class.desc)?;
        }

        let handle = self.insert_object_placeholder();
        self.set_object(handle, JObject { classes })?;
        Ok(handle)
    }

    /// Add a caller-built array.
    ///
    /// Primitive arrays are compacted to their wire bytes so decoding and
    /// re-encoding large primitive arrays stays at one byte per byte.
    pub fn new_array(&mut self, desc: Handle, elems: Vec<JValue>) -> Result<Handle> {
        self.expect_desc(desc)?;
        let name = self.descriptor(desc)?.name.clone();
        let data = match primitive_array_typecode(&name) {
            Some(typecode) => ArrayData::Primitive {
                typecode,
                bytes: primitive_bytes(typecode, &elems)?,
            },
            None => ArrayData::Values(elems),
        };
        let handle = self.insert_array_placeholder(desc);
        self.set_array_data(handle, data)?;
        Ok(handle)
    }

    /// Add a caller-built primitive array from raw big-endian wire bytes.
    ///
    /// `typecode` must match the descriptor class name (`[I`, `[B`, ...) and
    /// `bytes.len()` must be a multiple of the element width.
    pub fn new_primitive_array(
        &mut self,
        desc: Handle,
        typecode: FieldType,
        bytes: Vec<u8>,
    ) -> Result<Handle> {
        self.expect_desc(desc)?;
        let width = typecode.primitive_width().ok_or_else(|| {
            Error::InvalidSchema(format!("{typecode:?} is not a primitive array type"))
        })?;
        if bytes.len() % width != 0 {
            return Err(Error::FieldMismatch {
                message: format!(
                    "primitive array of {typecode:?} needs a multiple of {width} bytes, got {}",
                    bytes.len()
                ),
            });
        }
        let expected = format!("[{}", char::from(typecode.code()));
        if self.descriptor(desc)?.name != expected {
            return Err(Error::InvalidSchema(format!(
                "class `{}` does not match primitive array type {typecode:?}",
                self.descriptor(desc)?.name
            )));
        }
        let handle = self.insert_array_placeholder(desc);
        self.set_array_data(handle, ArrayData::Primitive { typecode, bytes })?;
        Ok(handle)
    }

    /// Add a caller-built enum constant.
    pub fn new_enum(&mut self, desc: Handle, constant: Handle) -> Result<Handle> {
        self.expect_desc(desc)?;
        if !matches!(self.value_for_handle(constant)?, JValue::Str(_)) {
            return Err(Error::FieldMismatch {
                message: "an enum constant name must be a Java String".to_owned(),
            });
        }
        let handle = self.insert_enum_placeholder(desc);
        self.set_enum_constant(handle, constant)?;
        Ok(handle)
    }

    /// Append one entry to a decoded `HashMap`/`LinkedHashMap`.
    ///
    /// The custom-data header is updated in place; Java's `readObject` reads
    /// `capacity`/`size` and reconstructs the table, so the caller does not have
    /// to rebuild the whole map.
    pub fn insert_map_entry(&mut self, map: &JValue, key: JValue, value: JValue) -> Result<()> {
        let JValue::Obj(handle) = map else {
            return Err(Error::FieldMismatch {
                message: "insert_map_entry expects a map object".to_owned(),
            });
        };

        let layer = {
            let object = match self.slots.get(handle.0 as usize) {
                Some(Slot::Obj(object)) => object,
                _ => return Err(Error::BadHandle(handle.0)),
            };
            object
                .classes
                .iter()
                .position(|class| {
                    self.descriptor(class.desc)
                        .map(|desc| desc.name == crate::builtin::HASH_MAP)
                        .unwrap_or(false)
                })
                .ok_or_else(|| Error::FieldMismatch {
                    message: "object is not a java.util.HashMap".to_owned(),
                })?
        };

        let object = match self.slots.get_mut(handle.0 as usize) {
            Some(Slot::Obj(object)) => object,
            _ => return Err(Error::BadHandle(handle.0)),
        };
        let class = &mut object.classes[layer];
        let custom = class.custom.as_mut().ok_or_else(|| Error::FieldMismatch {
            message: "java.util.HashMap has no custom data".to_owned(),
        })?;

        let Some(JValue::Block(block)) = custom.first_mut() else {
            return Err(Error::FieldMismatch {
                message: "java.util.HashMap custom data has no header block".to_owned(),
            });
        };
        if block.long || block.bytes.len() != 8 {
            return Err(Error::FieldMismatch {
                message: "java.util.HashMap header block must be 8 short-header bytes".to_owned(),
            });
        }

        let size =
            i32::from_be_bytes(
                block.bytes[4..8]
                    .try_into()
                    .map_err(|_| Error::FieldMismatch {
                        message: "java.util.HashMap header block is truncated".to_owned(),
                    })?,
            );
        let size = size.checked_add(1).ok_or_else(|| Error::FieldMismatch {
            message: "java.util.HashMap entry count overflows".to_owned(),
        })?;
        block.bytes[4..8].copy_from_slice(&size.to_be_bytes());
        custom.push(key);
        custom.push(value);
        Ok(())
    }

    /// Replace one field of an object (used by mutation tests and callers).
    pub fn set_field(&mut self, object: &JValue, name: &str, value: JValue) -> Result<bool> {
        let JValue::Obj(handle) = object else {
            return Err(Error::FieldMismatch {
                message: "set_field expects an object".to_owned(),
            });
        };

        match self.slots.get_mut(handle.0 as usize) {
            Some(Slot::Obj(object)) => {
                for class in &mut object.classes {
                    for (field_name, field_value) in &mut class.fields {
                        if field_name == name {
                            *field_value = value;
                            return Ok(true);
                        }
                    }
                }
                Ok(false)
            }
            _ => Err(Error::BadHandle(handle.0)),
        }
    }

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

    /// Borrow the arena slot behind a handle without recursively expanding it.
    pub fn resolve(&self, handle: Handle) -> Result<Ref<'_>> {
        match self.slot(handle)? {
            Slot::Desc(desc) => Ok(Ref::Desc(desc)),
            Slot::Str { value, .. } => Ok(Ref::Str(value)),
            Slot::Arr {
                data: ArrayData::Values(elems),
                ..
            } => Ok(Ref::Arr(elems)),
            Slot::Arr {
                data: ArrayData::Primitive { typecode, bytes },
                ..
            } => Ok(Ref::PrimitiveArr {
                typecode: *typecode,
                bytes: bytes.as_slice(),
            }),
            Slot::Obj(object) => Ok(Ref::Obj(object)),
            Slot::Enum {
                constant: Some(constant),
                ..
            } => Ok(Ref::Enum(self.str(*constant)?)),
            Slot::Enum { constant: None, .. } => Err(Error::FieldMismatch {
                message: "enum constant was not read".to_owned(),
            }),
        }
    }

    /// The object behind an `Obj` value, or `None` for other value kinds.
    pub fn as_object(&self, value: &JValue) -> Result<Option<&JObject>> {
        let JValue::Obj(handle) = value else {
            return Ok(None);
        };

        match self.slot(*handle)? {
            Slot::Obj(object) => Ok(Some(object)),
            _ => Err(Error::BadHandle(handle.0)),
        }
    }

    /// Element values of an object array, or `None` for other value kinds.
    ///
    /// Primitive arrays are stored compactly and return `None` here; use
    /// [`Graph::as_primitive_array`] for them.
    pub fn as_array(&self, value: &JValue) -> Result<Option<&[JValue]>> {
        let JValue::Arr(handle) = value else {
            return Ok(None);
        };

        match self.slot(*handle)? {
            Slot::Arr {
                data: ArrayData::Values(elems),
                ..
            } => Ok(Some(elems)),
            Slot::Arr { .. } => Ok(None),
            _ => Err(Error::BadHandle(handle.0)),
        }
    }

    /// Compact view of a primitive array, or `None` for object arrays and
    /// non-array values.
    pub fn as_primitive_array<'a>(&'a self, value: &JValue) -> Result<Option<PrimitiveArray<'a>>> {
        let JValue::Arr(handle) = value else {
            return Ok(None);
        };

        match self.slot(*handle)? {
            Slot::Arr {
                data: ArrayData::Primitive { typecode, bytes },
                ..
            } => Ok(Some(PrimitiveArray {
                typecode: *typecode,
                bytes,
            })),
            Slot::Arr { .. } => Ok(None),
            _ => Err(Error::BadHandle(handle.0)),
        }
    }

    /// The constant name of an `Enum` value, or `None` for other value kinds.
    pub fn as_enum_constant(&self, value: &JValue) -> Result<Option<&str>> {
        let JValue::Enum(handle) = value else {
            return Ok(None);
        };

        match self.slot(*handle)? {
            Slot::Enum {
                constant: Some(constant),
                ..
            } => Ok(Some(self.str(*constant)?)),
            Slot::Enum { constant: None, .. } => Err(Error::FieldMismatch {
                message: "enum constant was not read".to_owned(),
            }),
            _ => Err(Error::BadHandle(handle.0)),
        }
    }

    /// Semantic built-in value (Date/BigInteger/BigDecimal/java.time), if known.
    pub fn builtin(&self, value: &JValue) -> Result<Option<crate::builtin::BuiltinValue>> {
        crate::builtin::Registry::new().decode(self, value)
    }

    /// Map view for a `HashMap`/`LinkedHashMap` object; `None` otherwise.
    pub fn as_map<'a>(&'a self, value: &'a JValue) -> Result<Option<MapView<'a>>> {
        builtin::map_view(self, value)
    }

    /// List view for an `ArrayList`/`LinkedList` object; `None` otherwise.
    pub fn as_list<'a>(&'a self, value: &'a JValue) -> Result<Option<Vec<&'a JValue>>> {
        builtin::list_view(self, value)
    }

    /// Set view for a `HashSet` object; `None` otherwise.
    pub fn as_set<'a>(&'a self, value: &'a JValue) -> Result<Option<Vec<&'a JValue>>> {
        builtin::set_view(self, value)
    }

    /// Most-derived class name for an object/array/enum, or `java.lang.String`.
    pub fn class_name<'a>(&'a self, value: &JValue) -> Result<&'a str> {
        match value {
            JValue::Str(_) => Ok("java.lang.String"),
            JValue::Arr(handle) => match self.slot(*handle)? {
                Slot::Arr { desc, .. } => Ok(&self.descriptor(*desc)?.name),
                _ => Err(Error::BadHandle(handle.0)),
            },
            JValue::Obj(handle) => match self.slot(*handle)? {
                Slot::Obj(object) => {
                    let class = object.classes.last().ok_or_else(|| Error::FieldMismatch {
                        message: "object has no class data".to_owned(),
                    })?;
                    Ok(&self.descriptor(class.desc)?.name)
                }
                _ => Err(Error::BadHandle(handle.0)),
            },
            JValue::Enum(handle) => match self.slot(*handle)? {
                Slot::Enum { desc, .. } => Ok(&self.descriptor(*desc)?.name),
                _ => Err(Error::BadHandle(handle.0)),
            },
            _ => Err(Error::FieldMismatch {
                message: "value is not a reference type".to_owned(),
            }),
        }
    }

    /// Find a field by name across all class layers (parent → child).
    pub fn field<'a>(&'a self, value: &JValue, name: &str) -> Result<Option<&'a JValue>> {
        let Some(object) = self.as_object(value)? else {
            return Ok(None);
        };

        for class in &object.classes {
            for (field_name, field_value) in &class.fields {
                if field_name == name {
                    return Ok(Some(field_value));
                }
            }
        }
        Ok(None)
    }

    /// Find a required field, with the enclosing class name in the error.
    pub fn require_field<'a>(&'a self, value: &JValue, name: &str) -> Result<&'a JValue> {
        if let Some(field) = self.field(value, name)? {
            return Ok(field);
        }
        let class = self.class_name(value).unwrap_or("<non-object>");
        Err(Error::FieldMismatch {
            message: format!("`{class}` has no field `{name}`"),
        })
    }

    /// Flattened fields in classdata order (parent → child).
    pub fn fields<'a>(&'a self, value: &JValue) -> Result<Vec<(&'a str, &'a JValue)>> {
        let Some(object) = self.as_object(value)? else {
            return Ok(Vec::new());
        };

        let mut fields = Vec::new();
        for class in &object.classes {
            for (name, field_value) in &class.fields {
                fields.push((name.as_str(), field_value));
            }
        }
        Ok(fields)
    }

    /// Most-derived descriptor for an object/array/enum value.
    pub fn descriptor_of(&self, value: &JValue) -> Result<&ClassDesc> {
        match value {
            JValue::Arr(handle) => match self.slot(*handle)? {
                Slot::Arr { desc, .. } => self.descriptor(*desc),
                _ => Err(Error::BadHandle(handle.0)),
            },
            JValue::Obj(handle) => match self.slot(*handle)? {
                Slot::Obj(object) => {
                    let class = object.classes.last().ok_or_else(|| Error::FieldMismatch {
                        message: "object has no class data".to_owned(),
                    })?;
                    self.descriptor(class.desc)
                }
                _ => Err(Error::BadHandle(handle.0)),
            },
            JValue::Enum(handle) => match self.slot(*handle)? {
                Slot::Enum { desc, .. } => self.descriptor(*desc),
                _ => Err(Error::BadHandle(handle.0)),
            },
            _ => Err(Error::FieldMismatch {
                message: "value has no class descriptor".to_owned(),
            }),
        }
    }

    /// Class descriptor by arena handle.
    pub fn descriptor(&self, handle: Handle) -> Result<&ClassDesc> {
        match self.slot(handle)? {
            Slot::Desc(desc) => Ok(desc),
            _ => Err(Error::BadHandle(handle.0)),
        }
    }

    /// Interpret a primitive or a boxed `java.lang.*` number as `i64`.
    pub fn as_i64(&self, value: &JValue) -> Result<i64> {
        match value {
            JValue::Byte(value) => Ok(i64::from(*value)),
            JValue::Short(value) => Ok(i64::from(*value)),
            JValue::Int(value) => Ok(i64::from(*value)),
            JValue::Long(value) => Ok(*value),
            JValue::Char(value) => Ok(i64::from(*value)),
            JValue::Obj(_) => {
                let class = self.class_name(value)?;
                let field = match class {
                    "java.lang.Byte"
                    | "java.lang.Short"
                    | "java.lang.Integer"
                    | "java.lang.Long"
                    | "java.lang.Character" => "value",
                    _ => {
                        return Err(Error::FieldMismatch {
                            message: format!("`{class}` is not an integer"),
                        })
                    }
                };
                let Some(value) = self.field(value, field)? else {
                    return Err(Error::FieldMismatch {
                        message: format!("`{class}` has no `{field}` field"),
                    });
                };
                self.as_i64(value)
            }
            _ => Err(Error::FieldMismatch {
                message: format!("expected an integer, got {value:?}"),
            }),
        }
    }

    /// Interpret a primitive or boxed number as `i32`.
    pub fn as_i32(&self, value: &JValue) -> Result<i32> {
        let value = self.as_i64(value)?;
        i32::try_from(value).map_err(|_| Error::FieldMismatch {
            message: format!("integer {value} does not fit in i32"),
        })
    }

    /// Interpret a primitive or boxed floating-point value as `f64`.
    pub fn as_f64(&self, value: &JValue) -> Result<f64> {
        match value {
            JValue::Float(value) => Ok(f64::from(*value)),
            JValue::Double(value) => Ok(*value),
            JValue::Obj(_) => {
                let class = self.class_name(value)?;
                let field = match class {
                    "java.lang.Float" | "java.lang.Double" => "value",
                    _ => return self.as_i64(value).map(|value| value as f64),
                };
                let Some(value) = self.field(value, field)? else {
                    return Err(Error::FieldMismatch {
                        message: format!("`{class}` has no `{field}` field"),
                    });
                };
                self.as_f64(value)
            }
            _ => self.as_i64(value).map(|value| value as f64),
        }
    }

    /// Interpret a boolean or boxed `java.lang.Boolean`.
    pub fn as_bool(&self, value: &JValue) -> Result<bool> {
        match value {
            JValue::Bool(value) => Ok(*value),
            JValue::Obj(_) if self.class_name(value)? == "java.lang.Boolean" => {
                let Some(value) = self.field(value, "value")? else {
                    return Err(Error::FieldMismatch {
                        message: "`java.lang.Boolean` has no `value` field".to_owned(),
                    });
                };
                self.as_bool(value)
            }
            _ => Err(Error::FieldMismatch {
                message: format!("expected a boolean, got {value:?}"),
            }),
        }
    }

    /// Interpret a `char` or boxed `java.lang.Character` as a UTF-16 code unit.
    pub fn as_char(&self, value: &JValue) -> Result<u16> {
        match value {
            JValue::Char(value) => Ok(*value),
            JValue::Obj(_) if self.class_name(value)? == "java.lang.Character" => {
                let Some(value) = self.field(value, "value")? else {
                    return Err(Error::FieldMismatch {
                        message: "`java.lang.Character` has no `value` field".to_owned(),
                    });
                };
                self.as_char(value)
            }
            _ => Err(Error::FieldMismatch {
                message: format!("expected a char, got {value:?}"),
            }),
        }
    }

    /// Scalar view of a primitive, `String` or boxed `java.lang.*` scalar.
    ///
    /// Arrays, objects (non-scalar), enums, blocks and resets return `None`.
    pub fn as_scalar<'a>(&'a self, value: &JValue) -> Result<Option<Scalar<'a>>> {
        let scalar = match value {
            JValue::Null => Scalar::Null,
            JValue::Bool(value) => Scalar::Bool(*value),
            JValue::Byte(value) => Scalar::I64(i64::from(*value)),
            JValue::Short(value) => Scalar::I64(i64::from(*value)),
            JValue::Int(value) => Scalar::I64(i64::from(*value)),
            JValue::Long(value) => Scalar::I64(*value),
            JValue::Float(value) => Scalar::F64(f64::from(*value)),
            JValue::Double(value) => Scalar::F64(*value),
            JValue::Char(value) => Scalar::Char(*value),
            JValue::Str(_) => Scalar::String(self.as_str(value)?),
            JValue::Obj(_) => match self.class_name(value)? {
                "java.lang.Boolean" => Scalar::Bool(self.as_bool(value)?),
                "java.lang.Byte" | "java.lang.Short" | "java.lang.Integer" | "java.lang.Long" => {
                    Scalar::I64(self.as_i64(value)?)
                }
                "java.lang.Float" | "java.lang.Double" => Scalar::F64(self.as_f64(value)?),
                "java.lang.Character" => Scalar::Char(self.as_char(value)?),
                _ => return Ok(None),
            },
            JValue::Arr(_) | JValue::Enum(_) | JValue::Block(_) | JValue::Reset => return Ok(None),
        };
        Ok(Some(scalar))
    }

    pub(crate) fn push_item(&mut self, value: JValue) {
        self.items.push(value);
    }

    pub(crate) fn insert_str(&mut self, value: String, long: bool) -> Handle {
        self.push_slot(Slot::Str { value, long })
    }

    pub(crate) fn insert_desc(&mut self, desc: ClassDesc) -> Handle {
        self.push_slot(Slot::Desc(desc))
    }

    pub(crate) fn set_desc_super(
        &mut self,
        handle: Handle,
        super_desc: Option<Handle>,
    ) -> Result<()> {
        match self.slots.get_mut(handle.0 as usize) {
            Some(Slot::Desc(desc)) => {
                desc.super_desc = super_desc;
                Ok(())
            }
            _ => Err(Error::BadHandle(handle.0)),
        }
    }

    pub(crate) fn set_desc_fields(&mut self, handle: Handle, fields: Vec<FieldDesc>) -> Result<()> {
        match self.slots.get_mut(handle.0 as usize) {
            Some(Slot::Desc(desc)) => {
                desc.fields = fields;
                Ok(())
            }
            _ => Err(Error::BadHandle(handle.0)),
        }
    }

    pub(crate) fn set_desc_annotations(
        &mut self,
        handle: Handle,
        annotations: Vec<JValue>,
    ) -> Result<()> {
        match self.slots.get_mut(handle.0 as usize) {
            Some(Slot::Desc(desc)) => {
                desc.annotations = annotations;
                Ok(())
            }
            _ => Err(Error::BadHandle(handle.0)),
        }
    }

    pub(crate) fn insert_array_placeholder(&mut self, desc: Handle) -> Handle {
        self.push_slot(Slot::Arr {
            desc,
            data: ArrayData::Values(Vec::new()),
        })
    }

    pub(crate) fn set_array_data(&mut self, handle: Handle, data: ArrayData) -> Result<()> {
        match self.slots.get_mut(handle.0 as usize) {
            Some(Slot::Arr { data: target, .. }) => {
                *target = data;
                Ok(())
            }
            _ => Err(Error::BadHandle(handle.0)),
        }
    }

    pub(crate) fn set_array_elements(&mut self, handle: Handle, elems: Vec<JValue>) -> Result<()> {
        self.set_array_data(handle, ArrayData::Values(elems))
    }

    pub(crate) fn set_array_primitive(
        &mut self,
        handle: Handle,
        typecode: FieldType,
        bytes: Vec<u8>,
    ) -> Result<()> {
        self.set_array_data(handle, ArrayData::Primitive { typecode, bytes })
    }

    pub(crate) fn insert_object_placeholder(&mut self) -> Handle {
        self.push_slot(Slot::Obj(JObject::default()))
    }

    pub(crate) fn set_object(&mut self, handle: Handle, object: JObject) -> Result<()> {
        match self.slots.get_mut(handle.0 as usize) {
            Some(Slot::Obj(target)) => {
                *target = object;
                Ok(())
            }
            _ => Err(Error::BadHandle(handle.0)),
        }
    }

    pub(crate) fn insert_enum_placeholder(&mut self, desc: Handle) -> Handle {
        self.push_slot(Slot::Enum {
            desc,
            constant: None,
        })
    }

    pub(crate) fn set_enum_constant(&mut self, handle: Handle, constant: Handle) -> Result<()> {
        match self.slots.get_mut(handle.0 as usize) {
            Some(Slot::Enum {
                constant: target, ..
            }) => {
                *target = Some(constant);
                Ok(())
            }
            _ => Err(Error::BadHandle(handle.0)),
        }
    }

    pub(crate) fn expect_desc(&self, handle: Handle) -> Result<&ClassDesc> {
        self.descriptor(handle)
    }

    pub(crate) fn value_for_handle(&self, handle: Handle) -> Result<JValue> {
        match self.slot(handle)? {
            Slot::Str { .. } => Ok(JValue::Str(handle)),
            Slot::Arr { .. } => Ok(JValue::Arr(handle)),
            Slot::Obj(_) => Ok(JValue::Obj(handle)),
            Slot::Enum { .. } => Ok(JValue::Enum(handle)),
            Slot::Desc(_) => Err(Error::FieldMismatch {
                message: "a class descriptor cannot be used as a value".to_owned(),
            }),
        }
    }

    pub(crate) fn string_value(&self, handle: Handle) -> Result<&str> {
        self.str(handle)
    }

    pub(crate) fn string_parts(&self, handle: Handle) -> Result<(&str, bool)> {
        match self.slots.get(handle.0 as usize) {
            Some(Slot::Str { value, long }) => Ok((value, *long)),
            _ => Err(Error::BadHandle(handle.0)),
        }
    }

    pub(crate) fn array_parts(&self, handle: Handle) -> Result<(Handle, &ArrayData)> {
        match self.slots.get(handle.0 as usize) {
            Some(Slot::Arr { desc, data }) => Ok((*desc, data)),
            _ => Err(Error::BadHandle(handle.0)),
        }
    }

    pub(crate) fn object_parts(&self, handle: Handle) -> Result<&JObject> {
        match self.slots.get(handle.0 as usize) {
            Some(Slot::Obj(object)) => Ok(object),
            _ => Err(Error::BadHandle(handle.0)),
        }
    }

    pub(crate) fn enum_parts(&self, handle: Handle) -> Result<(Handle, Handle)> {
        match self.slots.get(handle.0 as usize) {
            Some(Slot::Enum {
                desc,
                constant: Some(constant),
            }) => Ok((*desc, *constant)),
            Some(Slot::Enum { constant: None, .. }) => Err(Error::FieldMismatch {
                message: "enum constant was not read".to_owned(),
            }),
            _ => Err(Error::BadHandle(handle.0)),
        }
    }

    fn slot(&self, handle: Handle) -> Result<&Slot> {
        self.slots
            .get(handle.0 as usize)
            .ok_or(Error::BadHandle(handle.0))
    }

    fn push_slot(&mut self, slot: Slot) -> Handle {
        let handle = Handle(self.slots.len() as u32);
        self.slots.push(slot);
        handle
    }

    fn str(&self, handle: Handle) -> Result<&str> {
        match self.slots.get(handle.0 as usize) {
            Some(Slot::Str { value, .. }) => Ok(value),
            _ => Err(Error::BadHandle(handle.0)),
        }
    }
}

fn primitive_array_typecode(name: &str) -> Option<FieldType> {
    let bytes = name.as_bytes();
    if bytes.len() != 2 || bytes[0] != b'[' {
        return None;
    }
    let typecode = FieldType::from_code(bytes[1])?;
    typecode.is_primitive().then_some(typecode)
}

fn primitive_bytes(typecode: FieldType, elems: &[JValue]) -> Result<Vec<u8>> {
    let width = typecode.primitive_width().ok_or_else(|| {
        Error::InvalidSchema(format!("{typecode:?} is not a primitive array type"))
    })?;
    let capacity = elems
        .len()
        .checked_mul(width)
        .ok_or_else(|| Error::InvalidSchema("primitive array size overflows".to_owned()))?;
    let mut bytes = Vec::with_capacity(capacity);

    for elem in elems {
        match (typecode, elem) {
            (FieldType::Byte, JValue::Byte(value)) => bytes.push(*value as u8),
            (FieldType::Boolean, JValue::Bool(value)) => bytes.push(u8::from(*value)),
            (FieldType::Char, JValue::Char(value)) => bytes.extend_from_slice(&value.to_be_bytes()),
            (FieldType::Short, JValue::Short(value)) => {
                bytes.extend_from_slice(&value.to_be_bytes())
            }
            (FieldType::Int, JValue::Int(value)) => bytes.extend_from_slice(&value.to_be_bytes()),
            (FieldType::Long, JValue::Long(value)) => bytes.extend_from_slice(&value.to_be_bytes()),
            (FieldType::Float, JValue::Float(value)) => {
                bytes.extend_from_slice(&value.to_bits().to_be_bytes())
            }
            (FieldType::Double, JValue::Double(value)) => {
                bytes.extend_from_slice(&value.to_bits().to_be_bytes())
            }
            (typecode, value) => {
                return Err(Error::FieldMismatch {
                    message: format!("array of {typecode:?} cannot hold {value:?}"),
                })
            }
        }
    }

    Ok(bytes)
}
