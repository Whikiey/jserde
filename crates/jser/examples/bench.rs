//! Small dependency-free benchmark harness for P7.
//!
//!   cargo run --release -p jser --example bench -- [megabytes]
//!
//! It reports decode throughput for the 100 MB-ish block stream and a few
//! fixture-shaped streams, plus the process peak RSS on Linux (`VmHWM`).

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use jser::{decode_stream_with_allowlist, Allowlist};

fn allowlist() -> Allowlist {
    Allowlist::strict()
        .allow("Fixtures$*")
        .unwrap()
        .allow("java.math.*")
        .unwrap()
        .allow("java.time.*")
        .unwrap()
}

fn main() {
    let megabytes = std::env::args()
        .nth(1)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(16);

    let testdata = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/testdata");
    for (name, file) in [
        ("string-70k", "fixtures/codec/06-string-long.ser"),
        ("point", "fixtures/object/20-object-point.ser"),
        ("hashmap", "fixtures/object/29-collection-hashmap.ser"),
    ] {
        let bytes = fs::read(testdata.join(file)).expect("fixture");
        let iterations = 200;
        let start = Instant::now();
        for _ in 0..iterations {
            let graph = decode_stream_with_allowlist(&bytes, &allowlist()).expect("decode");
            std::hint::black_box(graph);
        }
        let elapsed = start.elapsed().as_secs_f64();
        let throughput = (bytes.len() as f64 * iterations as f64) / elapsed / 1_000_000.0;
        println!("{name:12} {:>8} B  {throughput:10.2} MB/s", bytes.len());
    }

    let size = megabytes.saturating_mul(1024 * 1024);
    let mut stream = Vec::with_capacity(5 + 4 + size);
    stream.extend_from_slice(&[0xAC, 0xED, 0x00, 0x05]);
    stream.push(0x7A); // TC_BLOCKDATALONG
    stream.extend_from_slice(&(size as u32).to_be_bytes());
    stream.resize(5 + 4 + size, 0);

    let start = Instant::now();
    let graph = decode_stream_with_allowlist(&stream, &Allowlist::strict()).expect("decode");
    let elapsed = start.elapsed().as_secs_f64();
    let throughput = (stream.len() as f64) / elapsed / 1_000_000.0;
    println!(
        "block-{megabytes}MB   {:>8} B  {throughput:10.2} MB/s  items={}",
        stream.len(),
        graph.items().len()
    );

    if let Ok(status) = fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if let Some(value) = line.strip_prefix("VmHWM:") {
                println!("peak RSS: {}", value.trim());
            }
        }
    }
}
