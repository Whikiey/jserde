//! Direct serde ↔ [`JValue`] bridge (no JSON intermediate).
//!
//! The serializer writes schema-typed values straight into a [`Graph`]; the
//! deserializer reads a decoded value using its schema. Only the features the
//! JSON bridge supported are implemented: scalars, `String`, `Option`, arrays,
//! nested POJOs, maps, sets and enums.

use std::fmt;

use jser::builtin::{
    new_hash_map_entries, new_hash_set, new_java_array, new_linked_hash_map_entries,
};
use jser::{ClassData, ClassDesc, Error, FieldDesc, FieldType, Graph, JValue};
use serde::de::{
    self, DeserializeOwned, DeserializeSeed, EnumAccess, MapAccess, SeqAccess, VariantAccess,
    Visitor,
};
use serde::ser::{self, SerializeMap, SerializeSeq, SerializeStruct};
use serde::Serialize as SerializeTrait;

use crate::shared;
use crate::{EnumSchema, JavaSchema, MapSchema, SchemaField, SchemaValue};

/// serde error wrapper around [`jser::Error`] (the orphan rule prevents
/// implementing serde traits directly on `jser::Error`).
#[derive(Debug)]
pub(crate) struct SerdeError(pub Error);

impl fmt::Display for SerdeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

impl std::error::Error for SerdeError {}

impl ser::Error for SerdeError {
    fn custom<T: fmt::Display>(message: T) -> Self {
        Self(Error::FieldMismatch {
            message: message.to_string(),
        })
    }
}

impl de::Error for SerdeError {
    fn custom<T: fmt::Display>(message: T) -> Self {
        Self(Error::FieldMismatch {
            message: message.to_string(),
        })
    }
}

impl From<Error> for SerdeError {
    fn from(error: Error) -> Self {
        Self(error)
    }
}

impl From<SerdeError> for Error {
    fn from(error: SerdeError) -> Self {
        error.0
    }
}

fn err(message: impl fmt::Display) -> SerdeError {
    SerdeError(Error::FieldMismatch {
        message: message.to_string(),
    })
}

/// Schema context for one value.
#[derive(Clone)]
pub(crate) enum Ctx {
    Object(JavaSchema),
    Field(SchemaField),
    Value(SchemaValue),
    Map { map: MapSchema, linked: bool },
    Set(SchemaValue),
    Enum(EnumSchema),
}

impl Ctx {
    fn typecode(&self) -> Option<FieldType> {
        match self {
            Self::Field(field) => Some(field.typecode),
            Self::Value(value) => Some(value.typecode),
            _ => None,
        }
    }

    fn nested(&self) -> Option<JavaSchema> {
        match self {
            Self::Object(schema) => Some(schema.clone()),
            Self::Field(field) => field.nested.as_deref().cloned(),
            Self::Value(value) => value.nested.as_deref().cloned(),
            _ => None,
        }
    }

    fn enumeration(&self) -> Option<EnumSchema> {
        match self {
            Self::Field(field) => field.enumeration.as_deref().cloned(),
            Self::Enum(schema) => Some(schema.clone()),
            _ => None,
        }
    }

    fn map(&self) -> Option<(MapSchema, bool)> {
        match self {
            Self::Map { map, linked } => Some((map.clone(), *linked)),
            Self::Field(field) => field.map.as_deref().map(|map| {
                (
                    map.clone(),
                    field.class_name1 == Some(crate::LINKED_HASH_MAP),
                )
            }),
            _ => None,
        }
    }

    fn seq_kind(&self) -> Result<SeqKind, SerdeError> {
        match self {
            Self::Set(element) => Ok(SeqKind::Set(element.clone())),
            Self::Field(field) => {
                if let Some(set) = &field.set {
                    return Ok(SeqKind::Set(set.as_ref().clone()));
                }
                if field.typecode == FieldType::Array {
                    let class = field
                        .class_name1
                        .ok_or_else(|| err("array field has no className1"))?;
                    return Ok(SeqKind::Array {
                        class_name1: class,
                        element: array_element(class)?,
                    });
                }
                Err(err("schema field is not an array or set"))
            }
            Self::Value(value) if value.typecode == FieldType::Array => {
                let class = value
                    .class_name1
                    .ok_or_else(|| err("array value has no className1"))?;
                Ok(SeqKind::Array {
                    class_name1: class,
                    element: array_element(class)?,
                })
            }
            _ => Err(err("schema value is not an array or set")),
        }
    }
}

