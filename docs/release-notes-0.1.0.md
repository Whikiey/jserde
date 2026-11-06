# jserde 0.1.0 — Release Candidate

Release date: 2026-11-06  
Status: published to crates.io (`jserde`, `jserde-derive`, `jserde-serde`).

## Highlights

- Pure-Rust Java Object Serialization reader/writer, compatible with
  `ObjectOutputStream` / `ObjectInputStream`, with no runtime JVM.
- Dynamic `Graph`/`JValue` API with class descriptors, inheritance, custom
  `writeObject` blocks, arrays, enums, collections, shared references, cycles,
  `TC_RESET`, and unknown-class writeback.
- Typed API (`decode_typed` / `encode_typed`) and `#[derive(JavaSerializable)]`,
  including nested/generic/composite fields, inheritance, enums, and SUID
  calculation.
- Schema-driven `jserde-serde` bridge with nested POJOs, Map/Set/enum fields and
  roots, multi-root/`TC_RESET` streams, and non-cyclic `Shared<T>` identity.
- Built-in semantic views for Date/BigInteger/BigDecimal, java.time, common
  value classes, and extended containers.
- Strict-by-default allowlist and bounded decoder; streaming `decode_from`
  input and `encode_to` output APIs.
- 43 JDK-generated fixtures (79031 bytes), byte-exact replay for regular
  fixtures, Java readback interop, SUID comparison, 198 tests, 30-minute fuzz
  gate, and a benchmark/baseline gate.

## Compatibility

- Java 17 fixture baseline; wire format is Java 1.2+ for supported classes.
- Rust 1.77 MSRV; Linux/macOS/Windows CI.
- Java 8+ classes are supported when their wire shape is covered by the
  implementation; unsupported classes remain opaque or return explicit errors.

## Known limitations

- Generic `Externalizable` semantics, proxy invocation semantics, `TC_CLASS`
  class loading, and RMI/JNDI are out of scope; class/proxy wire fidelity is
  supported.
- serde is a tree model; full object cycles require the dynamic `Graph` API.
- Zero-copy decoding from the input buffer is not implemented (closed scope);
  `as_str`/`as_primitive_array` are arena borrows.
- `java.util.Optional` is not `Serializable`.
- The allowlist is not a gadget sandbox.

## Verification

```text
cargo test --workspace               198 passed
fixtures                             43 cases / 79031 bytes
Java interop                         42 regular fixtures + SUID
fuzz decode                          1801 s / 5,000,272 runs / 476 MB
fuzz roundtrip                       301 s / 3,397,428 runs
fuzz mutf8                           301 s / 68,445,913 runs
```
