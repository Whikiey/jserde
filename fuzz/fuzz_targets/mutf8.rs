#![no_main]

use jser::wire::mutf8;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = mutf8::decode(data) {
        let encoded = mutf8::encode(&text);
        if let Ok(decoded) = mutf8::decode(&encoded) {
            assert_eq!(decoded, text);
        }
    }
});