fn array_element(class_name1: &'static str) -> Result<SchemaValue, SerdeError> {
    let bytes = class_name1.as_bytes();
    if bytes.len() == 2 && bytes[0] == b'[' {
        if let Some(typecode) = FieldType::from_code(bytes[1]) {
            if typecode.is_primitive() {
                return Ok(SchemaValue {
                    typecode,
                    class_name1: None,
                    nested: None,
                });
            }
        }
    }
    if class_name1 == "[Ljava.lang.String;" {
        return Ok(SchemaValue {
            typecode: FieldType::Object,
            class_name1: Some("Ljava/lang/String;"),
            nested: None,
        });
    }
    Err(err(format!(
        "serde bridge does not support array element `{class_name1}`"
    )))
}

fn field_descriptor(field: &SchemaField) -> FieldDesc {
    FieldDesc {
        name: field.name.to_owned(),
        typecode: field.typecode,
        class_name1: field.class_name1.map(str::to_owned).or_else(|| {
            field
                .enumeration
                .as_ref()
                .map(|schema| format!("L{};", schema.class.replace('.', "/")))
        }),
        class_name_handle: None,
    }
}

#[derive(Clone)]
enum SeqKind {
    Array {
        class_name1: &'static str,
        element: SchemaValue,
    },
    Set(SchemaValue),
}

// ============================== serializer ==============================

pub(crate) fn to_jvalue<T: SerializeTrait + ?Sized>(
    value: &T,
    ctx: Ctx,
    graph: &mut Graph,
) -> Result<JValue, SerdeError> {
    shared::begin_encode();
    value.serialize(DirectSerializer { graph, ctx })
}

pub(crate) struct DirectSerializer<'a> {
    graph: &'a mut Graph,
    ctx: Ctx,
}

impl<'a> DirectSerializer<'a> {
    fn integer(self, value: i128) -> Result<JValue, SerdeError> {
        let typecode = self
            .ctx
            .typecode()
            .ok_or_else(|| err("schema context is not a scalar"))?;
        let range = |name: &str| err(format!("{value} does not fit in {name}"));
        Ok(match typecode {
            FieldType::Byte => JValue::Byte(i8::try_from(value).map_err(|_| range("byte"))?),
            FieldType::Short => JValue::Short(i16::try_from(value).map_err(|_| range("short"))?),
            FieldType::Int => JValue::Int(i32::try_from(value).map_err(|_| range("int"))?),
            FieldType::Long => JValue::Long(i64::try_from(value).map_err(|_| range("long"))?),
            _ => return Err(err("schema does not accept an integer")),
        })
    }

    fn float(self, value: f64, want_f32: bool) -> Result<JValue, SerdeError> {
        match self.ctx.typecode() {
            Some(FieldType::Float) if want_f32 => Ok(JValue::Float(value as f32)),
            Some(FieldType::Double) if !want_f32 => Ok(JValue::Double(value)),
            _ => Err(err("schema does not accept this float type")),
        }
    }

    fn string(self, value: &str) -> Result<JValue, SerdeError> {
        let is_string = match &self.ctx {
            Ctx::Field(field) => {
                field.typecode == FieldType::Object
                    && field.class_name1 == Some("Ljava/lang/String;")
            }
            Ctx::Value(value) => {
                value.typecode == FieldType::Object
                    && value.class_name1 == Some("Ljava/lang/String;")
            }
            _ => false,
        };
        if !is_string {
            return Err(err("schema does not accept a String"));
        }
        Ok(JValue::Str(self.graph.new_string(value)))
    }

