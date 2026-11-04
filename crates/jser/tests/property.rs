//! Property-based tests (P9-26).
//!
//! `proptest`/`quickcheck` are not available in the offline dependency cache,
//! so this file uses a tiny deterministic xorshift generator with fixed seeds.
//! It is not a full property-testing framework (no automatic shrinking), but it
//! exercises the same invariants over many generated inputs and is completely
//! dependency-free/reproducible:
//!
//! - Unicode strings (including NUL/CJK/emoji/random scalars) round-trip;
//! - `TC_LONGSTRING` boundary lengths round-trip;
//! - primitive vectors and `HashMap<String, i32>` round-trip;
//! - re-encoding a decoded value is byte-stable;
//! - random/truncated bytes never panic (decoder limits apply);
//! - mutating decoded fields survives encode/decode.

mod common;

use std::collections::HashMap;

use common::{fixture_allowlist, read_fixture};
use jser::{
    decode_stream_with_allowlist, decode_stream_with_limits, decode_typed, encode_stream,
    encode_typed, JValue, Limits,
};

/// Deterministic xorshift64 generator (test-only).
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.0 = value;
        value
    }

    fn below(&mut self, limit: usize) -> usize {
        if limit == 0 {
            0
        } else {
            (self.next_u64() as usize) % limit
        }
    }

    fn i32(&mut self) -> i32 {
        self.next_u64() as i32
    }
}

const CHARS: [char; 8] = ['a', 'Z', '0', '\0', '中', '😀', '\u{7F}', 'é'];

fn random_string(rng: &mut Rng, max_len: usize) -> String {
    let len = rng.below(max_len + 1);
    (0..len)
        .map(|_| {
            if rng.below(4) == 0 {
                char::from_u32(rng.below(0x11_0000) as u32).unwrap_or('?')
            } else {
                CHARS[rng.below(CHARS.len())]
            }
        })
        .collect()
}

fn small_limits() -> Limits {
    Limits {
        max_depth: 32,
        max_elements: 10_000,
        max_stream_size: 4096,
    }
}

#[test]
fn random_strings_round_trip() {
    for seed in 0..200 {
        let mut rng = Rng::new(seed);
        let value = random_string(&mut rng, 64);
        let bytes = encode_typed(value.clone()).unwrap();
        let decoded: String = decode_typed(&bytes).unwrap();
        assert_eq!(decoded, value, "seed {seed}");
    }
}

#[test]
fn long_strings_round_trip() {
    for (seed, len) in [65_534usize, 65_535, 65_536, 70_000, 130_000]
        .into_iter()
        .enumerate()
    {
        let mut rng = Rng::new(seed as u64);
        let value: String = (0..len).map(|_| CHARS[rng.below(CHARS.len())]).collect();
        let bytes = encode_typed(value.clone()).unwrap();
        let decoded: String = decode_typed(&bytes).unwrap();
        assert_eq!(decoded, value, "length {len}");
    }
}

#[test]
fn random_primitive_vectors_round_trip() {
    for seed in 0..100 {
        let mut rng = Rng::new(seed);
        let value: Vec<i32> = (0..rng.below(65)).map(|_| rng.i32()).collect();
        let bytes = encode_typed(value.clone()).unwrap();
        let decoded: Vec<i32> = decode_typed(&bytes).unwrap();
        assert_eq!(decoded, value, "seed {seed}");
    }
}

#[test]
fn random_maps_round_trip() {
    for seed in 0..100 {
        let mut rng = Rng::new(seed);
        let mut value = HashMap::new();
        for _ in 0..rng.below(33) {
            let key = random_string(&mut rng, 16);
            value.insert(key, rng.i32());
        }
        let bytes = encode_typed(value.clone()).unwrap();
        let decoded: HashMap<String, i32> = decode_typed(&bytes).unwrap();
        assert_eq!(decoded, value, "seed {seed}");
    }
}

#[test]
fn reencoding_a_decoded_value_is_stable() {
    for seed in 0..100 {
        let mut rng = Rng::new(seed);
        let value = random_string(&mut rng, 128);
        let first = encode_typed(value).unwrap();
        let decoded: String = decode_typed(&first).unwrap();
        let second = encode_typed(decoded).unwrap();
        assert_eq!(first, second, "string seed {seed}");

        let vector: Vec<i32> = (0..rng.below(65)).map(|_| rng.i32()).collect();
        let first = encode_typed(vector).unwrap();
        let decoded: Vec<i32> = decode_typed(&first).unwrap();
        let second = encode_typed(decoded).unwrap();
        assert_eq!(first, second, "vector seed {seed}");
    }
}

#[test]
fn random_bytes_never_panic() {
    for seed in 0..300 {
        let mut rng = Rng::new(seed);
        let len = rng.below(256);
        let mut bytes: Vec<u8> = (0..len).map(|_| rng.next_u64() as u8).collect();
        if rng.below(2) == 0 {
            bytes.splice(0..0, [0xAC, 0xED, 0x00, 0x05]);
        }
        let _ = decode_stream_with_limits(&bytes, small_limits());
    }
}

#[test]
fn truncated_streams_never_panic() {
    for seed in 0..100 {
        let mut rng = Rng::new(seed);
        let value = random_string(&mut rng, 256);
        let bytes = encode_typed(value).unwrap();
        let cut = rng.below(bytes.len() + 1);
        let _ = decode_stream_with_limits(&bytes[..cut], small_limits());
    }
}

#[test]
fn mutating_random_point_fields_round_trips() {
    let original = read_fixture("object/20");
    for seed in 0..100 {
        let mut rng = Rng::new(seed);
        let mut graph = decode_stream_with_allowlist(&original, &fixture_allowlist()).unwrap();
        let point = graph.items()[0].clone();
        let x = rng.i32();
        let y = rng.i32();
        assert!(graph.set_field(&point, "x", JValue::Int(x)).unwrap());
        assert!(graph.set_field(&point, "y", JValue::Int(y)).unwrap());

        let encoded = encode_stream(&graph).unwrap();
        let decoded = decode_stream_with_allowlist(&encoded, &fixture_allowlist()).unwrap();
        let point = &decoded.items()[0];
        assert_eq!(
            decoded
                .as_i64(decoded.require_field(point, "x").unwrap())
                .unwrap(),
            i64::from(x)
        );
        assert_eq!(
            decoded
                .as_i64(decoded.require_field(point, "y").unwrap())
                .unwrap(),
            i64::from(y)
        );
    }
}
