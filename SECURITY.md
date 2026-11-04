# Security

Java deserialization is a well-known attack surface. `jser` is a decoder of an
untrusted-on-the-wire format; it is **not** a sandbox.

Current implementation: v0.1.0, P0–P7 code line plus P8-2, strict allowlist by
default, 144 tests. The remaining security-relevant gaps are listed in
`docs/implementation-plan.md` §20 (notably: allowlist is not a gadget sandbox,
the 30-minute fuzz workflow is in place but deferred until feature work is
complete). Primitive arrays now use compact raw-byte storage, so the previous
memory caveat for 100 MB `int[]` inputs no longer applies.

## Default policy

- `decode_stream` / `Reader::new` / `decode_typed` use `Allowlist::strict()`:
  only `java.lang.*` and `java.util.*` are accepted.
- Additional classes must be granted explicitly:

```rust
let allowlist = jser::Allowlist::strict()
    .allow("com.example.*")?
    .allow("Fixtures$*")?;
let graph = jser::decode_stream_with_allowlist(bytes, &allowlist)?;
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
let limits = jser::Limits { max_depth: 256, max_elements: 1_000_000, max_stream_size: 32 << 20 };
let reader = jser::Reader::with_allowlist(bytes, limits, allowlist)?;
```

Random/truncated input is expected to return `Err`; it must not panic. Fuzz
targets live in `fuzz/`, and `cargo test -p jser --test fuzz_smoke` is the
offline smoke test.

## Dependency audit

`cargo deny check` (configuration in `deny.toml`) is part of CI: advisories
(yanked denied), license allowlist (MIT/Apache-2.0/BSD/ISC/Unicode/Unlicense/
Zlib), wildcard ban and registry pinning. Run it locally before releases.

## Reporting

This is a pre-release project without a private security channel yet. Do not
deploy it as a trust boundary; report issues to the repository maintainers.
