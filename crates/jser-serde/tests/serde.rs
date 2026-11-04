mod common {
    pub fn allowlist() -> jser::Allowlist {
        let mut allowlist = jser::Allowlist::strict();
        allowlist.add("Fixtures$*").unwrap();
        allowlist
    }
}

use std::collections::{HashMap, HashSet};

use jser::Allowlist;
use jser::FieldType;
use jser_serde::{
    from_slice, from_slice_enum, from_slice_map, from_slice_multi, from_slice_set, to_vec,
    to_vec_enum, to_vec_map, to_vec_set, EnumConstant, EnumSchema, JavaSchema, MapSchema,
    MultiRootWriter, SchemaField, SchemaValue,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Point {
    x: i32,
    y: i32,
}

fn point_schema() -> JavaSchema {
    JavaSchema {
        class: "Fixtures$Point",
        suid: 1,
        fields: vec![
            SchemaField {
                name: "x",
                typecode: FieldType::Int,
                class_name1: None,
                nested: None,
                map: None,
                set: None,
                enumeration: None,
            },
            SchemaField {
                name: "y",
                typecode: FieldType::Int,
                class_name1: None,
                nested: None,
                map: None,
                set: None,
                enumeration: None,
            },
        ],
    }
}

fn point_fixture() -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/testdata/fixtures/object/20-object-point.ser"),
    )
    .unwrap()
}

#[test]
fn serde_reads_a_java_fixture() {
    let point: Point = from_slice(&point_fixture(), &point_schema(), &common::allowlist()).unwrap();
    assert_eq!(point, Point { x: 1, y: 2 });
}

#[test]
fn serde_writes_a_java_stream() {
    let bytes = to_vec(&Point { x: 7, y: 8 }, &point_schema()).unwrap();
    let point: Point = from_slice(&bytes, &point_schema(), &common::allowlist()).unwrap();
    assert_eq!(point, Point { x: 7, y: 8 });
}

#[test]
fn serde_rejects_multiple_roots() {
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/testdata/fixtures/object/33-ref-shared-top.ser"),
    )
    .unwrap();
    let error = from_slice::<Point>(&bytes, &point_schema(), &common::allowlist()).unwrap_err();
    assert!(error.to_string().contains("exactly one"));
}

#[test]
fn serde_rejects_a_schema_mismatch() {
    let mut schema = point_schema();
    schema.class = "example.Other";
    let error = from_slice::<Point>(&point_fixture(), &schema, &common::allowlist()).unwrap_err();
    assert!(error.to_string().contains("schema expects"));
}

