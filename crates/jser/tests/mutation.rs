//! P3 mutation / construction tests: the encoder must be driven by the graph,
//! not by a cached copy of the original bytes.

mod common;

use common::{decode_fixture, read_fixture};
use jser::builtin::{new_hash_map, new_java_array};
use jser::{encode_stream, ClassData, ClassDesc, FieldDesc, FieldType, Graph, JValue};

#[test]
fn mutating_a_field_changes_the_encoded_stream() {
    let original = read_fixture("object/20");
    let mut graph = decode_fixture(&original).unwrap();
    let point = graph.items()[0].clone();

    assert!(graph
        .set_field(&point, "x", JValue::Int(99))
        .expect("set_field"));
    assert_eq!(
        graph
            .as_i64(graph.field(&point, "x").unwrap().unwrap())
            .unwrap(),
        99
    );

    let encoded = encode_stream(&graph).unwrap();
    assert_ne!(
        encoded, original,
        "encoder must not echo the original bytes"
    );

    let decoded = decode_fixture(&encoded).unwrap();
    let point = &decoded.items()[0];
    assert_eq!(
        decoded
            .as_i64(decoded.field(point, "x").unwrap().unwrap())
            .unwrap(),
        99
    );
    assert_eq!(
        decoded
            .as_i64(decoded.field(point, "y").unwrap().unwrap())
            .unwrap(),
        2
    );
}

#[test]
fn appending_a_map_entry_updates_the_custom_block() {
    let original = read_fixture("object/29");
    let mut graph = decode_fixture(&original).unwrap();
    let map = graph.items()[0].clone();

    let value = {
        let view = graph.as_map(&map).unwrap().unwrap();
        view.entries[0].1.clone()
    };
    let key = graph.new_string("four");
    graph
        .insert_map_entry(&map, JValue::Str(key), value)
        .unwrap();

    let encoded = encode_stream(&graph).unwrap();
    let decoded = decode_fixture(&encoded).unwrap();
    let map = &decoded.items()[0];
    let view = decoded.as_map(map).unwrap().unwrap();

    assert_eq!(view.entries.len(), 4);
    let (_, actual) = view
        .entries
        .iter()
        .find(|(key, _)| decoded.as_str(key).ok() == Some("four"))
        .expect("new entry");
    assert_eq!(decoded.as_i64(actual).unwrap(), 1);
}

#[test]
fn encodes_a_constructed_point() {
    let mut graph = Graph::new();
    let desc = graph
        .add_class_desc(ClassDesc {
            name: "Fixtures$Point".to_owned(),
            suid: 1,
            flags: 0x02,
            fields: vec![
                FieldDesc {
                    name: "x".to_owned(),
                    typecode: FieldType::Int,
                    class_name1: None,
                    class_name_handle: None,
                },
                FieldDesc {
                    name: "y".to_owned(),
                    typecode: FieldType::Int,
                    class_name1: None,
                    class_name_handle: None,
                },
            ],
            super_desc: None,
            annotations: Vec::new(),
        })
        .unwrap();

    let object = graph
        .new_object(vec![ClassData {
            desc,
            fields: vec![
                ("x".to_owned(), JValue::Int(7)),
                ("y".to_owned(), JValue::Int(8)),
            ],
            custom: None,
        }])
        .unwrap();
    graph.add_item(JValue::Obj(object));

    let encoded = encode_stream(&graph).unwrap();
    let decoded = decode_fixture(&encoded).unwrap();
    let point = &decoded.items()[0];

    assert_eq!(decoded.class_name(point).unwrap(), "Fixtures$Point");
    assert_eq!(
        decoded
            .as_i64(decoded.field(point, "x").unwrap().unwrap())
            .unwrap(),
        7
    );
    assert_eq!(
        decoded
            .as_i64(decoded.field(point, "y").unwrap().unwrap())
            .unwrap(),
        8
    );
}

#[test]
fn encodes_a_constructed_hash_map() {
    let mut graph = Graph::new();
    let map = new_hash_map(&mut graph, &[("alpha", "one"), ("beta", "two")]).unwrap();
    graph.add_item(JValue::Obj(map));

    let encoded = encode_stream(&graph).unwrap();
    let decoded = decode_fixture(&encoded).unwrap();
    let map = &decoded.items()[0];
    let view = decoded.as_map(map).unwrap().unwrap();

    assert_eq!(decoded.class_name(map).unwrap(), "java.util.HashMap");
    assert_eq!(view.entries.len(), 2);
    assert_eq!(view.capacity, 16);
    assert_eq!(
        decoded
            .as_str(
                view.entries
                    .iter()
                    .find(|(key, _)| decoded.as_str(key).ok() == Some("alpha"))
                    .unwrap()
                    .1
            )
            .unwrap(),
        "one"
    );
}

fn scalar_field(name: &str, typecode: FieldType) -> FieldDesc {
    FieldDesc {
        name: name.to_owned(),
        typecode,
        class_name1: match typecode {
            FieldType::Object => Some("Ljava/lang/String;".to_owned()),
            _ => None,
        },
        class_name_handle: None,
    }
}

