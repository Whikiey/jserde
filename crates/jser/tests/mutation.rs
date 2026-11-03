//! P3 mutation / construction tests: the encoder must be driven by the graph,
//! not by a cached copy of the original bytes.

mod common;

use common::read_fixture;
use jser::builtin::new_hash_map;
use jser::{
    decode_stream, encode_stream, ClassData, ClassDesc, FieldDesc, FieldType, Graph, JValue,
};

#[test]
fn mutating_a_field_changes_the_encoded_stream() {
    let original = read_fixture("object/20");
    let mut graph = decode_stream(&original).unwrap();
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

    let decoded = decode_stream(&encoded).unwrap();
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
    let mut graph = decode_stream(&original).unwrap();
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
    let decoded = decode_stream(&encoded).unwrap();
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
    let decoded = decode_stream(&encoded).unwrap();
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
    let decoded = decode_stream(&encoded).unwrap();
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
