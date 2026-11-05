# API stability and compatibility

`jserde` is pre-1.0 (`0.1.x`). This document explains what the version number
means and what users can rely on.

## Version policy

- `0.1.z`: patch releases should be source-compatible bug/security fixes.
- `0.x.0`: may contain breaking API changes; the changelog calls them out.
- `1.0`: freezes the public API and starts normal semver compatibility.
- MSRV is **Rust 1.77**. Raising it is treated as a minor-version change and is
  announced in the changelog.

## Public surface

The public API is what is exported from:

- `jserde` (including `jserde::wire`, `jserde::builtin`, `jserde::suid`)
- `jserde_derive`
- `jserde_serde`

Items marked `pub` in those crates are public. Modules `pub(crate)` and
`#[doc(hidden)]` items are implementation details.

The dynamic `Graph`/`JValue` API is the compatibility baseline; typed and serde
convenience layers may evolve faster, but breaking changes are still listed in
the changelog.

## Wire compatibility

- Fixtures are generated with **Java 17** and the committed bytes are the
  source of truth.
- The Java serialization wire format itself is stable since Java 1.2; classes
  outside the supported matrix remain opaque or return explicit errors.
- `serialVersionUID` values are data, not inferred policy. Prefer an explicit
  `#[java(suid = ...)]`; computed values are documented as version-sensitive.

## Scope boundaries

The following are intentionally out of scope (see `requirements.md` §5):

- general `Externalizable` behavior;
- dynamic-proxy invocation and `TC_CLASS` class loading;
- RMI/JNDI;
- full gadget sandboxing;
- input zero-copy (`Graph<'a>` lifetime redesign) and full serde cycles.

Wire-level `TC_CLASS` / proxy / `TC_EXCEPTION` replay is supported where
documented, but does not imply Java execution semantics.

## Deprecation

Before 1.0, deprecated items may be removed in the next minor release. After
1.0, deprecations remain for at least one minor release and carry a migration
note in the changelog.

## Feature flags

The core has no feature flags. `jserde-serde` is a separate crate so the core
does not depend on Serde. New optional features must be additive and documented
in `Cargo.toml`, README, and `CHANGELOG.md`.
