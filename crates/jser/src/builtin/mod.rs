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
pub const LINKED_HASH_SET: &str = "java.util.LinkedHashSet";
pub const TREE_MAP: &str = "java.util.TreeMap";
pub const TREE_SET: &str = "java.util.TreeSet";
pub const ARRAY_DEQUE: &str = "java.util.ArrayDeque";
pub const PRIORITY_QUEUE: &str = "java.util.PriorityQueue";
pub const VECTOR: &str = "java.util.Vector";
pub const STACK: &str = "java.util.Stack";
pub const HASHTABLE: &str = "java.util.Hashtable";
pub const PROPERTIES: &str = "java.util.Properties";
pub const ARRAYS_ARRAY_LIST: &str = "java.util.Arrays$ArrayList";
pub const UNMODIFIABLE_LIST: &str = "java.util.Collections$UnmodifiableList";
pub const SYNCHRONIZED_LIST: &str = "java.util.Collections$SynchronizedList";

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

/// Map view for `HashMap`/`LinkedHashMap`, `TreeMap` and `Hashtable`/`Properties`.
pub fn map_view<'a>(graph: &'a Graph, value: &'a JValue) -> Result<Option<MapView<'a>>> {
    let Some(object) = graph.as_object(value)? else {
        return Ok(None);
    };
    if let Some(hash_map) = layer(graph, object, HASH_MAP)? {
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

        let entries = pair_entries(&contents[1..], size, HASH_MAP)?;
        return Ok(Some(MapView {
            entries,
            load_factor,
            capacity,
            ordered: layer(graph, object, LINKED_HASH_MAP)?.is_some(),
        }));
    }
    if let Some(tree_map) = layer(graph, object, TREE_MAP)? {
        let contents = custom_contents(tree_map, TREE_MAP)?;
        let header = header_block(contents, TREE_MAP, 4)?;
        let mut reader = StreamReader::new(header);
        let size = collection_size(reader.read_i32()?, TREE_MAP)?;
        let entry_count = size.checked_mul(2).ok_or_else(|| Error::FieldMismatch {
            message: "`java.util.TreeMap` entry count overflows".to_owned(),
        })?;
        check_length(contents, entry_count, 1, TREE_MAP)?;
        return Ok(Some(MapView {
            entries: pair_entries(&contents[1..], size, TREE_MAP)?,
            load_factor: 0.0,
            capacity: i32::try_from(size).unwrap_or(i32::MAX),
            ordered: true,
        }));
    }
    if let Some(table) = layer(graph, object, HASHTABLE)? {
        let contents = custom_contents(table, HASHTABLE)?;
        let header = header_block(contents, HASHTABLE, 8)?;
        let mut reader = StreamReader::new(header);
        let capacity = reader.read_i32()?;
        let size = collection_size(reader.read_i32()?, HASHTABLE)?;
        let entry_count = size.checked_mul(2).ok_or_else(|| Error::FieldMismatch {
            message: "`java.util.Hashtable` entry count overflows".to_owned(),
        })?;
        check_length(contents, entry_count, 1, HASHTABLE)?;
        let load_factor = match layer_field(table, "loadFactor") {
            Some(JValue::Float(load_factor)) => *load_factor,
            _ => 0.75,
        };
        return Ok(Some(MapView {
            entries: pair_entries(&contents[1..], size, HASHTABLE)?,
            load_factor,
            capacity,
            ordered: false,
        }));
    }

    Ok(None)
}

fn pair_entries<'a>(
    items: &'a [JValue],
    size: usize,
    name: &str,
) -> Result<Vec<(&'a JValue, &'a JValue)>> {
    let mut entries = Vec::with_capacity(size);
    let mut items = items.iter();
    for _ in 0..size {
        let key = items.next().ok_or_else(|| Error::FieldMismatch {
            message: format!("{name} entry count is inconsistent"),
        })?;
        let value = items.next().ok_or_else(|| Error::FieldMismatch {
            message: format!("{name} entry count is inconsistent"),
        })?;
        entries.push((key, value));
    }
    Ok(entries)
}

