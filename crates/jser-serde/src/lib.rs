//! Optional, schema-driven serde bridge for `jser`.
//!
//! serde cannot carry Java class names, type codes or `serialVersionUID`, so
//! this crate requires an explicit [`JavaSchema`]. It supports a **single root
//! POJO** whose fields may be scalars, `String`, `Option<T>`, arrays, nested
//! POJOs ([`SchemaField::nested`]), `HashMap`/`LinkedHashMap` fields
//! ([`SchemaField::map`]), `HashSet` fields ([`SchemaField::set`]) and Java
//! enums ([`SchemaField::enumeration`]).
//! Map/set roots are available through [`to_vec_map`]/[`from_slice_map`] and
//! [`to_vec_set`]/[`from_slice_set`]; enum roots through
//! [`to_vec_enum`]/[`from_slice_enum`]. Multi-root streams (with optional
//! `TC_RESET` markers) use [`MultiRootWriter`] and [`from_slice_multi`].
//! Values are serde trees: shared references decode as duplicated subtrees and
//! cycles are not representable (use [`jser::Graph`] for true identity/cycles).
//! Arrays of objects and unknown classes are rejected.

use jser::builtin::{
    new_hash_map_entries, new_hash_set, new_java_array, new_linked_hash_map_entries,
};
use jser::{
    decode_stream_with_allowlist, decode_with_allowlist, encode_stream, Allowlist, ClassData,
    ClassDesc, Error, FieldDesc, FieldType, Graph, JValue, Result,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{Map, Value};

const HASH_MAP: &str = "Ljava/util/HashMap;";
const LINKED_HASH_MAP: &str = "Ljava/util/LinkedHashMap;";
const HASH_SET: &str = "Ljava/util/HashSet;";

/// Schema of one value inside a map or set.
#[derive(Debug, Clone)]
pub struct SchemaValue {
    pub typecode: FieldType,
    /// JVM descriptor for object/array values.
    pub class_name1: Option<&'static str>,
    /// Nested POJO schema for object values.
    pub nested: Option<Box<JavaSchema>>,
}

/// Map field layout; `HashMap` unless the field descriptor is `LinkedHashMap`.
#[derive(Debug, Clone)]
pub struct MapSchema {
    pub key: SchemaValue,
    pub value: SchemaValue,
}

/// One serde-name ↔ Java-constant mapping for [`EnumSchema`].
#[derive(Debug, Clone)]
pub struct EnumConstant {
    /// Name serde uses in JSON (usually the Rust variant name).
    pub serde: &'static str,
    /// Java enum constant name (usually UPPER_SNAKE).
    pub java: &'static str,
}

/// Schema of a Java enum (`TC_ENUM`, SUID 0, flags `0x12`).
#[derive(Debug, Clone)]
pub struct EnumSchema {
    pub class: &'static str,
    pub constants: Vec<EnumConstant>,
}

impl EnumSchema {
    fn java_name(&self, serde: &str) -> Result<&'static str> {
        self.constants
            .iter()
            .find(|constant| constant.serde == serde)
            .map(|constant| constant.java)
            .ok_or_else(|| Error::FieldMismatch {
                message: format!("unknown enum constant `{serde}` for `{}`", self.class),
            })
    }

    fn serde_name(&self, java: &str) -> Result<&'static str> {
        self.constants
            .iter()
            .find(|constant| constant.java == java)
            .map(|constant| constant.serde)
            .ok_or_else(|| Error::FieldMismatch {
                message: format!("unknown Java enum constant `{java}` for `{}`", self.class),
            })
    }
}

/// One field of a Java class exposed to serde.
#[derive(Debug, Clone)]
pub struct SchemaField {
    /// Java field name (also the serde/JSON field name).
    pub name: &'static str,
    pub typecode: FieldType,
    /// JVM descriptor for object/array fields.
    pub class_name1: Option<&'static str>,
    /// Schema of a nested POJO field; `None` for scalars, String and arrays.
    pub nested: Option<Box<JavaSchema>>,
    /// Map field (`HashMap`/`LinkedHashMap`); mutually exclusive with nested/set/enum.
    pub map: Option<Box<MapSchema>>,
    /// Set field (`HashSet`); mutually exclusive with nested/map/enum.
    pub set: Option<Box<SchemaValue>>,
    /// Enum field (`TC_ENUM`); mutually exclusive with nested/map/set.
    pub enumeration: Option<Box<EnumSchema>>,
}

