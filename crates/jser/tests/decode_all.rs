//! P2 exit check: every committed fixture must decode without an error.

mod common;

use common::{decode_fixture, load_manifest, read_fixture};

#[test]
fn every_fixture_decodes() {
    let manifest = load_manifest();

    for case in &manifest.cases {
        let bytes = read_fixture(&case.id);
        if case.expect.get("kind").and_then(serde_json::Value::as_str) == Some("exception") {
            assert!(
                matches!(decode_fixture(&bytes), Err(jser::Error::JavaException(_))),
                "{} must report JavaException",
                case.id
            );
            continue;
        }
        decode_fixture(&bytes).unwrap_or_else(|err| {
            panic!(
                "{} ({} bytes, {}) failed to decode: {err}",
                case.id,
                case.size,
                case.features.join(", ")
            )
        });
    }
}
