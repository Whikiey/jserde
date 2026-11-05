//! Small dependency-free benchmark harness for P7/P8.
//!
//!   cargo run --release -p jserde --example bench -- [megabytes]
//!   cargo run --release -p jserde --example bench -- array [elements] [repeats]
//!
//! The first form reports decode throughput for a synthetic 100 MB block
//! stream and a few fixture-shaped streams. The `array` form builds a real
//! `int[]` stream (25M elements ≈ 100 MB by default) and decodes it, which is
//! the P8-2/AC-06 workload. Peak RSS is printed on Linux (`VmHWM`); on other
//! platforms measure the process externally.

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use jserde::{decode_stream_with_allowlist, Allowlist};

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
    if std::env::args().nth(1).as_deref() == Some("array") {
        let elements = std::env::args()
            .nth(2)
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(25_000_000);
        let repeats = std::env::args()
            .nth(3)
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(1)
            .max(1);
        bench_primitive_array(elements, repeats);
        report_peak_rss();
        return;
    }

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

fn bench_primitive_array(elements: usize, repeats: usize) {
    let length = i32::try_from(elements).expect("element count must fit in i32");
    let header = 4   // stream header
        + 1          // TC_ARRAY
        + 1          // TC_CLASSDESC
        + 2          // class name length
        + 2          // "[I"
        + 8          // array SUID
        + 1          // flags
        + 2          // field count
        + 1          // TC_ENDBLOCKDATA
        + 1          // TC_NULL superclass
        + 4; // array length

    let mut stream = Vec::with_capacity(header + elements * 4);
    stream.extend_from_slice(&[0xAC, 0xED, 0x00, 0x05]);
    stream.push(0x75); // TC_ARRAY
    stream.push(0x72); // TC_CLASSDESC
    stream.extend_from_slice(&2u16.to_be_bytes());
    stream.extend_from_slice(b"[I");
    stream.extend_from_slice(&5_600_894_804_908_749_477i64.to_be_bytes()); // [I SUID
    stream.push(0x02); // SC_SERIALIZABLE
    stream.extend_from_slice(&0u16.to_be_bytes()); // no fields
    stream.push(0x78); // TC_ENDBLOCKDATA
    stream.push(0x70); // TC_NULL superclass
    stream.extend_from_slice(&length.to_be_bytes());
    stream.resize(header + elements * 4, 0);

    let start = Instant::now();
    let mut decoded_len = 0;
    for _ in 0..repeats {
        let graph =
            decode_stream_with_allowlist(&stream, &Allowlist::strict()).expect("decode int[]");
        decoded_len = graph
            .as_primitive_array(&graph.items()[0])
            .expect("primitive array view")
            .expect("int[] storage")
            .len();
        std::hint::black_box(&graph);
    }
    let elapsed = start.elapsed().as_secs_f64();
    assert_eq!(decoded_len, elements);

    let throughput = (stream.len() as f64 * repeats as f64) / elapsed / 1_000_000.0;
    println!(
        "int-array-{elements}  {:>9} B  {throughput:10.2} MB/s  repeats={repeats} items=1 len={decoded_len}",
        stream.len()
    );
}

fn report_peak_rss() {
    if let Ok(status) = fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if let Some(value) = line.strip_prefix("VmHWM:") {
                println!("peak RSS: {}", value.trim());
            }
        }
    }
}
