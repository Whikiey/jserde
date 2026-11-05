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
  (Duration, Instant, LocalDate, LocalTime, LocalDateTime, ZonedDateTime,
  Year, YearMonth, MonthDay, Period, ZoneId, ZoneOffset, OffsetTime,
  OffsetDateTime).
- Strict-by-default `Allowlist` with exact/prefix/array rules.
- Derive unit-only Rust enums as Java enums (`TC_ENUM`, flags `0x12`, SUID 0,
  `java.lang.Enum` superclass); variant names default to UPPER_SNAKE_CASE.
- serde bridge: direct `Serializer`/`Deserializer` over `JValue` for typed
  paths (no JSON intermediate); `from_slice_roots` stays a JSON view.
- serde bridge: multi-root streams with optional `TC_RESET` markers
  (`MultiRootWriter`, `from_slice_multi`/`from_slice_roots`); shared references
  decode as duplicated values (cycles stay in the dynamic `Graph` API).
- serde bridge: enum fields (`SchemaField::enumeration` with a serde↔Java
  constant map) and enum roots (`to_vec_enum`/`from_slice_enum`).
- serde bridge: Map/Set fields (`SchemaField::map`/`set`, HashMap/
  LinkedHashMap/HashSet) plus map/set root helpers; nested POJO values in
  maps; `LinkedHashMap` writer.
- Derive inheritance via a `#[java(flatten)]` superclass field: the parent
  layer is written before the child layer with a shared `super_desc`.
- Custom object arrays in derive (`Vec<Custom>` via `ARRAY_DESCRIPTOR`); array
  class SUIDs are computed by the default algorithm for any element class.
- Nested `JavaSerializable` fields (`DESCRIPTOR` const) and `Option<Custom>`
  in derive; recursive `SchemaField::nested` in the serde bridge.
- Derived `from_jvalue` validates Java class name and SUID before reading
  fields (P9-19); class/serialVersionUID mismatches return `FieldMismatch`
  with both SUIDs.
- `TC_EXCEPTION` parsing (P9-3): `Error::JavaException` carries the exception
  class, `Throwable.detailMessage` and summarized fields; `object/50` fixture.
- Derive combination fields (`Option<Vec<T>>`, `Option<HashMap>`, `Vec<Option<T>>`,
  including custom element arrays) and typed `IntoJValue` for `&[T]`, `[T; N]`,
  `BTreeMap`/`BTreeSet` (P9-18/P9-20).
- Streaming input `decode_from<R: Read>` and output `encode_to<W: Write>` with a
  hard stream-size limit (P9-4); the encoder currently stages one encoded graph.
- Exact `HashMap` write layout (`HashMapLayout` + `new_hash_map_entries_with_layout`)
  for capacity/threshold/loadFactor matching a specific JDK construction (P9-6).
- Generic `#[derive(JavaSerializable)]` structs (JVM descriptor fields are
  monomorphized; `schema()` now returns `Vec<JavaField>`).
- `jser_serde::Shared<T>` (`Rc<T>`) preserves non-cyclic shared pointer
  identity within one serde call; full cycles are closed as a scope decision
  and remain supported by the dynamic `Graph` API (P9-12, partial accepted).
- Scope decision: P9-5 zero-copy/borrowed input decoding is closed (not planned;
  existing `as_str`/`as_primitive_array` views are arena borrows, not input
  zero-copy).
- Schema-driven `jser-serde` bridge (single root; recursive nested POJOs).
- Compact primitive-array storage (`PrimitiveArray` view over raw wire bytes);
  a 25M-element `int[]` now decodes at ~2.0× input RSS instead of ~9×.
- Documentation closeout: compiled `jser-serde` example, resolved design §8
  questions (Java 17 baseline, no crates.io release yet, performance targets),
  `cargo doc -D warnings` in CI.
- `Graph::set_map_entry` (replace/append with primitive boxing) and
  `set_array_element`; broader mutation tests (all scalars, maps, object
  arrays, Java Mixed read-back).
- Nightly bench regression gate (`docs/bench_gate.py`, baseline JSON,
  throughput threshold and `int[]` RSS ratio).
- CI matrix (Linux/macOS/Windows), MSRV 1.77 check, i686 check and
  `cargo-deny` (licenses/bans/sources/advisories) audit.
- EnumSet/EnumMap support: SerializationProxy/keyType decoding views and
  `new_enum_set`/`new_enum_map` constructors.
- `TC_PROXYCLASSDESC` fidelity (interface list, super `Proxy`, `h` handler)
  with byte-exact replay; invocation semantics stay out of scope.
- `TC_CLASS` fidelity (`JValue::Class`, shared class-object references,
  `new_class`) with byte-exact replay.
- Extended container views (`as_list`/`as_set`/`as_map`) for LinkedHashSet,
  TreeMap/TreeSet, ArrayDeque, PriorityQueue, Vector/Stack, Hashtable, Properties,
  Collections wrappers and Arrays$ArrayList, plus 11 write-side constructors.
- Semantic views for common value classes: UUID, Currency, Locale, BitSet,
  StringBuilder/StringBuffer, Pattern and java.sql.Date/Time/Timestamp.
- Dependency-free seeded property tests (`tests/property.rs`): strings, long
  strings, vectors, maps, re-encode stability, random/truncated bytes, mutation.
- Hybrid nested fixture `object/44`: Map × array × custom class with shared
  references and a self-cycle; Rust build + decode/replay regressions.
- 43 JDK-generated fixtures / 79031 bytes, byte-exact replay, Java `Verify`
  interop (every fixture re-encoded by Rust and read back), SUID comparison,
  deterministic fuzz smoke and a benchmark example.
- P8/P9: compact primitive arrays, nested derive/serde, Map/Set/enum serde,
  multi-root + `TC_RESET`, `TC_CLASS`/proxy fidelity, extended containers
  (EnumSet/EnumMap included), common value classes, property/mutation tests,
  cross-platform/MSRV/i686/cargo-deny CI and bench gate.
- Three-level nested POJO fixture (`object/41`) and regression tests.
- P7 hardening: panic audit, compact bulk decoding of large top-level blocks,
  libFuzzer targets, nightly-ready CI smoke, README/SECURITY/CHANGELOG.

### Known limitations

- Generic `Externalizable` semantics, dynamic-proxy invocation semantics,
  `TC_CLASS` class-loading semantics and RMI/JNDI are out of scope; P9-1/2 only
  provide byte/handle-fidelity replay.
- serde is a tree model: shared references decode as duplicated values and
  cycles are not representable; object-array fields are unsupported.
- `Collections$SynchronizedList` decode support exists, but it is not in the
  cross-JDK fixture because its computed SUID varies between JDK builds.
- `java.util.Optional` is not `Serializable`.
- Allowlist is not a gadget sandbox.
- AC-04: the 30-minute decode fuzz workflow is in place but deferred until
  feature work is complete. The offline smoke test is always on.
- AC-06: synthetic 100 MB block and a real 25M-element `int[]` stay under the
  3× memory target with compact primitive arrays (local measurement).
