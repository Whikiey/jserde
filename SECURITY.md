# Security

Java deserialization is a well-known attack surface. `jserde` is a decoder of an
untrusted-on-the-wire format; it is **not** a sandbox.

Current implementation: v0.1.0, P0–P9 code line, strict allowlist by default,
198 tests. The 30-minute fuzz gate passed locally (decode 1801s, peak 476 MB).
The remaining security-relevant gaps are listed in
`docs/implementation-plan.md` §20/§21 (notably: allowlist is not a gadget
sandbox). P9-5/P9-12 are closed scope decisions; streaming IO is implemented.
Primitive arrays use compact raw-byte storage; 25M-element `int[]` is under the
3× local memory target.

## Default policy

- `decode_stream` / `Reader::new` / `decode_typed` use `Allowlist::strict()`:
  only `java.lang.*` and `java.util.*` are accepted.
- Additional classes must be granted explicitly:

```rust
let allowlist = jserde::Allowlist::strict()
    .allow("com.example.*")?
    .allow("Fixtures$*")?;
let graph = jserde::decode_stream_with_allowlist(bytes, &allowlist)?;
```

- `Allowlist::all()` exists for trusted test/fixture streams only. Do not use it
  on untrusted input.

## Allowlist is not a sandbox

Granting a class permits its `readObject`/`readResolve` behavior on the Java
side. Gadget chains often live inside otherwise "safe" packages. The allowlist
reduces the set of instantiable classes; it does not prove that an allowed
class is safe. There is no gadget-chain detection, classloader isolation or
JVM-style security manager.

## Resource limits

`Limits` restricts nesting depth, total elements and stream size:

```rust
let limits = jserde::Limits { max_depth: 256, max_elements: 1_000_000, max_stream_size: 32 << 20 };
let reader = jserde::Reader::with_allowlist(bytes, limits, allowlist)?;
```

Random/truncated input is expected to return `Err`; it must not panic. Streaming
`decode_from` enforces `max_stream_size` in `ReadStream`; `encode_to` currently
stages one encoded graph before writing. Fuzz targets live in `fuzz/`, and
`cargo test -p jserde --test fuzz_smoke` is the offline smoke test.

## Dependency audit

`cargo deny check` (configuration in `deny.toml`) is part of CI: advisories
(yanked denied), license allowlist (MIT/Apache-2.0/BSD/ISC/Unicode/Unlicense/
Zlib), wildcard ban and registry pinning. Run it locally before releases.

## Reporting

This is a pre-release project without a private security channel yet. Do not
deploy it as a trust boundary; report issues to the repository maintainers.
