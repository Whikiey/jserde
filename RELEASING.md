# Releasing jserde

This file is the release runbook for the three-crate workspace:

- `jserde` — core reader/writer
- `jserde-derive` — `#[derive(JavaSerializable)]`
- `jserde-serde` — optional schema-driven serde bridge

The current version is **0.1.0**. It is a release candidate until the steps
below are completed and the tags/crates are published.

See [`docs/api-stability.md`](docs/api-stability.md) for the versioning and
deprecation policy referenced by this runbook.

## 1. Release decision

Before publishing, confirm:

- [ ] version is intentional (`0.1.0`, not a local dev version);
- [ ] crates.io names `jserde`, `jserde-derive`, `jserde-serde` are available;
- [ ] the repository URL/homepage is filled in the manifests (currently
      intentionally omitted because no public repository URL is set);
- [ ] the copyright holder in each `LICENSE-MIT` is correct (root and crates);
- [ ] a private security-reporting channel exists, or `SECURITY.md` clearly
      says reports should go through the public issue tracker.

## 2. Preconditions

Run from a clean checkout:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

cd docs/testdata
./gen/gen.sh --check
./gen/interop.sh
```

Also confirm the latest evidence for:

- AC-04: `nightly.yml` fuzz job / local `fuzz-decode.log` is green;
- AC-06: `docs/bench.md` numbers and `docs/bench-baseline.json` still match;
- CI matrix (Linux/macOS/Windows), MSRV 1.77, i686, and cargo-deny are green.

`cargo deny check` is recommended before the first publish:

```bash
cargo deny check
```

## 3. Packaging check

Inspect exactly what will be uploaded:

```bash
cargo package --list -p jserde --allow-dirty
cargo package --list -p jserde-derive --allow-dirty
cargo package --list -p jserde-serde --allow-dirty
```

Do not publish while the list contains:

- `target/`, `fuzz/corpus/`, or local logs;
- test-only fixture binaries that are not intended for the crate;
- a stale `Cargo.toml` dependency path.

Build the package tarballs and run the package tests:

```bash
cargo package -p jserde --allow-dirty
cargo package -p jserde-derive --allow-dirty
cargo package -p jserde-serde --allow-dirty
```

## 4. Version and changelog

1. Set `workspace.package.version` in the root `Cargo.toml`.
2. Update `CHANGELOG.md` from `[Unreleased]` to the release date.
3. Add release notes (template below).
4. Regenerate `Cargo.lock` with `cargo check --workspace`.
5. Commit the version/changelog change.

For a pre-1.0 crate, breaking changes bump the minor version (`0.1` → `0.2`);
patch releases must remain source-compatible.

## 5. Publish order

Publish `jserde-derive` **before** `jserde`: the core crate has a dev-dependency
on `jserde-derive` (`version = "0.1.0"`), and Cargo validates that published
dev-dependency when packaging the core crate.

```bash
cargo publish -p jserde-derive
cargo publish -p jserde
cargo publish -p jserde-serde
```

If a dry run is requested, start with the dependency-free crate:

```bash
cargo publish --dry-run -p jserde-derive --registry crates-io
```

The core dry run can only succeed after `jserde-derive 0.1.0` is visible on
crates.io; `jserde-serde` likewise waits for `jserde 0.1.0`.

## 6. Tag and GitHub release

```bash
git tag -a v0.1.0 -m "jserde 0.1.0"
git push origin main --tags
```

Create a GitHub release from the tag, using the release notes in
`docs/release-notes-0.1.0.md` if present. Attach the fuzz/bench evidence only
if it is not already in the repository.

## 7. Post-publish checks

- [ ] `cargo search jserde` resolves the expected version;
- [ ] docs.rs builds `jserde`, `jserde-derive`, `jserde-serde`;
- [ ] `cargo add jserde` in a clean temp project compiles;
- [ ] `cargo add jserde-derive` + derive example compiles;
- [ ] `cargo add jserde-serde` + schema example compiles;
- [ ] Chinese/English README installation instructions match the published names.

## 8. Rollback / yank

Never overwrite a published crate. If a release is broken:

1. publish a fixed patch/minor version;
2. `cargo yank --version <version> -p <crate>`;
3. document the yank and replacement in `CHANGELOG.md` and the GitHub release.

## 9. Release notes template

```markdown
## jserde X.Y.Z — YYYY-MM-DD

### Added
- ...

### Changed
- ...

### Fixed
- ...

### Compatibility
- Java 17 baseline fixtures; wire format compatible with Java 1.2+.
- MSRV: 1.77.

### Verification
- cargo test --workspace: N passed
- fixtures: N cases / N bytes
- Java interop: N checks
- fuzz: AC-04 evidence
```
