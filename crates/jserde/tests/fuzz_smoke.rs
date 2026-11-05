//! P7-1 offline fuzz smoke: deterministic mutations/truncations must never panic.
//!
//! The long-running `cargo fuzz` targets live in `fuzz/`; this test is the
//! always-on CI smoke because it needs no external fuzzing toolchain.

mod common;

use common::{fixture_allowlist, load_manifest, read_fixture};
use jserde::decode_stream_with_allowlist;

fn lcg(state: &mut u64) -> u8 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    (*state >> 33) as u8
}

#[test]
fn mutated_fixtures_never_panic() {
    let allowlist = fixture_allowlist();

    for case in &load_manifest().cases {
        let original = read_fixture(&case.id);
        let mut state = 0x1234_5678_9abc_def0u64;

        for length in [
            0,
            1,
            2,
            3,
            4,
            5,
            8,
            13,
            original.len() / 2,
            original.len() - 1,
        ] {
            if length <= original.len() {
                let mut bytes = original.clone();
                bytes.truncate(length);
                let _ = decode_stream_with_allowlist(&bytes, &allowlist);
            }
        }

        for _ in 0..64 {
            let mut bytes = original.clone();
            if bytes.is_empty() {
                continue;
            }
            let index = usize::from(lcg(&mut state)) % bytes.len();
            bytes[index] ^= lcg(&mut state);
            let _ = decode_stream_with_allowlist(&bytes, &allowlist);
        }
    }

    for length in 0..256usize {
        let mut bytes = vec![0u8; length];
        let mut state = 0xdead_beef_cafe_babeu64;
        for byte in &mut bytes {
            *byte = lcg(&mut state);
        }
        let _ = decode_stream_with_allowlist(&bytes, &allowlist);
    }
}
