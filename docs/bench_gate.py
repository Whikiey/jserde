#!/usr/bin/env python3
"""Benchmark regression gate (P9-30).

Runs the dependency-free `bench` example and compares decode throughput with
`docs/bench-baseline.json`:

  * required metrics must reach `threshold * observed_mbps` (default 35%,
    i.e. fail on a >2.8x slowdown);
  * the `int[]` workload must stay below `max_rss_ratio` x input size.

The thresholds are deliberately loose: CI machines differ, so this gate catches
severe regressions, not small noise. Peak RSS is only reported on Linux (the
nightly runner); elsewhere the ratio check is skipped with a warning.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LINE = re.compile(r"^(?P<name>[\w.-]+)\s+(?P<bytes>\d+) B\s+(?P<mbps>[\d.]+) MB/s")
RSS = re.compile(r"^peak RSS:\s*(?P<kb>\d+) kB")


def run_bench(arguments: list[str]) -> tuple[dict[str, dict[str, float]], int | None, str]:
    command = [
        "cargo",
        "run",
        "--release",
        "-p",
        "jserde",
        "--example",
        "bench",
        "--",
        *arguments,
    ]
    completed = subprocess.run(
        command,
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    sys.stdout.write(completed.stdout)
    if completed.stderr:
        sys.stderr.write(completed.stderr)
    if completed.returncode != 0:
        print(f"bench command failed ({completed.returncode}): {' '.join(command)}", file=sys.stderr)
        sys.exit(1)

    metrics: dict[str, dict[str, float]] = {}
    rss_kb = None
    for line in completed.stdout.splitlines():
        match = LINE.match(line)
        if match:
            metrics[match["name"]] = {
                "bytes": float(match["bytes"]),
                "mbps": float(match["mbps"]),
            }
        rss_match = RSS.match(line)
        if rss_match:
            rss_kb = int(rss_match["kb"])
    return metrics, rss_kb, completed.stdout


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", default=ROOT / "docs/bench-baseline.json", type=pathlib.Path)
    parser.add_argument("--threshold", type=float, help="override baseline threshold")
    arguments = parser.parse_args()

    baseline = json.loads(arguments.baseline.read_text(encoding="utf-8"))
    threshold = arguments.threshold if arguments.threshold is not None else baseline["threshold"]
    required = baseline["required"]
    observed = baseline["observed_mbps"]

    print("== synthetic block stream ==")
    block_metrics, _, _ = run_bench(["100"])
    print("== primitive int[] (25M elements) ==")
    array_metrics, rss_kb, _ = run_bench(["array", "25000000", "100"])

    metrics = {**block_metrics, **array_metrics}
    failures: list[str] = []
    checked = 0

    print("\nmetric                         actual      floor    result")
    for name in required:
        if name not in metrics:
            failures.append(f"missing metric `{name}` in bench output")
            continue
        floor = observed[name] * threshold
        actual = metrics[name]["mbps"]
        ok = actual >= floor
        checked += 1
        print(f"{name:28} {actual:9.2f}  {floor:9.2f}  {'PASS' if ok else 'FAIL'}")
        if not ok:
            failures.append(
                f"`{name}` {actual:.2f} MB/s < {floor:.2f} MB/s "
                f"({threshold:.0%} of baseline {observed[name]:.2f})"
            )

    if rss_kb is not None:
        name = "int-array-25000000"
        if name in metrics:
            ratio = (rss_kb * 1024) / metrics[name]["bytes"]
            limit = baseline["max_rss_ratio"]
            ok = ratio <= limit
            print(f"{'int[] peak RSS ratio':28} {ratio:9.3f}x {limit:8.1f}x  {'PASS' if ok else 'FAIL'}")
            if not ok:
                failures.append(f"int[] peak RSS {ratio:.2f}x exceeds {limit:.1f}x input")
        else:
            failures.append("int[] metric missing; cannot check RSS ratio")
    else:
        print("int[] peak RSS ratio: not available on this platform (skip)")

    if not checked:
        failures.append("no required metrics were parsed from bench output")

    if failures:
        print("\nbench gate FAILED:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1

    print("\nbench gate OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
