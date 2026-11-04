//! Top-level stream decoder.
//!
//! P1 handled the descriptor-free codec subset. P2 adds class descriptors,
//! objects (including inheritance and custom `writeObject` data), arrays,
//! enums, shared references, cycles and `TC_RESET`, with configurable limits.

use crate::allowlist::Allowlist;
use crate::error::{Error, Result};
use crate::value::{
    BlockData, ClassData, ClassDesc, FieldDesc, FieldType, Graph, Handle, JObject, JValue,
};
use crate::wire::handle::WireHandles;
use crate::wire::mutf8;
use crate::wire::reader::StreamReader;
use crate::wire::{
    STREAM_MAGIC, STREAM_VERSION, TC_ARRAY, TC_BLOCKDATA, TC_BLOCKDATALONG, TC_CLASS, TC_CLASSDESC,
    TC_ENDBLOCKDATA, TC_ENUM, TC_EXCEPTION, TC_LONGSTRING, TC_NULL, TC_OBJECT, TC_PROXYCLASSDESC,
    TC_REFERENCE, TC_RESET, TC_STRING,
};

/// Decoder resource limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Maximum nesting depth for objects/arrays/custom data.
    pub max_depth: usize,
    /// Maximum number of arena values, fields and collection elements.
    pub max_elements: usize,
    /// Maximum input size in bytes.
    pub max_stream_size: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_depth: 512,
            max_elements: 64 * 1024 * 1024,
            max_stream_size: 256 * 1024 * 1024,
        }
    }
}

/// Streaming decoder. Owns the arena while values are read.
#[derive(Debug)]
pub struct Reader<'a> {
    input: StreamReader<'a>,
    graph: Graph,
    handles: WireHandles,
    limits: Limits,
    allowlist: Allowlist,
    elements: usize,
}

impl<'a> Reader<'a> {
    /// Validate the stream header and create a reader with default limits.
    pub fn new(bytes: &'a [u8]) -> Result<Self> {
        Self::with_allowlist(bytes, Limits::default(), Allowlist::strict())
    }

    /// Validate the stream header with explicit limits and the strict allowlist.
    pub fn with_limits(bytes: &'a [u8], limits: Limits) -> Result<Self> {
        Self::with_allowlist(bytes, limits, Allowlist::strict())
    }

    /// Validate the stream header with explicit limits and an allowlist.
    pub fn with_allowlist(bytes: &'a [u8], limits: Limits, allowlist: Allowlist) -> Result<Self> {
        if bytes.len() > limits.max_stream_size {
            return Err(Error::LimitExceeded {
                kind: "stream size",
                limit: limits.max_stream_size,
                actual: bytes.len(),
            });
        }

        let mut input = StreamReader::new(bytes);

        let magic = input.read_u16()?;
        if magic != STREAM_MAGIC {
            return Err(Error::BadMagic { found: magic });
        }

        let version = input.read_u16()?;
        if version != STREAM_VERSION {
            return Err(Error::UnsupportedVersion { found: version });
        }

        Ok(Self {
            input,
            graph: Graph::default(),
            handles: WireHandles::new(),
            limits,
            allowlist,
            elements: 0,
        })
    }

    /// Read the next top-level element, or `None` at end of stream.
    pub fn read_next(&mut self) -> Result<Option<JValue>> {
        if self.input.is_empty() {
            return Ok(None);
        }

        let tag = self.input.read_u8()?;
        let value = self.read_content(tag, 0)?;
        self.graph.push_item(value.clone());
        Ok(Some(value))
    }

    /// Bulk decoder path: move the value into the arena instead of cloning it.
    /// Large top-level block-data items otherwise double the peak memory.
    fn read_next_in_place(&mut self) -> Result<bool> {
        if self.input.is_empty() {
            return Ok(false);
        }

        let tag = self.input.read_u8()?;
        let value = self.read_content(tag, 0)?;
        self.graph.push_item(value);
        Ok(true)
    }

    /// Arena accumulated so far.
    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    /// Consume the reader and return the accumulated arena.
    pub fn into_graph(self) -> Graph {
        self.graph
    }