    fn unit_variant(self, variant: &str) -> Result<JValue, SerdeError> {
        let schema = self
            .ctx
            .enumeration()
            .ok_or_else(|| err("enum value but schema is not an enum"))?;
        let java_name = schema.java_name(variant)?;
        let enum_desc = self.graph.add_class_desc(ClassDesc {
            name: "java.lang.Enum".to_owned(),
            suid: 0,
            flags: 0x12,
            fields: Vec::new(),
            super_desc: None,
            annotations: Vec::new(),
            proxy_interfaces: None,
        })?;
        let desc = self.graph.add_class_desc(ClassDesc {
            name: schema.class.to_owned(),
            suid: 0,
            flags: 0x12,
            fields: Vec::new(),
            super_desc: Some(enum_desc),
            annotations: Vec::new(),
            proxy_interfaces: None,
        })?;
        let constant = self.graph.new_string(java_name);
        Ok(JValue::Enum(self.graph.new_enum(desc, constant)?))
    }
}

impl<'a> ser::Serializer for DirectSerializer<'a> {
    type Ok = JValue;
    type Error = SerdeError;
    type SerializeSeq = SeqSerializer<'a>;
    type SerializeTuple = ser::Impossible<JValue, SerdeError>;
    type SerializeTupleStruct = ser::Impossible<JValue, SerdeError>;
    type SerializeTupleVariant = ser::Impossible<JValue, SerdeError>;
    type SerializeMap = MapSerializer<'a>;
    type SerializeStruct = StructSerializer<'a>;
    type SerializeStructVariant = ser::Impossible<JValue, SerdeError>;

    fn serialize_bool(self, value: bool) -> Result<JValue, SerdeError> {
        match self.ctx.typecode() {
            Some(FieldType::Boolean) => Ok(JValue::Bool(value)),
            _ => Err(err("schema does not accept a boolean")),
        }
    }

    fn serialize_i8(self, value: i8) -> Result<JValue, SerdeError> {
        self.integer(i128::from(value))
    }
    fn serialize_i16(self, value: i16) -> Result<JValue, SerdeError> {
        self.integer(i128::from(value))
    }
    fn serialize_i32(self, value: i32) -> Result<JValue, SerdeError> {
        self.integer(i128::from(value))
    }
    fn serialize_i64(self, value: i64) -> Result<JValue, SerdeError> {
        self.integer(i128::from(value))
    }
    fn serialize_i128(self, value: i128) -> Result<JValue, SerdeError> {
        self.integer(value)
    }
    fn serialize_u8(self, value: u8) -> Result<JValue, SerdeError> {
        self.integer(i128::from(value))
    }
    fn serialize_u16(self, value: u16) -> Result<JValue, SerdeError> {
        self.integer(i128::from(value))
    }
    fn serialize_u32(self, value: u32) -> Result<JValue, SerdeError> {
        self.integer(i128::from(value))
    }
    fn serialize_u64(self, value: u64) -> Result<JValue, SerdeError> {
        self.integer(i128::from(value))
    }
    fn serialize_u128(self, value: u128) -> Result<JValue, SerdeError> {
        i128::try_from(value)
            .map_err(|_| err("u128 value out of range"))
            .and_then(|value| self.integer(value))
    }

    fn serialize_f32(self, value: f32) -> Result<JValue, SerdeError> {
        self.float(f64::from(value), true)
    }
    fn serialize_f64(self, value: f64) -> Result<JValue, SerdeError> {
        self.float(value, false)
    }

    fn serialize_char(self, value: char) -> Result<JValue, SerdeError> {
        match self.ctx.typecode() {
            Some(FieldType::Char) => Ok(JValue::Char(value as u16)),
            _ => Err(err("schema does not accept a char")),
        }
    }

