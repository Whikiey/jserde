//! Graph encoder: the inverse of [`crate::reader`].
//!
//! The encoder is deliberately metadata-preserving: an untouched graph decoded
//! from a fixture must replay byte-for-byte (design §5.1), while caller-built or
//! mutated graphs are written with the same layout rules.

use std::collections::HashMap;

use crate::error::{Error, Result};
use crate::value::{ArrayData, BlockData, FieldType, Graph, Handle, JValue};
use crate::wire::handle::BASE_WIRE_HANDLE;
use crate::wire::mutf8;
use crate::wire::writer::StreamWriter;
use crate::wire::{
    STREAM_MAGIC, STREAM_VERSION, TC_ARRAY, TC_BLOCKDATA, TC_BLOCKDATALONG, TC_CLASSDESC,
    TC_ENDBLOCKDATA, TC_ENUM, TC_LONGSTRING, TC_NULL, TC_OBJECT, TC_REFERENCE, TC_RESET, TC_STRING,
};

/// Encode all top-level items of a graph with the Java stream header.
pub fn encode_stream(graph: &Graph) -> Result<Vec<u8>> {
    encode(graph, graph.items())
}

/// Encode the given top-level items with the Java stream header.
pub fn encode(graph: &Graph, items: &[JValue]) -> Result<Vec<u8>> {
    let mut encoder = Encoder::new(graph);
    encoder.write_header();
    for item in items {
        encoder.write_value(item)?;
    }
    Ok(encoder.finish())
}

struct Encoder<'a> {
    graph: &'a Graph,
    out: StreamWriter,
    handles: HashMap<Handle, u32>,
    next_wire: u32,
}

impl<'a> Encoder<'a> {
    fn new(graph: &'a Graph) -> Self {
        Self {
            graph,
            out: StreamWriter::new(),
            handles: HashMap::new(),
            next_wire: BASE_WIRE_HANDLE,
        }
    }

    fn finish(self) -> Vec<u8> {
        self.out.into_bytes()
    }

    fn write_header(&mut self) {
        self.out.write_u16(STREAM_MAGIC);
        self.out.write_u16(STREAM_VERSION);
    }

    fn assign(&mut self, handle: Handle) -> u32 {
        let wire = self.next_wire;
        self.next_wire = self.next_wire.wrapping_add(1);
        self.handles.insert(handle, wire);
        wire
    }

    fn assign_synthetic(&mut self) -> u32 {
        let wire = self.next_wire;
        self.next_wire = self.next_wire.wrapping_add(1);
        wire
    }

    fn reset_handles(&mut self) {
        self.handles.clear();
        self.next_wire = BASE_WIRE_HANDLE;
    }

    fn write_value(&mut self, value: &JValue) -> Result<()> {
        match value {
            JValue::Null => self.out.write_u8(TC_NULL),
            JValue::Bool(value) => self.out.write_u8(u8::from(*value)),
            JValue::Byte(value) => self.out.write_i8(*value),
            JValue::Char(value) => self.out.write_u16(*value),
            JValue::Short(value) => self.out.write_i16(*value),
            JValue::Int(value) => self.out.write_i32(*value),
            JValue::Long(value) => self.out.write_i64(*value),
            JValue::Float(value) => self.out.write_f32(*value),
            JValue::Double(value) => self.out.write_f64(*value),
            JValue::Str(handle) => self.write_string(*handle)?,
            JValue::Arr(handle) => self.write_array(*handle)?,
            JValue::Obj(handle) => self.write_object(*handle)?,
            JValue::Enum(handle) => self.write_enum(*handle)?,
            JValue::Block(block) => self.write_block(block)?,
            JValue::Reset => {
                self.out.write_u8(TC_RESET);
                self.reset_handles();
            }
        }
        Ok(())
    }

    fn write_string(&mut self, handle: Handle) -> Result<()> {
        if let Some(wire) = self.handles.get(&handle).copied() {
            self.out.write_u8(TC_REFERENCE);
            self.out.write_u32(wire);
            return Ok(());
        }

        let (bytes, long) = {
            let (text, long) = self.graph.string_parts(handle)?;
            (mutf8::encode(text), long)
        };

        if long {
            self.out.write_u8(TC_LONGSTRING);
            self.assign(handle);
            self.out.write_u64(bytes.len() as u64);
        } else {
            if bytes.len() > usize::from(u16::MAX) {
                return Err(Error::InvalidLength {
                    kind: "String",
                    value: bytes.len() as u64,
                });
            }
            self.out.write_u8(TC_STRING);
            self.assign(handle);
            self.out.write_u16(bytes.len() as u16);
        }
        self.out.write_bytes(&bytes);
        Ok(())
    }

