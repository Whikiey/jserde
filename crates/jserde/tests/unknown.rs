//! P4 unknown-class writeback (AC-05): classes with no Rust definition still
//! decode into the dynamic graph and re-encode byte-for-byte.

mod common;

use common::{decode_fixture, read_fixture};
use jserde::encode_stream;

#[test]
fn unknown_classes_replay_byte_exactly() {
    for id in ["object/23", "object/36", "object/39"] {
        let bytes = read_fixture(id);
        let graph = decode_fixture(&bytes).unwrap_or_else(|err| panic!("{id} decode: {err}"));
        let encoded = encode_stream(&graph).unwrap_or_else(|err| panic!("{id} encode: {err}"));

        assert_eq!(encoded, bytes, "{id} must replay byte-for-byte");
    }
}
