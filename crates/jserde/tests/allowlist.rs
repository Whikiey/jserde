//! P6-3 allowlist: strict by default, with explicit exact/prefix entries.

mod common;

use common::read_fixture;
use jserde::{decode_stream, decode_stream_with_allowlist, Allowlist, Error};

#[test]
fn strict_default_rejects_custom_classes() {
    let error = decode_stream(&read_fixture("object/20")).unwrap_err();
    assert!(matches!(
        error,
        Error::UnsupportedClass(ref name) if name == "Fixtures$Point"
    ));
}

#[test]
fn strict_default_allows_java_util_and_java_lang() {
    // HashMap + Integer + Number are all in java.util/java.lang.
    let graph = decode_stream(&read_fixture("object/29")).unwrap();
    assert_eq!(graph.items().len(), 1);
}

#[test]
fn explicit_class_and_prefix_can_be_allowed() {
    let mut point = Allowlist::strict();
    point.add("Fixtures$Point").unwrap();
    decode_stream_with_allowlist(&read_fixture("object/20"), &point).unwrap();

    let mut fixtures = Allowlist::strict();
    fixtures.add("Fixtures$*").unwrap();
    decode_stream_with_allowlist(&read_fixture("object/22"), &fixtures).unwrap();
}

#[test]
fn superclass_must_also_be_allowed() {
    let mut allowlist = Allowlist::strict();
    allowlist.add("Fixtures$Derived").unwrap();

    let error = decode_stream_with_allowlist(&read_fixture("object/22"), &allowlist).unwrap_err();
    assert!(matches!(
        error,
        Error::UnsupportedClass(ref name) if name == "Fixtures$Base"
    ));
}

#[test]
fn java_math_is_not_in_the_strict_default() {
    let error = decode_stream(&read_fixture("object/36")).unwrap_err();
    assert!(matches!(
        error,
        Error::UnsupportedClass(ref name) if name == "java.math.BigInteger"
    ));

    decode_stream_with_allowlist(&read_fixture("object/36"), &Allowlist::all()).unwrap();
}

#[test]
fn arrays_follow_their_element_class() {
    let strict = Allowlist::strict();
    assert!(strict.is_allowed("[Ljava.lang.String;"));
    assert!(strict.is_allowed("[[I"));
    assert!(!strict.is_allowed("[LFixtures$Point;"));

    let mut fixtures = Allowlist::strict();
    fixtures.add("Fixtures$*").unwrap();
    assert!(fixtures.is_allowed("[LFixtures$Point;"));
}

#[test]
fn invalid_patterns_are_rejected() {
    let mut allowlist = Allowlist::new();
    assert!(matches!(allowlist.add(""), Err(Error::InvalidSchema(_))));
    assert!(matches!(
        allowlist.add("java.*.Foo"),
        Err(Error::InvalidSchema(_))
    ));
    assert!(matches!(
        allowlist.add("java*"),
        Err(Error::InvalidSchema(_))
    ));
}