/// List view for `ArrayList`/`LinkedList`, `Vector`/`Stack`, `ArrayDeque`,
/// `PriorityQueue`, `Arrays$ArrayList` and the unmodifiable/synchronized list
/// wrappers.
pub fn list_view<'a>(graph: &'a Graph, value: &'a JValue) -> Result<Option<Vec<&'a JValue>>> {
    let Some(object) = graph.as_object(value)? else {
        return Ok(None);
    };

    if let Some(class) = layer(graph, object, ARRAY_LIST)? {
        return custom_elements(class, ARRAY_LIST, 4);
    }
    if let Some(class) = layer(graph, object, LINKED_LIST)? {
        return custom_elements(class, LINKED_LIST, 4);
    }
    if let Some(class) = layer(graph, object, ARRAY_DEQUE)? {
        return custom_elements(class, ARRAY_DEQUE, 4);
    }
    if let Some(class) = layer(graph, object, PRIORITY_QUEUE)? {
        let contents = custom_contents(class, PRIORITY_QUEUE)?;
        let header = header_block(contents, PRIORITY_QUEUE, 4)?;
        let mut reader = StreamReader::new(header);
        let _capacity = reader.read_i32()?;
        let size =
            graph.as_i32(
                layer_field(class, "size").ok_or_else(|| Error::FieldMismatch {
                    message: "java.util.PriorityQueue has no size field".to_owned(),
                })?,
            )?;
        let size = collection_size(size, PRIORITY_QUEUE)?;
        check_length(contents, size, 1, PRIORITY_QUEUE)?;
        return Ok(Some(contents[1..].iter().collect()));
    }
    if let Some(class) = layer(graph, object, VECTOR)? {
        let count = graph.as_i32(layer_field(class, "elementCount").ok_or_else(|| {
            Error::FieldMismatch {
                message: "java.util.Vector has no elementCount field".to_owned(),
            }
        })?)?;
        let count = collection_size(count, VECTOR)?;
        let data = layer_field(class, "elementData").ok_or_else(|| Error::FieldMismatch {
            message: "java.util.Vector has no elementData field".to_owned(),
        })?;
        let elements = graph.as_array(data)?.ok_or_else(|| Error::FieldMismatch {
            message: "java.util.Vector elementData is not an object array".to_owned(),
        })?;
        if count > elements.len() {
            return Err(Error::FieldMismatch {
                message: format!("java.util.Vector elementCount {count} exceeds capacity"),
            });
        }
        return Ok(Some(elements[..count].iter().collect()));
    }
    if let Some(class) = layer(graph, object, ARRAYS_ARRAY_LIST)? {
        let data = layer_field(class, "a").ok_or_else(|| Error::FieldMismatch {
            message: "java.util.Arrays$ArrayList has no `a` field".to_owned(),
        })?;
        let elements = graph.as_array(data)?.ok_or_else(|| Error::FieldMismatch {
            message: "java.util.Arrays$ArrayList `a` is not an object array".to_owned(),
        })?;
        return Ok(Some(elements.iter().collect()));
    }
    for wrapper in [UNMODIFIABLE_LIST, SYNCHRONIZED_LIST] {
        if let Some(class) = layer(graph, object, wrapper)? {
            let backing = layer_field(class, "list").ok_or_else(|| Error::FieldMismatch {
                message: format!("{wrapper} has no `list` field"),
            })?;
            return list_view(graph, backing);
        }
    }

    Ok(None)
}

fn custom_elements<'a>(
    class: &'a ClassData,
    name: &str,
    header_width: usize,
) -> Result<Option<Vec<&'a JValue>>> {
    let contents = class
        .custom
        .as_deref()
        .ok_or_else(|| Error::FieldMismatch {
            message: format!("`{name}` has no custom data"),
        })?;
    let header = header_block(contents, name, header_width)?;
    let mut reader = StreamReader::new(header);
    let size = collection_size(reader.read_i32()?, name)?;
    check_length(contents, size, 1, name)?;
    Ok(Some(contents[1..].iter().collect()))
}