    fn write_new_string(&mut self, text: &str) -> Result<()> {
        let bytes = mutf8::encode(text);
        if bytes.len() > usize::from(u16::MAX) {
            self.out.write_u8(TC_LONGSTRING);
            self.assign_synthetic();
            self.out.write_u64(bytes.len() as u64);
        } else {
            self.out.write_u8(TC_STRING);
            self.assign_synthetic();
            self.out.write_u16(bytes.len() as u16);
        }
        self.out.write_bytes(&bytes);
        Ok(())
    }

    fn write_block(&mut self, block: &BlockData) -> Result<()> {
        if block.long {
            let length = u32::try_from(block.bytes.len()).map_err(|_| Error::InvalidLength {
                kind: "block",
                value: block.bytes.len() as u64,
            })?;
            self.out.write_u8(TC_BLOCKDATALONG);
            self.out.write_u32(length);
        } else {
            let length = u8::try_from(block.bytes.len()).map_err(|_| Error::InvalidLength {
                kind: "block",
                value: block.bytes.len() as u64,
            })?;
            self.out.write_u8(TC_BLOCKDATA);
            self.out.write_u8(length);
        }
        self.out.write_bytes(&block.bytes);
        Ok(())
    }

    fn write_class_desc(&mut self, handle: Handle) -> Result<()> {
        if let Some(wire) = self.handles.get(&handle).copied() {
            self.out.write_u8(TC_REFERENCE);
            self.out.write_u32(wire);
            return Ok(());
        }

        let desc = self.graph.descriptor(handle)?.clone();
        self.out.write_u8(TC_CLASSDESC);

        // The JDK assigns the descriptor handle before writing its fields, so
        // `className1` strings get later wire handles (fixture 34).
        self.assign(handle);

        self.out.write_utf(&desc.name)?;
        self.out.write_i64(desc.suid);
        self.out.write_u8(desc.flags);
        let field_count = u16::try_from(desc.fields.len()).map_err(|_| {
            Error::InvalidSchema(format!("class `{}` has too many fields", desc.name))
        })?;
        self.out.write_u16(field_count);

        for field in &desc.fields {
            self.out.write_u8(field.typecode.code());
            self.out.write_utf(&field.name)?;

            if !field.typecode.is_primitive() {
                if let Some(class_name_handle) = field.class_name_handle {
                    self.write_string(class_name_handle)?;
                } else {
                    let class_name1 = field.class_name1.as_deref().ok_or_else(|| {
                        Error::InvalidSchema(format!(
                            "field `{}.{}` has no JVM descriptor",
                            desc.name, field.name
                        ))
                    })?;
                    self.write_new_string(class_name1)?;
                }
            }
        }

        for annotation in &desc.annotations {
            self.write_value(annotation)?;
        }
        self.out.write_u8(TC_ENDBLOCKDATA);

        match desc.super_desc {
            Some(super_desc) => self.write_class_desc(super_desc)?,
            None => self.out.write_u8(TC_NULL),
        }
        Ok(())
    }

    fn write_object(&mut self, handle: Handle) -> Result<()> {
        if let Some(wire) = self.handles.get(&handle).copied() {
            self.out.write_u8(TC_REFERENCE);
            self.out.write_u32(wire);
            return Ok(());
        }

        let object = self.graph.object_parts(handle)?.clone();
        let desc = object
            .classes
            .last()
            .ok_or_else(|| Error::FieldMismatch {
                message: "object has no class data".to_owned(),
            })?
            .desc;

        self.out.write_u8(TC_OBJECT);
        self.write_class_desc(desc)?;
        self.assign(handle);

        for class in &object.classes {
            let class_desc = self.graph.descriptor(class.desc)?.clone();
            if class_desc.fields.len() != class.fields.len() {
                return Err(Error::FieldMismatch {
                    message: format!(
                        "class `{}` has {} field description(s) but {} value(s)",
                        class_desc.name,
                        class_desc.fields.len(),
                        class.fields.len()
                    ),
                });
            }

            for (field, (_, value)) in class_desc.fields.iter().zip(&class.fields) {
                if field.typecode.is_primitive() {
                    self.write_primitive(field.typecode, value)?;
                } else {
                    self.write_value(value)?;
                }
            }

            if let Some(custom) = &class.custom {
                for item in custom {
                    self.write_value(item)?;
                }
                self.out.write_u8(TC_ENDBLOCKDATA);
            }
        }

        Ok(())
    }

