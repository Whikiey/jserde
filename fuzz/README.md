# Fuzz targets

Optional `cargo-fuzz` harnesses for the three P7 targets:

```bash
cargo install cargo-fuzz
cargo +nightly fuzz run decode
cargo +nightly fuzz run roundtrip
cargo +nightly fuzz run mutf8
```

Seed the corpus from the committed fixtures:

```bash
mkdir -p fuzz/corpus/decode
cp docs/testdata/fixtures/codec/*.ser docs/testdata/fixtures/object/*.ser fuzz/corpus/decode/
```

The always-on CI smoke test is `cargo test -p jser --test fuzz_smoke` (no
nightly/libFuzzer dependency). A 30-minute fuzz run is AC-04 and is run
manually or in a nightly job.
