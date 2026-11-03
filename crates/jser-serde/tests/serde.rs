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
            },
            SchemaField {
                name: "y",
                typecode: FieldType::Int,
                class_name1: None,
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
        }],
    };
    let value = Numbers { a: vec![1, 2, 3] };
    let bytes = to_vec(&value, &schema).unwrap();
    let decoded: Numbers = from_slice(&bytes, &schema, &Allowlist::all()).unwrap();
    assert_eq!(decoded, value);
}