    fn check_depth(&self, depth: usize) -> Result<()> {
        if depth > self.limits.max_depth {
            return Err(Error::LimitExceeded {
                kind: "depth",
                limit: self.limits.max_depth,
                actual: depth,
            });
        }
        Ok(())
    }

    fn charge(&mut self, count: usize) -> Result<()> {
        let total = self.elements.saturating_add(count);
        if total > self.limits.max_elements {
            return Err(Error::LimitExceeded {
                kind: "elements",
                limit: self.limits.max_elements,
                actual: total,
            });
        }
        self.elements = total;
        Ok(())
    }

    fn read_content(&mut self, tag: u8, depth: usize) -> Result<JValue> {
        self.check_depth(depth)?;
        self.charge(1)?;

        match tag {
            TC_NULL => Ok(JValue::Null),
            TC_STRING => self.read_string(false),
            TC_LONGSTRING => self.read_string(true),
            TC_REFERENCE => self.read_reference(),
            TC_OBJECT => self.read_object(depth),
            TC_ARRAY => self.read_array(depth),
            TC_ENUM => self.read_enum(depth),
            TC_BLOCKDATA => self.read_block(false),
            TC_BLOCKDATALONG => self.read_block(true),
            TC_RESET => {
                self.handles.reset();
                Ok(JValue::Reset)
            }
            TC_EXCEPTION => Err(self.read_exception(depth)?),
            TC_CLASS => self.read_class_value(depth),
            TC_PROXYCLASSDESC => Err(Error::UnsupportedClass("dynamic proxy".to_owned())),
            TC_CLASSDESC => Err(Error::FieldMismatch {
                message: "a class descriptor is not a stream value".to_owned(),
            }),
            TC_ENDBLOCKDATA => Err(Error::BadTag(tag)),
            _ => Err(Error::BadTag(tag)),
        }
    }

    /// Read `TC_CLASS`: a class object referencing a class descriptor.
    fn read_class_value(&mut self, depth: usize) -> Result<JValue> {
        let desc = self.read_class_desc_required(depth)?;
        let handle = self.graph.insert_class(desc)?;
        self.handles.assign(handle);
        Ok(JValue::Class(handle))
    }

    fn read_exception(&mut self, depth: usize) -> Result<Error> {
        let tag = self.input.read_u8()?;
        let value = self.read_content(tag, depth + 1)?;
        let class_name = self
            .graph
            .class_name(&value)
            .unwrap_or("unknown exception class");
        Ok(Error::UnsupportedClass(class_name.to_owned()))
    }

    fn read_string(&mut self, long: bool) -> Result<JValue> {
        let length = if long {
            let value = self.input.read_u64()?;
            usize::try_from(value).map_err(|_| Error::InvalidLength {
                kind: "String",
                value,
            })?
        } else {
            usize::from(self.input.read_u16()?)
        };

        let offset = self.input.position();
        let bytes = self.input.read_exact(length)?;
        let value = mutf8::decode_at(bytes, offset)?;

        let handle = self.graph.insert_str(value, long);
        self.handles.assign(handle);
        Ok(JValue::Str(handle))
    }

    fn read_utf(&mut self) -> Result<String> {
        let length = usize::from(self.input.read_u16()?);
        let offset = self.input.position();
        let bytes = self.input.read_exact(length)?;
        mutf8::decode_at(bytes, offset)
    }

    fn read_reference(&mut self) -> Result<JValue> {
        let wire = self.input.read_u32()?;
        let handle = self.handles.get(wire).ok_or(Error::BadHandle(wire))?;
        self.graph.value_for_handle(handle)
    }

    fn read_block(&mut self, long: bool) -> Result<JValue> {
        let length = if long {
            let value = self.input.read_u32()?;
            usize::try_from(value).map_err(|_| Error::InvalidLength {
                kind: "block",
                value: u64::from(value),
            })?
        } else {
            usize::from(self.input.read_u8()?)
        };

        let bytes = self.input.read_exact(length)?.to_vec();
        Ok(JValue::Block(BlockData { bytes, long }))
    }

