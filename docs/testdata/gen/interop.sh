#!/usr/bin/env bash
# Rust -> Java interop check (design §5.1 / plan P3-8..P3-10).
#
# Requires JDK 17+ and cargo. Verifies that Rust-written streams can be read by
# ObjectInputStream. Does not modify fixtures or manifest.json.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(cd "$here/../../.." && pwd)
cd "$root/docs/testdata"

command -v java >/dev/null 2>&1 || {
    echo "需要 JDK 17+：java 不在 PATH 中" >&2
    exit 1
}
command -v javac >/dev/null 2>&1 || {
    echo "需要 JDK 17+：javac 不在 PATH 中" >&2
    exit 1
}

./gen/gen.sh --check >/dev/null
mkdir -p build/rust

cargo run --quiet --manifest-path "$root/Cargo.toml" -p jser --example emit
javac -cp build/classes -d build/classes gen/Verify.java

verify() {
    java -cp build/classes Verify "$@"
}

verify build/rust/point.ser 'Fixtures$Point' x=1 y=2
verify build/rust/point-mutated.ser 'Fixtures$Point' x=99 y=2
verify build/rust/point-constructed.ser 'Fixtures$Point' x=7 y=8
verify build/rust/mixed.ser 'Fixtures$Mixed' i=7 s=mix b=true
verify build/rust/map.ser 'java.util.HashMap' one=1 two=2 three=3
verify build/rust/map-mutated.ser 'java.util.HashMap' one=1 two=2 three=3 four=1
verify build/rust/map-constructed.ser 'java.util.HashMap' alpha=one beta=two
verify build/rust/node-cycle.ser 'Fixtures$Node' name=a
verify build/rust/enum.ser 'Fixtures$Color'

echo "interop OK"
