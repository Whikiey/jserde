//! P2 resource limits and malformed-input behavior.

mod common;

use common::read_fixture;
use jser::{decode_stream, Error, Limits, Reader};

fn limits(max_depth: usize, max_elements: usize, max_stream_size: usize) -> Limits {
    Limits {
        max_depth,
        max_elements,
        max_stream_size,
    }
}

#[test]
fn rejects_a_stream_larger_than_the_limit() {
    let bytes = read_fixture("object/20");
    let error = Reader::with_limits(&bytes, limits(512, 64, 4)).unwrap_err();
    assert!(matches!(
        error,
        Error::LimitExceeded {
            kind: "stream size",
            ..
        }
    ));
}

#[test]
fn rejects_too_many_elements() {
    let bytes = read_fixture("object/25");
    let mut reader = Reader::with_limits(&bytes, limits(512, 2, 1024)).unwrap();

    let error = reader.read_next().unwrap_err();
    assert!(matches!(
        error,
        Error::LimitExceeded {
            kind: "elements",
            ..
        }
    ));
}

#[test]
fn rejects_too_deep_nesting() {
    let bytes = read_fixture("object/27");
    let mut reader = Reader::with_limits(&bytes, limits(0, 64, 1024)).unwrap();

    let error = reader.read_next().unwrap_err();
    assert!(matches!(error, Error::LimitExceeded { kind: "depth", .. }));
}

#[test]
fn truncated_object_returns_an_error_instead_of_panicking() {
    let mut bytes = read_fixture("object/20");
    bytes.truncate(20);

    let error = decode_stream(&bytes).unwrap_err();
    assert!(matches!(error, Error::Truncated { .. }));
}

#[test]
fn invalid_reference_handle_returns_an_error() {
    let mut bytes = read_fixture("object/33");
    let length = bytes.len();
    bytes[length - 4..].copy_from_slice(&0x00_7E_00_09u32.to_be_bytes());

    let error = decode_stream(&bytes).unwrap_err();
    assert!(matches!(error, Error::BadHandle(0x00_7E_00_09)));
}

#[test]
fn negative_array_length_returns_an_error() {
    let mut bytes = read_fixture("object/25");
    bytes[0x17..0x1B].copy_from_slice(&(-1i32).to_be_bytes());

    let error = decode_stream(&bytes).unwrap_err();
    assert!(matches!(error, Error::InvalidLength { kind: "array", .. }));
}

#[test]
fn random_bytes_never_panic() {
    let mut state = 0x1234_5678_9ABC_DEF0u64;
    for length in 0..64usize {
        let mut bytes = vec![0u8; length];
        for byte in &mut bytes {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            *byte = state as u8;
        }

        // Only the error/ok distinction matters here: the contract is "no
        // panic", not "every random stream is valid".
        let _ = decode_stream(&bytes);
    }
}