    /// Read a `TC_CLASSDESC` (or a `TC_REFERENCE` to one).
    fn read_class_desc_required(&mut self, depth: usize) -> Result<Handle> {
        let tag = self.input.read_u8()?;
        match tag {
            TC_CLASSDESC => self.read_class_desc_chain(depth),
            TC_REFERENCE => self.read_desc_reference(),
            TC_NULL => Err(Error::FieldMismatch {
                message: "object/array/enum has a null class descriptor".to_owned(),
            }),
            _ => Err(Error::BadTag(tag)),
        }
    }

    /// Parse a descriptor node and its superclass chain.
    ///
    /// The JDK assigns the descriptor's wire handle after the class's own
    /// fields/annotation and before the superclass descriptor. The loop below
    /// preserves that order, which matters for `TC_REFERENCE` targets.
    fn read_class_desc_chain(&mut self, depth: usize) -> Result<Handle> {
        let first = self.read_class_desc_node(depth)?;
        let mut current = first;
        let mut chain_len = 1usize;

        loop {
            let tag = self.input.read_u8()?;
            match tag {
                TC_NULL => break,
                TC_REFERENCE => {
                    let super_desc = self.read_desc_reference()?;
                    self.graph.set_desc_super(current, Some(super_desc))?;
                    break;
                }
                TC_CLASSDESC => {
                    let super_desc = self.read_class_desc_node(depth)?;
                    self.graph.set_desc_super(current, Some(super_desc))?;
                    current = super_desc;
                    chain_len += 1;
                    if chain_len > self.limits.max_depth {
                        return Err(Error::LimitExceeded {
                            kind: "class hierarchy depth",
                            limit: self.limits.max_depth,
                            actual: chain_len,
                        });
                    }
                }
                TC_PROXYCLASSDESC => {
                    return Err(Error::UnsupportedClass("dynamic proxy".to_owned()))
                }
                _ => return Err(Error::BadTag(tag)),
            }
        }

        Ok(first)
    }

    /// Parse one `TC_CLASSDESC` node (the tag has already been consumed).
    fn read_class_desc_node(&mut self, depth: usize) -> Result<Handle> {
        let name = self.read_utf()?;
        self.allowlist.check(&name)?;
        let suid = self.input.read_i64()?;
        let flags = self.input.read_u8()?;
        let field_count = usize::from(self.input.read_u16()?);
        self.charge(field_count)?;

        // The JDK assigns the descriptor's wire handle before writing its
        // fields, so `className1` strings get later handles. `34` references
        // the Node descriptor as 0x7E0000 even though it contains two
        // `Ljava/lang/String;` field descriptors.
        let handle = self.graph.insert_desc(ClassDesc {
            name,
            suid,
            flags,
            fields: Vec::new(),
            super_desc: None,
            annotations: Vec::new(),
        });
        self.handles.assign(handle);

        let mut fields = Vec::with_capacity(field_count);
        for _ in 0..field_count {
            let code = self.input.read_u8()?;
            let typecode = FieldType::from_code(code).ok_or(Error::BadTag(code))?;
            let name = self.read_utf()?;
            let (class_name1, class_name_handle) = if typecode.is_primitive() {
                (None, None)
            } else {
                let handle = self.read_string_handle()?;
                let class_name1 = self.graph.string_value(handle)?.to_owned();
                (Some(class_name1), Some(handle))
            };
            fields.push(FieldDesc {
                name,
                typecode,
                class_name1,
                class_name_handle,
            });
        }
        self.graph.set_desc_fields(handle, fields)?;

        let annotations = self.read_class_annotation(depth)?;
        self.graph.set_desc_annotations(handle, annotations)?;
        Ok(handle)
    }

    fn read_class_annotation(&mut self, depth: usize) -> Result<Vec<JValue>> {
        let mut contents = Vec::new();
        loop {
            let tag = self.input.read_u8()?;
            if tag == TC_ENDBLOCKDATA {
                return Ok(contents);
            }
            contents.push(self.read_content(tag, depth + 1)?);
        }
    }

