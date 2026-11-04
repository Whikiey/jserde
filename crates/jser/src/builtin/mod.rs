//! Built-in Java classes and collection views.
//!
//! P2 decodes `ArrayList`, `LinkedList`, `HashMap`, `LinkedHashMap` and
//! `HashSet`; their wire layouts differ from plain field-by-field objects and
//! are parsed from the preserved custom data blocks here.
//!
//! P6 adds semantic views for `Date`, `BigInteger`, `BigDecimal` and
//! `java.time.*` in [`semantic`]; the reader still keeps them as generic
//! `JValue::Obj` values so byte replay is unaffected.

pub mod semantic;

use crate::error::{Error, Result};
use crate::value::{
    BlockData, ClassData, ClassDesc, FieldDesc, FieldType, Graph, Handle, JObject, JValue, MapView,
};
use crate::wire::reader::StreamReader;
pub use semantic::{
    BigDecimal, BigInteger, BitSet, BuiltinValue, Currency, Duration, Instant, JavaDate, LocalDate,
    LocalDateTime, LocalTime, Locale, MonthDay, OffsetDateTime, OffsetTime, Pattern, Period,
    Registry, SqlDate, SqlTime, SqlTimestamp, StringBufferValue, StringBuilderValue, Uuid, Year,
    YearMonth, ZoneId, ZoneOffset, ZonedDateTime,
};

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
        let key = items.next().ok_or_else(|| Error::FieldMismatch {
            message: "java.util.HashMap entry count is inconsistent".to_owned(),
        })?;
        let value = items.next().ok_or_else(|| Error::FieldMismatch {
            message: "java.util.HashMap entry count is inconsistent".to_owned(),
        })?;
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
    let mut values = Vec::with_capacity(entries.len());
    for (key, value) in entries {
        values.push((
            JValue::Str(graph.new_string(*key)),
            JValue::Str(graph.new_string(*value)),
        ));
    }
    new_hash_map_entries(graph, values)
}

