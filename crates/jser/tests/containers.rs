//! P9-23/25: decode views for the extended collection containers (`object/46`).

mod common;

use common::{decode_fixture, read_fixture};
use jser::builtin::{
    new_array_deque, new_arrays_array_list, new_enum_map, new_enum_set, new_hashtable,
    new_linked_hash_set, new_priority_queue, new_properties, new_stack, new_tree_map, new_tree_set,
    new_unmodifiable_list, new_vector,
};
use jser::{encode_stream, ClassDesc, Graph, JValue};

fn graph() -> Graph {
    decode_fixture(&read_fixture("object/46")).unwrap()
}

fn item(graph: &Graph, index: usize) -> &JValue {
    let array = graph.as_array(&graph.items()[0]).unwrap().unwrap();
    &array[index]
}

fn strings(graph: &Graph, values: &[&JValue]) -> Vec<String> {
    values
        .iter()
        .map(|value| graph.as_str(value).unwrap().to_owned())
        .collect()
}

fn unbox_i64(graph: &Graph, value: &JValue) -> i64 {
    let inner = graph.require_field(value, "value").unwrap();
    graph.as_i64(inner).unwrap()
}

#[test]
fn decodes_extended_containers() {
    let graph = graph();

    let linked_set: Vec<String> = strings(&graph, &graph.as_set(item(&graph, 0)).unwrap().unwrap());
    assert_eq!(linked_set, ["a", "b"]);

    let tree_map = graph.as_map(item(&graph, 1)).unwrap().unwrap();
    assert_eq!(tree_map.entries.len(), 2);
    let tree_entry = |name: &str| {
        tree_map
            .entries
            .iter()
            .find(|(key, _)| graph.as_str(key).ok() == Some(name))
            .map(|(_, value)| *value)
            .unwrap()
    };
    assert_eq!(unbox_i64(&graph, tree_entry("a")), 1);
    assert_eq!(unbox_i64(&graph, tree_entry("b")), 2);

    let tree_set: Vec<String> = strings(&graph, &graph.as_set(item(&graph, 2)).unwrap().unwrap());
    assert_eq!(tree_set, ["x", "y"]);

    let deque: Vec<String> = strings(&graph, &graph.as_list(item(&graph, 3)).unwrap().unwrap());
    assert_eq!(deque, ["p", "q"]);

    let queue: Vec<i64> = graph
        .as_list(item(&graph, 4))
        .unwrap()
        .unwrap()
        .iter()
        .map(|value| unbox_i64(&graph, value))
        .collect();
    let mut sorted = queue.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, [1, 2, 3]);

    let vector: Vec<String> = strings(&graph, &graph.as_list(item(&graph, 5)).unwrap().unwrap());
    assert_eq!(vector, ["v1", "v2"]);

    let stack: Vec<String> = strings(&graph, &graph.as_list(item(&graph, 6)).unwrap().unwrap());
    assert_eq!(stack, ["s1", "s2"]);

    let table = graph.as_map(item(&graph, 7)).unwrap().unwrap();
    let table_entry = |name: &str| {
        table
            .entries
            .iter()
            .find(|(key, _)| graph.as_str(key).ok() == Some(name))
            .map(|(_, value)| *value)
            .unwrap()
    };
    assert_eq!(unbox_i64(&graph, table_entry("h1")), 1);
    assert_eq!(unbox_i64(&graph, table_entry("h2")), 2);

    let properties = graph.as_map(item(&graph, 8)).unwrap().unwrap();
    assert_eq!(properties.entries.len(), 1);
    assert_eq!(graph.as_str(properties.entries[0].0).unwrap(), "k");
    assert_eq!(graph.as_str(properties.entries[0].1).unwrap(), "v");

    let unmodifiable: Vec<String> =
        strings(&graph, &graph.as_list(item(&graph, 9)).unwrap().unwrap());
    assert_eq!(unmodifiable, ["u1", "u2"]);

    let arrays_list: Vec<String> =
        strings(&graph, &graph.as_list(item(&graph, 10)).unwrap().unwrap());
    assert_eq!(arrays_list, ["a1", "a2"]);
}

