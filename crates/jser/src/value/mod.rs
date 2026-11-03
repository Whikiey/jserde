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

#[derive(Debug)]
enum Slot {
    Desc(ClassDesc),
    Str {
        value: String,
        long: bool,
    },
    Arr {
        desc: Handle,
        elems: Vec<JValue>,
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
            Slot::Arr { elems, .. } => Ok(Ref::Arr(elems)),
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

    /// The elements of an `Arr` value, or `None` for other value kinds.
    pub fn as_array(&self, value: &JValue) -> Result<Option<&[JValue]>> {
        let JValue::Arr(handle) = value else {
            return Ok(None);
        };

        match self.slot(*handle)? {
            Slot::Arr { elems, .. } => Ok(Some(elems)),
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
            elems: Vec::new(),
        })
    }

    pub(crate) fn set_array_elements(&mut self, handle: Handle, elems: Vec<JValue>) -> Result<()> {
        match self.slots.get_mut(handle.0 as usize) {
            Some(Slot::Arr { elems: target, .. }) => {
                *target = elems;
                Ok(())
            }
            _ => Err(Error::BadHandle(handle.0)),
        }
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