    fn serialize_str(self, value: &str) -> Result<JValue, SerdeError> {
        self.string(value)
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<JValue, SerdeError> {
        let class = match &self.ctx {
            Ctx::Field(field) => field.class_name1,
            Ctx::Value(value) => value.class_name1,
            _ => None,
        };
        if class != Some("[B") {
            return Err(err("schema does not accept bytes"));
        }
        let elements = value.iter().map(|byte| JValue::Byte(*byte as i8)).collect();
        Ok(JValue::Arr(new_java_array(self.graph, "[B", elements)?))
    }

    fn serialize_none(self) -> Result<JValue, SerdeError> {
        Ok(JValue::Null)
    }

    fn serialize_some<T: SerializeTrait + ?Sized>(self, value: &T) -> Result<JValue, SerdeError> {
        value.serialize(DirectSerializer {
            graph: self.graph,
            ctx: self.ctx,
        })
    }

    fn serialize_unit(self) -> Result<JValue, SerdeError> {
        Err(err("unit values are not supported"))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<JValue, SerdeError> {
        Err(err("unit structs are not supported"))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<JValue, SerdeError> {
        self.unit_variant(variant)
    }

    fn serialize_newtype_struct<T: SerializeTrait + ?Sized>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<JValue, SerdeError> {
        if name == "__jser_shared" {
            let pointer = shared::current_pointer()
                .ok_or_else(|| err("Shared<T> is only supported inside the schema bridge"))?;
            if let Some(value) = shared::encoded(pointer) {
                return Ok(value);
            }
            let value = value.serialize(DirectSerializer {
                graph: self.graph,
                ctx: self.ctx,
            })?;
            if shared::handle_of(&value).is_none() {
                return Err(err("Shared<T> must contain a reference value"));
            }
            shared::remember_encoded(pointer, value.clone());
            return Ok(value);
        }
        value.serialize(DirectSerializer {
            graph: self.graph,
            ctx: self.ctx,
        })
    }

    fn serialize_newtype_variant<T: SerializeTrait + ?Sized>(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<JValue, SerdeError> {
        Err(err("enum variants with data are not supported"))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<SeqSerializer<'a>, SerdeError> {
        let kind = self.ctx.seq_kind()?;
        Ok(SeqSerializer {
            graph: self.graph,
            kind,
            values: Vec::new(),
        })
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, SerdeError> {
        Err(err("tuples are not supported"))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, SerdeError> {
        Err(err("tuple structs are not supported"))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, SerdeError> {
        Err(err("enum variants with data are not supported"))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<MapSerializer<'a>, SerdeError> {
        let (map, linked) = self
            .ctx
            .map()
            .ok_or_else(|| err("schema context is not a map"))?;
        Ok(MapSerializer {
            graph: self.graph,
            key: map.key,
            value: map.value,
            linked,
            entries: Vec::new(),
            pending_key: None,
        })
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<StructSerializer<'a>, SerdeError> {
        let schema = self
            .ctx
            .nested()
            .ok_or_else(|| err("schema context is not a nested object"))?;
        Ok(StructSerializer {
            graph: self.graph,
            schema,
            fields: Vec::new(),
        })
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, SerdeError> {
        Err(err("enum variants with data are not supported"))
    }
}

pub(crate) struct SeqSerializer<'a> {
    graph: &'a mut Graph,
    kind: SeqKind,
    values: Vec<JValue>,
}

impl<'a> SerializeSeq for SeqSerializer<'a> {
    type Ok = JValue;
    type Error = SerdeError;

    fn serialize_element<T: SerializeTrait + ?Sized>(
        &mut self,
        value: &T,
    ) -> Result<(), SerdeError> {
        let element = match &self.kind {
            SeqKind::Array { element, .. } | SeqKind::Set(element) => element.clone(),
        };
        let jvalue = value.serialize(DirectSerializer {
            graph: &mut *self.graph,
            ctx: Ctx::Value(element),
        })?;
        self.values.push(jvalue);
        Ok(())
    }

    fn end(self) -> Result<JValue, SerdeError> {
        match self.kind {
            SeqKind::Set(_) => Ok(JValue::Obj(new_hash_set(self.graph, self.values)?)),
            SeqKind::Array { class_name1, .. } => Ok(JValue::Arr(new_java_array(
                self.graph,
                class_name1,
                self.values,
            )?)),
        }
    }
}

pub(crate) struct MapSerializer<'a> {
    graph: &'a mut Graph,
    key: SchemaValue,
    value: SchemaValue,
    linked: bool,
    entries: Vec<(JValue, JValue)>,
    pending_key: Option<JValue>,
}

impl<'a> SerializeMap for MapSerializer<'a> {
    type Ok = JValue;
    type Error = SerdeError;

    fn serialize_key<T: SerializeTrait + ?Sized>(&mut self, key: &T) -> Result<(), SerdeError> {
        let jvalue = key.serialize(DirectSerializer {
            graph: &mut *self.graph,
            ctx: Ctx::Value(self.key.clone()),
        })?;
        self.pending_key = Some(jvalue);
        Ok(())
    }

    fn serialize_value<T: SerializeTrait + ?Sized>(&mut self, value: &T) -> Result<(), SerdeError> {
        let key = self
            .pending_key
            .take()
            .ok_or_else(|| err("map value without a key"))?;
        let jvalue = value.serialize(DirectSerializer {
            graph: &mut *self.graph,
            ctx: Ctx::Value(self.value.clone()),
        })?;
        self.entries.push((key, jvalue));
        Ok(())
    }

    fn end(self) -> Result<JValue, SerdeError> {
        if self.pending_key.is_some() {
            return Err(err("map key without a value"));
        }
        let handle = if self.linked {
            new_linked_hash_map_entries(self.graph, self.entries, false)?
        } else {
            new_hash_map_entries(self.graph, self.entries)?
        };
        Ok(JValue::Obj(handle))
    }
}

pub(crate) struct StructSerializer<'a> {
    graph: &'a mut Graph,
    schema: JavaSchema,
    fields: Vec<(String, JValue)>,
}

impl<'a> SerializeStruct for StructSerializer<'a> {
    type Ok = JValue;
    type Error = SerdeError;

    fn serialize_field<T: SerializeTrait + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), SerdeError> {
        let field = self
            .schema
            .fields
            .iter()
            .find(|field| field.name == key)
            .cloned()
            .ok_or_else(|| err(format!("unknown field `{key}`")))?;
        let jvalue = value.serialize(DirectSerializer {
            graph: &mut *self.graph,
            ctx: Ctx::Field(field),
        })?;
        self.fields.push((key.to_owned(), jvalue));
        Ok(())
    }

    fn end(self) -> Result<JValue, SerdeError> {
        let mut ordered = Vec::with_capacity(self.schema.fields.len());
        for field in &self.schema.fields {
            let value = self
                .fields
                .iter()
                .find(|(name, _)| name == field.name)
                .map(|(_, value)| value.clone())
                .ok_or_else(|| err(format!("missing field `{}`", field.name)))?;
            ordered.push((field.name.to_owned(), value));
        }
        let descriptors = self.schema.fields.iter().map(field_descriptor).collect();
        let desc = self.graph.add_class_desc(ClassDesc {
            name: self.schema.class.to_owned(),
            suid: self.schema.suid,
            flags: 0x02,
            fields: descriptors,
            super_desc: None,
            annotations: Vec::new(),
            proxy_interfaces: None,
        })?;
        let object = self.graph.new_object(vec![ClassData {
            desc,
            fields: ordered,
            custom: None,
        }])?;
        Ok(JValue::Obj(object))
    }
}

// ============================== deserializer ==============================

/// Borrowed or materialized value (primitive arrays materialize per element).
pub(crate) enum Holder<'de> {
    Borrowed(&'de JValue),
    Owned(JValue),
}

impl Holder<'_> {
    fn get(&self) -> &JValue {
        match self {
            Self::Borrowed(value) => value,
            Self::Owned(value) => value,
        }
    }
}

pub(crate) fn from_jvalue<'de, T: DeserializeOwned>(
    value: &'de JValue,
    graph: &'de Graph,
    ctx: Ctx,
) -> Result<T, SerdeError> {
    shared::begin_decode();
    let _guard = shared::handle_of(value).map(shared::enter_handle);
    T::deserialize({
        let deserializer: DirectDeserializer<'de> =
            DirectDeserializer::from_holder(graph, Holder::Borrowed(value), ctx);
        deserializer
    })
}

pub(crate) struct DirectDeserializer<'de> {
    graph: &'de Graph,
    value: Holder<'de>,
    ctx: Ctx,
}

impl<'de> DirectDeserializer<'de> {
    fn from_holder(graph: &'de Graph, value: Holder<'de>, ctx: Ctx) -> Self {
        Self { graph, value, ctx }
    }

