//! Java "modified UTF-8" (CESU-8-style) codec.

use jserde::wire::mutf8::{decode, decode_at, encode, encoded_len};
use jserde::Error;

#[test]
fn decodes_ascii_nul_and_bmp() {
    assert_eq!(decode(b"hello").unwrap(), "hello");
    assert_eq!(decode(&[0xC0, 0x80]).unwrap(), "\0");
    assert_eq!(decode(&[0xC3, 0xA9]).unwrap(), "é");
    assert_eq!(
        decode(&[0xE4, 0xB8, 0xAD, 0xE6, 0x96, 0x87]).unwrap(),
        "中文"
    );
}

#[test]
fn decodes_a_surrogate_pair_into_one_char() {
    let bytes = [0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80];
    assert_eq!(decode(&bytes).unwrap(), "😀");
}

#[test]
fn encodes_supplementary_chars_as_two_three_byte_units() {
    assert_eq!(encode("😀"), [0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]);
    assert_eq!(encode("\0"), [0xC0, 0x80]);
    assert_eq!(encode("é"), [0xC3, 0xA9]);
    assert_eq!(encode("中"), [0xE4, 0xB8, 0xAD]);
}

#[test]
fn encode_and_decode_round_trip() {
    for text in [
        "",
        "hello",
        "a\0b",
        "aé中",
        "😀中文",
        "x".repeat(70_000).as_str(),
    ] {
        let bytes = encode(text);
        assert_eq!(
            encoded_len(text),
            bytes.len(),
            "length mismatch for {text:?}"
        );
        assert_eq!(decode(&bytes).unwrap(), text);
    }
}

#[test]
fn rejects_illegal_or_truncated_sequences() {
    let cases: &[&[u8]] = &[
        &[0x00],                   // NUL must be C0 80
        &[0xC0, 0x81],             // overlong NUL
        &[0xC1, 0xBF],             // overlong 2-byte
        &[0xE0, 0x80, 0x80],       // overlong 3-byte
        &[0xF0, 0x9F, 0x98, 0x80], // standard 4-byte UTF-8 is not MUTF-8
        &[0xC3],                   // truncated 2-byte
        &[0xE4, 0xB8],             // truncated 3-byte
        &[0xED, 0xA0, 0xBD],       // unpaired high surrogate
        &[0xED, 0xB8, 0x80],       // unpaired low surrogate
        &[0xC3, 0x28],             // bad continuation
        &[0x80],                   // stray continuation
    ];

    for bytes in cases {
        let error = decode(bytes).unwrap_err();
        assert!(
            matches!(error, Error::MalformedString { .. }),
            "expected MalformedString for {bytes:02X?}, got {error:?}"
        );
    }
}

#[test]
fn reports_the_byte_offset() {
    let error = decode_at(&[0xFF], 10).unwrap_err();
    assert!(matches!(error, Error::MalformedString { offset: 10 }));
}
