# Changelog

## 0.1.0 (unreleased)

### Added

- Pure-Rust Java Object Serialization reader/writer (`AC ED 00 05`).
- Modified UTF-8, block data, wire handles, `TC_RESET`, multi-element streams.
- Class descriptors, objects, inheritance, custom `writeObject`, arrays, enums.
- `HashMap`/`LinkedHashMap`/`HashSet`/`ArrayList`/`LinkedList` codecs.
- Shared references and cycles; unknown-class writeback.
- `decode_typed`/`encode_typed`, `FromJValue`/`IntoJValue`.
- `#[derive(JavaSerializable)]` with `class`/`suid`/`rename`/`transient`.
- `computeDefaultSUID` + `.class` parser + `suid` binary.
- Semantic views for `Date`, `BigInteger`, `BigDecimal` and `java.time`
  (Duration, Instant, LocalDate, LocalTime, LocalDateTime, ZonedDateTime).
- Strict-by-default `Allowlist` with exact/prefix/array rules.
- serde bridge: Map/Set fields (`SchemaField::map`/`set`, HashMap/
  LinkedHashMap/HashSet) plus map/set root helpers; nested POJO values in
  maps; `LinkedHashMap` writer.
- Derive inheritance via a `#[java(flatten)]` superclass field: the parent
  layer is written before the child layer with a shared `super_desc`.
- Custom object arrays in derive (`Vec<Custom>` via `ARRAY_DESCRIPTOR`); array
  class SUIDs are computed by the default algorithm for any element class.
- Nested `JavaSerializable` fields (`DESCRIPTOR` const) and `Option<Custom>`
  in derive; recursive `SchemaField::nested` in the serde bridge.
- Schema-driven `jser-serde` bridge (single root; recursive nested POJOs).
- Compact primitive-array storage (`PrimitiveArray` view over raw wire bytes);
  a 25M-element `int[]` now decodes at ~2.0× input RSS instead of ~9×.
- 34 JDK-generated fixtures, byte-exact replay, Java `Verify` interop,
  SUID comparison, deterministic fuzz smoke and a benchmark example.
- Three-level nested POJO fixture (`object/41`) and regression tests.
- P7 hardening: panic audit, compact bulk decoding of large top-level blocks,
  libFuzzer targets, nightly-ready CI smoke, README/SECURITY/CHANGELOG.

### Known limitations

- Generic `Externalizable`, dynamic proxies, `TC_CLASS`, RMI/JNDI are out of scope.
- serde bridge: no object arrays, cycles/shared refs, `TC_RESET` or
  multi-root streams.
- `Year`/`YearMonth`/`MonthDay`/`Period` semantic views are not implemented.
- `java.util.Optional` is not `Serializable`.
- Allowlist is not a gadget sandbox.
- AC-04: the 30-minute decode fuzz workflow is in place but deferred until
  feature work is complete. The offline smoke test is always on.
- AC-06: the synthetic 100 MB block stream and a real 25M-element `int[]`
  both stay under the 3× memory target with compact primitive arrays.