fn round_trip(build: impl FnOnce(&mut Graph) -> jser::Result<jser::Handle>) -> Graph {
    let mut graph = Graph::new();
    let value = build(&mut graph).unwrap();
    graph.add_item(JValue::Obj(value));
    let bytes = encode_stream(&graph).unwrap();
    decode_fixture(&bytes).unwrap()
}

fn string_values(graph: &mut Graph, values: &[&str]) -> Vec<JValue> {
    values
        .iter()
        .map(|value| JValue::Str(graph.new_string(*value)))
        .collect()
}

#[test]
fn constructs_extended_containers() {
    let graph = round_trip(|graph| {
        let elements = string_values(graph, &["a", "b"]);
        new_linked_hash_set(graph, elements)
    });
    assert_eq!(
        strings(&graph, &graph.as_set(&graph.items()[0]).unwrap().unwrap()),
        ["a", "b"]
    );

    let graph = round_trip(|graph| {
        let key_a = graph.new_string("a");
        let key_b = graph.new_string("b");
        new_tree_map(
            graph,
            vec![
                (JValue::Str(key_a), JValue::Int(1)),
                (JValue::Str(key_b), JValue::Int(2)),
            ],
        )
    });
    let view = graph.as_map(&graph.items()[0]).unwrap().unwrap();
    assert_eq!(view.entries.len(), 2);
    assert_eq!(unbox_i64(&graph, view.entries[0].1), 1);
    assert_eq!(unbox_i64(&graph, view.entries[1].1), 2);

    let graph = round_trip(|graph| {
        let elements = string_values(graph, &["x", "y"]);
        new_tree_set(graph, elements)
    });
    assert_eq!(
        strings(&graph, &graph.as_set(&graph.items()[0]).unwrap().unwrap()),
        ["x", "y"]
    );

    let graph = round_trip(|graph| {
        let elements = string_values(graph, &["p", "q"]);
        new_array_deque(graph, elements)
    });
    assert_eq!(
        strings(&graph, &graph.as_list(&graph.items()[0]).unwrap().unwrap()),
        ["p", "q"]
    );

    let graph = round_trip(|graph| {
        new_priority_queue(graph, vec![JValue::Int(3), JValue::Int(1), JValue::Int(2)])
    });
    let mut queue: Vec<i64> = graph
        .as_list(&graph.items()[0])
        .unwrap()
        .unwrap()
        .iter()
        .map(|value| unbox_i64(&graph, value))
        .collect();
    queue.sort_unstable();
    assert_eq!(queue, [1, 2, 3]);

    let graph = round_trip(|graph| {
        let elements = string_values(graph, &["v1", "v2"]);
        new_vector(graph, elements)
    });
    assert_eq!(
        strings(&graph, &graph.as_list(&graph.items()[0]).unwrap().unwrap()),
        ["v1", "v2"]
    );

    let graph = round_trip(|graph| {
        let elements = string_values(graph, &["s1", "s2"]);
        new_stack(graph, elements)
    });
    assert_eq!(
        strings(&graph, &graph.as_list(&graph.items()[0]).unwrap().unwrap()),
        ["s1", "s2"]
    );

    let graph = round_trip(|graph| {
        let key_h1 = graph.new_string("h1");
        let key_h2 = graph.new_string("h2");
        new_hashtable(
            graph,
            vec![
                (JValue::Str(key_h1), JValue::Int(1)),
                (JValue::Str(key_h2), JValue::Int(2)),
            ],
        )
    });
    let view = graph.as_map(&graph.items()[0]).unwrap().unwrap();
    let find = |name: &str| {
        view.entries
            .iter()
            .find(|(key, _)| graph.as_str(key).ok() == Some(name))
            .map(|(_, value)| *value)
            .unwrap()
    };
    assert_eq!(unbox_i64(&graph, find("h1")), 1);
    assert_eq!(unbox_i64(&graph, find("h2")), 2);

    let graph = round_trip(|graph| {
        let key = graph.new_string("k");
        let value = graph.new_string("v");
        new_properties(graph, vec![(JValue::Str(key), JValue::Str(value))])
    });
    let view = graph.as_map(&graph.items()[0]).unwrap().unwrap();
    assert_eq!(graph.as_str(view.entries[0].0).unwrap(), "k");
    assert_eq!(graph.as_str(view.entries[0].1).unwrap(), "v");

    let graph = round_trip(|graph| {
        let elements = string_values(graph, &["a1", "a2"]);
        new_arrays_array_list(graph, elements)
    });
    assert_eq!(
        strings(&graph, &graph.as_list(&graph.items()[0]).unwrap().unwrap()),
        ["a1", "a2"]
    );

    let graph = round_trip(|graph| {
        let elements = string_values(graph, &["u1", "u2"]);
        new_unmodifiable_list(graph, elements)
    });
    assert_eq!(
        strings(&graph, &graph.as_list(&graph.items()[0]).unwrap().unwrap()),
        ["u1", "u2"]
    );
}

