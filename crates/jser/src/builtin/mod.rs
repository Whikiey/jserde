//! Built-in Java classes and collection views.
//!
//! P2 decodes `ArrayList`, `LinkedList`, `HashMap`, `LinkedHashMap` and
//! `HashSet`; their wire layouts differ from plain field-by-field objects and
//! are parsed from the preserved custom data blocks here. Semantic codecs for
//! `Date`, `BigInteger`, `BigDecimal` and `java.time.*` are added in P6.

use crate::error::{Error, Result};
use crate::value::{
    BlockData, ClassData, ClassDesc, FieldDesc, FieldType, Graph, Handle, JObject, JValue, MapView,
};
use crate::wire::reader::StreamReader;

pub const ARRAY_LIST: &str = "java.util.ArrayList";
pub const LINKED_LIST: &str = "java.util.LinkedList";
pub const HASH_MAP: &str = "java.util.HashMap";
pub const LINKED_HASH_MAP: &str = "java.util.LinkedHashMap";
pub const HASH_SET: &str = "java.util.HashSet";

fn layer<'a>(graph: &Graph, object: &'a JObject, name: &str) -> Result<Option<&'a ClassData>> {
    for class in &object.classes {
        if graph.descriptor(class.desc)?.name == name {
            return Ok(Some(class));
        }
    }
    Ok(None)
}

fn layer_field<'a>(class: &'a ClassData, name: &str) -> Option<&'a JValue> {
    class
        .fields
        .iter()
        .find(|(field, _)| field == name)
        .map(|(_, value)| value)
}

fn custom_contents<'a>(class: &'a ClassData, class_name: &str) -> Result<&'a [JValue]> {
    class.custom.as_deref().ok_or_else(|| Error::FieldMismatch {
        message: format!("`{class_name}` has no custom data"),
    })
}

fn header_block<'a>(contents: &'a [JValue], class_name: &str, width: usize) -> Result<&'a [u8]> {
    match contents.first() {
        Some(JValue::Block(block)) if block.bytes.len() == width => Ok(&block.bytes),
        Some(other) => Err(Error::FieldMismatch {
            message: format!(
                "`{class_name}` custom data starts with {other:?}, expected a {width}-byte block"
            ),
        }),
        None => Err(Error::FieldMismatch {
            message: format!("`{class_name}` custom data is empty"),
        }),
    }
}

fn collection_size(size: i32, class_name: &str) -> Result<usize> {
    usize::try_from(size).map_err(|_| Error::FieldMismatch {
        message: format!("`{class_name}` has a negative element count: {size}"),
    })
}

fn check_length(contents: &[JValue], size: usize, header: usize, class_name: &str) -> Result<()> {
    let expected = size
        .checked_add(header)
        .ok_or_else(|| Error::FieldMismatch {
            message: format!("`{class_name}` element count overflows"),
        })?;

    if contents.len() != expected {
        return Err(Error::FieldMismatch {
            message: format!(
                "`{class_name}` declares {size} element(s) but carries {} content item(s)",
                contents.len().saturating_sub(header)
            ),
        });
    }
    Ok(())
}

/// Map view for `HashMap`/`LinkedHashMap`.
pub fn map_view<'a>(graph: &'a Graph, value: &'a JValue) -> Result<Option<MapView<'a>>> {
    let Some(object) = graph.as_object(value)? else {
        return Ok(None);
    };
    let Some(hash_map) = layer(graph, object, HASH_MAP)? else {
        return Ok(None);
    };

    let load_factor = match layer_field(hash_map, "loadFactor") {
        Some(JValue::Float(load_factor)) => *load_factor,
        _ => {
            return Err(Error::FieldMismatch {
                message: "`java.util.HashMap` has no float `loadFactor` field".to_owned(),
            })
        }
    };

    let contents = custom_contents(hash_map, HASH_MAP)?;
    let header = header_block(contents, HASH_MAP, 8)?;
    let mut reader = StreamReader::new(header);
    let capacity = reader.read_i32()?;
    let size = collection_size(reader.read_i32()?, HASH_MAP)?;
    debug_assert!(reader.is_empty());

    let entry_count = size.checked_mul(2).ok_or_else(|| Error::FieldMismatch {
        message: "`java.util.HashMap` entry count overflows".to_owned(),
    })?;
    check_length(contents, entry_count, 1, HASH_MAP)?;

    let mut entries = Vec::with_capacity(size);
    let mut items = contents[1..].iter();
    for _ in 0..size {
        let key = items.next().expect("length checked above");
        let value = items.next().expect("length checked above");
        entries.push((key, value));
    }

    Ok(Some(MapView {
        entries,
        load_factor,
        capacity,
        ordered: layer(graph, object, LINKED_HASH_MAP)?.is_some(),
    }))
}

