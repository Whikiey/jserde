#!/usr/bin/env bash
# 生成 / 校验基准数据。
#
#   ./gen.sh           重新生成 fixtures/ 与 manifest.json（需要 JDK 17+）
#   ./gen.sh --check   只校验（CI 用）：生成到 build/ 再与已提交内容逐字节比对
#
# 步骤：编译 Fixtures.java → JDK 生成 → 手写推导对拍 → 生成并校验 manifest。
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(cd "$here/.." && pwd)
cd "$root"

mode=build
case "${1:-}" in
    "" | build) mode=build ;;
    --check | check) mode=check ;;
    *)
        echo "用法: $(basename "$0") [--check]" >&2
        exit 2
        ;;
esac

command -v javac >/dev/null 2>&1 || {
    echo "需要 JDK 17+：javac 不在 PATH 中" >&2
    exit 1
}

jdk_version=$(java -version 2>&1 | head -1)
echo "JDK: $jdk_version"

mkdir -p build/classes
javac -d build/classes gen/Fixtures.java

run_fixtures() { # $1=codec 目录 $2=object 目录；stdout=SUID 表，stderr=进度
    java -cp build/classes Fixtures "$1" "$2" > build/suid.txt 2> build/gen.log
    tail -2 build/gen.log
}

cross_check() { # $1=手写推导输出目录，与 fixtures/codec 比对
    rm -rf "$1"
    python3 gen/build_codec_fixtures.py "$1" > /dev/null
    if ! diff -r "$1" fixtures/codec > /dev/null; then
        echo "✗ 手写推导与 JDK 输出不一致（以 JDK 为准，修正 gen/build_codec_fixtures.py）" >&2
        diff -r "$1" fixtures/codec >&2 || true
        exit 1
    fi
    echo "✓ codec 对拍通过：手写推导 == JDK 输出（12 个用例逐字节一致）"
}

if [ "$mode" = check ]; then
    rm -rf build/check
    mkdir -p build/check/codec build/check/object
    run_fixtures build/check/codec build/check/object
    for tier in codec object; do
        if ! diff -r "build/check/$tier" "fixtures/$tier" > /dev/null; then
            echo "✗ fixtures/$tier 与重新生成的结果不一致" >&2
            diff -r "build/check/$tier" "fixtures/$tier" >&2 || true
            exit 1
        fi
    done
    echo "✓ fixtures 与重新生成结果逐字节一致"
    cross_check build/check/codec-py
    JDK_VERSION="$jdk_version" python3 gen/build_manifest.py --check
    echo "全部一致 ✅"
else
    run_fixtures fixtures/codec fixtures/object
    cross_check build/codec
    JDK_VERSION="$jdk_version" python3 gen/build_manifest.py
    echo "SUID 表 -> build/suid.txt（供 Rust 侧 computeDefaultSUID 对拍）"
fi
