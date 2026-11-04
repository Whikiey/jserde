//! Mixed nesting: Map × array × custom class, including shared references.
//!
//! The stream is built entirely in Rust (no JDK), then decoded, re-encoded and
//! decoded again. It covers `HashMap<String, Object[]>` where the array holds a
//! custom class whose fields are another `HashMap` and an `int[]`, and where
//! the same `HashMap` is referenced twice (wire `TC_REFERENCE`).

mod common;

use std::collections::HashMap;

use common::fixture_allowlist;
use jser::builtin::{new_hash_map_entries, new_java_array};
use jser::{
    decode_stream_with_allowlist, decode_typed_with_allowlist, encode_stream, encode_typed,
    ClassData, ClassDesc, FieldDesc, FieldType, FromJValue, Graph, IntoJValue, JValue,
};

fn nested_stream() -> Vec<u8> {
    let mut graph = Graph::new();

    // tags: HashMap<String, Integer>{"a": 1}
    let key = JValue::Str(graph.new_string("a"));
    let tags = new_hash_map_entries(&mut graph, vec![(key, JValue::Int(1))]).unwrap();

    // scores: int[]{1, 2}
    let scores = new_java_array(&mut graph, "[I", vec![JValue::Int(1), JValue::Int(2)]).unwrap();

    // Fixtures$Widget{String name; int[] scores; HashMap tags}
    let desc = graph
        .add_class_desc(ClassDesc {
            name: "Fixtures$Widget".to_owned(),
            suid: 1,
            flags: 0x02,
            fields: vec![
                FieldDesc {
                    name: "name".to_owned(),
                    typecode: FieldType::Object,
                    class_name1: Some("Ljava/lang/String;".to_owned()),
                    class_name_handle: None,
                },
                FieldDesc {
                    name: "scores".to_owned(),
                    typecode: FieldType::Array,
                    class_name1: Some("[I".to_owned()),
                    class_name_handle: None,
                },
                FieldDesc {
                    name: "tags".to_owned(),
                    typecode: FieldType::Object,
                    class_name1: Some("Ljava/util/HashMap;".to_owned()),
                    class_name_handle: None,
                },
            ],
            super_desc: None,
            annotations: Vec::new(),
        })
        .unwrap();

    let name = JValue::Str(graph.new_string("w1"));
    let widget = graph
        .new_object(vec![ClassData {
            desc,
            fields: vec![
                ("name".to_owned(), name),
                ("scores".to_owned(), JValue::Arr(scores)),
                ("tags".to_owned(), JValue::Obj(tags)),
            ],
            custom: None,
        }])
        .unwrap();

    // Object[]{widget, tags}: the map is referenced twice on the wire.
    let payload = new_java_array(
        &mut graph,
        "[Ljava.lang.Object;",
        vec![JValue::Obj(widget), JValue::Obj(tags)],
    )
    .unwrap();

    // HashMap<String, Object[]>{"payload": payload}
    let key = JValue::Str(graph.new_string("payload"));
    let outer = new_hash_map_entries(&mut graph, vec![(key, JValue::Arr(payload))]).unwrap();
    graph.add_item(JValue::Obj(outer));

    encode_stream(&graph).unwrap()
}

fn assert_nested(graph: &Graph) {
    let outer = graph.as_map(&graph.items()[0]).unwrap().unwrap();
    let (_, payload) = outer
        .entries
        .iter()
        .find(|(key, _)| graph.as_str(key).ok() == Some("payload"))
        .copied()
        .unwrap();

    let elements = graph.as_array(payload).unwrap().unwrap();
    assert_eq!(elements.len(), 2);

    let widget = &elements[0];
    assert_eq!(graph.class_name(widget).unwrap(), "Fixtures$Widget");
    let name = graph.require_field(widget, "name").unwrap();
    assert_eq!(graph.as_str(name).unwrap(), "w1");

    let scores = graph
        .as_primitive_array(graph.require_field(widget, "scores").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(scores.typecode(), FieldType::Int);
    assert_eq!(scores.get(0), Some(JValue::Int(1)));
    assert_eq!(scores.get(1), Some(JValue::Int(2)));

    let tags_field = graph.require_field(widget, "tags").unwrap();
    assert_eq!(
        &elements[1], tags_field,
        "second Object[] element must be the same map handle"
    );
    let tags = graph.as_map(tags_field).unwrap().unwrap();
    assert_eq!(tags.entries.len(), 1);
    assert_eq!(graph.as_i64(tags.entries[0].1).unwrap(), 1);
}

#[test]
fn map_array_custom_class_nest_and_share_references() {
    let bytes = nested_stream();
    let graph = decode_stream_with_allowlist(&bytes, &fixture_allowlist()).unwrap();
    assert_nested(&graph);

    let reencoded = encode_stream(&graph).unwrap();
    assert_eq!(
        reencoded, bytes,
        "constructed nested stream should replay byte-exactly"
    );

    let second = decode_stream_with_allowlist(&reencoded, &fixture_allowlist()).unwrap();
    assert_nested(&second);
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Point {
    x: i32,
    y: i32,
}

impl FromJValue for Point {
    fn from_jvalue(value: &JValue, graph: &Graph) -> jser::Result<Self> {
        Ok(Self {
            x: i32::from_jvalue(graph.require_field(value, "x")?, graph)?,
            y: i32::from_jvalue(graph.require_field(value, "y")?, graph)?,
        })
    }
}

impl IntoJValue for Point {
    fn into_jvalue(self, graph: &mut Graph) -> jser::Result<JValue> {
        let desc = graph.add_class_desc(ClassDesc {
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
        })?;
        let object = graph.new_object(vec![ClassData {
            desc,
            fields: vec![
                ("x".to_owned(), JValue::Int(self.x)),
                ("y".to_owned(), JValue::Int(self.y)),
            ],
            custom: None,
        }])?;
        Ok(JValue::Obj(object))
    }
}

#[test]
fn typed_map_of_vec_of_custom_round_trips() {
    let mut map = HashMap::new();
    map.insert(
        "a".to_owned(),
        vec![Point { x: 1, y: 2 }, Point { x: 3, y: 4 }],
    );

    let bytes = encode_typed(map.clone()).unwrap();
    let decoded: HashMap<String, Vec<Point>> =
        decode_typed_with_allowlist(&bytes, &fixture_allowlist()).unwrap();
    assert_eq!(decoded, map);
}