/// Explicit Java class metadata for the serde bridge.
#[derive(Debug, Clone)]
pub struct JavaSchema {
    pub class: &'static str,
    pub suid: i64,
    pub fields: Vec<SchemaField>,
}

/// Serialize `value` and write one Java object stream.
pub fn to_vec<T: Serialize>(value: &T, schema: &JavaSchema) -> Result<Vec<u8>> {
    validate_schema(schema)?;
    let json = serde_json::to_value(value).map_err(serde_error)?;

    let mut graph = Graph::new();
    let object = encode_object(&json, schema, &mut graph)?;
    graph.add_item(object);
    encode_stream(&graph)
}

fn validate_schema(schema: &JavaSchema) -> Result<()> {
    for field in &schema.fields {
        let layouts = u8::from(field.nested.is_some())
            + u8::from(field.map.is_some())
            + u8::from(field.set.is_some())
            + u8::from(field.enumeration.is_some());
        if layouts > 1 {
            return Err(Error::InvalidSchema(format!(
                "field `{}` has conflicting nested/map/set/enum layouts",
                field.name
            )));
        }
        if layouts > 0 && field.typecode != FieldType::Object {
            return Err(Error::InvalidSchema(format!(
                "field `{}` with nested/map/set/enum must have typecode Object",
                field.name
            )));
        }
        if let Some(nested) = &field.nested {
            validate_schema(nested)?;
        }
        if let Some(map) = &field.map {
            if !matches!(field.class_name1, Some(HASH_MAP) | Some(LINKED_HASH_MAP)) {
                return Err(Error::InvalidSchema(format!(
                    "map field `{}` needs a HashMap/LinkedHashMap descriptor",
                    field.name
                )));
            }
            validate_value(&map.key, true)?;
            validate_value(&map.value, false)?;
        }
        if let Some(element) = &field.set {
            if field.class_name1 != Some(HASH_SET) {
                return Err(Error::InvalidSchema(format!(
                    "set field `{}` needs a HashSet descriptor",
                    field.name
                )));
            }
            validate_value(element, false)?;
        }
        if let Some(enumeration) = &field.enumeration {
            validate_enum(enumeration)?;
            if let Some(descriptor) = field.class_name1 {
                let expected = format!("L{};", enumeration.class.replace('.', "/"));
                if descriptor != expected {
                    return Err(Error::InvalidSchema(format!(
                        "enum field `{}` descriptor `{descriptor}` does not match `{expected}`",
                        field.name
                    )));
                }
            }
        }
    }
    Ok(())
}

fn validate_enum(schema: &EnumSchema) -> Result<()> {
    if schema.constants.is_empty() {
        return Err(Error::InvalidSchema(format!(
            "enum `{}` needs at least one constant",
            schema.class
        )));
    }
    let mut serde_names = std::collections::HashSet::new();
    let mut java_names = std::collections::HashSet::new();
    for constant in &schema.constants {
        if !serde_names.insert(constant.serde) || !java_names.insert(constant.java) {
            return Err(Error::InvalidSchema(format!(
                "enum `{}` has duplicate constants",
                schema.class
            )));
        }
    }
    Ok(())
}

fn validate_value(value: &SchemaValue, is_key: bool) -> Result<()> {
    if value.nested.is_some() && value.typecode != FieldType::Object {
        return Err(Error::InvalidSchema(
            "nested value must have typecode Object".to_owned(),
        ));
    }
    if is_key && (value.nested.is_some() || value.typecode == FieldType::Array) {
        return Err(Error::InvalidSchema(
            "map keys must be scalar or String".to_owned(),
        ));
    }
    if let Some(nested) = &value.nested {
        validate_schema(nested)?;
    }
    Ok(())
}

