//! P0 error-model contract: every variant must be constructible and produce an
//! actionable display string (class names, offsets, limits, handles).

use jser::Error;

#[test]
fn every_error_has_an_actionable_display() {
    let cases = vec![
        (Error::BadMagic { found: 0x1234 }, "0x1234"),
        (Error::UnsupportedVersion { found: 0x0006 }, "0x0006"),
        (
            Error::Truncated {
                needed: 4,
                remaining: 1,
            },
            "need 4",
        ),
        (Error::BadTag(0xFF), "0xFF"),
        (Error::BadHandle(0x7E_0001), "0x7E0001"),
        (Error::MalformedString { offset: 3 }, "offset 3"),
        (
            Error::UnsupportedClass("com.example.Foo".to_owned()),
            "com.example.Foo",
        ),
        (
            Error::FieldMismatch {
                message: "expected int for `x`".to_owned(),
            },
            "expected int",
        ),
        (
            Error::LimitExceeded {
                kind: "depth",
                limit: 1024,
                actual: 1025,
            },
            "depth",
        ),
        (
            Error::InvalidSchema("missing CLASS".to_owned()),
            "missing CLASS",
        ),
    ];

    for (error, needle) in cases {
        let message = error.to_string();
        assert!(!message.is_empty(), "an error must have a message");
        assert!(
            message.contains(needle),
            "`{message}` should contain `{needle}`"
        );

        let _: &dyn std::error::Error = &error;
    }
}

#[test]
fn errors_debug_format_includes_the_variant() {
    let error = Error::BadMagic { found: 0x0000 };
    let debug = format!("{error:?}");

    assert!(
        debug.contains("BadMagic"),
        "unexpected debug output: {debug}"
    );
}