/// List view for `ArrayList`/`LinkedList`.
pub fn list_view<'a>(graph: &'a Graph, value: &'a JValue) -> Result<Option<Vec<&'a JValue>>> {
    let Some(object) = graph.as_object(value)? else {
        return Ok(None);
    };

    let (class, name) = if let Some(class) = layer(graph, object, ARRAY_LIST)? {
        (class, ARRAY_LIST)
    } else if let Some(class) = layer(graph, object, LINKED_LIST)? {
        (class, LINKED_LIST)
    } else {
        return Ok(None);
    };

    let contents = custom_contents(class, name)?;
    let header = header_block(contents, name, 4)?;
    let mut reader = StreamReader::new(header);
    let size = collection_size(reader.read_i32()?, name)?;
    check_length(contents, size, 1, name)?;

    Ok(Some(contents[1..].iter().collect()))
}

/// Set view for `HashSet`.
pub fn set_view<'a>(graph: &'a Graph, value: &'a JValue) -> Result<Option<Vec<&'a JValue>>> {
    let Some(object) = graph.as_object(value)? else {
        return Ok(None);
    };
    let Some(set) = layer(graph, object, HASH_SET)? else {
        return Ok(None);
    };

    let contents = custom_contents(set, HASH_SET)?;
    let header = header_block(contents, HASH_SET, 12)?;
    let mut reader = StreamReader::new(header);
    let _capacity = reader.read_i32()?;
    let _load_factor = reader.read_f32()?;
    let size = collection_size(reader.read_i32()?, HASH_SET)?;
    check_length(contents, size, 1, HASH_SET)?;

    Ok(Some(contents[1..].iter().collect()))
}

/// Build a `java.util.HashMap` from string-to-string entries.
///
/// This is the caller-facing construction path used to prove that collection
/// encoding is not limited to re-encoding fixture bytes. The classdata uses
/// `loadFactor`/`threshold`; the custom block carries `capacity`/`size` plus the
/// entries, matching JDK 17 (`29`).
pub fn new_hash_map(graph: &mut Graph, entries: &[(&str, &str)]) -> Result<Handle> {
    let size = i32::try_from(entries.len())
        .map_err(|_| Error::InvalidSchema("HashMap has too many entries".to_owned()))?;
    let capacity = table_size_for(entries.len())?;
    let threshold = (capacity as f32 * 0.75) as i32;

    let desc = graph.add_class_desc(ClassDesc {
        name: HASH_MAP.to_owned(),
        suid: 362_498_820_763_181_265,
        flags: 0x03,
        fields: vec![
            FieldDesc {
                name: "loadFactor".to_owned(),
                typecode: FieldType::Float,
                class_name1: None,
                class_name_handle: None,
            },
            FieldDesc {
                name: "threshold".to_owned(),
                typecode: FieldType::Int,
                class_name1: None,
                class_name_handle: None,
            },
        ],
        super_desc: None,
        annotations: Vec::new(),
    })?;

    let mut header = Vec::with_capacity(8);
    header.extend_from_slice(&capacity.to_be_bytes());
    header.extend_from_slice(&size.to_be_bytes());

    let mut custom = Vec::with_capacity(1 + entries.len() * 2);
    custom.push(JValue::Block(BlockData {
        bytes: header,
        long: false,
    }));
    for (key, value) in entries {
        let key = graph.new_string(*key);
        let value = graph.new_string(*value);
        custom.push(JValue::Str(key));
        custom.push(JValue::Str(value));
    }

    graph.new_object(vec![ClassData {
        desc,
        fields: vec![
            ("loadFactor".to_owned(), JValue::Float(0.75)),
            ("threshold".to_owned(), JValue::Int(threshold)),
        ],
        custom: Some(custom),
    }])
}

fn table_size_for(size: usize) -> Result<i32> {
    let needed = ((size as f64) / 0.75).ceil() as usize;
    let mut capacity = 16usize;
    while capacity < needed {
        capacity = capacity
            .checked_mul(2)
            .ok_or_else(|| Error::InvalidSchema("HashMap capacity overflows".to_owned()))?;
    }
    i32::try_from(capacity)
        .map_err(|_| Error::InvalidSchema("HashMap capacity overflows".to_owned()))
}