fn encode_object(value: &Value, schema: &JavaSchema, graph: &mut Graph) -> Result<JValue> {
    let object = value.as_object().ok_or_else(|| Error::FieldMismatch {
        message: format!(
            "serde value for `{}` must be an object/struct",
            schema.class
        ),
    })?;

    let mut field_descs = Vec::with_capacity(schema.fields.len());
    let mut fields = Vec::with_capacity(schema.fields.len());

    for field in &schema.fields {
        let raw = object.get(field.name).ok_or_else(|| Error::FieldMismatch {
            message: format!("serde value has no field `{}`", field.name),
        })?;
        let value = if let Some(enumeration) = &field.enumeration {
            encode_enum(raw, enumeration, graph)?
        } else if let Some(map) = &field.map {
            encode_map(raw, map, field.class_name1, graph)?
        } else if let Some(element) = &field.set {
            encode_set(raw, element, graph)?
        } else {
            match &field.nested {
                Some(nested) if !raw.is_null() => encode_object(raw, nested, graph)?,
                Some(_) => JValue::Null,
                None => json_to_jvalue(raw, field.typecode, field.class_name1, graph)?,
            }
        };
        let class_name1 = field.class_name1.map(str::to_owned).or_else(|| {
            field
                .enumeration
                .as_ref()
                .map(|enumeration| format!("L{};", enumeration.class.replace('.', "/")))
        });
        field_descs.push(FieldDesc {
            name: field.name.to_owned(),
            typecode: field.typecode,
            class_name1,
            class_name_handle: None,
        });
        fields.push((field.name.to_owned(), value));
    }

    let desc = graph.add_class_desc(ClassDesc {
        name: schema.class.to_owned(),
        suid: schema.suid,
        flags: 0x02,
        fields: field_descs,
        super_desc: None,
        annotations: Vec::new(),
    })?;
    let object = graph.new_object(vec![ClassData {
        desc,
        fields,
        custom: None,
    }])?;
    Ok(JValue::Obj(object))
}

/// Read one Java object stream and deserialize it into `T`.
pub fn from_slice<T: DeserializeOwned>(
    bytes: &[u8],
    schema: &JavaSchema,
    allowlist: &Allowlist,
) -> Result<T> {
    validate_schema(schema)?;
    let graph = decode_with_allowlist(bytes, allowlist)?;
    let value = &graph.items()[0];
    let json = decode_object(&graph, value, schema)?;
    serde_json::from_value(json).map_err(serde_error)
}

/// Builds a multi-root stream, optionally interleaving `TC_RESET`.
///
/// Each [`push`](MultiRootWriter::push) serializes one root; [`reset`](MultiRootWriter::reset)
/// appends a `TC_RESET` marker so the following roots start a fresh wire
/// handle table. Root values are serde trees: shared Rust references are
/// written as duplicated subtrees (the dynamic [`jser::Graph`] API is the way
/// to write true identity/cycles).
#[derive(Debug, Default)]
pub struct MultiRootWriter {
    graph: Graph,
}

impl MultiRootWriter {
    /// Create an empty writer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Serialize `value` with `schema` and append it as the next root.
    pub fn push<T: Serialize + ?Sized>(&mut self, schema: &JavaSchema, value: &T) -> Result<()> {
        validate_schema(schema)?;
        let json = serde_json::to_value(value).map_err(serde_error)?;
        let object = encode_object(&json, schema, &mut self.graph)?;
        self.graph.add_item(object);
        Ok(())
    }

    /// Append a `TC_RESET` marker between roots.
    pub fn reset(&mut self) {
        self.graph.add_reset();
    }

    /// Encode the accumulated roots into one stream.
    pub fn finish(self) -> Result<Vec<u8>> {
        encode_stream(&self.graph)
    }
}

