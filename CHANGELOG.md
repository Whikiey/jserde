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
- Schema-driven `jser-serde` bridge (single-root flat POJO).
- 34 JDK-generated fixtures, byte-exact replay, Java `Verify` interop,
  SUID comparison, deterministic fuzz smoke and a benchmark example.

### Known limitations

- Generic `Externalizable`, dynamic proxies, `TC_CLASS`, RMI/JNDI are out of scope.
- serde bridge: no nested POJOs, Map/Set fields, cycles/shared refs, `TC_RESET`
  or multi-root streams.
- `Year`/`YearMonth`/`MonthDay`/`Period` semantic views are not implemented.
- `java.util.Optional` is not `Serializable`.
- Allowlist is not a gadget sandbox.
