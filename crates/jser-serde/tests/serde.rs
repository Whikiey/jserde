mod common {
    pub fn allowlist() -> jser::Allowlist {
        let mut allowlist = jser::Allowlist::strict();
        allowlist.add("Fixtures$*").unwrap();
        allowlist
    }
}

use jser::Allowlist;
use jser::FieldType;
use jser_serde::{from_slice, to_vec, JavaSchema, SchemaField};
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
            },
            SchemaField {
                name: "y",
                typecode: FieldType::Int,
                class_name1: None,
                nested: None,
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

fn outer_schema() -> JavaSchema {
    JavaSchema {
        class: "example.Outer",
        suid: 1,
        fields: vec![
            SchemaField {
                name: "inner",
                typecode: FieldType::Object,
                class_name1: Some("Lexample/Inner;"),
                nested: Some(Box::new(JavaSchema {
                    class: "example.Inner",
                    suid: 1,
                    fields: vec![
                        SchemaField {
                            name: "id",
                            typecode: FieldType::Int,
                            class_name1: None,
                            nested: None,
                        },
                        SchemaField {
                            name: "label",
                            typecode: FieldType::Object,
                            class_name1: Some("Ljava/lang/String;"),
                            nested: None,
                        },
                    ],
                })),
            },
            SchemaField {
                name: "note",
                typecode: FieldType::Object,
                class_name1: Some("Ljava/lang/String;"),
                nested: None,
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