/// Decode every non-`TC_RESET` root and return its JSON value.
///
/// `schemas` must provide one [`JavaSchema`] per root, in order. `TC_RESET`
/// markers are skipped; they only reset wire handles.
pub fn from_slice_roots(
    bytes: &[u8],
    schemas: &[&JavaSchema],
    allowlist: &Allowlist,
) -> Result<Vec<Value>> {
    for schema in schemas {
        validate_schema(schema)?;
    }
    let graph = decode_stream_with_allowlist(bytes, allowlist)?;
    let mut roots = Vec::new();
    let mut index = 0;

    for item in graph.items() {
        if matches!(item, JValue::Reset) {
            continue;
        }
        let schema = schemas.get(index).ok_or_else(|| Error::FieldMismatch {
            message: format!(
                "stream has more roots than the {} schema(s) provided",
                schemas.len()
            ),
        })?;
        roots.push(decode_object(&graph, item, schema)?);
        index += 1;
    }

    if index != schemas.len() {
        return Err(Error::FieldMismatch {
            message: format!("stream has {index} root(s), expected {}", schemas.len()),
        });
    }
    Ok(roots)
}

/// Deserialize a multi-root stream into `T` (for example a tuple or `Vec`).
pub fn from_slice_multi<T: DeserializeOwned>(
    bytes: &[u8],
    schemas: &[&JavaSchema],
    allowlist: &Allowlist,
) -> Result<T> {
    let roots = from_slice_roots(bytes, schemas, allowlist)?;
    serde_json::from_value(Value::Array(roots)).map_err(serde_error)
}

fn decode_object(graph: &Graph, value: &JValue, schema: &JavaSchema) -> Result<Value> {
    let class = graph.class_name(value)?;
    if class != schema.class {
        return Err(Error::FieldMismatch {
            message: format!(
                "schema expects `{}`, stream contains `{class}`",
                schema.class
            ),
        });
    }
    let descriptor = graph.descriptor_of(value)?;
    if descriptor.suid != schema.suid {
        return Err(Error::FieldMismatch {
            message: format!(
                "schema SUID {} does not match stream SUID {}",
                schema.suid, descriptor.suid
            ),
        });
    }

    let mut object = Map::with_capacity(schema.fields.len());
    for field in &schema.fields {
        let field_value = graph.require_field(value, field.name)?;
        let json = if let Some(enumeration) = &field.enumeration {
            decode_enum(graph, field_value, enumeration)?
        } else if let Some(map) = &field.map {
            decode_map(graph, field_value, map)?
        } else if let Some(element) = &field.set {
            decode_set(graph, field_value, element)?
        } else {
            match &field.nested {
                Some(nested) if !matches!(field_value, JValue::Null) => {
                    decode_object(graph, field_value, nested)?
                }
                Some(_) => Value::Null,
                None => jvalue_to_json(graph, field_value, field.typecode, field.class_name1)?,
            }
        };
        object.insert(field.name.to_owned(), json);
    }

    Ok(Value::Object(object))
}

fn encode_map(
    raw: &Value,
    map: &MapSchema,
    class_name1: Option<&'static str>,
    graph: &mut Graph,
) -> Result<JValue> {
    if raw.is_null() {
        return Ok(JValue::Null);
    }
    let object = raw
        .as_object()
        .ok_or_else(|| type_error("a JSON object for a Map field"))?;
    let mut entries = Vec::with_capacity(object.len());
    for (key, value) in object {
        let key = json_key_to_jvalue(key, &map.key, graph)?;
        let value = json_to_value(value, &map.value, graph)?;
        entries.push((key, value));
    }
    let handle = if class_name1 == Some(LINKED_HASH_MAP) {
        new_linked_hash_map_entries(graph, entries, false)?
    } else {
        new_hash_map_entries(graph, entries)?
    };
    Ok(JValue::Obj(handle))
}

fn encode_set(raw: &Value, element: &SchemaValue, graph: &mut Graph) -> Result<JValue> {
    if raw.is_null() {
        return Ok(JValue::Null);
    }
    let array = raw
        .as_array()
        .ok_or_else(|| type_error("a JSON array for a Set field"))?;
    let mut values = Vec::with_capacity(array.len());
    for item in array {
        values.push(json_to_value(item, element, graph)?);
    }
    Ok(JValue::Obj(new_hash_set(graph, values)?))
}

