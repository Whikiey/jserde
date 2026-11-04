//! Optional, schema-driven serde bridge for `jser`.
//!
//! serde cannot carry Java class names, type codes or `serialVersionUID`, so
//! this crate requires an explicit [`JavaSchema`]. It supports a **single root
//! POJO** whose fields may be scalars, `String`, `Option<T>`, arrays, or nested
//! POJOs via [`SchemaField::nested`]. Cycles, shared references, `TC_RESET`,
//! multiple roots, Map/Set fields and arrays of objects are rejected.

use jser::builtin::new_java_array;
use jser::{
    decode_with_allowlist, encode_stream, Allowlist, ClassData, ClassDesc, Error, FieldDesc,
    FieldType, Graph, JValue, Result,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{Map, Value};

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
        if field.nested.is_some() && field.typecode != FieldType::Object {
            return Err(Error::InvalidSchema(format!(
                "nested field `{}` must have typecode Object",
                field.name
            )));
        }
        if let Some(nested) = &field.nested {
            validate_schema(nested)?;
        }
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
        let value = match &field.nested {
            Some(nested) if !raw.is_null() => encode_object(raw, nested, graph)?,
            Some(_) => JValue::Null,
            None => json_to_jvalue(raw, field.typecode, field.class_name1, graph)?,
        };
        field_descs.push(FieldDesc {
            name: field.name.to_owned(),
            typecode: field.typecode,
            class_name1: field.class_name1.map(str::to_owned),
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
        let json = match &field.nested {
            Some(nested) if !matches!(field_value, JValue::Null) => {
                decode_object(graph, field_value, nested)?
            }
            Some(_) => Value::Null,
            None => jvalue_to_json(graph, field_value, field.typecode, field.class_name1)?,
        };
        object.insert(field.name.to_owned(), json);
    }

    Ok(Value::Object(object))
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