    fn field_access(&self, schema: JavaSchema) -> Result<FieldAccess<'de>, SerdeError> {
        let object = self.value.get().clone();
        let class = self.graph.class_name(&object)?;
        if class != schema.class {
            return Err(err(format!(
                "schema expects `{}`, stream contains `{class}`",
                schema.class
            )));
        }
        let descriptor = self.graph.descriptor_of(&object)?;
        if descriptor.suid != schema.suid {
            return Err(err(format!(
                "schema SUID {} does not match stream SUID {}",
                schema.suid, descriptor.suid
            )));
        }
        Ok(FieldAccess {
            graph: self.graph,
            object,
            schema,
            index: 0,
        })
    }

    fn entries_access(&self, map: MapSchema) -> Result<EntriesAccess<'de>, SerdeError> {
        let view = self
            .graph
            .as_map(self.value.get())?
            .ok_or_else(|| err("expected a Java Map"))?;
        Ok(EntriesAccess {
            graph: self.graph,
            entries: view
                .entries
                .iter()
                .map(|(key, value)| ((*key).clone(), (*value).clone()))
                .collect(),
            key: map.key,
            value: map.value,
            index: 0,
        })
    }

    fn items_access(&self, kind: SeqKind) -> Result<ItemsAccess<'de>, SerdeError> {
        match kind {
            SeqKind::Set(element) => {
                let values = self
                    .graph
                    .as_set(self.value.get())?
                    .ok_or_else(|| err("expected a Java Set"))?;
                Ok(ItemsAccess {
                    graph: self.graph,
                    items: SeqItems::Owned(values.iter().map(|value| (*value).clone()).collect()),
                    element,
                    index: 0,
                })
            }
            SeqKind::Array {
                class_name1,
                element,
            } => {
                let items = if class_name1.len() == 2 {
                    SeqItems::Primitive(
                        self.graph
                            .as_primitive_array(self.value.get())?
                            .ok_or_else(|| err("expected a primitive array"))?,
                    )
                } else {
                    let values = self
                        .graph
                        .as_array(self.value.get())?
                        .ok_or_else(|| err("expected an object array"))?;
                    SeqItems::Owned(values.to_vec())
                };
                Ok(ItemsAccess {
                    graph: self.graph,
                    items,
                    element,
                    index: 0,
                })
            }
        }
    }
}

