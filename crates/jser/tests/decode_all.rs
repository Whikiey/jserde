//! P2 exit check: every committed fixture must decode without an error.

mod common;

use common::{decode_fixture, load_manifest, read_fixture};

#[test]
fn every_fixture_decodes() {
    let manifest = load_manifest();

    for case in &manifest.cases {
        let bytes = read_fixture(&case.id);
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
