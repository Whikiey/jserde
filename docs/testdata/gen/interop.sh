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
cargo run --quiet --manifest-path "$root/Cargo.toml" -p jser-serde --example serde_emit
javac -cp build/classes -d build/classes gen/Verify.java

expected_suid=$(awk '$1 == "Fixtures$NoSuid" { print $2 }' build/suid.txt)
actual_suid=$(cargo run --quiet --manifest-path "$root/Cargo.toml" -p jser --bin suid -- 'build/classes/Fixtures$NoSuid.class')
if [ "$actual_suid" != "$expected_suid" ]; then
    echo "SUID mismatch: Rust=$actual_suid JDK=$expected_suid" >&2
    exit 1
fi
echo "SUID OK Fixtures\$NoSuid = $actual_suid"

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
verify build/rust/typed-map.ser 'java.util.HashMap' alpha=1 beta=2
verify build/rust/typed-list.ser 'java.util.ArrayList' 0=1 1=2 2=3
verify build/rust/point-derived.ser 'Fixtures$Point' x=9 y=10
verify build/rust/point-serde.ser 'Fixtures$Point' x=11 y=12
verify build/rust/node-cycle.ser 'Fixtures$Node' name=a
verify build/rust/enum.ser 'Fixtures$Color'

# AC-02 coverage: every object-like fixture re-encoded by Rust is read back by
# ObjectInputStream. Primitive/block streams use their own reader modes.
for f in build/rust/all/*.ser; do
    case "$(basename "$f")" in
        09-* | 11-*) continue ;;
    esac
    verify --read "$f"
done
verify --primitives build/rust/all/09-primitives-block.ser i=1 z=true d=1.5
verify --blocks build/rust/all/11-blockdata-long.ser 2000

echo "interop OK"