#[test]
fn serde_supports_array_fields() {
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Numbers {
        a: Vec<i32>,
    }

    let schema = JavaSchema {
        class: "example.Numbers",
        suid: 1,
        fields: vec![SchemaField {
            name: "a",
            typecode: FieldType::Array,
            class_name1: Some("[I"),
            nested: None,
            map: None,
            set: None,
            enumeration: None,
        }],
    };
    let value = Numbers { a: vec![1, 2, 3] };
    let bytes = to_vec(&value, &schema).unwrap();
    let decoded: Numbers = from_slice(&bytes, &schema, &Allowlist::all()).unwrap();
    assert_eq!(decoded, value);
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Inner {
    id: i32,
    label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Outer {
    inner: Inner,
    note: String,
}

fn inner_schema() -> JavaSchema {
    JavaSchema {
        class: "example.Inner",
        suid: 1,
        fields: vec![
            SchemaField {
                name: "id",
                typecode: FieldType::Int,
                class_name1: None,
                nested: None,
                map: None,
                set: None,
                enumeration: None,
            },
            SchemaField {
                name: "label",
                typecode: FieldType::Object,
                class_name1: Some("Ljava/lang/String;"),
                nested: None,
                map: None,
                set: None,
                enumeration: None,
            },
        ],
    }
}

fn outer_schema() -> JavaSchema {
    JavaSchema {
        class: "example.Outer",
        suid: 1,
        fields: vec![
            SchemaField {
                name: "inner",
                typecode: FieldType::Object,
                class_name1: Some("Lexample/Inner;"),
                nested: Some(Box::new(inner_schema())),
                map: None,
                set: None,
                enumeration: None,
            },
            SchemaField {
                name: "note",
                typecode: FieldType::Object,
                class_name1: Some("Ljava/lang/String;"),
                nested: None,
                map: None,
                set: None,
                enumeration: None,
            },
        ],
    }
}

#[test]
fn serde_supports_nested_objects() {
    let value = Outer {
        inner: Inner {
            id: 5,
            label: "x".to_owned(),
        },
        note: "y".to_owned(),
    };
    let bytes = to_vec(&value, &outer_schema()).unwrap();
    let decoded: Outer = from_slice(&bytes, &outer_schema(), &Allowlist::all()).unwrap();
    assert_eq!(decoded, value);

    let graph = jser::decode_stream_with_allowlist(&bytes, &Allowlist::all()).unwrap();
    let outer = &graph.items()[0];
    assert_eq!(graph.class_name(outer).unwrap(), "example.Outer");
    let inner = graph.require_field(outer, "inner").unwrap();
    assert_eq!(graph.class_name(inner).unwrap(), "example.Inner");
    assert_eq!(
        graph
            .as_i32(graph.require_field(inner, "id").unwrap())
            .unwrap(),
        5
    );
}

fn string_value() -> SchemaValue {
    SchemaValue {
        typecode: FieldType::Object,
        class_name1: Some("Ljava/lang/String;"),
        nested: None,
    }
}

fn int_value() -> SchemaValue {
    SchemaValue {
        typecode: FieldType::Int,
        class_name1: None,
        nested: None,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Collections {
    map: HashMap<String, i32>,
    linked: HashMap<String, String>,
    set: HashSet<String>,
}

fn collections_schema() -> JavaSchema {
    JavaSchema {
        class: "example.Collections",
        suid: 1,
        fields: vec![
            SchemaField {
                name: "map",
                typecode: FieldType::Object,
                class_name1: Some("Ljava/util/HashMap;"),
                nested: None,
                map: Some(Box::new(MapSchema {
                    key: string_value(),
                    value: int_value(),
                })),
                set: None,
                enumeration: None,
            },
            SchemaField {
                name: "linked",
                typecode: FieldType::Object,
                class_name1: Some("Ljava/util/LinkedHashMap;"),
                nested: None,
                map: Some(Box::new(MapSchema {
                    key: string_value(),
                    value: string_value(),
                })),
                set: None,
                enumeration: None,
            },
            SchemaField {
                name: "set",
                typecode: FieldType::Object,
                class_name1: Some("Ljava/util/HashSet;"),
                nested: None,
                map: None,
                set: Some(Box::new(string_value())),
                enumeration: None,
            },
        ],
    }
}

#[test]
fn serde_supports_map_and_set_fields() {
    let mut allowlist = Allowlist::strict();
    allowlist.add("example.*").unwrap();

    let value = Collections {
        map: [("b".to_owned(), 2), ("a".to_owned(), 1)]
            .into_iter()
            .collect(),
        linked: [
            ("x".to_owned(), "1".to_owned()),
            ("y".to_owned(), "2".to_owned()),
        ]
        .into_iter()
        .collect(),
        set: ["u".to_owned(), "v".to_owned()].into_iter().collect(),
    };

    let bytes = to_vec(&value, &collections_schema()).unwrap();
    let decoded: Collections = from_slice(&bytes, &collections_schema(), &allowlist).unwrap();
    assert_eq!(decoded, value);

    let graph = jser::decode_stream_with_allowlist(&bytes, &allowlist).unwrap();
    let item = &graph.items()[0];

    let map = graph
        .as_map(graph.require_field(item, "map").unwrap())
        .unwrap()
        .unwrap();
    assert!(!map.ordered);
    assert_eq!(map.entries.len(), 2);

    let linked = graph
        .as_map(graph.require_field(item, "linked").unwrap())
        .unwrap()
        .unwrap();
    assert!(linked.ordered);
    assert_eq!(linked.entries.len(), 2);

    let set = graph
        .as_set(graph.require_field(item, "set").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(set.len(), 2);
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct WithInnerMap {
    by_name: HashMap<String, Inner>,
}

#[test]
fn serde_supports_nested_pojo_values_in_maps() {
    let schema = JavaSchema {
        class: "example.WithInnerMap",
        suid: 1,
        fields: vec![SchemaField {
            name: "by_name",
            typecode: FieldType::Object,
            class_name1: Some("Ljava/util/HashMap;"),
            nested: None,
            map: Some(Box::new(MapSchema {
                key: string_value(),
                value: SchemaValue {
                    typecode: FieldType::Object,
                    class_name1: Some("Lexample/Inner;"),
                    nested: Some(Box::new(inner_schema())),
                },
            })),
            set: None,
            enumeration: None,
        }],
    };

    let value = WithInnerMap {
        by_name: [(
            "a".to_owned(),
            Inner {
                id: 1,
                label: "x".to_owned(),
            },
        )]
        .into_iter()
        .collect(),
    };
    let bytes = to_vec(&value, &schema).unwrap();
    let decoded: WithInnerMap = from_slice(&bytes, &schema, &Allowlist::all()).unwrap();
    assert_eq!(decoded, value);

    let graph = jser::decode_stream_with_allowlist(&bytes, &Allowlist::all()).unwrap();
    let item = &graph.items()[0];
    let by_name = graph.require_field(item, "by_name").unwrap();
    let view = graph.as_map(by_name).unwrap().unwrap();
    let (_, inner) = view.entries[0];
    assert_eq!(graph.class_name(inner).unwrap(), "example.Inner");
}

#[test]
fn serde_supports_map_and_set_roots() {
    let map: HashMap<String, i32> = [("a".to_owned(), 1), ("b".to_owned(), 2)]
        .into_iter()
        .collect();
    let bytes = to_vec_map(&map, &string_value(), &int_value(), false).unwrap();
    let decoded: HashMap<String, i32> =
        from_slice_map(&bytes, &string_value(), &int_value(), &Allowlist::strict()).unwrap();
    assert_eq!(decoded, map);

    let linked: HashMap<String, i32> = [("a".to_owned(), 1)].into_iter().collect();
    let bytes = to_vec_map(&linked, &string_value(), &int_value(), true).unwrap();
    let graph = jser::decode_stream(&bytes).unwrap();
    assert!(graph.as_map(&graph.items()[0]).unwrap().unwrap().ordered);

    let set: HashSet<String> = ["u".to_owned(), "v".to_owned()].into_iter().collect();
    let bytes = to_vec_set(&set, &string_value()).unwrap();
    let decoded: HashSet<String> =
        from_slice_set(&bytes, &string_value(), &Allowlist::strict()).unwrap();
    assert_eq!(decoded, set);

    let int_keys: HashMap<i32, String> = [(1, "one".to_owned()), (2, "two".to_owned())]
        .into_iter()
        .collect();
    let bytes = to_vec_map(&int_keys, &int_value(), &string_value(), false).unwrap();
    let decoded: HashMap<i32, String> =
        from_slice_map(&bytes, &int_value(), &string_value(), &Allowlist::strict()).unwrap();
    assert_eq!(decoded, int_keys);
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum Color {
    Red,
    Green,
    Blue,
}

fn color_schema() -> EnumSchema {
    EnumSchema {
        class: "example.Color",
        constants: vec![
            EnumConstant {
                serde: "Red",
                java: "RED",
            },
            EnumConstant {
                serde: "Green",
                java: "GREEN",
            },
            EnumConstant {
                serde: "Blue",
                java: "BLUE",
            },
        ],
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Palette {
    name: String,
    color: Color,
}

#[test]
fn serde_supports_enum_fields() {
    let schema = JavaSchema {
        class: "example.Palette",
        suid: 1,
        fields: vec![
            SchemaField {
                name: "name",
                typecode: FieldType::Object,
                class_name1: Some("Ljava/lang/String;"),
                nested: None,
                map: None,
                set: None,
                enumeration: None,
            },
            SchemaField {
                name: "color",
                typecode: FieldType::Object,
                class_name1: Some("Lexample/Color;"),
                nested: None,
                map: None,
                set: None,
                enumeration: Some(Box::new(color_schema())),
            },
        ],
    };

    let value = Palette {
        name: "main".to_owned(),
        color: Color::Green,
    };
    let bytes = to_vec(&value, &schema).unwrap();
    let decoded: Palette = from_slice(&bytes, &schema, &Allowlist::all()).unwrap();
    assert_eq!(decoded, value);

    let graph = jser::decode_stream_with_allowlist(&bytes, &Allowlist::all()).unwrap();
    let color = graph.require_field(&graph.items()[0], "color").unwrap();
    assert_eq!(graph.class_name(color).unwrap(), "example.Color");
    assert_eq!(graph.as_enum_constant(color).unwrap(), Some("GREEN"));
    let desc = graph.descriptor_of(color).unwrap();
    assert_eq!(desc.suid, 0);
    assert_eq!(desc.flags, 0x12);
    assert_eq!(
        graph.descriptor(desc.super_desc.unwrap()).unwrap().name,
        "java.lang.Enum"
    );
}

#[test]
fn serde_supports_enum_roots() {
    let schema = color_schema();

    let bytes = to_vec_enum(&Color::Blue, &schema).unwrap();
    let decoded: Color = from_slice_enum(&bytes, &schema, &Allowlist::all()).unwrap();
    assert_eq!(decoded, Color::Blue);
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Label {
    text: String,
}

fn label_schema() -> JavaSchema {
    JavaSchema {
        class: "example.Label",
        suid: 1,
        fields: vec![SchemaField {
            name: "text",
            typecode: FieldType::Object,
            class_name1: Some("Ljava/lang/String;"),
            nested: None,
            map: None,
            set: None,
            enumeration: None,
        }],
    }
}

#[test]
fn serde_supports_multi_root_streams_and_resets() {
    let mut writer = MultiRootWriter::new();
    writer.push(&point_schema(), &Point { x: 1, y: 2 }).unwrap();
    writer.reset();
    writer
        .push(
            &label_schema(),
            &Label {
                text: "hi".to_owned(),
            },
        )
        .unwrap();
    let bytes = writer.finish().unwrap();

    let graph = jser::decode_stream_with_allowlist(&bytes, &Allowlist::all()).unwrap();
    assert_eq!(graph.items().len(), 3);
    assert!(matches!(graph.items()[1], jser::JValue::Reset));

    let roots: (Point, Label) = from_slice_multi(
        &bytes,
        &[&point_schema(), &label_schema()],
        &Allowlist::all(),
    )
    .unwrap();
    assert_eq!(roots.0, Point { x: 1, y: 2 });
    assert_eq!(
        roots.1,
        Label {
            text: "hi".to_owned()
        }
    );

    let mut writer = MultiRootWriter::new();
    writer.push(&point_schema(), &Point { x: 3, y: 4 }).unwrap();
    writer
        .push(
            &label_schema(),
            &Label {
                text: "a".to_owned(),
            },
        )
        .unwrap();
    let bytes = writer.finish().unwrap();
    let roots: (Point, Label) = from_slice_multi(
        &bytes,
        &[&point_schema(), &label_schema()],
        &Allowlist::all(),
    )
    .unwrap();
    assert_eq!(roots.0, Point { x: 3, y: 4 });
    assert_eq!(
        roots.1,
        Label {
            text: "a".to_owned()
        }
    );
}

#[test]
fn serde_expands_shared_references_as_values() {
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct SharedInner {
        id: i32,
    }
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct SharedRoot {
        a: SharedInner,
        b: SharedInner,
    }

    // Build a stream where `a` and `b` point at the same object (TC_REFERENCE).
    let mut graph = jser::Graph::new();
    let inner_desc = graph
        .add_class_desc(jser::ClassDesc {
            name: "example.SharedInner".to_owned(),
            suid: 1,
            flags: 0x02,
            fields: vec![jser::FieldDesc {
                name: "id".to_owned(),
                typecode: FieldType::Int,
                class_name1: None,
                class_name_handle: None,
            }],
            super_desc: None,
            annotations: Vec::new(),
            proxy_interfaces: None,
        })
        .unwrap();
    let inner = graph
        .new_object(vec![jser::ClassData {
            desc: inner_desc,
            fields: vec![("id".to_owned(), jser::JValue::Int(5))],
            custom: None,
        }])
        .unwrap();
    let root_desc = graph
        .add_class_desc(jser::ClassDesc {
            name: "example.SharedRoot".to_owned(),
            suid: 1,
            flags: 0x02,
            fields: vec![
                jser::FieldDesc {
                    name: "a".to_owned(),
                    typecode: FieldType::Object,
                    class_name1: Some("Lexample/SharedInner;".to_owned()),
                    class_name_handle: None,
                },
                jser::FieldDesc {
                    name: "b".to_owned(),
                    typecode: FieldType::Object,
                    class_name1: Some("Lexample/SharedInner;".to_owned()),
                    class_name_handle: None,
                },
            ],
            super_desc: None,
            annotations: Vec::new(),
            proxy_interfaces: None,
        })
        .unwrap();
    let root = graph
        .new_object(vec![jser::ClassData {
            desc: root_desc,
            fields: vec![
                ("a".to_owned(), jser::JValue::Obj(inner)),
                ("b".to_owned(), jser::JValue::Obj(inner)),
            ],
            custom: None,
        }])
        .unwrap();
    graph.add_item(jser::JValue::Obj(root));
    let bytes = jser::encode_stream(&graph).unwrap();

    let inner_schema = JavaSchema {
        class: "example.SharedInner",
        suid: 1,
        fields: vec![SchemaField {
            name: "id",
            typecode: FieldType::Int,
            class_name1: None,
            nested: None,
            map: None,
            set: None,
            enumeration: None,
        }],
    };
    let root_schema = JavaSchema {
        class: "example.SharedRoot",
        suid: 1,
        fields: vec![
            SchemaField {
                name: "a",
                typecode: FieldType::Object,
                class_name1: Some("Lexample/SharedInner;"),
                nested: Some(Box::new(inner_schema.clone())),
                map: None,
                set: None,
                enumeration: None,
            },
            SchemaField {
                name: "b",
                typecode: FieldType::Object,
                class_name1: Some("Lexample/SharedInner;"),
                nested: Some(Box::new(inner_schema)),
                map: None,
                set: None,
                enumeration: None,
            },
        ],
    };

    let decoded: SharedRoot = from_slice(&bytes, &root_schema, &Allowlist::all()).unwrap();
    assert_eq!(
        decoded,
        SharedRoot {
            a: SharedInner { id: 5 },
            b: SharedInner { id: 5 },
        }
    );
}
