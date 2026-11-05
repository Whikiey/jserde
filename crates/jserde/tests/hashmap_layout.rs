//! P9-6: caller can write the exact `HashMap` capacity/threshold layout that a
//! JDK construction path would produce.

mod common;

use common::decode_fixture;
use jserde::builtin::new_hash_map_with_layout;
use jserde::{encode_stream, HashMapLayout, JValue};

#[test]
fn writes_an_exact_hash_map_layout() {
    let layout = HashMapLayout {
        capacity: 32,
        threshold: 24,
        load_factor: 0.75,
    };
    let mut graph = jserde::Graph::new();
    let map = new_hash_map_with_layout(&mut graph, &[("a", "one"), ("b", "two")], layout).unwrap();
    graph.add_item(JValue::Obj(map));

    let bytes = encode_stream(&graph).unwrap();
    let decoded = decode_fixture(&bytes).unwrap();
    let view = decoded.as_map(&decoded.items()[0]).unwrap().unwrap();

    assert_eq!(view.capacity, 32);
    assert_eq!(view.threshold, 24);
    assert_eq!(view.load_factor, 0.75);
    assert_eq!(view.entries.len(), 2);

    // A fresh empty map with the same explicit layout keeps its header exact.
    let mut graph = jserde::Graph::new();
    let map = new_hash_map_with_layout(&mut graph, &[], layout).unwrap();
    graph.add_item(JValue::Obj(map));
    let bytes = encode_stream(&graph).unwrap();
    let decoded = decode_fixture(&bytes).unwrap();
    let view = decoded.as_map(&decoded.items()[0]).unwrap().unwrap();
    assert_eq!(view.capacity, 32);
    assert_eq!(view.threshold, 24);
    assert!(view.entries.is_empty());
}

#[test]
fn rejects_an_invalid_capacity() {
    let mut graph = jserde::Graph::new();
    let error = new_hash_map_with_layout(
        &mut graph,
        &[],
        HashMapLayout {
            capacity: 30,
            threshold: 12,
            load_factor: 0.75,
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("power of two"), "{error}");
}
