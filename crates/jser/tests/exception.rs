//! P9-3: a `TC_EXCEPTION` stream carries the exception object and must be
//! reported as a structured `Error::JavaException`, not a generic class error.

mod common;

use common::read_fixture;
use jser::{decode_stream, Error};

#[test]
fn reports_a_structured_java_exception() {
    let bytes = read_fixture("object/50");
    let error = decode_stream(&bytes).expect_err("TC_EXCEPTION must abort decoding");

    let Error::JavaException(info) = error else {
        panic!("expected JavaException, got {error:?}");
    };
    assert_eq!(info.class, "java.io.IOException");
    assert_eq!(info.message.as_deref(), Some("exception-message"));
    assert!(
        info.fields
            .iter()
            .any(|field| field.name == "detailMessage" && field.value == "exception-message"),
        "fields: {:?}",
        info.fields
    );
    assert!(
        info.fields
            .iter()
            .any(|field| field.name == "stackTrace" && field.value.starts_with("<array")),
        "fields: {:?}",
        info.fields
    );
}
