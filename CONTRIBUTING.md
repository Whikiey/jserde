# Contributing to jserde

`jserde` is a pure-Rust Java Object Serialization implementation. The
repository rules and fact-source order are in [AGENT.md](AGENT.md); this file
is the short contributor checklist.

## Setup

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

JDK 17 is required only for fixture regeneration and Java interop:

```bash
cd docs/testdata
./gen/gen.sh --check
./gen/interop.sh
```

## Development rules

- The committed fixture bytes under `docs/testdata/fixtures/` are the source of
  truth. Do not hand-edit `.ser` files or manifest sizes/hashes.
- New fixtures are generated through `docs/testdata/gen/gen.sh`.
- Follow TDD: add the failing test first, then implement.
- Keep the public API documented; run `RUSTDOCFLAGS="-D warnings" cargo doc`.
- Core crates forbid `unsafe` and deny `unwrap`/`expect`.
- Do not change protocol semantics for OUT items; add explicit errors instead.
- Commit each logical change and update `docs/devlog.md`.

## Pull request checklist

- [ ] tests added/updated;
- [ ] `cargo test --workspace` passes;
- [ ] clippy, fmt, doc checks pass;
- [ ] fixture/interop checks pass when fixtures or codec behavior changed;
- [ ] docs/CHANGELOG updated for user-visible changes;
- [ ] no secrets, local session logs, fuzz artifacts, or generated logs committed.

## Security

Do not report a suspected gadget-chain or denial-of-service issue in a public
issue before coordinating with the maintainers. See [SECURITY.md](SECURITY.md).

By contributing, you agree that your contribution is licensed under
MIT OR Apache-2.0.