fn enum_class_and_value(graph: &mut Graph) -> (JValue, JValue) {
    let desc = graph
        .add_class_desc(ClassDesc {
            name: "Fixtures$Color".to_owned(),
            suid: 0,
            flags: 0x12,
            fields: Vec::new(),
            super_desc: None,
            annotations: Vec::new(),
            proxy_interfaces: None,
        })
        .unwrap();
    let class = graph.new_class(desc).unwrap();
    let constant = graph.new_string("RED");
    let value = graph.new_enum(desc, constant).unwrap();
    (JValue::Class(class), JValue::Enum(value))
}

#[test]
fn decodes_and_constructs_enum_containers() {
    let graph = decode_fixture(&read_fixture("object/49")).unwrap();
    let set = graph.as_set(item(&graph, 0)).unwrap().unwrap();
    assert_eq!(set.len(), 1);
    assert_eq!(graph.as_enum_constant(set[0]).unwrap(), Some("RED"));

    let map = graph.as_map(item(&graph, 1)).unwrap().unwrap();
    assert_eq!(map.entries.len(), 1);
    assert_eq!(
        graph.as_enum_constant(map.entries[0].0).unwrap(),
        Some("RED")
    );
    assert_eq!(graph.as_str(map.entries[0].1).unwrap(), "red");

    let mut graph = Graph::new();
    let (class, value) = enum_class_and_value(&mut graph);
    let set = new_enum_set(&mut graph, class, vec![value]).unwrap();
    graph.add_item(JValue::Obj(set));
    let decoded = decode_fixture(&encode_stream(&graph).unwrap()).unwrap();
    let set = decoded.as_set(&decoded.items()[0]).unwrap().unwrap();
    assert_eq!(set.len(), 1);
    assert_eq!(decoded.as_enum_constant(set[0]).unwrap(), Some("RED"));

    let mut graph = Graph::new();
    let (class, value) = enum_class_and_value(&mut graph);
    let text = graph.new_string("red");
    let map = new_enum_map(&mut graph, class, vec![(value, JValue::Str(text))]).unwrap();
    graph.add_item(JValue::Obj(map));
    let decoded = decode_fixture(&encode_stream(&graph).unwrap()).unwrap();
    let map = decoded.as_map(&decoded.items()[0]).unwrap().unwrap();
    assert_eq!(map.entries.len(), 1);
    assert_eq!(
        decoded.as_enum_constant(map.entries[0].0).unwrap(),
        Some("RED")
    );
    assert_eq!(decoded.as_str(map.entries[0].1).unwrap(), "red");
}
