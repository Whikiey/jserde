//! End-to-end codec fixtures (01–12): header, strings, null, blocks, reset and
//! wire handle references. These are the P1 exit tests.

mod common;

use common::{load_manifest, read_fixture};
use jser::wire::StreamReader;
use jser::{decode, decode_stream, Error, JValue};

fn expected(id: &str) -> serde_json::Value {
    let manifest = load_manifest();
    manifest
        .cases
        .into_iter()
        .find(|case| case.id == id)
        .unwrap_or_else(|| panic!("manifest has no case `{id}`"))
        .expect
}

#[test]
fn decodes_string_cases_from_the_manifest() {
    for id in [
        "codec/01", "codec/02", "codec/03", "codec/04", "codec/05", "codec/12",
    ] {
        let graph = decode_stream(&read_fixture(id)).unwrap();
        let items = graph.items();
        assert_eq!(items.len(), 1, "{id} should be a single element");

        let expected = expected(id);
        let expected = expected["value"].as_str().unwrap();
        assert_eq!(graph.as_str(&items[0]).unwrap(), expected, "{id}");
        assert!(!graph.str_is_long(&items[0]).unwrap(), "{id}");
    }
}

#[test]
fn decodes_a_long_string_over_65535_bytes() {
    let graph = decode_stream(&read_fixture("codec/06")).unwrap();
    let items = graph.items();
    let value = graph.as_str(&items[0]).unwrap();

    assert_eq!(value.len(), 70_000);
    assert!(value.chars().all(|ch| ch == 'x'));
    assert!(graph.str_is_long(&items[0]).unwrap());
}

#[test]
fn decodes_null() {
    let graph = decode_stream(&read_fixture("codec/07")).unwrap();
    let items = graph.items();

    assert_eq!(items.len(), 1);
    assert!(matches!(&items[0], JValue::Null));
}

#[test]
fn decodes_shared_string_as_a_handle_reference() {
    let graph = decode_stream(&read_fixture("codec/08")).unwrap();
    let items = graph.items();

    assert_eq!(items.len(), 2);
    assert_eq!(items[0], items[1], "second write must be TC_REFERENCE");
    assert_eq!(graph.as_str(&items[0]).unwrap(), "ab");
}

#[test]
fn decodes_a_merged_primitive_block() {
    let graph = decode_stream(&read_fixture("codec/09")).unwrap();
    let items = graph.items();

    assert_eq!(items.len(), 1);
    let JValue::Block(block) = &items[0] else {
        panic!("expected a block, got {:?}", items[0]);
    };
    assert_eq!(block.bytes.len(), 13);
    assert!(!block.long);

    let mut reader = StreamReader::new(&block.bytes);
    assert_eq!(reader.read_i32().unwrap(), 1);
    assert!(reader.read_bool().unwrap());
    assert_eq!(reader.read_f64().unwrap(), 1.5);
    assert!(reader.is_empty());
}

#[test]
fn decodes_reset_as_a_stream_item() {
    let graph = decode_stream(&read_fixture("codec/10")).unwrap();
    let items = graph.items();

    assert_eq!(items.len(), 3);
    assert_eq!(graph.as_str(&items[0]).unwrap(), "a");
    assert!(matches!(&items[1], JValue::Reset));
    assert_eq!(graph.as_str(&items[2]).unwrap(), "b");
    assert_ne!(items[0], items[2], "reset must start a new handle table");
}

#[test]
fn preserves_long_and_short_block_boundaries() {
    let graph = decode_stream(&read_fixture("codec/11")).unwrap();
    let items = graph.items();

    assert_eq!(items.len(), 2);
    let JValue::Block(first) = &items[0] else {
        panic!("expected a long block");
    };
    let JValue::Block(second) = &items[1] else {
        panic!("expected a second long block");
    };

    assert!(first.long);
    assert_eq!(first.bytes.len(), 1024);
    // 976 > 255, so the JDK also uses TC_BLOCKDATALONG for the remainder.
    assert!(second.long);
    assert_eq!(second.bytes.len(), 976);
    assert_eq!(first.bytes[0], 0);
    assert_eq!(first.bytes[1023], 0xFF);
    assert_eq!(second.bytes[0], 0x00);
    assert_eq!(second.bytes[975], 0xCF);
}

#[test]
fn rejects_bad_magic_and_version() {
    let error = decode_stream(&[0x00, 0x00, 0x00, 0x05]).unwrap_err();
    assert!(matches!(error, Error::BadMagic { found: 0x0000 }));

    let mut bytes = read_fixture("codec/01");
    bytes[3] = 0x06;
    let error = decode_stream(&bytes).unwrap_err();
    assert!(matches!(error, Error::UnsupportedVersion { found: 0x0006 }));
}

#[test]
fn truncated_string_returns_an_error() {
    let mut bytes = read_fixture("codec/01");
    bytes.truncate(6);

    let error = decode_stream(&bytes).unwrap_err();
    assert!(matches!(error, Error::Truncated { .. }));
}

#[test]
fn decode_requires_a_single_element() {
    let graph = decode(&read_fixture("codec/01")).unwrap();
    assert_eq!(graph.items().len(), 1);

    let error = decode(&read_fixture("codec/10")).unwrap_err();
    assert!(matches!(error, Error::FieldMismatch { .. }));
}
