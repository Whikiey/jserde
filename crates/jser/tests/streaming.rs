//! P9-4: decode from `Read` and encode to `Write` without requiring a full
//! in-memory byte slice at the API boundary.

mod common;

use std::io::Cursor;

use common::{fixture_allowlist, load_manifest, read_fixture};
use jser::{
    decode_from, decode_from_with_options, encode_stream, encode_to, Allowlist, Error, Limits,
};

#[test]
fn decoding_from_a_reader_matches_slice_decoding() {
    let bytes = read_fixture("object/29");
    let graph = decode_from(Cursor::new(bytes.clone())).unwrap();
    assert_eq!(encode_stream(&graph).unwrap(), bytes);
}

#[test]
fn streaming_decoder_replays_all_regular_fixtures() {
    for case in load_manifest().cases {
        if case.expect.get("kind").and_then(serde_json::Value::as_str) == Some("exception") {
            continue;
        }
        let bytes = read_fixture(&case.id);
        let graph =
            jser::decode_from_with_allowlist(Cursor::new(bytes.clone()), &fixture_allowlist())
                .unwrap_or_else(|error| panic!("{}: {error}", case.id));
        assert_eq!(
            encode_stream(&graph).unwrap(),
            bytes,
            "{} did not replay through decode_from",
            case.id
        );
    }
}

#[test]
fn encoding_to_a_writer_matches_encode_stream() {
    let bytes = read_fixture("object/29");
    let graph = decode_from(Cursor::new(bytes.clone())).unwrap();
    let mut out = Vec::new();
    encode_to(&graph, &mut out).unwrap();
    assert_eq!(out, bytes);
}

#[test]
fn truncated_reader_returns_an_error() {
    let bytes = read_fixture("object/29");
    let error = decode_from(Cursor::new(&bytes[..bytes.len() - 1])).unwrap_err();
    assert!(
        matches!(error, Error::Truncated { .. } | Error::Io(_)),
        "{error:?}"
    );
}

#[test]
fn reader_stream_size_limit_applies() {
    let bytes = read_fixture("object/29");
    let limits = Limits {
        max_stream_size: 32,
        ..Limits::default()
    };
    let error =
        decode_from_with_options(Cursor::new(bytes), limits, Allowlist::strict()).unwrap_err();
    assert!(matches!(error, Error::LimitExceeded { kind, .. } if kind == "stream size"));
}