macro_rules! deserialize_integer {
    ($method:ident, $visit:ident, $ty:ty) => {
        fn $method<V>(self, visitor: V) -> Result<V::Value, SerdeError>
        where
            V: Visitor<'de>,
        {
            let number = self.graph.as_i64(self.value.get())?;
            let value = <$ty>::try_from(number)
                .map_err(|_| err(format!("{number} does not fit in {}", stringify!($ty))))?;
            visitor.$visit(value)
        }
    };
}

impl<'de> de::Deserializer<'de> for DirectDeserializer<'de> {
    type Error = SerdeError;

    fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        Err(err("serde bridge requires schema-typed deserialization"))
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_bool(self.graph.as_bool(self.value.get())?)
    }

    deserialize_integer!(deserialize_i8, visit_i8, i8);
    deserialize_integer!(deserialize_i16, visit_i16, i16);
    deserialize_integer!(deserialize_i32, visit_i32, i32);
    deserialize_integer!(deserialize_i64, visit_i64, i64);

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        let number = self.graph.as_i64(self.value.get())?;
        let value = u8::try_from(number).map_err(|_| err("u8 out of range"))?;
        visitor.visit_u8(value)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        let number = self.graph.as_i64(self.value.get())?;
        let value = u16::try_from(number).map_err(|_| err("u16 out of range"))?;
        visitor.visit_u16(value)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        let number = self.graph.as_i64(self.value.get())?;
        let value = u32::try_from(number).map_err(|_| err("u32 out of range"))?;
        visitor.visit_u32(value)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        let number = self.graph.as_i64(self.value.get())?;
        let value = u64::try_from(number).map_err(|_| err("u64 out of range"))?;
        visitor.visit_u64(value)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_f32(self.graph.as_f64(self.value.get())? as f32)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_f64(self.graph.as_f64(self.value.get())?)
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        let unit = self.graph.as_char(self.value.get())?;
        let character = char::from_u32(u32::from(unit))
            .ok_or_else(|| err("unpaired UTF-16 surrogate in char"))?;
        visitor.visit_char(character)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_str(self.graph.as_str(self.value.get())?)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_string(self.graph.as_str(self.value.get())?.to_owned())
    }

    fn deserialize_bytes<V>(self, _visitor: V) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        Err(err("byte buffers are not supported"))
    }

    fn deserialize_byte_buf<V>(self, _visitor: V) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        Err(err("byte buffers are not supported"))
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        if matches!(self.value.get(), JValue::Null) {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        if matches!(self.value.get(), JValue::Null) {
            visitor.visit_unit()
        } else {
            Err(err("expected null"))
        }
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        let kind = self.ctx.seq_kind()?;
        visitor.visit_seq(self.items_access(kind)?)
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        _visitor: V,
    ) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        Err(err("tuple structs are not supported"))
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        let (map, _linked) = self
            .ctx
            .map()
            .ok_or_else(|| err("schema context is not a map"))?;
        visitor.visit_map(self.entries_access(map)?)
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        let schema = self
            .ctx
            .nested()
            .ok_or_else(|| err("schema context is not a nested object"))?;
        visitor.visit_map(self.field_access(schema)?)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        let schema = self
            .ctx
            .enumeration()
            .ok_or_else(|| err("schema context is not an enum"))?;
        let class = self.graph.class_name(self.value.get())?;
        if class != schema.class {
            return Err(err(format!(
                "schema expects enum `{}`, stream contains `{class}`",
                schema.class
            )));
        }
        let java_name = self
            .graph
            .as_enum_constant(self.value.get())?
            .ok_or_else(|| err("expected a Java enum"))?;
        let serde_name = schema.serde_name(java_name)?.to_owned();
        visitor.visit_enum(UnitEnum { name: serde_name })
    }

    fn deserialize_identifier<V>(self, _visitor: V) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        Err(err("identifier deserialization is not supported"))
    }

    fn deserialize_ignored_any<V>(self, _visitor: V) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        Err(err("ignored_any is not supported"))
    }
}