#[test]
fn mutates_every_scalar_field() {
    let mut graph = Graph::new();
    let desc = graph
        .add_class_desc(ClassDesc {
            name: "Fixtures$AllTypes".to_owned(),
            suid: 1,
            flags: 0x02,
            fields: vec![
                scalar_field("b", FieldType::Byte),
                scalar_field("c", FieldType::Char),
                scalar_field("d", FieldType::Double),
                scalar_field("f", FieldType::Float),
                scalar_field("i", FieldType::Int),
                scalar_field("l", FieldType::Long),
                scalar_field("s", FieldType::Short),
                scalar_field("text", FieldType::Object),
                scalar_field("z", FieldType::Boolean),
            ],
            super_desc: None,
            annotations: Vec::new(),
        })
        .unwrap();
    let text = graph.new_string("start");
    let object = graph
        .new_object(vec![ClassData {
            desc,
            fields: vec![
                ("b".to_owned(), JValue::Byte(1)),
                ("c".to_owned(), JValue::Char(u16::from(b'A'))),
                ("d".to_owned(), JValue::Double(1.5)),
                ("f".to_owned(), JValue::Float(2.5)),
                ("i".to_owned(), JValue::Int(3)),
                ("l".to_owned(), JValue::Long(4)),
                ("s".to_owned(), JValue::Short(5)),
                ("text".to_owned(), JValue::Str(text)),
                ("z".to_owned(), JValue::Bool(true)),
            ],
            custom: None,
        }])
        .unwrap();
    graph.add_item(JValue::Obj(object));

    let object = JValue::Obj(object);
    for (name, value) in [
        ("b", JValue::Byte(-7)),
        ("c", JValue::Char(u16::from(b'Z'))),
        ("d", JValue::Double(9.25)),
        ("f", JValue::Float(-3.5)),
        ("i", JValue::Int(42)),
        ("l", JValue::Long(9_000_000_000)),
        ("s", JValue::Short(-300)),
        ("z", JValue::Bool(false)),
    ] {
        assert!(graph.set_field(&object, name, value).unwrap(), "{name}");
    }
    let changed = graph.new_string("changed");
    assert!(graph
        .set_field(&object, "text", JValue::Str(changed))
        .unwrap());

    let encoded = encode_stream(&graph).unwrap();
    let decoded = decode_fixture(&encoded).unwrap();
    let value = &decoded.items()[0];

    assert_eq!(decoded.class_name(value).unwrap(), "Fixtures$AllTypes");
    assert_eq!(
        decoded
            .as_i64(decoded.require_field(value, "b").unwrap())
            .unwrap(),
        -7
    );
    assert_eq!(
        decoded
            .as_char(decoded.require_field(value, "c").unwrap())
            .unwrap(),
        u16::from(b'Z')
    );
    assert_eq!(
        decoded
            .as_f64(decoded.require_field(value, "d").unwrap())
            .unwrap(),
        9.25
    );
    assert_eq!(
        decoded
            .as_f64(decoded.require_field(value, "f").unwrap())
            .unwrap(),
        -3.5f32 as f64
    );
    assert_eq!(
        decoded
            .as_i64(decoded.require_field(value, "i").unwrap())
            .unwrap(),
        42
    );
    assert_eq!(
        decoded
            .as_i64(decoded.require_field(value, "l").unwrap())
            .unwrap(),
        9_000_000_000
    );
    assert_eq!(
        decoded
            .as_i64(decoded.require_field(value, "s").unwrap())
            .unwrap(),
        -300
    );
    assert_eq!(
        decoded
            .as_str(decoded.require_field(value, "text").unwrap())
            .unwrap(),
        "changed"
    );
    assert!(!decoded
        .as_bool(decoded.require_field(value, "z").unwrap())
        .unwrap());
}

#[test]
fn mutates_map_entries_in_place_and_by_append() {
    let original = read_fixture("object/29");
    let mut graph = decode_fixture(&original).unwrap();
    let map = graph.items()[0].clone();
    let (key, key_text) = {
        let view = graph.as_map(&map).unwrap().unwrap();
        let key = view.entries[0].0.clone();
        let text = graph.as_str(&key).unwrap().to_owned();
        (key, text)
    };

    assert!(graph.set_map_entry(&map, key, JValue::Int(42)).unwrap());
    let new_key = graph.new_string("added");
    assert!(!graph
        .set_map_entry(&map, JValue::Str(new_key), JValue::Int(7))
        .unwrap());

    let encoded = encode_stream(&graph).unwrap();
    let decoded = decode_fixture(&encoded).unwrap();
    let map = &decoded.items()[0];
    let view = decoded.as_map(map).unwrap().unwrap();
    assert_eq!(view.entries.len(), 4);

    let find = |name: &str| {
        view.entries
            .iter()
            .find(|(key, _)| decoded.as_str(key).ok() == Some(name))
            .map(|(_, value)| value)
            .unwrap()
    };
    for (name, expected) in [(key_text.as_str(), 42), ("added", 7)] {
        let value = find(name);
        assert_eq!(decoded.class_name(value).unwrap(), "java.lang.Integer");
        assert_eq!(
            decoded
                .as_i64(decoded.require_field(value, "value").unwrap())
                .unwrap(),
            expected
        );
    }
}

#[test]
fn mutates_an_object_array_element() {
    let mut graph = Graph::new();
    let first = graph.new_string("a");
    let second = graph.new_string("b");
    let array = new_java_array(
        &mut graph,
        "[Ljava.lang.String;",
        vec![JValue::Str(first), JValue::Str(second)],
    )
    .unwrap();
    graph.add_item(JValue::Arr(array));

    let changed = graph.new_string("changed");
    graph
        .set_array_element(&JValue::Arr(array), 1, JValue::Str(changed))
        .unwrap();

    let encoded = encode_stream(&graph).unwrap();
    let decoded = decode_fixture(&encoded).unwrap();
    let value = &decoded.items()[0];

    assert_eq!(decoded.class_name(value).unwrap(), "[Ljava.lang.String;");
    let elements = decoded.as_array(value).unwrap().unwrap();
    assert_eq!(elements.len(), 2);
    assert_eq!(decoded.as_str(&elements[0]).unwrap(), "a");
    assert_eq!(decoded.as_str(&elements[1]).unwrap(), "changed");
}
