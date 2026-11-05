//! P9-1: `TC_CLASS` fidelity (class objects, shared references, replay).

mod common;

use common::{decode_fixture, read_fixture};
use jserde::{encode_stream, ClassDesc, FieldDesc, FieldType, Graph, JValue};

fn class_name(graph: &Graph, value: &JValue) -> String {
    let JValue::Class(handle) = value else {
        panic!("expected a class value, got {value:?}");
    };
    graph
        .descriptor(graph.class_desc(*handle).unwrap())
        .unwrap()
        .name
        .clone()
}

#[test]
fn decodes_class_objects_and_shared_class_references() {
    let bytes = read_fixture("object/47");
    let graph = decode_fixture(&bytes).unwrap();
    let elements = graph.as_array(&graph.items()[0]).unwrap().unwrap();
    assert_eq!(elements.len(), 4);

    let names: Vec<String> = elements
        .iter()
        .map(|value| class_name(&graph, value))
        .collect();
    assert_eq!(
        names,
        [
            "java.lang.String",
            "java.lang.String",
            "Fixtures$Color",
            "Fixtures$Mixed"
        ]
    );

    let JValue::Class(first) = elements[0] else {
        unreachable!();
    };
    let JValue::Class(second) = elements[1] else {
        unreachable!();
    };
    assert_eq!(
        first, second,
        "the second String.class must be a TC_REFERENCE to the same class object"
    );

    assert_eq!(encode_stream(&graph).unwrap(), bytes);
}

#[test]
fn constructs_and_replays_a_class_object() {
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
            proxy_interfaces: None,
        })
        .unwrap();
    let class = graph.new_class(desc).unwrap();
    graph.add_item(JValue::Class(class));

    let bytes = encode_stream(&graph).unwrap();
    let decoded = decode_fixture(&bytes).unwrap();
    let value = &decoded.items()[0];
    assert_eq!(class_name(&decoded, value), "Fixtures$Point");

    assert_eq!(encode_stream(&decoded).unwrap(), bytes);
}
