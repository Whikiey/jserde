//! P8-4: explicit negative coverage for OUT/malformed protocol paths.

use jserde::{decode_stream, decode_stream_with_allowlist, Allowlist, Error};

fn header() -> Vec<u8> {
    vec![0xAC, 0xED, 0x00, 0x05]
}

fn utf(value: &str) -> Vec<u8> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(2 + bytes.len());
    out.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
    out.extend_from_slice(bytes);
    out
}

/// A minimal class descriptor with no fields and a null superclass.
fn class_desc(name: &str, flags: u8) -> Vec<u8> {
    let mut out = vec![0x72]; // TC_CLASSDESC
    out.extend(utf(name));
    out.extend_from_slice(&1i64.to_be_bytes()); // serialVersionUID
    out.push(flags);
    out.extend_from_slice(&0u16.to_be_bytes()); // field count
    out.push(0x78); // TC_ENDBLOCKDATA (classAnnotation)
    out.push(0x70); // TC_NULL (superClassDesc)
    out
}

fn object_with_desc(desc: Vec<u8>) -> Vec<u8> {
    let mut out = header();
    out.push(0x73); // TC_OBJECT
    out.extend(desc);
    out
}

#[test]
fn rejects_legacy_externalizable_without_block_data() {
    let mut bytes = object_with_desc(class_desc("example.Legacy", 0x04));
    bytes.push(0x78); // no classdata, just an end marker to make truncation distinct

    let error = decode_stream_with_allowlist(&bytes, &Allowlist::all()).unwrap_err();
    assert!(
        matches!(error, Error::UnsupportedClass(ref name) if name == "example.Legacy"),
        "{error:?}"
    );
}

#[test]
fn rejects_a_non_serializable_class_descriptor() {
    let bytes = object_with_desc(class_desc("example.NotSerializable", 0x00));
    let error = decode_stream_with_allowlist(&bytes, &Allowlist::all()).unwrap_err();
    assert!(
        matches!(error, Error::UnsupportedClass(ref name) if name == "example.NotSerializable"),
        "{error:?}"
    );
}

#[test]
fn rejects_a_null_class_descriptor_for_tc_class() {
    let mut bytes = header();
    bytes.push(0x76); // TC_CLASS
    bytes.push(0x70); // TC_NULL

    let error = decode_stream(&bytes).unwrap_err();
    assert!(matches!(error, Error::FieldMismatch { .. }), "{error:?}");
}

#[test]
fn rejects_a_bad_tag_after_tc_class() {
    let mut bytes = header();
    bytes.push(0x76); // TC_CLASS
    bytes.push(0x73); // TC_OBJECT is not a valid class descriptor here

    let error = decode_stream(&bytes).unwrap_err();
    assert!(matches!(error, Error::BadTag(0x73)), "{error:?}");
}

#[test]
fn rejects_a_top_level_proxy_class_descriptor() {
    let mut bytes = header();
    bytes.push(0x7D); // TC_PROXYCLASSDESC

    let error = decode_stream_with_allowlist(&bytes, &Allowlist::all()).unwrap_err();
    assert!(
        matches!(error, Error::UnsupportedClass(ref name) if name == "dynamic proxy"),
        "{error:?}"
    );
}

#[test]
fn rejects_a_top_level_class_descriptor() {
    let mut bytes = header();
    bytes.push(0x72); // TC_CLASSDESC

    let error = decode_stream(&bytes).unwrap_err();
    assert!(matches!(error, Error::FieldMismatch { .. }), "{error:?}");
}

#[test]
fn rejects_endblockdata_as_a_top_level_value() {
    let mut bytes = header();
    bytes.push(0x78); // TC_ENDBLOCKDATA

    let error = decode_stream(&bytes).unwrap_err();
    assert!(matches!(error, Error::BadTag(0x78)), "{error:?}");
}

#[test]
fn rejects_a_negative_proxy_interface_count() {
    let mut bytes = header();
    bytes.push(0x73); // TC_OBJECT
    bytes.push(0x7D); // TC_PROXYCLASSDESC
    bytes.extend_from_slice(&(-1i32).to_be_bytes());

    let error = decode_stream_with_allowlist(&bytes, &Allowlist::all()).unwrap_err();
    assert!(
        matches!(
            error,
            Error::InvalidLength {
                kind: "proxy interface count",
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn rejects_a_bad_proxy_superclass_tag() {
    let mut bytes = header();
    bytes.push(0x73); // TC_OBJECT
    bytes.push(0x7D); // TC_PROXYCLASSDESC
    bytes.extend_from_slice(&0i32.to_be_bytes()); // no interfaces
    bytes.push(0x78); // classAnnotation end
    bytes.push(0x99); // invalid superclass tag

    let error = decode_stream_with_allowlist(&bytes, &Allowlist::all()).unwrap_err();
    assert!(matches!(error, Error::BadTag(0x99)), "{error:?}");
}

#[test]
fn rejects_a_proxy_interface_that_is_not_allowlisted() {
    let mut bytes = header();
    bytes.push(0x73); // TC_OBJECT
    bytes.push(0x7D); // TC_PROXYCLASSDESC
    bytes.extend_from_slice(&1i32.to_be_bytes());
    bytes.extend(utf("example.NotAllowed"));
    bytes.push(0x78); // classAnnotation end
    bytes.push(0x70); // super

    let error = decode_stream(&bytes).unwrap_err();
    assert!(
        matches!(error, Error::UnsupportedClass(ref name) if name == "example.NotAllowed"),
        "{error:?}"
    );
}
