//! P2 object fixtures (20, 22, 23, 33–35, 37, 39): class descriptors,
//! inheritance, custom `writeObject` data, shared references, cycles and reset.
//!
//! Array/enum/collection fixtures live in `object_containers.rs`.

mod common;

use common::{load_manifest, read_fixture};
use jser::wire::{mutf8, StreamReader};
use jser::{decode_stream, Graph, JValue};

fn graph(id: &str) -> Graph {
    decode_stream(&read_fixture(id)).unwrap_or_else(|err| panic!("{id}: {err}"))
}

fn expect(id: &str) -> serde_json::Value {
    load_manifest()
        .cases
        .into_iter()
        .find(|case| case.id == id)
        .unwrap_or_else(|| panic!("manifest has no case `{id}`"))
        .expect
}

fn object<'a>(graph: &'a Graph, value: &JValue) -> &'a jser::JObject {
    graph
        .as_object(value)
        .unwrap_or_else(|err| panic!("expected object: {err}"))
        .unwrap_or_else(|| panic!("expected object, got {value:?}"))
}

fn field<'a>(graph: &'a Graph, value: &'a JValue, name: &str) -> &'a JValue {
    graph
        .field(value, name)
        .unwrap_or_else(|err| panic!("field `{name}`: {err}"))
        .unwrap_or_else(|| panic!("missing field `{name}`"))
}

fn integer(graph: &Graph, value: &JValue) -> i64 {
    graph
        .as_i64(value)
        .unwrap_or_else(|err| panic!("expected integer, got {value:?}: {err}"))
}

#[test]
fn decodes_a_simple_pojo() {
    let graph = graph("object/20");
    let items = graph.items();
    assert_eq!(items.len(), 1);

    let value = &items[0];
    let expected = expect("object/20");
    assert_eq!(
        graph.class_name(value).unwrap(),
        expected["class"].as_str().unwrap()
    );

    assert_eq!(integer(&graph, field(&graph, value, "x")), 1);
    assert_eq!(integer(&graph, field(&graph, value, "y")), 2);
}

#[test]
fn decodes_inheritance_parent_fields_first() {
    let graph = graph("object/22");
    let value = &graph.items()[0];
    let data = object(&graph, value);
    let expected = expect("object/22");

    let names: Vec<&str> = data
        .classes
        .iter()
        .map(|class| graph.descriptor(class.desc).unwrap().name.as_str())
        .collect();
    assert_eq!(names, ["Fixtures$Base", "Fixtures$Derived"]);

    let derived = graph
        .descriptor(data.classes[1].desc)
        .expect("derived descriptor");
    assert_eq!(
        derived.super_desc,
        Some(data.classes[0].desc),
        "child descriptor must point at its parent"
    );

    assert_eq!(integer(&graph, field(&graph, value, "a")), 1);
    assert_eq!(
        graph.as_str(field(&graph, value, "b")).unwrap(),
        expected["fields"]["b"].as_str().unwrap()
    );
}

#[test]
fn decodes_custom_write_object_data() {
    let graph = graph("object/23");
    let value = &graph.items()[0];
    let data = object(&graph, value);

    assert_eq!(integer(&graph, field(&graph, value, "value")), 42);

    let custom = data.classes[0]
        .custom
        .as_ref()
        .expect("CustomWritable must have custom data");
    assert_eq!(custom.len(), 1, "writeInt + writeUTF share one block");

    let JValue::Block(block) = &custom[0] else {
        panic!("expected a block, got {:?}", custom[0]);
    };
    assert!(!block.long);

    let mut reader = StreamReader::new(&block.bytes);
    assert_eq!(reader.read_i32().unwrap(), 1234);
    let length = usize::from(reader.read_u16().unwrap());
    let utf = mutf8::decode(reader.read_exact(length).unwrap()).unwrap();
    assert_eq!(utf, "custom");
    assert!(reader.is_empty());
}

#[test]
fn decodes_shared_top_level_objects_by_handle() {
    let graph = graph("object/33");
    let items = graph.items();

    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0], items[1],
        "the second top-level write must be a TC_REFERENCE"
    );

    let value = &items[0];
    assert_eq!(integer(&graph, field(&graph, value, "x")), 5);
    assert_eq!(integer(&graph, field(&graph, value, "y")), 6);
}

#[test]
fn decodes_a_reference_cycle() {
    let graph = graph("object/34");
    let value = &graph.items()[0];

    assert_eq!(graph.as_str(field(&graph, value, "name")).unwrap(), "a");

    let second = field(&graph, value, "next");
    assert_eq!(graph.as_str(field(&graph, second, "name")).unwrap(), "b");
    assert_eq!(
        field(&graph, second, "next"),
        value,
        "b.next must resolve back to a, not a copy"
    );
}

#[test]
fn reset_clears_wire_handles_but_keeps_arena_values() {
    let graph = graph("object/35");
    let items = graph.items();

    assert_eq!(items.len(), 3);
    assert!(matches!(&items[1], JValue::Reset));

    let first = &items[0];
    let second = &items[2];
    assert_ne!(first, second, "reset must allocate new handles");

    assert_eq!(integer(&graph, field(&graph, first, "x")), 1);
    assert_eq!(integer(&graph, field(&graph, first, "y")), 2);
    assert_eq!(integer(&graph, field(&graph, second, "x")), 3);
    assert_eq!(integer(&graph, field(&graph, second, "y")), 4);

    let first_desc = object(&graph, first).classes[0].desc;
    let second_desc = object(&graph, second).classes[0].desc;
    assert_ne!(
        first_desc, second_desc,
        "the class descriptor is written again after reset"
    );
}

#[test]
fn transient_fields_are_absent_from_the_descriptor() {
    let graph = graph("object/37");
    let value = &graph.items()[0];
    let data = object(&graph, value);

    assert_eq!(integer(&graph, field(&graph, value, "a")), 1);
    assert_eq!(graph.as_str(field(&graph, value, "s")).unwrap(), "s");
    assert!(
        graph.field(value, "t").unwrap().is_none(),
        "transient field `t` must not be serialized"
    );

    let fields: Vec<&str> = data.classes[0]
        .fields
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(fields, ["a", "s"]);
}

#[test]
fn decodes_the_jdk_computed_suid() {
    let graph = graph("object/39");
    let value = &graph.items()[0];
    let data = object(&graph, value);
    let expected = expect("object/39");

    let desc = graph.descriptor(data.classes[0].desc).unwrap();
    assert_eq!(desc.suid, expected["suid"].as_i64().unwrap());
    assert_eq!(
        desc.name,
        expected["class"].as_str().unwrap_or("Fixtures$NoSuid")
    );

    assert_eq!(integer(&graph, field(&graph, value, "x")), 1);
    assert_eq!(graph.as_str(field(&graph, value, "y")).unwrap(), "y");
}