/// Build a `java.util.HashMap` from already-built entry values.
///
/// Keys/values must be Java objects (or `null`), not inline primitives: the
/// entries are written as object contents after the custom data header.
pub fn new_hash_map_entries(graph: &mut Graph, entries: Vec<(JValue, JValue)>) -> Result<Handle> {
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
        let key = box_primitive(graph, key)?;
        let value = box_primitive(graph, value)?;
        check_collection_value(&key, "HashMap key")?;
        check_collection_value(&value, "HashMap value")?;
        custom.push(key);
        custom.push(value);
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

/// Build a `java.util.LinkedHashMap` from already-built entry values.
///
/// `LinkedHashMap` adds an `accessOrder` field on top of `HashMap`; the entries
/// live in the parent `HashMap` layer's custom block.
pub fn new_linked_hash_map_entries(
    graph: &mut Graph,
    entries: Vec<(JValue, JValue)>,
    access_order: bool,
) -> Result<Handle> {
    let size = i32::try_from(entries.len())
        .map_err(|_| Error::InvalidSchema("LinkedHashMap has too many entries".to_owned()))?;
    let capacity = table_size_for(entries.len())?;
    let threshold = (capacity as f32 * 0.75) as i32;

    let hash_map = graph.add_class_desc(ClassDesc {
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

    let linked = graph.add_class_desc(ClassDesc {
        name: LINKED_HASH_MAP.to_owned(),
        suid: 3_801_124_242_820_219_131,
        flags: 0x02,
        fields: vec![FieldDesc {
            name: "accessOrder".to_owned(),
            typecode: FieldType::Boolean,
            class_name1: None,
            class_name_handle: None,
        }],
        super_desc: Some(hash_map),
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
        let key = box_primitive(graph, key)?;
        let value = box_primitive(graph, value)?;
        check_collection_value(&key, "LinkedHashMap key")?;
        check_collection_value(&value, "LinkedHashMap value")?;
        custom.push(key);
        custom.push(value);
    }

    graph.new_object(vec![
        ClassData {
            desc: hash_map,
            fields: vec![
                ("loadFactor".to_owned(), JValue::Float(0.75)),
                ("threshold".to_owned(), JValue::Int(threshold)),
            ],
            custom: Some(custom),
        },
        ClassData {
            desc: linked,
            fields: vec![("accessOrder".to_owned(), JValue::Bool(access_order))],
            custom: None,
        },
    ])
}

/// Build a `java.util.ArrayList` from object elements.
pub fn new_array_list(graph: &mut Graph, elements: Vec<JValue>) -> Result<Handle> {
    let elements = box_elements(graph, elements)?;
    for element in &elements {
        check_collection_value(element, "ArrayList element")?;
    }

    let desc = graph.add_class_desc(ClassDesc {
        name: ARRAY_LIST.to_owned(),
        suid: 8_683_452_581_122_892_189,
        flags: 0x03,
        fields: vec![FieldDesc {
            name: "size".to_owned(),
            typecode: FieldType::Int,
            class_name1: None,
            class_name_handle: None,
        }],
        super_desc: None,
        annotations: Vec::new(),
    })?;

    let size = i32::try_from(elements.len())
        .map_err(|_| Error::InvalidSchema("ArrayList has too many elements".to_owned()))?;
    let mut custom = Vec::with_capacity(1 + elements.len());
    custom.push(JValue::Block(BlockData {
        bytes: size.to_be_bytes().to_vec(),
        long: false,
    }));
    custom.extend(elements);

    graph.new_object(vec![ClassData {
        desc,
        fields: vec![("size".to_owned(), JValue::Int(size))],
        custom: Some(custom),
    }])
}

/// Build a `java.util.HashSet` from object elements.
pub fn new_hash_set(graph: &mut Graph, elements: Vec<JValue>) -> Result<Handle> {
    let elements = box_elements(graph, elements)?;
    for element in &elements {
        check_collection_value(element, "HashSet element")?;
    }

    let desc = graph.add_class_desc(ClassDesc {
        name: HASH_SET.to_owned(),
        suid: -5_024_744_406_713_321_676,
        flags: 0x03,
        fields: Vec::new(),
        super_desc: None,
        annotations: Vec::new(),
    })?;

    let size = i32::try_from(elements.len())
        .map_err(|_| Error::InvalidSchema("HashSet has too many elements".to_owned()))?;
    let capacity = table_size_for(elements.len())?;
    let mut header = Vec::with_capacity(12);
    header.extend_from_slice(&capacity.to_be_bytes());
    header.extend_from_slice(&0.75f32.to_bits().to_be_bytes());
    header.extend_from_slice(&size.to_be_bytes());

    let mut custom = Vec::with_capacity(1 + elements.len());
    custom.push(JValue::Block(BlockData {
        bytes: header,
        long: false,
    }));
    custom.extend(elements);

    graph.new_object(vec![ClassData {
        desc,
        fields: Vec::new(),
        custom: Some(custom),
    }])
}

fn check_collection_value(value: &JValue, what: &str) -> Result<()> {
    match value {
        JValue::Null | JValue::Str(_) | JValue::Arr(_) | JValue::Obj(_) | JValue::Enum(_) => Ok(()),
        _ => Err(Error::FieldMismatch {
            message: format!("{what} must be a Java object or null, got {value:?}"),
        }),
    }
}

fn box_elements(graph: &mut Graph, elements: Vec<JValue>) -> Result<Vec<JValue>> {
    let mut boxed = Vec::with_capacity(elements.len());
    for element in elements {
        boxed.push(box_primitive(graph, element)?);
    }
    Ok(boxed)
}

/// Box a primitive value into its `java.lang.*` wrapper object.
///
/// Collection elements are objects in Java, so `Vec<i32>`/`HashMap<String,i32>`
/// must be encoded as `Integer`/... objects rather than bare classdata bytes.
pub fn box_primitive(graph: &mut Graph, value: JValue) -> Result<JValue> {
    let boxed = match value {
        JValue::Bool(value) => box_wrapper(
            graph,
            "java.lang.Boolean",
            -3_665_804_199_014_368_530,
            FieldType::Boolean,
            JValue::Bool(value),
            false,
        )?,
        JValue::Byte(value) => box_wrapper(
            graph,
            "java.lang.Byte",
            -7_183_698_231_559_129_828,
            FieldType::Byte,
            JValue::Byte(value),
            true,
        )?,
        JValue::Char(value) => box_wrapper(
            graph,
            "java.lang.Character",
            3_786_198_910_865_385_080,
            FieldType::Char,
            JValue::Char(value),
            false,
        )?,
        JValue::Short(value) => box_wrapper(
            graph,
            "java.lang.Short",
            7_515_723_908_773_894_738,
            FieldType::Short,
            JValue::Short(value),
            true,
        )?,
        JValue::Int(value) => box_wrapper(
            graph,
            "java.lang.Integer",
            1_360_826_667_806_852_920,
            FieldType::Int,
            JValue::Int(value),
            true,
        )?,
        JValue::Long(value) => box_wrapper(
            graph,
            "java.lang.Long",
            4_290_774_380_558_885_855,
            FieldType::Long,
            JValue::Long(value),
            true,
        )?,
        JValue::Float(value) => box_wrapper(
            graph,
            "java.lang.Float",
            -2_671_257_302_660_747_028,
            FieldType::Float,
            JValue::Float(value),
            true,
        )?,
        JValue::Double(value) => box_wrapper(
            graph,
            "java.lang.Double",
            -9_172_774_392_245_257_468,
            FieldType::Double,
            JValue::Double(value),
            true,
        )?,
        JValue::Null | JValue::Str(_) | JValue::Arr(_) | JValue::Obj(_) | JValue::Enum(_) => {
            return Ok(value)
        }
        JValue::Block(_) | JValue::Reset => {
            return Err(Error::FieldMismatch {
                message: "a block/reset cannot be a collection element".to_owned(),
            })
        }
    };
    Ok(boxed)
}

fn box_wrapper(
    graph: &mut Graph,
    class: &str,
    suid: i64,
    typecode: FieldType,
    value: JValue,
    has_number_super: bool,
) -> Result<JValue> {
    let number = if has_number_super {
        Some(graph.add_class_desc(ClassDesc {
            name: "java.lang.Number".to_owned(),
            suid: -8_742_448_824_652_078_965,
            flags: 0x02,
            fields: Vec::new(),
            super_desc: None,
            annotations: Vec::new(),
        })?)
    } else {
        None
    };

    let desc = graph.add_class_desc(ClassDesc {
        name: class.to_owned(),
        suid,
        flags: 0x02,
        fields: vec![FieldDesc {
            name: "value".to_owned(),
            typecode,
            class_name1: None,
            class_name_handle: None,
        }],
        super_desc: number,
        annotations: Vec::new(),
    })?;

    let mut classes = Vec::new();
    if let Some(number) = number {
        classes.push(ClassData {
            desc: number,
            fields: Vec::new(),
            custom: None,
        });
    }
    classes.push(ClassData {
        desc,
        fields: vec![("value".to_owned(), value)],
        custom: None,
    });

    Ok(JValue::Obj(graph.new_object(classes)?))
}

/// Build a Java array with a known one-dimensional array class.
///
/// Only the array classes needed by the derive layer are supported; object
/// elements must already be references (or `null`).
pub fn new_java_array(
    graph: &mut Graph,
    class_name: &str,
    elements: Vec<JValue>,
) -> Result<Handle> {
    let suid = array_suid(class_name).ok_or_else(|| {
        Error::InvalidSchema(format!("unsupported Java array class `{class_name}`"))
    })?;

    if let Some(typecode) = primitive_array_element(class_name) {
        for element in &elements {
            if !matches_primitive(typecode, element) {
                return Err(Error::FieldMismatch {
                    message: format!(
                        "array `{class_name}` expects {typecode:?} elements, got {element:?}"
                    ),
                });
            }
        }
    } else {
        for element in &elements {
            check_collection_value(element, "array element")?;
        }
    }

    let desc = graph.add_class_desc(ClassDesc {
        name: class_name.to_owned(),
        suid,
        flags: 0x02,
        fields: Vec::new(),
        super_desc: None,
        annotations: Vec::new(),
    })?;
    graph.new_array(desc, elements)
}

fn primitive_array_element(class_name: &str) -> Option<FieldType> {
    let bytes = class_name.as_bytes();
    if bytes.len() == 2 && bytes[0] == b'[' {
        let typecode = FieldType::from_code(bytes[1])?;
        if typecode.is_primitive() {
            return Some(typecode);
        }
    }
    None
}

fn matches_primitive(typecode: FieldType, value: &JValue) -> bool {
    matches!(
        (typecode, value),
        (FieldType::Byte, JValue::Byte(_))
            | (FieldType::Char, JValue::Char(_))
            | (FieldType::Double, JValue::Double(_))
            | (FieldType::Float, JValue::Float(_))
            | (FieldType::Int, JValue::Int(_))
            | (FieldType::Long, JValue::Long(_))
            | (FieldType::Short, JValue::Short(_))
            | (FieldType::Boolean, JValue::Bool(_))
    )
}

fn array_suid(class_name: &str) -> Option<i64> {
    crate::suid::array_class_suid(class_name).ok()
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