    fn write_array(&mut self, handle: Handle) -> Result<()> {
        if let Some(wire) = self.handles.get(&handle).copied() {
            self.out.write_u8(TC_REFERENCE);
            self.out.write_u32(wire);
            return Ok(());
        }

        let graph = self.graph;
        let (desc, data) = graph.array_parts(handle)?;
        let name = graph.descriptor(desc)?.name.clone();
        let element = array_element(&name)?;

        let length = match (element, data) {
            (
                ArrayElement::Primitive(typecode),
                ArrayData::Primitive {
                    typecode: stored,
                    bytes,
                },
            ) => {
                if *stored != typecode {
                    return Err(Error::FieldMismatch {
                        message: format!(
                            "array `{name}` stores {stored:?} elements but declares {typecode:?}"
                        ),
                    });
                }
                let width = typecode
                    .primitive_width()
                    .ok_or_else(|| Error::FieldMismatch {
                        message: format!("`{name}` is not a primitive array"),
                    })?;
                if bytes.len() % width != 0 {
                    return Err(Error::FieldMismatch {
                        message: format!(
                            "array `{name}` has {} bytes, not a multiple of {width}",
                            bytes.len()
                        ),
                    });
                }
                bytes.len() / width
            }
            (ArrayElement::Object, ArrayData::Values(elems)) => elems.len(),
            _ => {
                return Err(Error::FieldMismatch {
                    message: format!("array `{name}` storage does not match its descriptor"),
                })
            }
        };

        self.out.write_u8(TC_ARRAY);
        self.write_class_desc(desc)?;
        self.assign(handle);
        let length = i32::try_from(length)
            .map_err(|_| Error::InvalidSchema(format!("array `{name}` has too many elements")))?;
        self.out.write_i32(length);

        match (element, data) {
            (ArrayElement::Primitive(_), ArrayData::Primitive { bytes, .. }) => {
                self.out.write_bytes(bytes);
            }
            (ArrayElement::Object, ArrayData::Values(elems)) => {
                for value in elems {
                    self.write_value(value)?;
                }
            }
            _ => {
                return Err(Error::FieldMismatch {
                    message: format!("array `{name}` storage does not match its descriptor"),
                })
            }
        }

        Ok(())
    }

    fn write_enum(&mut self, handle: Handle) -> Result<()> {
        if let Some(wire) = self.handles.get(&handle).copied() {
            self.out.write_u8(TC_REFERENCE);
            self.out.write_u32(wire);
            return Ok(());
        }

        let (desc, constant) = self.graph.enum_parts(handle)?;
        self.out.write_u8(TC_ENUM);
        self.write_class_desc(desc)?;
        self.assign(handle);
        self.write_string(constant)?;
        Ok(())
    }

    fn write_primitive(&mut self, typecode: FieldType, value: &JValue) -> Result<()> {
        match (typecode, value) {
            (FieldType::Byte, JValue::Byte(value)) => self.out.write_i8(*value),
            (FieldType::Char, JValue::Char(value)) => self.out.write_u16(*value),
            (FieldType::Double, JValue::Double(value)) => self.out.write_f64(*value),
            (FieldType::Float, JValue::Float(value)) => self.out.write_f32(*value),
            (FieldType::Int, JValue::Int(value)) => self.out.write_i32(*value),
            (FieldType::Long, JValue::Long(value)) => self.out.write_i64(*value),
            (FieldType::Short, JValue::Short(value)) => self.out.write_i16(*value),
            (FieldType::Boolean, JValue::Bool(value)) => self.out.write_u8(u8::from(*value)),
            (FieldType::Object | FieldType::Array, _) => {
                return Err(Error::FieldMismatch {
                    message: "object/array field cannot be written as a primitive".to_owned(),
                })
            }
            (typecode, value) => {
                return Err(Error::FieldMismatch {
                    message: format!("field of type {typecode:?} cannot hold {value:?}"),
                })
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
enum ArrayElement {
    Primitive(FieldType),
    Object,
}

fn array_element(name: &str) -> Result<ArrayElement> {
    let bytes = name.as_bytes();
    if bytes.first() != Some(&b'[') {
        return Err(Error::FieldMismatch {
            message: format!("`{name}` is not an array class"),
        });
    }

    if bytes.len() == 2 {
        if let Some(typecode) = FieldType::from_code(bytes[1]) {
            if typecode.is_primitive() {
                return Ok(ArrayElement::Primitive(typecode));
            }
        }
    }

    Ok(ArrayElement::Object)
}
