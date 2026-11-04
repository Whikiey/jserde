//! Optional, schema-driven serde bridge for `jser`.
//!
//! serde cannot carry Java class names, type codes or `serialVersionUID`, so
//! this crate requires an explicit [`JavaSchema`]. Typed encode/decode goes
//! through the direct [`JValue`] bridge in `direct` (no JSON intermediate):
//! fields may be scalars, `String`, `Option<T>`, arrays, nested POJOs
//! ([`SchemaField::nested`]), `HashMap`/`LinkedHashMap` fields
//! ([`SchemaField::map`]), `HashSet` fields ([`SchemaField::set`]) and Java
//! enums ([`SchemaField::enumeration`]).
//!
//! [`to_vec_map`]/[`from_slice_map`], [`to_vec_set`]/[`from_slice_set`] and
//! [`to_vec_enum`]/[`from_slice_enum`] cover map/set/enum roots;
//! [`MultiRootWriter`]/[`from_slice_multi`] cover multi-root streams with
//! optional `TC_RESET` markers. [`from_slice_roots`] is the JSON-view helper
//! (it returns `serde_json::Value` by design). Values are serde trees: shared
//! references decode as duplicated subtrees and cycles are not representable
//! (use [`jser::Graph`] for true identity/cycles). Arrays of objects and
//! unknown classes are rejected.

mod direct;

use jser::{
    decode_stream_with_allowlist, decode_with_allowlist, encode_stream, Allowlist, Error,
    FieldType, Graph, JValue, Result,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{Map, Value};

use direct::Ctx;

pub(crate) const HASH_MAP: &str = "Ljava/util/HashMap;";
pub(crate) const LINKED_HASH_MAP: &str = "Ljava/util/LinkedHashMap;";
pub(crate) const HASH_SET: &str = "Ljava/util/HashSet;";

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
    let mut graph = Graph::new();
    let object =
        direct::to_jvalue(value, Ctx::Object(schema.clone()), &mut graph).map_err(Error::from)?;
    graph.add_item(object);
    encode_stream(&graph)
}

/// Read one Java object stream and deserialize it into `T`.
pub fn from_slice<T: DeserializeOwned>(
    bytes: &[u8],
    schema: &JavaSchema,
    allowlist: &Allowlist,
) -> Result<T> {
    validate_schema(schema)?;
    let graph = decode_with_allowlist(bytes, allowlist)?;
    direct::from_jvalue(&graph.items()[0], &graph, Ctx::Object(schema.clone())).map_err(Error::from)
}

/// Builds a multi-root stream, optionally interleaving `TC_RESET`.
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
        let object = direct::to_jvalue(value, Ctx::Object(schema.clone()), &mut self.graph)
            .map_err(Error::from)?;
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
/// This is the explicit JSON-view API; typed code should use
/// [`from_slice_multi`] instead.
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
    for schema in schemas {
        validate_schema(schema)?;
    }
    let graph = decode_stream_with_allowlist(bytes, allowlist)?;
    let mut items = Vec::new();
    for item in graph.items() {
        if !matches!(item, JValue::Reset) {
            items.push(item);
        }
    }
    if items.len() != schemas.len() {
        return Err(Error::FieldMismatch {
            message: format!(
                "stream has {} root(s), expected {}",
                items.len(),
                schemas.len()
            ),
        });
    }
    direct::from_roots(
        &graph,
        items,
        schemas.iter().map(|schema| (*schema).clone()).collect(),
    )
    .map_err(Error::from)
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
    let mut graph = Graph::new();
    let map = MapSchema {
        key: key.clone(),
        value: map_value.clone(),
    };
    let object =
        direct::to_jvalue(value, Ctx::Map { map, linked }, &mut graph).map_err(Error::from)?;
    graph.add_item(object);
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
    direct::from_jvalue(&graph.items()[0], &graph, Ctx::Map { map, linked: false })
        .map_err(Error::from)
}

/// Serialize a set value into a Java `HashSet` root object.
pub fn to_vec_set<T: Serialize>(value: &T, element: &SchemaValue) -> Result<Vec<u8>> {
    validate_value(element, false)?;
    let mut graph = Graph::new();
    let object =
        direct::to_jvalue(value, Ctx::Set(element.clone()), &mut graph).map_err(Error::from)?;
    graph.add_item(object);
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
    direct::from_jvalue(&graph.items()[0], &graph, Ctx::Set(element.clone())).map_err(Error::from)
}

/// Serialize a unit-variant value into a Java enum root object.
pub fn to_vec_enum<T: Serialize>(value: &T, schema: &EnumSchema) -> Result<Vec<u8>> {
    validate_enum(schema)?;
    let mut graph = Graph::new();
    let object =
        direct::to_jvalue(value, Ctx::Enum(schema.clone()), &mut graph).map_err(Error::from)?;
    graph.add_item(object);
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
    direct::from_jvalue(&graph.items()[0], &graph, Ctx::Enum(schema.clone())).map_err(Error::from)
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

// ==================== JSON-view decoding (from_slice_roots) ====================

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