fn json_to_value(value: &Value, schema: &SchemaValue, graph: &mut Graph) -> Result<JValue> {
    if value.is_null() {
        return Ok(JValue::Null);
    }
    match &schema.nested {
        Some(nested) => encode_object(value, nested, graph),
        None => json_to_jvalue(value, schema.typecode, schema.class_name1, graph),
    }
}

fn json_key_to_jvalue(key: &str, schema: &SchemaValue, graph: &mut Graph) -> Result<JValue> {
    if schema.nested.is_some() || schema.typecode == FieldType::Array {
        return Err(Error::InvalidSchema(
            "map keys must be scalar or String".to_owned(),
        ));
    }

    let invalid = || Error::FieldMismatch {
        message: format!("map key `{key}` does not match {:?}", schema.typecode),
    };

    Ok(match schema.typecode {
        FieldType::Object => json_to_jvalue(
            &Value::String(key.to_owned()),
            schema.typecode,
            schema.class_name1,
            graph,
        )?,
        FieldType::Byte => JValue::Byte(key.parse().map_err(|_| invalid())?),
        FieldType::Short => JValue::Short(key.parse().map_err(|_| invalid())?),
        FieldType::Int => JValue::Int(key.parse().map_err(|_| invalid())?),
        FieldType::Long => JValue::Long(key.parse().map_err(|_| invalid())?),
        FieldType::Boolean => JValue::Bool(key.parse().map_err(|_| invalid())?),
        FieldType::Float => JValue::Float(key.parse().map_err(|_| invalid())?),
        FieldType::Double => JValue::Double(key.parse().map_err(|_| invalid())?),
        FieldType::Char => {
            let mut chars = key.chars();
            let value = chars.next().ok_or_else(invalid)?;
            if chars.next().is_some() {
                return Err(invalid());
            }
            JValue::Char(value as u16)
        }
        FieldType::Array => unreachable!("array keys are rejected above"),
    })
}

fn decode_map(graph: &Graph, value: &JValue, map: &MapSchema) -> Result<Value> {
    if matches!(value, JValue::Null) {
        return Ok(Value::Null);
    }
    let view = graph.as_map(value)?.ok_or_else(|| type_error("a Map"))?;
    let mut object = Map::with_capacity(view.entries.len());
    for (key, value) in view.entries {
        let key = value_to_json(graph, key, &map.key)?;
        let key = json_key_to_string(key)?;
        object.insert(key, value_to_json(graph, value, &map.value)?);
    }
    Ok(Value::Object(object))
}

fn decode_set(graph: &Graph, value: &JValue, element: &SchemaValue) -> Result<Value> {
    if matches!(value, JValue::Null) {
        return Ok(Value::Null);
    }
    let values = graph.as_set(value)?.ok_or_else(|| type_error("a Set"))?;
    let mut json = Vec::with_capacity(values.len());
    for value in values {
        json.push(value_to_json(graph, value, element)?);
    }
    Ok(Value::Array(json))
}

fn value_to_json(graph: &Graph, value: &JValue, schema: &SchemaValue) -> Result<Value> {
    if matches!(value, JValue::Null) {
        return Ok(Value::Null);
    }
    match &schema.nested {
        Some(nested) => decode_object(graph, value, nested),
        None => jvalue_to_json(graph, value, schema.typecode, schema.class_name1),
    }
}

