//! P5-2/P5-5: derive JavaSerializable and round-trip against the Java fixtures.

mod common;

use std::collections::HashMap;

use common::{fixture_allowlist, read_fixture};
use jser::{decode_typed_with_allowlist, encode_typed, Allowlist, JavaSerializable};
use jser_derive::JavaSerializable;

#[derive(JavaSerializable, Debug, Clone, PartialEq)]
#[java(class = "Fixtures$Point", suid = 1)]
struct Point {
    #[java(rename = "x")]
    java_x: i32,
    #[java(rename = "y")]
    java_y: i32,
}

#[derive(JavaSerializable, Debug, Clone, PartialEq)]
#[java(class = "Fixtures$Mixed", suid = 1)]
struct Mixed {
    i: i32,
    l: i64,
    s: String,
    a: Vec<i32>,
    b: bool,
    d: f64,
    f: f32,
    c: char,
    sh: i16,
    by: i8,
}

#[derive(JavaSerializable, Debug, Clone, PartialEq)]
#[java(class = "Fixtures$WithTransient", suid = 1)]
struct WithTransient {
    a: i32,
    s: String,
    #[java(transient)]
    t: String,
}

#[derive(JavaSerializable, Debug, Clone, PartialEq)]
#[java(class = "Fixtures$Derived", suid = 1)]
struct Derived {
    #[java(rename = "a")]
    base: i32,
    #[java(rename = "b")]
    value: String,
}

#[derive(JavaSerializable, Debug, Clone, PartialEq)]
#[java(class = "Fixtures$CustomWritable", suid = 1)]
struct CustomWritable {
    value: i32,
}

#[test]
fn derive_decodes_point_fixture() {
    let point: Point =
        decode_typed_with_allowlist(&read_fixture("object/20"), &fixture_allowlist()).unwrap();
    assert_eq!(
        point,
        Point {
            java_x: 1,
            java_y: 2
        }
    );
}

#[test]
fn derive_encodes_point_with_renamed_fields() {
    let bytes = encode_typed(Point {
        java_x: 7,
        java_y: 8,
    })
    .unwrap();
    let point: Point = decode_typed_with_allowlist(&bytes, &fixture_allowlist()).unwrap();
    assert_eq!(
        point,
        Point {
            java_x: 7,
            java_y: 8
        }
    );
}

#[test]
fn derive_handles_mixed_fields_in_java_order() {
    let names: Vec<&str> = Mixed::schema().iter().map(|field| field.name).collect();
    assert_eq!(names, ["b", "by", "c", "d", "f", "i", "l", "sh", "a", "s"]);

    let mixed: Mixed =
        decode_typed_with_allowlist(&read_fixture("object/21"), &fixture_allowlist()).unwrap();
    assert_eq!(mixed.i, 7);
    assert_eq!(mixed.a, [1, 2]);
    assert_eq!(mixed.s, "mix");
    assert_eq!(mixed.c, '中');

    let bytes = encode_typed(mixed.clone()).unwrap();
    let decoded: Mixed = decode_typed_with_allowlist(&bytes, &fixture_allowlist()).unwrap();
    assert_eq!(decoded, mixed);
}

#[test]
fn derive_skips_transient_fields() {
    let with_transient: WithTransient =
        decode_typed_with_allowlist(&read_fixture("object/37"), &fixture_allowlist()).unwrap();
    assert_eq!(with_transient.a, 1);
    assert_eq!(with_transient.s, "s");
    assert_eq!(with_transient.t, "");

    let bytes = encode_typed(WithTransient {
        a: 1,
        s: "s".to_owned(),
        t: "must not be written".to_owned(),
    })
    .unwrap();
    let decoded: WithTransient = decode_typed_with_allowlist(&bytes, &fixture_allowlist()).unwrap();
    assert_eq!(decoded, with_transient);
}

#[test]
fn derive_inherits_parent_fields_through_rename() {
    let derived: Derived =
        decode_typed_with_allowlist(&read_fixture("object/22"), &fixture_allowlist()).unwrap();
    assert_eq!(derived.base, 1);
    assert_eq!(derived.value, "b");
}

#[test]
fn derive_rejects_a_class_with_custom_write_method_but_reads_fields() {
    // The derived struct only knows the declared field; custom data is not
    // modeled by the derive layer. Encoding is still a valid object.
    let custom: CustomWritable =
        decode_typed_with_allowlist(&read_fixture("object/23"), &fixture_allowlist()).unwrap();
    assert_eq!(custom.value, 42);

    let bytes = encode_typed(CustomWritable { value: 42 }).unwrap();
    let decoded: CustomWritable =
        decode_typed_with_allowlist(&bytes, &fixture_allowlist()).unwrap();
    assert_eq!(decoded, custom);
}

#[test]
fn derive_supports_map_fields() {
    // A small typed check that HashMap field descriptors use java.util.HashMap.
    #[derive(JavaSerializable, Debug, Clone, PartialEq)]
    #[java(class = "example.WithMap", suid = 1)]
    struct WithMap {
        map: HashMap<String, i32>,
    }

    let value = WithMap {
        map: [("a".to_owned(), 1)].into_iter().collect(),
    };
    let bytes = encode_typed(value.clone()).unwrap();
    let decoded: WithMap = decode_typed_with_allowlist(&bytes, &Allowlist::all()).unwrap();
    assert_eq!(decoded, value);
}