    fn read_desc_reference(&mut self) -> Result<Handle> {
        let wire = self.input.read_u32()?;
        let handle = self.handles.get(wire).ok_or(Error::BadHandle(wire))?;
        self.graph.expect_desc(handle)?;
        Ok(handle)
    }

    fn read_string_handle(&mut self) -> Result<Handle> {
        let tag = self.input.read_u8()?;
        self.read_string_handle_with_tag(tag)
    }

    fn read_string_handle_with_tag(&mut self, tag: u8) -> Result<Handle> {
        let value = match tag {
            TC_STRING => self.read_string(false)?,
            TC_LONGSTRING => self.read_string(true)?,
            TC_REFERENCE => self.read_reference()?,
            _ => return Err(Error::BadTag(tag)),
        };

        match value {
            JValue::Str(handle) => Ok(handle),
            _ => Err(Error::FieldMismatch {
                message: "expected a Java String reference".to_owned(),
            }),
        }
    }

    fn read_object(&mut self, depth: usize) -> Result<JValue> {
        let desc = self.read_class_desc_required(depth)?;
        let handle = self.graph.insert_object_placeholder();
        self.handles.assign(handle);

        let mut layers = Vec::new();
        let mut current = Some(desc);
        while let Some(handle) = current {
            layers.push(handle);
            if layers.len() > self.limits.max_depth {
                return Err(Error::LimitExceeded {
                    kind: "class hierarchy depth",
                    limit: self.limits.max_depth,
                    actual: layers.len(),
                });
            }
            current = self.graph.descriptor(handle)?.super_desc;
        }
        layers.reverse();

        let mut classes = Vec::with_capacity(layers.len());
        for layer in layers {
            let desc = self.graph.descriptor(layer)?.clone();
            self.charge(desc.fields.len())?;

            let mut fields = Vec::with_capacity(desc.fields.len());
            for field in &desc.fields {
                let value = if field.typecode.is_primitive() {
                    self.read_primitive(field.typecode)?
                } else {
                    let tag = self.input.read_u8()?;
                    self.read_content(tag, depth + 1)?
                };
                fields.push((field.name.clone(), value));
            }

            let custom = if desc.has_custom_data() {
                Some(self.read_custom_data(depth + 1)?)
            } else if desc.is_externalizable() || (!desc.is_serializable() && !desc.is_enum()) {
                // An old-style Externalizable has no block boundary we can
                // safely skip; a non-serializable descriptor is equally opaque.
                return Err(Error::UnsupportedClass(desc.name));
            } else {
                None
            };

            classes.push(ClassData {
                desc: layer,
                fields,
                custom,
            });
        }

        self.graph.set_object(handle, JObject { classes })?;
        Ok(JValue::Obj(handle))
    }

    fn read_custom_data(&mut self, depth: usize) -> Result<Vec<JValue>> {
        let mut contents = Vec::new();
        loop {
            let tag = self.input.read_u8()?;
            if tag == TC_ENDBLOCKDATA {
                return Ok(contents);
            }
            contents.push(self.read_content(tag, depth)?);
        }
    }

    fn read_primitive(&mut self, typecode: FieldType) -> Result<JValue> {
        match typecode {
            FieldType::Byte => Ok(JValue::Byte(self.input.read_i8()?)),
            FieldType::Char => Ok(JValue::Char(self.input.read_u16()?)),
            FieldType::Double => Ok(JValue::Double(self.input.read_f64()?)),
            FieldType::Float => Ok(JValue::Float(self.input.read_f32()?)),
            FieldType::Int => Ok(JValue::Int(self.input.read_i32()?)),
            FieldType::Long => Ok(JValue::Long(self.input.read_i64()?)),
            FieldType::Short => Ok(JValue::Short(self.input.read_i16()?)),
            FieldType::Boolean => Ok(JValue::Bool(self.input.read_bool()?)),
            FieldType::Object | FieldType::Array => Err(Error::FieldMismatch {
                message: "object/array field cannot be read as a primitive".to_owned(),
            }),
        }
    }