struct FieldAccess<'de> {
    graph: &'de Graph,
    object: JValue,
    schema: JavaSchema,
    index: usize,
}

impl<'de> MapAccess<'de> for FieldAccess<'de> {
    type Error = SerdeError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, SerdeError>
    where
        K: DeserializeSeed<'de>,
    {
        if self.index >= self.schema.fields.len() {
            return Ok(None);
        }
        let name = self.schema.fields[self.index].name;
        seed.deserialize(de::value::StrDeserializer::<SerdeError>::new(name))
            .map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, SerdeError>
    where
        V: DeserializeSeed<'de>,
    {
        let field = self.schema.fields[self.index].clone();
        self.index += 1;
        let value = self.graph.require_field(&self.object, field.name)?.clone();
        let _guard = shared::handle_of(&value).map(shared::enter_handle);
        let deserializer: DirectDeserializer<'de> =
            DirectDeserializer::from_holder(self.graph, Holder::Owned(value), Ctx::Field(field));
        seed.deserialize(deserializer)
    }
}

struct EntriesAccess<'de> {
    graph: &'de Graph,
    entries: Vec<(JValue, JValue)>,
    key: SchemaValue,
    value: SchemaValue,
    index: usize,
}

impl<'de> MapAccess<'de> for EntriesAccess<'de> {
    type Error = SerdeError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, SerdeError>
    where
        K: DeserializeSeed<'de>,
    {
        if self.index >= self.entries.len() {
            return Ok(None);
        }
        let key = self.entries[self.index].0.clone();
        let _guard = shared::handle_of(&key).map(shared::enter_handle);
        let deserializer: DirectDeserializer<'de> = DirectDeserializer::from_holder(
            self.graph,
            Holder::Owned(key),
            Ctx::Value(self.key.clone()),
        );
        seed.deserialize(deserializer).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, SerdeError>
    where
        V: DeserializeSeed<'de>,
    {
        let value = self.entries[self.index].1.clone();
        self.index += 1;
        let _guard = shared::handle_of(&value).map(shared::enter_handle);
        let deserializer: DirectDeserializer<'de> = DirectDeserializer::from_holder(
            self.graph,
            Holder::Owned(value),
            Ctx::Value(self.value.clone()),
        );
        seed.deserialize(deserializer)
    }
}

