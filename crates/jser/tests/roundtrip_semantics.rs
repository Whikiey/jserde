//! P3 read → write → read checks for shared references, cycles and reset.
//! Byte identity is covered by `roundtrip_bytes.rs`; these tests assert that the
//! reconstructed graph still has the same handle-level structure.

mod common;

use common::read_fixture;
use jser::{decode_stream, encode_stream, Graph, JValue};

fn reencode(id: &str) -> Graph {
    let bytes = read_fixture(id);
    let graph = decode_stream(&bytes).unwrap_or_else(|err| panic!("{id} decode: {err}"));
    let encoded = encode_stream(&graph).unwrap_or_else(|err| panic!("{id} encode: {err}"));
    decode_stream(&encoded).unwrap_or_else(|err| panic!("{id} re-decode: {err}"))
}

fn field<'a>(graph: &'a Graph, value: &'a JValue, name: &str) -> &'a JValue {
    graph
        .field(value, name)
        .unwrap()
        .unwrap_or_else(|| panic!("missing field `{name}`"))
}

#[test]
fn shared_objects_stay_shared() {
    let graph = reencode("object/33");
    let items = graph.items();

    assert_eq!(items.len(), 2);
    assert_eq!(items[0], items[1]);
    assert_eq!(graph.as_i64(field(&graph, &items[0], "x")).unwrap(), 5);
}

#[test]
fn cycles_stay_cycles() {
    let graph = reencode("object/34");
    let a = &graph.items()[0];
    let b = field(&graph, a, "next");

    assert_eq!(graph.as_str(field(&graph, a, "name")).unwrap(), "a");
    assert_eq!(graph.as_str(field(&graph, b, "name")).unwrap(), "b");
    assert_eq!(
        field(&graph, b, "next"),
        a,
        "b.next must still point at a after re-encoding"
    );
}

#[test]
fn reset_still_separates_handle_tables() {
    let graph = reencode("object/35");
    let items = graph.items();

    assert_eq!(items.len(), 3);
    assert!(matches!(&items[1], JValue::Reset));
    assert_ne!(items[0], items[2]);
    assert_eq!(graph.as_i64(field(&graph, &items[0], "x")).unwrap(), 1);
    assert_eq!(graph.as_i64(field(&graph, &items[2], "x")).unwrap(), 3);
}

#[test]
fn linked_hash_map_keeps_entry_order() {
    let graph = reencode("object/30");
    let value = &graph.items()[0];
    let view = graph.as_map(value).unwrap().unwrap();

    assert!(view.ordered);
    let keys: Vec<&str> = view
        .entries
        .iter()
        .map(|(key, _)| graph.as_str(key).unwrap())
        .collect();
    assert_eq!(keys, ["a", "b"]);
}
