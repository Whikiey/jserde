//! P6/P7 follow-up: explicit multi-level POJO nesting (Outer -> Middle -> Leaf).

mod common;

use common::{decode_fixture, read_fixture};
use jserde::{encode_stream, JValue};

fn field<'a>(graph: &'a jserde::Graph, value: &'a JValue, name: &str) -> &'a JValue {
    graph
        .field(value, name)
        .unwrap()
        .unwrap_or_else(|| panic!("missing field `{name}`"))
}

#[test]
fn decodes_three_level_object_nesting() {
    let graph = decode_fixture(&read_fixture("object/41")).unwrap();
    let outer = &graph.items()[0];

    assert_eq!(graph.class_name(outer).unwrap(), "Fixtures$Outer");
    let middle = field(&graph, outer, "middle");
    assert_eq!(graph.class_name(middle).unwrap(), "Fixtures$Middle");
    assert_eq!(graph.as_str(field(&graph, middle, "name")).unwrap(), "mid");

    let leaf = field(&graph, middle, "leaf");
    assert_eq!(graph.class_name(leaf).unwrap(), "Fixtures$Leaf");
    assert_eq!(graph.as_i64(field(&graph, leaf, "value")).unwrap(), 42);
}

#[test]
fn mutates_a_nested_leaf_and_reencodes() {
    let original = read_fixture("object/41");
    let mut graph = decode_fixture(&original).unwrap();
    let outer = graph.items()[0].clone();
    let middle = field(&graph, &outer, "middle").clone();
    let leaf = field(&graph, &middle, "leaf").clone();

    assert!(graph.set_field(&leaf, "value", JValue::Int(99)).unwrap());

    let encoded = encode_stream(&graph).unwrap();
    assert_ne!(encoded, original);

    let decoded = decode_fixture(&encoded).unwrap();
    let outer = &decoded.items()[0];
    let middle = field(&decoded, outer, "middle");
    let leaf = field(&decoded, middle, "leaf");
    assert_eq!(decoded.as_i64(field(&decoded, leaf, "value")).unwrap(), 99);
    assert_eq!(
        decoded.as_str(field(&decoded, middle, "name")).unwrap(),
        "mid"
    );
}
