//! P3 byte-replay gate: an untouched decoded graph must encode back to the
//! exact fixture bytes. This is an internal regression gate (design §5.1), not
//! an acceptance criterion.

mod common;

use common::{decode_fixture, load_manifest, read_fixture};
use jser::encode_stream;

#[test]
fn every_fixture_replays_byte_exactly() {
    let manifest = load_manifest();

    for case in &manifest.cases {
        let bytes = read_fixture(&case.id);
        let graph = decode_fixture(&bytes).unwrap_or_else(|err| panic!("{}: {err}", case.id));
        let encoded = encode_stream(&graph).unwrap_or_else(|err| panic!("{}: {err}", case.id));

        assert_eq!(
            encoded,
            bytes,
            "{} did not replay byte-exactly ({} vs {} bytes)",
            case.id,
            encoded.len(),
            bytes.len()
        );
    }
}
