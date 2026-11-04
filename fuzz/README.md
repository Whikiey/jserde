# Fuzz targets

`cargo-fuzz` harnesses for the three P7/P8 targets. They are **not** part of the
workspace build: `fuzz/Cargo.toml` is its own workspace (`[workspace]`).

| Target | Invariant |
|---|---|
| `decode` | any byte sequence decodes to `Ok`/`Err`; never panics (AC-04) |
| `roundtrip` | decode → encode → decode never panics |
| `mutf8` | modified-UTF-8 decode/encode/decode is stable |

## Local run

```bash
cargo install cargo-fuzz            # once; requires network + nightly
cargo +nightly fuzz run decode -- -max_total_time=60 -rss_limit_mb=2048
```

`cargo-fuzz` needs nightly. A 60–300 second local run is a smoke check; the
authoritative 30-minute run is the nightly CI job below.

`fuzz/.gitignore` excludes `corpus/`, so seed it before a local run:

```bash
mkdir -p fuzz/corpus/decode fuzz/corpus/roundtrip fuzz/corpus/mutf8
find docs/testdata/fixtures -name '*.ser' -type f -exec cp {} fuzz/corpus/decode/ \;
find docs/testdata/fixtures -name '*.ser' -type f -exec cp {} fuzz/corpus/roundtrip/ \;
printf '\xc0\x80' > fuzz/corpus/mutf8/seed-overlong-nul
```

## Nightly CI (AC-04)

> **暂缓（2026-11-04）**：按用户决定，定时任务已停用，等功能开发完成后再启用；
> 目前只能从 Actions 手动触发。恢复时取消 `nightly.yml` 中 `schedule` 的注释即可。

[`.github/workflows/nightly.yml`](../.github/workflows/nightly.yml) runs on
manual dispatch (the daily 18:00 UTC / 02:00 Asia/Shanghai schedule is disabled
until feature work completes):

- `decode`: `-max_total_time=1800` (30 minutes) — this is the AC-04 run;
- `roundtrip` and `mutf8`: `-max_total_time=300` each;
- `-rss_limit_mb=2048` makes OOM a failure, `-timeout=30` catches hangs;
- corpora are seeded from `docs/testdata/fixtures/**/*.ser` plus a few
  hand-written MUTF-8 seeds, and cached between runs;
- on failure, `fuzz/artifacts/` and `fuzz/corpus/` are uploaded as artifacts.

Until the first green nightly run, AC-04 stays partial in
`docs/requirements.md`. The always-on CI check remains the offline smoke test
(`cargo test -p jser --test fuzz_smoke`), which needs no fuzzing toolchain.
