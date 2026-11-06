# jserde

A pure-Rust implementation of Java Object Serialization, interoperable with
the JDK's `ObjectOutputStream` / `ObjectInputStream` in both directions, with
**no runtime JVM dependency**.

[简体中文](README.md) | English

Current version: `0.1.0` (published to crates.io on 2026-11-06).

## Installation

```bash
# core library
cargo add jserde

# optional derive macros
cargo add jserde-derive

# optional schema-driven serde bridge
cargo add jserde-serde
```

GitHub: [Whikiey/jserde](https://github.com/Whikiey/jserde)

Crates: [jserde](https://crates.io/crates/jserde),
[jserde-derive](https://crates.io/crates/jserde-derive),
[jserde-serde](https://crates.io/crates/jserde-serde). Docs:
[docs.rs/jserde](https://docs.rs/jserde).

See [RELEASING.md](RELEASING.md) and the
[0.1.0 release notes](docs/release-notes-0.1.0.md).

## Compatibility

| Item | Value |
|---|---|
| Java baseline | Java 17 fixtures; wire format compatible with Java 1.2+ for supported classes |
| Rust MSRV | 1.77 |
| Platforms | Linux / macOS / Windows (CI matrix) |
| Runtime | No JVM, no `unsafe` |
| License | MIT OR Apache-2.0 |

During `0.x`, breaking API changes are allowed; semver compatibility is not
promised before 1.0. See [API stability](docs/api-stability.md) and
[RELEASING.md](RELEASING.md).

## Features

| Capability | Status |
|---|---|
| Stream header, `TC_STRING` / `TC_LONGSTRING`, modified UTF-8, block data, handles, `TC_RESET` | ✅ |
| `classDesc`, objects, inheritance, custom `writeObject`, arrays, enums | ✅ |
| `HashMap` / `LinkedHashMap` / `HashSet` / `ArrayList` / `LinkedList` | ✅ (`HashMapLayout` can pin exact capacity/threshold) |
| Shared references, cycles, multi-element streams | ✅ |
| Dynamic POJOs and unknown-class writeback | ✅ |
| `decode_typed` / `encode_typed`, `FromJValue` / `IntoJValue` (slices, arrays, BTree, composite fields) | ✅ |
| Streaming `decode_from<R: Read>` / `encode_to<W: Write>` | ✅ (incremental decode) |
| `#[derive(JavaSerializable)]`, `computeDefaultSUID` / `suid` tool | ✅ (generic structs supported) |
| Semantic views for `Date`, `BigInteger`, `BigDecimal`, `java.time` | ✅ |
| Strict-by-default allowlist | ✅ |
| Schema-driven serde bridge (nested POJOs, Map/Set/enum, multi-root) | ✅ (limits below; `Shared<T>` preserves non-cyclic identity) |
| 42 regular fixtures replayed byte-for-byte + structured `TC_EXCEPTION` errors | ✅ |
| Rust write → Java readback | ✅ (42 regular fixtures + constructed/typed/derive/serde) |

## Quick start

Read a stream:

```rust
use jserde::{decode_stream_with_allowlist, Allowlist};

fn main() -> jserde::Result<()> {
    let bytes = std::fs::read("stream.ser").map_err(|error| jserde::Error::FieldMismatch {
        message: error.to_string(),
    })?;
    let allowlist = Allowlist::strict().allow("com.example.*")?;
    let graph = decode_stream_with_allowlist(&bytes, &allowlist)?;

    for item in graph.items() {
        if let Some(map) = graph.as_map(item)? {
            for (key, value) in map.entries {
                println!("{:?} => {:?}", key, value);
            }
        }
    }
    Ok(())
}
```

Write a map:

```rust
use jserde::{builtin::new_hash_map, encode_stream, Graph, JValue};

fn main() -> jserde::Result<()> {
    let mut graph = Graph::new();
    let map = new_hash_map(&mut graph, &[("a", "one")])?;
    graph.add_item(JValue::Obj(map));
    let bytes = encode_stream(&graph)?;
    Ok(())
}
```

Derive:

```rust
use jserde_derive::JavaSerializable;

#[derive(JavaSerializable, Clone)]
#[java(class = "com.example.Point", suid = 1)]
struct Point {
    x: i32,
    y: i32,
}
```

## Security defaults

`decode_*` uses a **strict allowlist** by default: only `java.lang.*` and
`java.util.*` are accepted. Other classes must be granted explicitly via
`Allowlist`; fixture tests add `Fixtures$*`, `java.math.*`, and `java.time.*`.

**The allowlist is not a gadget sandbox.** An allowed class may still contain a
dangerous `readObject` implementation. See [SECURITY.md](SECURITY.md) for
resource limits and reporting guidance.

## Build and verify

```bash
cargo test --workspace          # 198 tests
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

cd docs/testdata
./gen/gen.sh --check       # 43 JDK-generated fixtures / 79031 bytes
./gen/interop.sh           # Rust write -> Java readback + SUID (JDK 17)

# optional
cargo run --release -p jserde --example bench -- 16
cargo test -p jserde --test fuzz_smoke
```

The 30-minute decode fuzz gate passed locally on 2026-11-06: 1801 s,
5,000,272 runs, peak RSS 476 MB. The nightly schedule is enabled in
`.github/workflows/nightly.yml`.

## Documentation

- [Requirements](docs/requirements.md)
- [Design](docs/design.md)
- [Implementation plan](docs/implementation-plan.md)
- [API stability](docs/api-stability.md)
- [Test data](docs/testdata/README.md)
- [Benchmark](docs/bench.md)
- [Releasing](RELEASING.md)
- [Contributing](CONTRIBUTING.md)
- [0.1.0 release notes](docs/release-notes-0.1.0.md)
- [Coding guide (Chinese)](AGENT.md)

## Known limitations

- Generic `Externalizable` semantics, dynamic-proxy invocation semantics,
  `TC_CLASS` class loading, and RMI/JNDI are out of scope; `TC_CLASS` / proxy
  wire fidelity is supported.
- The dynamic `JValue` layer supports arbitrary nesting depth (subject to
  `max_depth`). Derive supports nested custom fields, `Option<Custom>`,
  `Vec<Custom>`, `#[java(flatten)]` inheritance, and unit-only enums. The serde
  bridge supports recursive `SchemaField` POJOs, Map/Set/enum fields and roots.
- serde is still a tree model: ordinary shared references expand by value;
  `jserde_serde::Shared<T>` preserves non-cyclic identity within one call;
  full cycles use the dynamic `Graph` API and are rejected by serde.
- `java.util.Optional` is not `Serializable`.
- Primitive arrays use compact raw storage (4 B per `int`); a 25M-element
  `int[]` stays near 2.03× input RSS in local measurements.
- Closed P9 scope: input zero-copy (`Graph<'a>` rewrite) and full serde cycles
  are not planned.
- The default allowlist is not a complete deserialization sandbox.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
