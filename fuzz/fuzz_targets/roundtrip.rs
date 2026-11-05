#![no_main]

use jserde::{decode_stream_with_allowlist, encode, Allowlist};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(graph) = decode_stream_with_allowlist(data, &Allowlist::all()) {
        if let Ok(encoded) = encode(&graph, graph.items()) {
            let _ = decode_stream_with_allowlist(&encoded, &Allowlist::all());
        }
    }
});
