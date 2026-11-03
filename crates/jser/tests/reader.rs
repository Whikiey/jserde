//! Bounded, big-endian wire cursor.

use jser::wire::StreamReader;
use jser::Error;

#[test]
fn reads_big_endian_integers_and_floats() {
    let mut reader = StreamReader::new(&[
        0x01, // u8
        0x12, 0x34, // u16
        0x00, 0x00, 0x00, 0x05, // u32
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x09, // u64
        0x01, 0x02, 0x03, 0x04, // i32
        0x3F, 0xC0, 0x00, 0x00, // f32 = 1.5
        0x3F, 0xF8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // f64 = 1.5
    ]);

    assert_eq!(reader.read_u8().unwrap(), 0x01);
    assert_eq!(reader.read_u16().unwrap(), 0x1234);
    assert_eq!(reader.read_u32().unwrap(), 5);
    assert_eq!(reader.read_u64().unwrap(), 9);
    assert_eq!(reader.read_i32().unwrap(), 0x0102_0304);
    assert_eq!(reader.read_f32().unwrap(), 1.5);
    assert_eq!(reader.read_f64().unwrap(), 1.5);
    assert!(reader.is_empty());
}

#[test]
fn tracks_position_and_remaining() {
    let mut reader = StreamReader::new(&[0xAA, 0xBB, 0xCC]);

    assert_eq!(reader.position(), 0);
    assert_eq!(reader.remaining(), 3);
    assert_eq!(reader.peek_u8().unwrap(), 0xAA);
    assert_eq!(reader.read_u8().unwrap(), 0xAA);
    assert_eq!(reader.position(), 1);
    assert_eq!(reader.remaining(), 2);
    assert_eq!(reader.read_exact(2).unwrap(), &[0xBB, 0xCC]);
    assert!(reader.is_empty());
}

#[test]
fn reports_truncation_instead_of_panicking() {
    let mut reader = StreamReader::new(&[0x01, 0x02]);

    let error = reader.read_u32().unwrap_err();
    assert!(matches!(
        error,
        Error::Truncated {
            needed: 4,
            remaining: 2
        }
    ));

    let mut reader = StreamReader::new(&[0x01, 0x02]);
    let error = reader.read_exact(3).unwrap_err();
    assert!(matches!(
        error,
        Error::Truncated {
            needed: 3,
            remaining: 2
        }
    ));
}

#[test]
fn peek_at_end_is_an_error() {
    let reader = StreamReader::new(&[]);

    assert!(matches!(
        reader.peek_u8().unwrap_err(),
        Error::Truncated {
            needed: 1,
            remaining: 0
        }
    ));
}

#[test]
fn reads_java_boolean_values() {
    let mut reader = StreamReader::new(&[0x00, 0x01]);
    assert!(!reader.read_bool().unwrap());
    assert!(reader.read_bool().unwrap());

    let mut reader = StreamReader::new(&[0x02]);
    assert!(matches!(
        reader.read_bool().unwrap_err(),
        Error::BadTag(0x02)
    ));
}