fn json_key_to_string(value: Value) -> Result<String> {
    match value {
        Value::String(value) => Ok(value),
        Value::Number(value) => Ok(value.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        _ => Err(Error::FieldMismatch {
            message: "map key must be scalar or String".to_owned(),
        }),
    }
}

/// Serialize a map value into a Java `HashMap`/`LinkedHashMap` root object.
pub fn to_vec_map<T: Serialize>(
    value: &T,
    key: &SchemaValue,
    map_value: &SchemaValue,
    linked: bool,
) -> Result<Vec<u8>> {
    validate_value(key, true)?;
    validate_value(map_value, false)?;
    let json = serde_json::to_value(value).map_err(serde_error)?;
    if !json.is_object() {
        return Err(Error::FieldMismatch {
            message: "map root must serialize to a JSON object".to_owned(),
        });
    }
    let mut graph = Graph::new();
    let map = MapSchema {
        key: key.clone(),
        value: map_value.clone(),
    };
    let class_name1 = Some(if linked { LINKED_HASH_MAP } else { HASH_MAP });
    let value = encode_map(&json, &map, class_name1, &mut graph)?;
    graph.add_item(value);
    encode_stream(&graph)
}

/// Read a Java map root and deserialize it into `T`.
pub fn from_slice_map<T: DeserializeOwned>(
    bytes: &[u8],
    key: &SchemaValue,
    map_value: &SchemaValue,
    allowlist: &Allowlist,
) -> Result<T> {
    validate_value(key, true)?;
    validate_value(map_value, false)?;
    let graph = decode_with_allowlist(bytes, allowlist)?;
    let map = MapSchema {
        key: key.clone(),
        value: map_value.clone(),
    };
    let json = decode_map(&graph, &graph.items()[0], &map)?;
    serde_json::from_value(json).map_err(serde_error)
}

/// Serialize a set value (JSON array) into a Java `HashSet` root object.
pub fn to_vec_set<T: Serialize>(value: &T, element: &SchemaValue) -> Result<Vec<u8>> {
    validate_value(element, false)?;
    let json = serde_json::to_value(value).map_err(serde_error)?;
    if !json.is_array() {
        return Err(Error::FieldMismatch {
            message: "set root must serialize to a JSON array".to_owned(),
        });
    }
    let mut graph = Graph::new();
    let value = encode_set(&json, element, &mut graph)?;
    graph.add_item(value);
    encode_stream(&graph)
}

/// Read a Java set root and deserialize it into `T`.
pub fn from_slice_set<T: DeserializeOwned>(
    bytes: &[u8],
    element: &SchemaValue,
    allowlist: &Allowlist,
) -> Result<T> {
    validate_value(element, false)?;
    let graph = decode_with_allowlist(bytes, allowlist)?;
    let json = decode_set(&graph, &graph.items()[0], element)?;
    serde_json::from_value(json).map_err(serde_error)
}

fn encode_enum(value: &Value, schema: &EnumSchema, graph: &mut Graph) -> Result<JValue> {
    if value.is_null() {
        return Ok(JValue::Null);
    }
    let serde_name = value
        .as_str()
        .ok_or_else(|| type_error("a string for an enum field"))?;
    let java_name = schema.java_name(serde_name)?;

    let enum_desc = graph.add_class_desc(ClassDesc {
        name: "java.lang.Enum".to_owned(),
        suid: 0,
        flags: 0x12,
        fields: Vec::new(),
        super_desc: None,
        annotations: Vec::new(),
    })?;
    let desc = graph.add_class_desc(ClassDesc {
        name: schema.class.to_owned(),
        suid: 0,
        flags: 0x12,
        fields: Vec::new(),
        super_desc: Some(enum_desc),
        annotations: Vec::new(),
    })?;
    let constant = graph.new_string(java_name);
    let handle = graph.new_enum(desc, constant)?;
    Ok(JValue::Enum(handle))
}

fn decode_enum(graph: &Graph, value: &JValue, schema: &EnumSchema) -> Result<Value> {
    if matches!(value, JValue::Null) {
        return Ok(Value::Null);
    }
    let class = graph.class_name(value)?;
    if class != schema.class {
        return Err(Error::FieldMismatch {
            message: format!(
                "schema expects enum `{}`, stream contains `{class}`",
                schema.class
            ),
        });
    }
    let java_name = graph
        .as_enum_constant(value)?
        .ok_or_else(|| type_error("a Java enum"))?;
    Ok(Value::String(schema.serde_name(java_name)?.to_owned()))
}

/// Serialize a unit-variant value into a Java enum root object.
pub fn to_vec_enum<T: Serialize>(value: &T, schema: &EnumSchema) -> Result<Vec<u8>> {
    validate_enum(schema)?;
    let json = serde_json::to_value(value).map_err(serde_error)?;
    if !json.is_string() {
        return Err(Error::FieldMismatch {
            message: "enum root must serialize to a string (unit variant)".to_owned(),
        });
    }
    let mut graph = Graph::new();
    let value = encode_enum(&json, schema, &mut graph)?;
    graph.add_item(value);
    encode_stream(&graph)
}

/// Read a Java enum root and deserialize it into `T`.
pub fn from_slice_enum<T: DeserializeOwned>(
    bytes: &[u8],
    schema: &EnumSchema,
    allowlist: &Allowlist,
) -> Result<T> {
    validate_enum(schema)?;
    let graph = decode_with_allowlist(bytes, allowlist)?;
    let json = decode_enum(&graph, &graph.items()[0], schema)?;
    serde_json::from_value(json).map_err(serde_error)
}

fn json_to_jvalue(
    value: &Value,
    typecode: FieldType,
    class_name1: Option<&str>,
    graph: &mut Graph,
) -> Result<JValue> {
    if value.is_null() {
        return Ok(JValue::Null);
    }

    match typecode {
        FieldType::Boolean => Ok(JValue::Bool(
            value.as_bool().ok_or_else(|| type_error("boolean"))?,
        )),
        FieldType::Byte => Ok(JValue::Byte(
            i8::try_from(value.as_i64().ok_or_else(|| type_error("byte integer"))?)
                .map_err(|_| type_error("i8 range"))?,
        )),
        FieldType::Char => {
            let text = value.as_str().ok_or_else(|| type_error("char string"))?;
            let mut chars = text.chars();
            let character = chars.next().ok_or_else(|| type_error("empty char"))?;
            if chars.next().is_some() {
                return Err(type_error("single char"));
            }
            Ok(JValue::Char(character as u16))
        }
        FieldType::Short => Ok(JValue::Short(
            i16::try_from(value.as_i64().ok_or_else(|| type_error("short integer"))?)
                .map_err(|_| type_error("i16 range"))?,
        )),
        FieldType::Int => Ok(JValue::Int(
            i32::try_from(value.as_i64().ok_or_else(|| type_error("int"))?)
                .map_err(|_| type_error("i32 range"))?,
        )),
        FieldType::Long => Ok(JValue::Long(
            value.as_i64().ok_or_else(|| type_error("long"))?,
        )),
        FieldType::Float => Ok(JValue::Float(
            value.as_f64().ok_or_else(|| type_error("float"))? as f32,
        )),
        FieldType::Double => Ok(JValue::Double(
            value.as_f64().ok_or_else(|| type_error("double"))?,
        )),
        FieldType::Object => match class_name1 {
            Some("Ljava/lang/String;") => Ok(JValue::Str(
                graph.new_string(
                    value
                        .as_str()
                        .ok_or_else(|| type_error("String"))?
                        .to_owned(),
                ),
            )),
            Some(other) => Err(Error::UnsupportedClass(format!(
                "serde bridge does not support object field `{other}`"
            ))),
            None => Err(Error::FieldMismatch {
                message: "object field has no className1".to_owned(),
            }),
        },
        FieldType::Array => {
            let class = class_name1.ok_or_else(|| Error::FieldMismatch {
                message: "array field has no className1".to_owned(),
            })?;
            let values = value.as_array().ok_or_else(|| type_error("array"))?;
            let element = array_element_descriptor(class)?;
            let mut elements = Vec::with_capacity(values.len());
            for value in values {
                elements.push(json_to_jvalue(
                    value,
                    element.typecode,
                    element.class_name1,
                    graph,
                )?);
            }
            Ok(JValue::Arr(new_java_array(graph, class, elements)?))
        }
    }
}

fn jvalue_to_json(
    graph: &Graph,
    value: &JValue,
    typecode: FieldType,
    class_name1: Option<&str>,
) -> Result<Value> {
    if matches!(value, JValue::Null) {
        return Ok(Value::Null);
    }

    match typecode {
        FieldType::Boolean => Ok(Value::Bool(graph.as_bool(value)?)),
        FieldType::Byte => Ok(Value::from(graph.as_i64(value)?)),
        FieldType::Char => {
            let unit = graph.as_char(value)?;
            let character =
                char::from_u32(u32::from(unit)).ok_or_else(|| Error::FieldMismatch {
                    message: "unpaired UTF-16 surrogate in char".to_owned(),
                })?;
            Ok(Value::String(character.to_string()))
        }
        FieldType::Short | FieldType::Int | FieldType::Long => {
            Ok(Value::from(graph.as_i64(value)?))
        }
        FieldType::Float | FieldType::Double => Ok(Value::from(graph.as_f64(value)?)),
        FieldType::Object => match class_name1 {
            Some("Ljava/lang/String;") => Ok(Value::String(graph.as_str(value)?.to_owned())),
            Some(other) => Err(Error::UnsupportedClass(format!(
                "serde bridge does not support object field `{other}`"
            ))),
            None => Err(Error::FieldMismatch {
                message: "object field has no className1".to_owned(),
            }),
        },
        FieldType::Array => {
            let class = class_name1.ok_or_else(|| Error::FieldMismatch {
                message: "array field has no className1".to_owned(),
            })?;
            let element = array_element_descriptor(class)?;

            if element.typecode.is_primitive() {
                let array = graph
                    .as_primitive_array(value)?
                    .ok_or_else(|| type_error("primitive array"))?;
                let mut json = Vec::with_capacity(array.len());
                for index in 0..array.len() {
                    let element_value = array
                        .get(index)
                        .ok_or_else(|| type_error("primitive element"))?;
                    json.push(jvalue_to_json(
                        graph,
                        &element_value,
                        element.typecode,
                        element.class_name1,
                    )?);
                }
                return Ok(Value::Array(json));
            }

            let elements = graph.as_array(value)?.ok_or_else(|| type_error("array"))?;
            let mut json = Vec::with_capacity(elements.len());
            for value in elements {
                json.push(jvalue_to_json(
                    graph,
                    value,
                    element.typecode,
                    element.class_name1,
                )?);
            }
            Ok(Value::Array(json))
        }
    }
}

struct ElementDescriptor {
    typecode: FieldType,
    class_name1: Option<&'static str>,
}

fn array_element_descriptor(class_name: &str) -> Result<ElementDescriptor> {
    let rest = class_name
        .strip_prefix('[')
        .ok_or_else(|| type_error("array class name"))?;
    if rest.len() == 1 {
        let typecode = FieldType::from_code(rest.as_bytes()[0])
            .ok_or_else(|| Error::UnsupportedClass(format!("array class `{class_name}`")))?;
        return Ok(ElementDescriptor {
            typecode,
            class_name1: None,
        });
    }
    if let Some(element) = rest
        .strip_prefix('L')
        .and_then(|value| value.strip_suffix(';'))
    {
        let descriptor = format!("L{};", element.replace('.', "/"));
        let class_name1 = match descriptor.as_str() {
            "Ljava/lang/String;" => "Ljava/lang/String;",
            other => {
                return Err(Error::UnsupportedClass(format!(
                    "serde bridge does not support array element `{other}`"
                )))
            }
        };
        return Ok(ElementDescriptor {
            typecode: FieldType::Object,
            class_name1: Some(class_name1),
        });
    }
    Err(Error::UnsupportedClass(format!(
        "serde bridge does not support nested array `{class_name}`"
    )))
}

fn type_error(expected: &str) -> Error {
    Error::FieldMismatch {
        message: format!("serde bridge expected {expected}"),
    }
}

fn serde_error(error: serde_json::Error) -> Error {
    Error::FieldMismatch {
        message: format!("serde: {error}"),
    }
}
