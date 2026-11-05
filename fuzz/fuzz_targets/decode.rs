#![no_main]

use jserde::{decode_stream_with_allowlist, Allowlist};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Any byte sequence must return an error or a graph; it must not panic.
    let _ = decode_stream_with_allowlist(data, &Allowlist::all());
});
