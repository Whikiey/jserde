//! P4 typed conversions: scalars, Option, Vec, HashMap, HashSet and a manual
//! POJO, in both directions.

mod common;

use std::collections::{HashMap, HashSet};

use common::read_fixture;
use jser::{
    decode_typed, encode_typed, ClassData, ClassDesc, Error, FieldDesc, FieldType, FromJValue,
    Graph, IntoJValue, JValue,
};

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
fn decodes_vecs_from_lists_and_arrays() {
    let strings: Vec<String> = decode_typed(&read_fixture("object/28")).unwrap();
    assert_eq!(strings, ["alpha", "beta"]);

    let integers: Vec<i32> = decode_typed(&read_fixture("object/32")).unwrap();
    assert_eq!(integers, [1, 2, 3]);

    let nested: Vec<Vec<i32>> = decode_typed(&read_fixture("object/27")).unwrap();
    assert_eq!(nested, vec![vec![1, 2], vec![3]]);
}

#[test]
fn decodes_options_inside_vectors() {
    let values: Vec<Option<String>> = decode_typed(&read_fixture("object/26")).unwrap();
    assert_eq!(values, [Some("a".to_owned()), Some("b".to_owned()), None]);
}

#[test]
fn decodes_maps_and_sets() {
    let map: HashMap<String, i32> = decode_typed(&read_fixture("object/29")).unwrap();
    assert_eq!(map.len(), 3);
    assert_eq!(map["one"], 1);
    assert_eq!(map["two"], 2);
    assert_eq!(map["three"], 3);

    let set: HashSet<String> = decode_typed(&read_fixture("object/31")).unwrap();
    assert_eq!(set.len(), 2);
    assert!(set.contains("x"));
    assert!(set.contains("y"));
}

#[test]
fn decodes_a_manual_pojo() {
    let point: Point = decode_typed(&read_fixture("object/20")).unwrap();
    assert_eq!(point, Point { x: 1, y: 2 });
}

#[test]
fn missing_fields_are_errors() {
    #[derive(Debug)]
    struct Missing;

    impl FromJValue for Missing {
        fn from_jvalue(value: &JValue, graph: &Graph) -> jser::Result<Self> {
            graph.require_field(value, "doesNotExist")?;
            Ok(Self)
        }
    }

    let error = decode_typed::<Missing>(&read_fixture("object/20")).unwrap_err();
    assert!(matches!(error, Error::FieldMismatch { .. }));
    assert!(error.to_string().contains("doesNotExist"));
}

#[test]
fn encodes_typed_scalars_and_collections() {
    let map: HashMap<String, i32> = [("a".to_owned(), 1), ("b".to_owned(), 2)]
        .into_iter()
        .collect();
    let bytes = encode_typed(map.clone()).unwrap();
    let decoded: HashMap<String, i32> = decode_typed(&bytes).unwrap();
    assert_eq!(decoded, map);

    let vector = vec![1i32, 2, 3];
    let bytes = encode_typed(vector.clone()).unwrap();
    let decoded: Vec<i32> = decode_typed(&bytes).unwrap();
    assert_eq!(decoded, vector);

    let set: HashSet<String> = ["x".to_owned(), "y".to_owned()].into_iter().collect();
    let bytes = encode_typed(set.clone()).unwrap();
    let decoded: HashSet<String> = decode_typed(&bytes).unwrap();
    assert_eq!(decoded, set);
}

#[test]
fn encodes_nested_point_vectors() {
    let points = vec![Point { x: 1, y: 2 }, Point { x: 3, y: 4 }];
    let bytes = encode_typed(points.clone()).unwrap();
    let decoded: Vec<Point> = decode_typed(&bytes).unwrap();
    assert_eq!(decoded, points);
}
