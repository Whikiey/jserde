//! P5-2/P5-5: derive JavaSerializable and round-trip against the Java fixtures.

mod common;

use std::collections::HashMap;

use common::{fixture_allowlist, read_fixture};
use jser::{
    decode_stream_with_allowlist, decode_typed_with_allowlist, encode_typed, Allowlist, Error,
    JavaSerializable,
};
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
#[java(class = "Fixtures$Point", suid = 2)]
struct WrongPointSuid {
    #[java(rename = "x")]
    x: i32,
    #[java(rename = "y")]
    y: i32,
}

#[derive(JavaSerializable, Debug, Clone, PartialEq)]
#[java(class = "Fixtures$OtherPoint", suid = 1)]
struct OtherPoint {
    #[java(rename = "x")]
    x: i32,
    #[java(rename = "y")]
    y: i32,
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
fn derive_rejects_a_mismatched_serial_version_uid() {
    let bytes = encode_typed(WrongPointSuid { x: 1, y: 2 }).unwrap();
    let error = decode_typed_with_allowlist::<Point>(&bytes, &fixture_allowlist()).unwrap_err();
    assert!(matches!(error, Error::FieldMismatch { .. }));
    let message = error.to_string();
    assert!(message.contains("serialVersionUID mismatch"), "{message}");
    assert!(message.contains("Fixtures$Point"), "{message}");
    assert!(message.contains("expected 1"), "{message}");
    assert!(message.contains("got 2"), "{message}");
}

#[test]
fn derive_rejects_a_mismatched_class_name() {
    let bytes = encode_typed(OtherPoint { x: 1, y: 2 }).unwrap();
    let error = decode_typed_with_allowlist::<Point>(&bytes, &fixture_allowlist()).unwrap_err();
    assert!(matches!(error, Error::FieldMismatch { .. }));
    let message = error.to_string();
    assert!(message.contains("class name mismatch"), "{message}");
    assert!(message.contains("Fixtures$Point"), "{message}");
    assert!(message.contains("Fixtures$OtherPoint"), "{message}");
    assert!(message.contains("SUID 1"), "{message}");
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

#[derive(JavaSerializable, Debug, Clone, PartialEq)]
#[java(class = "Fixtures$Inner", suid = 1)]
struct Inner {
    label: String,
    value: i32,
}

#[derive(JavaSerializable, Debug, Clone, PartialEq)]
#[java(class = "Fixtures$Outer", suid = 1)]
struct Outer {
    inner: Inner,
    note: String,
}

#[derive(JavaSerializable, Debug, Clone, PartialEq)]
#[java(class = "Fixtures$Maybe", suid = 1)]
struct Maybe {
    inner: Option<Inner>,
}

#[derive(JavaSerializable, Debug, Clone, PartialEq)]
#[java(class = "com.example.Deep", suid = 1)]
struct Deep {
    id: i32,
}

#[test]
fn derive_nests_custom_objects() {
    assert_eq!(<Inner as JavaSerializable>::DESCRIPTOR, "LFixtures$Inner;");
    assert_eq!(
        <Deep as JavaSerializable>::DESCRIPTOR,
        "Lcom/example/Deep;",
        "dots become slashes in the JVM descriptor"
    );
    assert_eq!(
        <Deep as JavaSerializable>::ARRAY_DESCRIPTOR,
        "[Lcom/example/Deep;"
    );

    let value = Outer {
        inner: Inner {
            label: "in".to_owned(),
            value: 7,
        },
        note: "out".to_owned(),
    };
    let bytes = encode_typed(value.clone()).unwrap();
    let decoded: Outer = decode_typed_with_allowlist(&bytes, &fixture_allowlist()).unwrap();
    assert_eq!(decoded, value);

    let graph = decode_stream_with_allowlist(&bytes, &fixture_allowlist()).unwrap();
    let outer = &graph.items()[0];
    assert_eq!(graph.class_name(outer).unwrap(), "Fixtures$Outer");
    let inner = graph.require_field(outer, "inner").unwrap();
    assert_eq!(graph.class_name(inner).unwrap(), "Fixtures$Inner");
    assert_eq!(
        graph
            .as_i32(graph.require_field(inner, "value").unwrap())
            .unwrap(),
        7
    );
    assert_eq!(
        graph
            .as_str(graph.require_field(inner, "label").unwrap())
            .unwrap(),
        "in"
    );
}

#[test]
fn derive_nests_optional_custom_objects() {
    let some = Maybe {
        inner: Some(Inner {
            label: "x".to_owned(),
            value: 1,
        }),
    };
    let bytes = encode_typed(some.clone()).unwrap();
    let decoded: Maybe = decode_typed_with_allowlist(&bytes, &fixture_allowlist()).unwrap();
    assert_eq!(decoded, some);

    let none = Maybe { inner: None };
    let bytes = encode_typed(none.clone()).unwrap();
    let decoded: Maybe = decode_typed_with_allowlist(&bytes, &fixture_allowlist()).unwrap();
    assert_eq!(decoded, none);
}

#[derive(JavaSerializable, Debug, Clone, PartialEq)]
#[java(class = "Fixtures$Bag", suid = 1)]
struct Bag {
    items: Vec<Inner>,
}

#[test]
fn typed_slices_and_arrays_of_custom_objects() {
    let items: &[Inner] = &[
        Inner {
            label: "s".to_owned(),
            value: 1,
        },
        Inner {
            label: "t".to_owned(),
            value: 2,
        },
    ];
    let bytes = encode_typed(items).unwrap();
    let decoded: Vec<Inner> = decode_typed_with_allowlist(&bytes, &Allowlist::all()).unwrap();
    assert_eq!(decoded, items);

    let array = [Inner {
        label: "u".to_owned(),
        value: 3,
    }];
    let bytes = encode_typed(array).unwrap();
    let decoded: Vec<Inner> = decode_typed_with_allowlist(&bytes, &Allowlist::all()).unwrap();
    assert_eq!(decoded[0].label, "u");
    assert_eq!(decoded[0].value, 3);
}

#[test]
fn derive_supports_custom_object_arrays() {
    assert_eq!(
        <Inner as JavaSerializable>::ARRAY_DESCRIPTOR,
        "[LFixtures$Inner;"
    );
    let schema = Bag::schema();
    let items_field = schema
        .iter()
        .find(|field| field.name == "items")
        .expect("items field");
    assert_eq!(items_field.class_name1, Some("[LFixtures$Inner;"));

    let value = Bag {
        items: vec![
            Inner {
                label: "a".to_owned(),
                value: 1,
            },
            Inner {
                label: "b".to_owned(),
                value: 2,
            },
        ],
    };
    let bytes = encode_typed(value.clone()).unwrap();
    let decoded: Bag = decode_typed_with_allowlist(&bytes, &fixture_allowlist()).unwrap();
    assert_eq!(decoded, value);

    let graph = decode_stream_with_allowlist(&bytes, &fixture_allowlist()).unwrap();
    let bag = &graph.items()[0];
    let items = graph.require_field(bag, "items").unwrap();
    assert_eq!(graph.class_name(items).unwrap(), "[LFixtures$Inner;");
    let elements = graph.as_array(items).unwrap().unwrap();
    assert_eq!(elements.len(), 2);
    assert_eq!(graph.class_name(&elements[1]).unwrap(), "Fixtures$Inner");

    let empty = Bag { items: Vec::new() };
    let bytes = encode_typed(empty.clone()).unwrap();
    let decoded: Bag = decode_typed_with_allowlist(&bytes, &fixture_allowlist()).unwrap();
    assert_eq!(decoded, empty);
}

#[derive(JavaSerializable, Debug, Clone, PartialEq)]
#[java(class = "example.Combos", suid = 1)]
struct Combos {
    maybe_values: Option<Vec<i32>>,
    maybe_map: Option<HashMap<String, i32>>,
    options: Vec<Option<String>>,
}

#[test]
fn derive_supports_combination_fields() {
    let value = Combos {
        maybe_values: Some(vec![1, 2]),
        maybe_map: Some([("k".to_owned(), 7)].into_iter().collect()),
        options: vec![Some("a".to_owned()), None],
    };
    let bytes = encode_typed(value.clone()).unwrap();
    let decoded: Combos = decode_typed_with_allowlist(&bytes, &Allowlist::all()).unwrap();
    assert_eq!(decoded, value);

    let graph = decode_stream_with_allowlist(&bytes, &Allowlist::all()).unwrap();
    let object = &graph.items()[0];
    let maybe_values = graph.require_field(object, "maybe_values").unwrap();
    assert_eq!(graph.class_name(maybe_values).unwrap(), "[I");
    let maybe_map = graph.require_field(object, "maybe_map").unwrap();
    assert_eq!(graph.class_name(maybe_map).unwrap(), "java.util.HashMap");
    let options = graph.require_field(object, "options").unwrap();
    assert_eq!(graph.class_name(options).unwrap(), "[Ljava.lang.String;");

    let empty = Combos {
        maybe_values: None,
        maybe_map: None,
        options: Vec::new(),
    };
    let bytes = encode_typed(empty.clone()).unwrap();
    let decoded: Combos = decode_typed_with_allowlist(&bytes, &Allowlist::all()).unwrap();
    assert_eq!(decoded, empty);
    let graph = decode_stream_with_allowlist(&bytes, &Allowlist::all()).unwrap();
    assert!(matches!(
        graph
            .require_field(&graph.items()[0], "maybe_values")
            .unwrap(),
        jser::JValue::Null
    ));
}

#[derive(JavaSerializable, Debug, Clone, PartialEq)]
#[java(class = "example.NestedCombos", suid = 1)]
struct NestedCombos {
    maybe: Option<Vec<Inner>>,
    options: Vec<Option<Inner>>,
}

#[test]
fn derive_supports_nested_combination_fields() {
    let value = NestedCombos {
        maybe: Some(vec![Inner {
            label: "a".to_owned(),
            value: 1,
        }]),
        options: vec![
            Some(Inner {
                label: "b".to_owned(),
                value: 2,
            }),
            None,
        ],
    };
    let bytes = encode_typed(value.clone()).unwrap();
    let decoded: NestedCombos = decode_typed_with_allowlist(&bytes, &Allowlist::all()).unwrap();
    assert_eq!(decoded, value);

    let graph = decode_stream_with_allowlist(&bytes, &Allowlist::all()).unwrap();
    let object = &graph.items()[0];
    assert_eq!(
        graph
            .class_name(graph.require_field(object, "maybe").unwrap())
            .unwrap(),
        "[LFixtures$Inner;"
    );
    assert_eq!(
        graph
            .class_name(graph.require_field(object, "options").unwrap())
            .unwrap(),
        "[LFixtures$Inner;"
    );

    let empty = NestedCombos {
        maybe: None,
        options: Vec::new(),
    };
    let bytes = encode_typed(empty.clone()).unwrap();
    let decoded: NestedCombos = decode_typed_with_allowlist(&bytes, &Allowlist::all()).unwrap();
    assert_eq!(decoded, empty);
}

#[derive(JavaSerializable, Debug, Clone, PartialEq)]
#[java(class = "example.GenericBox", suid = 1)]
struct GenericBox<T> {
    value: T,
}

#[test]
fn derive_supports_generic_structs() {
    let value = GenericBox {
        value: Inner {
            label: "generic".to_owned(),
            value: 11,
        },
    };
    let bytes = encode_typed(value.clone()).unwrap();
    let decoded: GenericBox<Inner> =
        decode_typed_with_allowlist(&bytes, &Allowlist::all()).unwrap();
    assert_eq!(decoded, value);

    let schema = <GenericBox<Inner> as JavaSerializable>::schema();
    assert_eq!(schema[0].name, "value");
    assert_eq!(schema[0].class_name1, Some("LFixtures$Inner;"));

    let graph = decode_stream_with_allowlist(&bytes, &Allowlist::all()).unwrap();
    let value = graph.require_field(&graph.items()[0], "value").unwrap();
    assert_eq!(graph.class_name(value).unwrap(), "Fixtures$Inner");
}

#[derive(JavaSerializable, Debug, Clone, PartialEq)]
#[java(class = "Fixtures$Base", suid = 1)]
struct Base {
    a: i32,
}

#[derive(JavaSerializable, Debug, Clone, PartialEq)]
#[java(class = "Fixtures$Inherited", suid = 1)]
struct Inherited {
    #[java(flatten)]
    base: Base,
    b: String,
}

#[test]
fn derive_flattens_a_superclass_layer() {
    let value = Inherited {
        base: Base { a: 1 },
        b: "b".to_owned(),
    };
    let bytes = encode_typed(value.clone()).unwrap();
    let decoded: Inherited = decode_typed_with_allowlist(&bytes, &fixture_allowlist()).unwrap();
    assert_eq!(decoded, value);

    let graph = decode_stream_with_allowlist(&bytes, &fixture_allowlist()).unwrap();
    let inherited = &graph.items()[0];
    let object = graph.as_object(inherited).unwrap().unwrap();
    assert_eq!(object.classes.len(), 2, "parent layer then child layer");
    assert_eq!(
        graph.descriptor(object.classes[0].desc).unwrap().name,
        "Fixtures$Base"
    );
    assert_eq!(
        graph.descriptor(object.classes[1].desc).unwrap().name,
        "Fixtures$Inherited"
    );
    assert_eq!(
        graph
            .descriptor(object.classes[1].desc)
            .unwrap()
            .super_desc
            .map(|handle| graph.descriptor(handle).unwrap().name.as_str()),
        Some("Fixtures$Base")
    );
    assert_eq!(
        graph
            .as_i32(graph.require_field(inherited, "a").unwrap())
            .unwrap(),
        1
    );
    assert_eq!(
        graph
            .as_str(graph.require_field(inherited, "b").unwrap())
            .unwrap(),
        "b"
    );
}

#[test]
fn derive_decodes_the_java_inheritance_fixture() {
    // object/22 is Fixtures$Base{a:int} <- Fixtures$Derived{b:String}.
    #[derive(JavaSerializable, Debug, Clone, PartialEq)]
    #[java(class = "Fixtures$Derived", suid = 1)]
    struct FixtureDerived {
        #[java(flatten)]
        base: Base,
        b: String,
    }

    let derived: FixtureDerived =
        decode_typed_with_allowlist(&read_fixture("object/22"), &fixture_allowlist()).unwrap();
    assert_eq!(
        derived,
        FixtureDerived {
            base: Base { a: 1 },
            b: "b".to_owned(),
        }
    );

    let bytes = encode_typed(derived.clone()).unwrap();
    let decoded: FixtureDerived =
        decode_typed_with_allowlist(&bytes, &fixture_allowlist()).unwrap();
    assert_eq!(decoded, derived);
}

#[derive(JavaSerializable, Debug, Clone, PartialEq)]
#[java(class = "Fixtures$Color", suid = 0)]
enum Color {
    Red,
    Green,
    #[java(rename = "BLUE")]
    Blue,
}

#[derive(JavaSerializable, Debug, Clone, PartialEq)]
#[java(class = "Fixtures$OtherColor", suid = 0)]
enum OtherColor {
    Red,
    Green,
}

#[test]
fn derive_enum_rejects_a_mismatched_class_name() {
    let bytes = encode_typed(OtherColor::Green).unwrap();
    let error = decode_typed_with_allowlist::<Color>(&bytes, &fixture_allowlist()).unwrap_err();
    assert!(matches!(error, Error::FieldMismatch { .. }));
    let message = error.to_string();
    assert!(message.contains("class name mismatch"), "{message}");
    assert!(message.contains("Fixtures$Color"), "{message}");
    assert!(message.contains("Fixtures$OtherColor"), "{message}");
}

#[test]
fn derive_supports_java_enums() {
    let color: Color =
        decode_typed_with_allowlist(&read_fixture("object/24"), &fixture_allowlist()).unwrap();
    assert_eq!(color, Color::Green);
    assert_eq!(<Color as JavaSerializable>::SUID, 0);

    for color in [Color::Red, Color::Green, Color::Blue] {
        let bytes = encode_typed(color.clone()).unwrap();
        let decoded: Color = decode_typed_with_allowlist(&bytes, &fixture_allowlist()).unwrap();
        assert_eq!(decoded, color);
    }

    let bytes = encode_typed(Color::Blue).unwrap();
    let graph = decode_stream_with_allowlist(&bytes, &fixture_allowlist()).unwrap();
    let value = &graph.items()[0];
    assert_eq!(graph.class_name(value).unwrap(), "Fixtures$Color");
    assert_eq!(graph.as_enum_constant(value).unwrap(), Some("BLUE"));
    let desc = graph.descriptor_of(value).unwrap();
    assert_eq!(desc.suid, 0);
    assert_eq!(desc.flags, 0x12);
    assert_eq!(
        graph.descriptor(desc.super_desc.unwrap()).unwrap().name,
        "java.lang.Enum"
    );
}
