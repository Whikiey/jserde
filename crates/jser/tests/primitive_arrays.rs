//! P8-2: primitive arrays use compact raw-byte storage.
//!
//! Object arrays keep one `JValue` per element; primitive arrays expose a
//! [`jser::PrimitiveArray`] view and `Vec<T>` decodes without materializing.

mod common;

use common::{decode_fixture, fixture_allowlist, read_fixture};
use jser::builtin::new_java_array;
use jser::{
    decode_stream_with_allowlist, decode_typed_with_allowlist, encode_stream, ClassDesc, FieldType,
    Graph, JValue,
};

fn fixture(id: &str) -> Graph {
    decode_fixture(&read_fixture(id)).unwrap_or_else(|err| panic!("{id}: {err}"))
}

#[test]
fn int_array_decodes_to_compact_storage() {
    let graph = fixture("object/25");
    let value = &graph.items()[0];

    let array = graph.as_primitive_array(value).unwrap().unwrap();
    assert_eq!(array.typecode(), FieldType::Int);
    assert_eq!(array.len(), 3);
    assert_eq!(array.bytes(), &[0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3]);
    assert_eq!(array.get(1), Some(JValue::Int(2)));
    assert_eq!(array.iter().count(), 3);

    // Primitive arrays are no longer exposed as `&[JValue]`.
    assert!(graph.as_array(value).unwrap().is_none());
}

#[test]
fn string_array_remains_object_storage() {
    let graph = fixture("object/26");
    let value = &graph.items()[0];

    let values = graph.as_array(value).unwrap().unwrap();
    assert_eq!(values.len(), 3);
    assert!(graph.as_primitive_array(value).unwrap().is_none());
}

#[test]
fn nested_int_array_uses_primitive_views() {
    let graph = fixture("object/27");
    let outer = &graph.items()[0];

    let rows = graph.as_array(outer).unwrap().unwrap();
    assert_eq!(rows.len(), 2);

    let first = graph.as_primitive_array(&rows[0]).unwrap().unwrap();
    assert_eq!(first.len(), 2);
    assert_eq!(first.get(1), Some(JValue::Int(2)));

    let second = graph.as_primitive_array(&rows[1]).unwrap().unwrap();
    assert_eq!(second.len(), 1);
    assert_eq!(second.get(0), Some(JValue::Int(3)));
}

#[test]
fn typed_vec_decodes_primitive_arrays_without_materializing_values() {
    let values: Vec<i32> =
        decode_typed_with_allowlist(&read_fixture("object/25"), &fixture_allowlist()).unwrap();
    assert_eq!(values, [1, 2, 3]);

    let nested: Vec<Vec<i32>> =
        decode_typed_with_allowlist(&read_fixture("object/27"), &fixture_allowlist()).unwrap();
    assert_eq!(nested, vec![vec![1, 2], vec![3]]);
}

#[test]
fn constructed_primitive_array_compacts_and_round_trips() {
    let mut graph = Graph::new();
    let array = new_java_array(
        &mut graph,
        "[I",
        vec![JValue::Int(1), JValue::Int(2), JValue::Int(3)],
    )
    .unwrap();
    graph.add_item(JValue::Arr(array));

    let bytes = encode_stream(&graph).unwrap();
    let decoded = decode_stream_with_allowlist(&bytes, &fixture_allowlist()).unwrap();
    let view = decoded
        .as_primitive_array(&decoded.items()[0])
        .unwrap()
        .unwrap();
    assert_eq!(view.typecode(), FieldType::Int);
    assert_eq!(view.bytes(), &[0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3]);
}

#[test]
fn new_primitive_array_accepts_raw_wire_bytes() {
    let mut graph = Graph::new();
    let desc = graph
        .add_class_desc(ClassDesc {
            name: "[S".to_owned(),
            suid: -1_188_055_269_542_874_886,
            flags: 0x02,
            fields: Vec::new(),
            super_desc: None,
            annotations: Vec::new(),
            proxy_interfaces: None,
        })
        .unwrap();
    let array = graph
        .new_primitive_array(desc, FieldType::Short, vec![0x00, 0x01, 0x00, 0x02])
        .unwrap();
    graph.add_item(JValue::Arr(array));

    let bytes = encode_stream(&graph).unwrap();
    let decoded = decode_stream_with_allowlist(&bytes, &fixture_allowlist()).unwrap();
    let view = decoded
        .as_primitive_array(&decoded.items()[0])
        .unwrap()
        .unwrap();
    assert_eq!(view.len(), 2);
    assert_eq!(view.get(0), Some(JValue::Short(1)));
    assert_eq!(view.get(1), Some(JValue::Short(2)));
}

#[test]
fn boolean_and_char_arrays_keep_their_width() {
    let mut graph = Graph::new();
    let booleans = new_java_array(
        &mut graph,
        "[Z",
        vec![JValue::Bool(true), JValue::Bool(false)],
    )
    .unwrap();
    let chars = new_java_array(&mut graph, "[C", vec![JValue::Char(0x4E2D)]).unwrap();
    graph.add_item(JValue::Arr(booleans));
    graph.add_item(JValue::Arr(chars));

    let bytes = encode_stream(&graph).unwrap();
    let decoded = decode_stream_with_allowlist(&bytes, &fixture_allowlist()).unwrap();

    let flags = decoded
        .as_primitive_array(&decoded.items()[0])
        .unwrap()
        .unwrap();
    assert_eq!(flags.typecode(), FieldType::Boolean);
    assert_eq!(flags.bytes(), &[1, 0]);

    let chars = decoded
        .as_primitive_array(&decoded.items()[1])
        .unwrap()
        .unwrap();
    assert_eq!(chars.typecode(), FieldType::Char);
    assert_eq!(chars.bytes().len(), 2);
    assert_eq!(chars.get(0), Some(JValue::Char(0x4E2D)));
}