enum SeqItems<'de> {
    Owned(Vec<JValue>),
    Primitive(jser::PrimitiveArray<'de>),
}

struct ItemsAccess<'de> {
    graph: &'de Graph,
    items: SeqItems<'de>,
    element: SchemaValue,
    index: usize,
}

impl<'de> SeqAccess<'de> for ItemsAccess<'de> {
    type Error = SerdeError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, SerdeError>
    where
        T: DeserializeSeed<'de>,
    {
        let value = match &self.items {
            SeqItems::Owned(items) => match items.get(self.index) {
                Some(value) => Holder::Owned(value.clone()),
                None => return Ok(None),
            },
            SeqItems::Primitive(array) => match array.get(self.index) {
                Some(value) => Holder::Owned(value),
                None => return Ok(None),
            },
        };
        self.index += 1;
        let _guard = shared::handle_of(value.get()).map(shared::enter_handle);
        let deserializer: DirectDeserializer<'de> =
            DirectDeserializer::from_holder(self.graph, value, Ctx::Value(self.element.clone()));
        seed.deserialize(deserializer).map(Some)
    }
}

struct UnitEnum {
    name: String,
}

struct UnitVariant;

impl<'de> EnumAccess<'de> for UnitEnum {
    type Error = SerdeError;
    type Variant = UnitVariant;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), SerdeError>
    where
        V: DeserializeSeed<'de>,
    {
        let value =
            seed.deserialize(de::value::StringDeserializer::<SerdeError>::new(self.name))?;
        Ok((value, UnitVariant))
    }
}

impl<'de> VariantAccess<'de> for UnitVariant {
    type Error = SerdeError;

    fn unit_variant(self) -> Result<(), SerdeError> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, _seed: T) -> Result<T::Value, SerdeError>
    where
        T: DeserializeSeed<'de>,
    {
        Err(err("enum variants with data are not supported"))
    }

    fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        Err(err("enum variants with data are not supported"))
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        Err(err("enum variants with data are not supported"))
    }
}

// ============================== multi-root ==============================

pub(crate) fn from_roots<T: DeserializeOwned>(
    graph: &Graph,
    items: Vec<&JValue>,
    schemas: Vec<JavaSchema>,
) -> Result<T, SerdeError> {
    shared::begin_decode();
    T::deserialize(RootsDeserializer {
        graph,
        items,
        schemas,
    })
}

struct RootsDeserializer<'de> {
    graph: &'de Graph,
    items: Vec<&'de JValue>,
    schemas: Vec<JavaSchema>,
}

impl<'de> de::Deserializer<'de> for RootsDeserializer<'de> {
    type Error = SerdeError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_seq(RootsAccess {
            graph: self.graph,
            items: self.items,
            schemas: self.schemas,
            index: 0,
        })
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, SerdeError>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string bytes
        byte_buf option unit unit_struct newtype_struct tuple_struct map struct
        enum identifier ignored_any
    }
}

struct RootsAccess<'de> {
    graph: &'de Graph,
    items: Vec<&'de JValue>,
    schemas: Vec<JavaSchema>,
    index: usize,
}

impl<'de> SeqAccess<'de> for RootsAccess<'de> {
    type Error = SerdeError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, SerdeError>
    where
        T: DeserializeSeed<'de>,
    {
        let Some(value) = self.items.get(self.index) else {
            return Ok(None);
        };
        let value = *value;
        let schema = self
            .schemas
            .get(self.index)
            .ok_or_else(|| err("missing root schema"))?
            .clone();
        self.index += 1;
        let value = value.clone();
        let _guard = shared::handle_of(&value).map(shared::enter_handle);
        let deserializer: DirectDeserializer<'de> =
            DirectDeserializer::from_holder(self.graph, Holder::Owned(value), Ctx::Object(schema));
        seed.deserialize(deserializer).map(Some)
    }
}