    fn read_array(&mut self, depth: usize) -> Result<JValue> {
        let desc = self.read_class_desc_required(depth)?;
        let length = self.input.read_i32()?;
        let length = usize::try_from(length).map_err(|_| Error::InvalidLength {
            kind: "array",
            value: length as u64,
        })?;
        self.charge(length)?;

        let handle = self.graph.insert_array_placeholder(desc);
        self.handles.assign(handle);

        let name = self.graph.descriptor(desc)?.name.clone();
        let element = array_element(&name)?;

        match element {
            ArrayElement::Primitive(typecode) => {
                let width = typecode
                    .primitive_width()
                    .ok_or_else(|| Error::FieldMismatch {
                        message: format!("`{name}` is not a primitive array"),
                    })?;
                let total = length.checked_mul(width).ok_or(Error::InvalidLength {
                    kind: "array",
                    value: length as u64,
                })?;
                let bytes = self.input.read_exact(total)?.to_vec();
                self.graph.set_array_primitive(handle, typecode, bytes)?;
            }
            ArrayElement::Object => {
                let mut elems = Vec::with_capacity(length.min(4096));
                for _ in 0..length {
                    let tag = self.input.read_u8()?;
                    elems.push(self.read_content(tag, depth + 1)?);
                }
                self.graph.set_array_elements(handle, elems)?;
            }
        }

        Ok(JValue::Arr(handle))
    }

    fn read_enum(&mut self, depth: usize) -> Result<JValue> {
        let desc = self.read_class_desc_required(depth)?;
        let handle = self.graph.insert_enum_placeholder(desc);
        self.handles.assign(handle);

        let tag = self.input.read_u8()?;
        let constant = self.read_string_handle_with_tag(tag)?;
        self.graph.set_enum_constant(handle, constant)?;
        Ok(JValue::Enum(handle))
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

/// Decode every top-level element into one arena with default limits.
pub fn decode_stream(bytes: &[u8]) -> Result<Graph> {
    decode_stream_with_limits(bytes, Limits::default())
}

/// Decode every top-level element into one arena with explicit limits.
pub fn decode_stream_with_limits(bytes: &[u8], limits: Limits) -> Result<Graph> {
    decode_stream_with_options(bytes, limits, Allowlist::strict())
}

/// Decode every top-level element with default limits and an explicit allowlist.
pub fn decode_stream_with_allowlist(bytes: &[u8], allowlist: &Allowlist) -> Result<Graph> {
    decode_stream_with_options(bytes, Limits::default(), allowlist.clone())
}

/// Decode every top-level element with explicit limits and allowlist.
pub fn decode_stream_with_options(
    bytes: &[u8],
    limits: Limits,
    allowlist: Allowlist,
) -> Result<Graph> {
    let mut reader = Reader::with_allowlist(bytes, limits, allowlist)?;
    while reader.read_next_in_place()? {}
    Ok(reader.into_graph())
}

/// Decode a stream that contains exactly one non-reset element.
pub fn decode(bytes: &[u8]) -> Result<Graph> {
    decode_with_limits(bytes, Limits::default())
}

/// Decode a single-element stream with explicit limits.
pub fn decode_with_limits(bytes: &[u8], limits: Limits) -> Result<Graph> {
    let graph = decode_stream_with_limits(bytes, limits)?;
    let items = graph.items();

    if items.len() != 1 || matches!(&items[0], JValue::Reset) {
        return Err(Error::FieldMismatch {
            message: "decode expects exactly one value; use decode_stream".to_owned(),
        });
    }

    Ok(graph)
}

/// Decode a single-element stream with an explicit allowlist.
pub fn decode_with_allowlist(bytes: &[u8], allowlist: &Allowlist) -> Result<Graph> {
    let graph = decode_stream_with_allowlist(bytes, allowlist)?;
    let items = graph.items();

    if items.len() != 1 || matches!(&items[0], JValue::Reset) {
        return Err(Error::FieldMismatch {
            message: "decode expects exactly one value; use decode_stream".to_owned(),
        });
    }

    Ok(graph)
}