/// Set view for `HashSet`/`LinkedHashSet` and `TreeSet`.
pub fn set_view<'a>(graph: &'a Graph, value: &'a JValue) -> Result<Option<Vec<&'a JValue>>> {
    let Some(object) = graph.as_object(value)? else {
        return Ok(None);
    };
    if let Some(set) = layer(graph, object, HASH_SET)? {
        let contents = custom_contents(set, HASH_SET)?;
        let header = header_block(contents, HASH_SET, 12)?;
        let mut reader = StreamReader::new(header);
        let _capacity = reader.read_i32()?;
        let _load_factor = reader.read_f32()?;
        let size = collection_size(reader.read_i32()?, HASH_SET)?;
        check_length(contents, size, 1, HASH_SET)?;
        return Ok(Some(contents[1..].iter().collect()));
    }
    if let Some(set) = layer(graph, object, TREE_SET)? {
        let contents = custom_contents(set, TREE_SET)?;
        if contents.len() < 2 || !matches!(contents[0], JValue::Null) {
            return Err(Error::FieldMismatch {
                message: "java.util.TreeSet comparator is not null".to_owned(),
            });
        }
        let header = header_block(&contents[1..], TREE_SET, 4)?;
        let mut reader = StreamReader::new(header);
        let size = collection_size(reader.read_i32()?, TREE_SET)?;
        check_length(contents, size, 2, TREE_SET)?;
        return Ok(Some(contents[2..].iter().collect()));
    }

    Ok(None)
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

/// Build a `java.util.LinkedHashSet` from object elements.
pub fn new_linked_hash_set(graph: &mut Graph, elements: Vec<JValue>) -> Result<Handle> {
    let elements = box_elements(graph, elements)?;
    for element in &elements {
        check_collection_value(element, "LinkedHashSet element")?;
    }
    let hash_set = graph.add_class_desc(ClassDesc {
        name: HASH_SET.to_owned(),
        suid: -5_024_744_406_713_321_676,
        flags: 0x03,
        fields: Vec::new(),
        super_desc: None,
        annotations: Vec::new(),
    })?;
    let linked = graph.add_class_desc(ClassDesc {
        name: LINKED_HASH_SET.to_owned(),
        suid: -2_851_667_679_971_038_690,
        flags: 0x02,
        fields: Vec::new(),
        super_desc: Some(hash_set),
        annotations: Vec::new(),
    })?;
    let size = i32::try_from(elements.len())
        .map_err(|_| Error::InvalidSchema("LinkedHashSet has too many elements".to_owned()))?;
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
    graph.new_object(vec![
        ClassData {
            desc: hash_set,
            fields: Vec::new(),
            custom: Some(custom),
        },
        ClassData {
            desc: linked,
            fields: Vec::new(),
            custom: None,
        },
    ])
}

/// Build a `java.util.TreeMap` from object entries (natural ordering).
pub fn new_tree_map(graph: &mut Graph, entries: Vec<(JValue, JValue)>) -> Result<Handle> {
    let mut entries = entries;
    for (key, value) in &mut entries {
        *key = box_primitive(graph, key.clone())?;
        *value = box_primitive(graph, value.clone())?;
        check_collection_value(key, "TreeMap key")?;
        check_collection_value(value, "TreeMap value")?;
    }
    let size = i32::try_from(entries.len())
        .map_err(|_| Error::InvalidSchema("TreeMap has too many entries".to_owned()))?;
    let desc = graph.add_class_desc(ClassDesc {
        name: TREE_MAP.to_owned(),
        suid: 919_286_545_866_124_006,
        flags: 0x03,
        fields: vec![FieldDesc {
            name: "comparator".to_owned(),
            typecode: FieldType::Object,
            class_name1: Some("Ljava/util/Comparator;".to_owned()),
            class_name_handle: None,
        }],
        super_desc: None,
        annotations: Vec::new(),
    })?;
    let mut custom = Vec::with_capacity(1 + entries.len() * 2);
    custom.push(JValue::Block(BlockData {
        bytes: size.to_be_bytes().to_vec(),
        long: false,
    }));
    for (key, value) in entries {
        custom.push(key);
        custom.push(value);
    }
    graph.new_object(vec![ClassData {
        desc,
        fields: vec![("comparator".to_owned(), JValue::Null)],
        custom: Some(custom),
    }])
}

/// Build a `java.util.TreeSet` from object elements (natural ordering).
pub fn new_tree_set(graph: &mut Graph, elements: Vec<JValue>) -> Result<Handle> {
    let elements = box_elements(graph, elements)?;
    for element in &elements {
        check_collection_value(element, "TreeSet element")?;
    }
    let size = i32::try_from(elements.len())
        .map_err(|_| Error::InvalidSchema("TreeSet has too many elements".to_owned()))?;
    let desc = graph.add_class_desc(ClassDesc {
        name: TREE_SET.to_owned(),
        suid: -2_479_143_000_061_671_589,
        flags: 0x03,
        fields: Vec::new(),
        super_desc: None,
        annotations: Vec::new(),
    })?;
    let mut custom = Vec::with_capacity(2 + elements.len());
    custom.push(JValue::Null);
    custom.push(JValue::Block(BlockData {
        bytes: size.to_be_bytes().to_vec(),
        long: false,
    }));
    custom.extend(elements);
    graph.new_object(vec![ClassData {
        desc,
        fields: Vec::new(),
        custom: Some(custom),
    }])
}

/// Build a `java.util.ArrayDeque` from object elements.
pub fn new_array_deque(graph: &mut Graph, elements: Vec<JValue>) -> Result<Handle> {
    let elements = box_elements(graph, elements)?;
    for element in &elements {
        check_collection_value(element, "ArrayDeque element")?;
    }
    let size = i32::try_from(elements.len())
        .map_err(|_| Error::InvalidSchema("ArrayDeque has too many elements".to_owned()))?;
    let desc = graph.add_class_desc(ClassDesc {
        name: ARRAY_DEQUE.to_owned(),
        suid: 2_340_985_798_034_038_923,
        flags: 0x03,
        fields: Vec::new(),
        super_desc: None,
        annotations: Vec::new(),
    })?;
    let mut custom = Vec::with_capacity(1 + elements.len());
    custom.push(JValue::Block(BlockData {
        bytes: size.to_be_bytes().to_vec(),
        long: false,
    }));
    custom.extend(elements);
    graph.new_object(vec![ClassData {
        desc,
        fields: Vec::new(),
        custom: Some(custom),
    }])
}

/// Build a `java.util.PriorityQueue` (natural ordering) from object elements.
pub fn new_priority_queue(graph: &mut Graph, elements: Vec<JValue>) -> Result<Handle> {
    let elements = box_elements(graph, elements)?;
    for element in &elements {
        check_collection_value(element, "PriorityQueue element")?;
    }
    let size = i32::try_from(elements.len())
        .map_err(|_| Error::InvalidSchema("PriorityQueue has too many elements".to_owned()))?;
    let capacity = size.max(1);
    let desc = graph.add_class_desc(ClassDesc {
        name: PRIORITY_QUEUE.to_owned(),
        suid: -7_720_805_057_305_804_111,
        flags: 0x03,
        fields: vec![
            FieldDesc {
                name: "size".to_owned(),
                typecode: FieldType::Int,
                class_name1: None,
                class_name_handle: None,
            },
            FieldDesc {
                name: "comparator".to_owned(),
                typecode: FieldType::Object,
                class_name1: Some("Ljava/util/Comparator;".to_owned()),
                class_name_handle: None,
            },
        ],
        super_desc: None,
        annotations: Vec::new(),
    })?;
    let mut custom = Vec::with_capacity(1 + elements.len());
    custom.push(JValue::Block(BlockData {
        bytes: capacity.to_be_bytes().to_vec(),
        long: false,
    }));
    custom.extend(elements);
    graph.new_object(vec![ClassData {
        desc,
        fields: vec![
            ("size".to_owned(), JValue::Int(size)),
            ("comparator".to_owned(), JValue::Null),
        ],
        custom: Some(custom),
    }])
}

/// Build a `java.util.Vector` from object elements (capacity = size).
pub fn new_vector(graph: &mut Graph, elements: Vec<JValue>) -> Result<Handle> {
    new_vector_like(graph, elements, false)
}

/// Build a `java.util.Stack` from object elements (capacity = max(10, size)).
pub fn new_stack(graph: &mut Graph, elements: Vec<JValue>) -> Result<Handle> {
    new_vector_like(graph, elements, true)
}

fn new_vector_like(graph: &mut Graph, elements: Vec<JValue>, stack: bool) -> Result<Handle> {
    let elements = box_elements(graph, elements)?;
    for element in &elements {
        check_collection_value(element, "Vector element")?;
    }
    let count = i32::try_from(elements.len())
        .map_err(|_| Error::InvalidSchema("Vector has too many elements".to_owned()))?;
    let capacity = if stack {
        elements.len().max(10)
    } else {
        elements.len()
    };
    let mut data = elements;
    data.resize(capacity, JValue::Null);
    let array = new_java_array(graph, "[Ljava.lang.Object;", data)?;
    let vector = graph.add_class_desc(ClassDesc {
        name: VECTOR.to_owned(),
        suid: -2_767_605_614_048_989_439,
        flags: 0x03,
        fields: vec![
            FieldDesc {
                name: "capacityIncrement".to_owned(),
                typecode: FieldType::Int,
                class_name1: None,
                class_name_handle: None,
            },
            FieldDesc {
                name: "elementCount".to_owned(),
                typecode: FieldType::Int,
                class_name1: None,
                class_name_handle: None,
            },
            FieldDesc {
                name: "elementData".to_owned(),
                typecode: FieldType::Object,
                class_name1: Some("[Ljava.lang.Object;".to_owned()),
                class_name_handle: None,
            },
        ],
        super_desc: None,
        annotations: Vec::new(),
    })?;
    let mut classes = vec![ClassData {
        desc: vector,
        fields: vec![
            ("capacityIncrement".to_owned(), JValue::Int(0)),
            ("elementCount".to_owned(), JValue::Int(count)),
            ("elementData".to_owned(), JValue::Arr(array)),
        ],
        custom: Some(Vec::new()),
    }];
    if stack {
        let stack_desc = graph.add_class_desc(ClassDesc {
            name: STACK.to_owned(),
            suid: 1_224_463_164_541_339_165,
            flags: 0x02,
            fields: Vec::new(),
            super_desc: Some(vector),
            annotations: Vec::new(),
        })?;
        classes.push(ClassData {
            desc: stack_desc,
            fields: Vec::new(),
            custom: None,
        });
    }
    graph.new_object(classes)
}

/// Build a `java.util.Hashtable` from object entries.
pub fn new_hashtable(graph: &mut Graph, entries: Vec<(JValue, JValue)>) -> Result<Handle> {
    new_hashtable_like(graph, entries, false)
}

/// Build a `java.util.Properties` from object entries (defaults = null).
pub fn new_properties(graph: &mut Graph, entries: Vec<(JValue, JValue)>) -> Result<Handle> {
    new_hashtable_like(graph, entries, true)
}

fn new_hashtable_like(
    graph: &mut Graph,
    entries: Vec<(JValue, JValue)>,
    properties: bool,
) -> Result<Handle> {
    let mut entries = entries;
    for (key, value) in &mut entries {
        *key = box_primitive(graph, key.clone())?;
        *value = box_primitive(graph, value.clone())?;
        check_collection_value(key, "Hashtable key")?;
        check_collection_value(value, "Hashtable value")?;
    }
    let size = i32::try_from(entries.len())
        .map_err(|_| Error::InvalidSchema("Hashtable has too many entries".to_owned()))?;
    let capacity = 11i32.max(size.saturating_mul(2));
    let threshold = (capacity as f32 * 0.75) as i32;
    let table = graph.add_class_desc(ClassDesc {
        name: HASHTABLE.to_owned(),
        suid: 1_421_746_759_512_286_392,
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
        custom.push(key);
        custom.push(value);
    }
    let mut classes = vec![ClassData {
        desc: table,
        fields: vec![
            ("loadFactor".to_owned(), JValue::Float(0.75)),
            ("threshold".to_owned(), JValue::Int(threshold)),
        ],
        custom: Some(custom),
    }];
    if properties {
        let properties_desc = graph.add_class_desc(ClassDesc {
            name: PROPERTIES.to_owned(),
            suid: 4_112_578_634_029_874_840,
            flags: 0x02,
            fields: vec![FieldDesc {
                name: "defaults".to_owned(),
                typecode: FieldType::Object,
                class_name1: Some("Ljava/util/Properties;".to_owned()),
                class_name_handle: None,
            }],
            super_desc: Some(table),
            annotations: Vec::new(),
        })?;
        classes.push(ClassData {
            desc: properties_desc,
            fields: vec![("defaults".to_owned(), JValue::Null)],
            custom: None,
        });
    }
    graph.new_object(classes)
}

/// Build a `java.util.Arrays$ArrayList` backed by an object array.
pub fn new_arrays_array_list(graph: &mut Graph, elements: Vec<JValue>) -> Result<Handle> {
    let elements = box_elements(graph, elements)?;
    for element in &elements {
        check_collection_value(element, "Arrays$ArrayList element")?;
    }
    let array = new_java_array(graph, "[Ljava.lang.Object;", elements)?;
    let desc = graph.add_class_desc(ClassDesc {
        name: ARRAYS_ARRAY_LIST.to_owned(),
        suid: -2_764_017_481_108_945_198,
        flags: 0x02,
        fields: vec![FieldDesc {
            name: "a".to_owned(),
            typecode: FieldType::Object,
            class_name1: Some("[Ljava.lang.Object;".to_owned()),
            class_name_handle: None,
        }],
        super_desc: None,
        annotations: Vec::new(),
    })?;
    graph.new_object(vec![ClassData {
        desc,
        fields: vec![("a".to_owned(), JValue::Arr(array))],
        custom: None,
    }])
}

/// Build a `Collections.unmodifiableList` around a new `ArrayList`.
pub fn new_unmodifiable_list(graph: &mut Graph, elements: Vec<JValue>) -> Result<Handle> {
    let backing = new_array_list(graph, elements)?;
    let collection = graph.add_class_desc(ClassDesc {
        name: "java.util.Collections$UnmodifiableCollection".to_owned(),
        suid: 1_820_017_752_578_914_078,
        flags: 0x02,
        fields: vec![FieldDesc {
            name: "c".to_owned(),
            typecode: FieldType::Object,
            class_name1: Some("Ljava/util/Collection;".to_owned()),
            class_name_handle: None,
        }],
        super_desc: None,
        annotations: Vec::new(),
    })?;
    let list = graph.add_class_desc(ClassDesc {
        name: UNMODIFIABLE_LIST.to_owned(),
        suid: -283_967_356_065_247_728,
        flags: 0x02,
        fields: vec![FieldDesc {
            name: "list".to_owned(),
            typecode: FieldType::Object,
            class_name1: Some("Ljava/util/List;".to_owned()),
            class_name_handle: None,
        }],
        super_desc: Some(collection),
        annotations: Vec::new(),
    })?;
    graph.new_object(vec![
        ClassData {
            desc: collection,
            fields: vec![("c".to_owned(), JValue::Obj(backing))],
            custom: None,
        },
        ClassData {
            desc: list,
            fields: vec![("list".to_owned(), JValue::Obj(backing))],
            custom: None,
        },
    ])
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
