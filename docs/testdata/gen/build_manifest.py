#!/usr/bin/env python3
"""生成 testdata/manifest.json。

manifest 是"用例 → 文件 → 期望值"的机读契约：Rust 侧的解码测试直接读它，
`requirements.md` 的 AC-01 就是"全部用例解码结果与 manifest 期望一致"。

用法：
    python3 gen/build_manifest.py [--check]

- 默认：扫描 fixtures/ 下所有 .ser，写入 manifest.json（含 size / sha256）。
- --check：只校验，不写文件；用于 CI（文件缺失、哈希漂移、有文件没登记都算失败）。
"""

from __future__ import annotations

import hashlib
import json
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "manifest.json"

# 期望值是**语义**描述，不含 wire 细节；wire 细节由 fixtures 里字节本身承担。
CASES = [
    # ---------------- codec 层：无类描述符的流原语 ----------------
    dict(
        id="codec/01", file="fixtures/codec/01-string-ascii.ser",
        case="顶层 String \"hello\"",
        features=["STREAM_HEADER", "TC_STRING"],
        expect={"kind": "string", "value": "hello"},
    ),
    dict(
        id="codec/02", file="fixtures/codec/02-string-empty.ser",
        case="空字符串",
        features=["TC_STRING", "零长度"],
        expect={"kind": "string", "value": ""},
    ),
    dict(
        id="codec/03", file="fixtures/codec/03-string-nul.ser",
        case="含 U+0000 的字符串 \"a\\0b\"",
        features=["修改版 UTF-8", "NUL -> C0 80"],
        expect={"kind": "string", "value": "a\u0000b"},
    ),
    dict(
        id="codec/04", file="fixtures/codec/04-string-cjk.ser",
        case="中日韩字符 \"中文\"",
        features=["修改版 UTF-8", "3 字节序列"],
        expect={"kind": "string", "value": "中文"},
    ),
    dict(
        id="codec/05", file="fixtures/codec/05-string-surrogate-pair.ser",
        case="补充平面字符 \"😀\"（U+1F600）",
        features=["修改版 UTF-8", "代理对 -> 两个 3 字节序列（6 字节）"],
        expect={"kind": "string", "value": "😀",
                "note": "标准 UTF-8 是 4 字节 F0 9F 98 80；Java 写 6 字节", },
    ),
    dict(
        id="codec/06", file="fixtures/codec/06-string-long.ser",
        case="70000 字符的字符串（UTF 字节数 > 65535）",
        features=["TC_LONGSTRING", "8 字节长度"],
        expect={"kind": "string", "value": {"repeat": "x", "count": 70000}},
    ),
    dict(
        id="codec/07", file="fixtures/codec/07-null.ser",
        case="顶层 null",
        features=["TC_NULL"],
        expect={"kind": "null"},
    ),
    dict(
        id="codec/08", file="fixtures/codec/08-string-shared-ref.ser",
        case="同一个 String 实例写两次",
        features=["句柄表", "TC_REFERENCE"],
        expect={"kind": "sequence", "items": [
            {"kind": "string", "value": "ab"},
            {"kind": "reference", "handle": "0x7E0000"},
        ]},
    ),
    dict(
        id="codec/09", file="fixtures/codec/09-primitives-block.ser",
        case="顶层 writeInt(1) + writeBoolean(true) + writeDouble(1.5)",
        features=["TC_BLOCKDATA", "块缓冲合并"],
        expect={"kind": "primitive_stream", "writes": [
            {"int": 1}, {"boolean": True}, {"double": 1.5},
        ], "note": "三个写操作合并为一个 13 字节的 TC_BLOCKDATA 块"},
    ),
    dict(
        id="codec/10", file="fixtures/codec/10-reset.ser",
        case="写 \"a\" → reset() → 写 \"b\"",
        features=["TC_RESET", "句柄表清空"],
        expect={"kind": "reset_stream", "first": "a", "second": "b"},
    ),
    dict(
        id="codec/11", file="fixtures/codec/11-blockdata-long.ser",
        case="顶层一次写 2000 字节原始数据",
        features=["TC_BLOCKDATALONG", "1024 字节块边界"],
        expect={"kind": "blockdata", "total_bytes": 2000,
                "blocks": [{"type": "long", "len": 1024}, {"type": "long", "len": 976}]},
    ),
    dict(
        id="codec/12", file="fixtures/codec/12-utf-widths.ser",
        case="混合 1/2/3 字节宽度的字符串 \"a\\u00e9\\u4e2d\"",
        features=["修改版 UTF-8", "宽度混合"],
        expect={"kind": "string", "value": "a\u00e9\u4e2d"},
    ),

    # ---------------- object 层：类描述符 / SUID / 集合自定义布局 ----------------
    dict(
        id="object/20", file="fixtures/object/20-object-point.ser",
        case="简单 POJO：Point{int x, int y}",
        features=["TC_OBJECT", "TC_CLASSDESC", "SC_SERIALIZABLE", "裸字节 classdata"],
        expect={"kind": "object", "class": "Fixtures$Point", "fields": {"x": 1, "y": 2}},
    ),
    dict(
        id="object/21", file="fixtures/object/21-object-mixed-fields.ser",
        case="10 个字段混排：9 个基本类型 + int[] + String",
        features=["字段排序规则", "基本类型裸字节", "TC_ARRAY 出现在 classdata 中"],
        expect={"kind": "object", "class": "Fixtures$Mixed", "fields": {
            "i": 7, "l": 8, "s": "mix", "a": [1, 2], "b": True,
            "d": 1.5, "f": 0.5, "c": "中", "sh": -3, "by": 9,
        }, "field_order": ["b", "by", "c", "d", "f", "i", "l", "sh", "a", "s"],
            "note": "先基本类型按名字升序，再对象/数组按名字升序；'a' 排最后但字母序最前"},
    ),
    dict(
        id="object/22", file="fixtures/object/22-object-inheritance.ser",
        case="继承：Derived extends Base，父类字段在父 classDesc 中",
        features=["superClassDesc 链", "父类字段先于子类字段"],
        expect={"kind": "object", "class": "Fixtures$Derived",
                "superclass": "Fixtures$Base", "fields": {"a": 1, "b": "b"}},
    ),
    dict(
        id="object/23", file="fixtures/object/23-object-custom-write-method.ser",
        case="自定义 writeObject：defaultWriteObject + writeInt + writeUTF",
        features=["SC_WRITE_METHOD", "TC_ENDBLOCKDATA", "自定义部分走块缓冲"],
        expect={"kind": "object", "class": "Fixtures$CustomWritable",
                "fields": {"value": 42},
                "custom_data": {"int": 1234, "utf": "custom"}},
    ),
    dict(
        id="object/24", file="fixtures/object/24-object-enum.ser",
        case="枚举常量 Color.GREEN",
        features=["TC_ENUM", "SC_ENUM", "SUID=0", "超类 java.lang.Enum"],
        expect={"kind": "enum", "class": "Fixtures$Color", "constant": "GREEN"},
    ),
    dict(
        id="object/25", file="fixtures/object/25-array-int.ser",
        case="int[]{1,2,3}",
        features=["TC_ARRAY", "数组类描述符", "数组类 SUID 非零"],
        expect={"kind": "array", "type": "int", "values": [1, 2, 3],
                "array_suid": "0x4DBA602676EAB2A5"},
    ),
    dict(
        id="object/26", file="fixtures/object/26-array-string-null.ser",
        case="String[]{\"a\",\"b\",null}",
        features=["TC_ARRAY", "对象数组", "元素为 TC_NULL"],
        expect={"kind": "array", "type": "java.lang.String", "values": ["a", "b", None]},
    ),
    dict(
        id="object/27", file="fixtures/object/27-array-2d.ser",
        case="int[][]{{1,2},{3}}",
        features=["TC_ARRAY", "数组的数组"],
        expect={"kind": "array", "type": "int[][]", "values": [[1, 2], [3]]},
    ),
    dict(
        id="object/28", file="fixtures/object/28-collection-arraylist.ser",
        case="ArrayList{\"alpha\",\"beta\"}",
        features=["java.util 自定义 writeObject", "size + 元素"],
        expect={"kind": "list", "class": "java.util.ArrayList",
                "values": ["alpha", "beta"]},
    ),
    dict(
        id="object/29", file="fixtures/object/29-collection-hashmap.ser",
        case="HashMap{\"one\":1,\"two\":2,\"three\":3}",
        features=["MAP 头部", "loadFactor+threshold 字段", "capacity+size 块", "同类元素回指类描述符"],
        expect={"kind": "map", "class": "java.util.HashMap",
                "load_factor": 0.75, "capacity": 16,
                "entries": {"one": 1, "two": 2, "three": 3},
                "note": "readObject 读但忽略 capacity；写一侧写合理值即可"},
    ),
    dict(
        id="object/30", file="fixtures/object/30-collection-linkedhashmap.ser",
        case="LinkedHashMap{\"a\":1,\"b\":2}（保持插入序）",
        features=["MAP 头部", "插入序"],
        expect={"kind": "map", "class": "java.util.LinkedHashMap",
                "ordered_entries": [["a", 1], ["b", 2]]},
    ),
    dict(
        id="object/31", file="fixtures/object/31-collection-hashset.ser",
        case="HashSet{\"x\",\"y\"}",
        features=["SET 头部", "capacity+loadFactor+size"],
        expect={"kind": "set", "class": "java.util.HashSet", "values": ["x", "y"]},
    ),
    dict(
        id="object/32", file="fixtures/object/32-collection-linkedlist.ser",
        case="LinkedList{1,2,3}",
        features=["有序列自定义 writeObject"],
        expect={"kind": "list", "class": "java.util.LinkedList", "values": [1, 2, 3]},
    ),
    dict(
        id="object/33", file="fixtures/object/33-ref-shared-top.ser",
        case="同一个 Point 实例写两次（顶层）",
        features=["句柄表", "对象级 TC_REFERENCE"],
        expect={"kind": "sequence", "items": [
            {"kind": "object", "class": "Fixtures$Point", "fields": {"x": 5, "y": 6}},
            {"kind": "reference", "note": "指向同一实例，不是新对象"},
        ]},
    ),
    dict(
        id="object/34", file="fixtures/object/34-ref-cycle.ser",
        case="循环引用：a.next=b, b.next=a",
        features=["循环引用", "回指未完成填充的对象"],
        expect={"kind": "cycle", "class": "Fixtures$Node", "graph": "a -> b -> a",
                "note": "b.next 先返回 a 的句柄，a 的字段晚于该回指填充"},
    ),
    dict(
        id="object/35", file="fixtures/object/35-reset-mid-stream.ser",
        case="写 Point(1,2) → reset() → 写 Point(3,4)",
        features=["TC_RESET", "类描述符与句柄表重置后重写"],
        expect={"kind": "reset_stream", "items": [
            {"kind": "object", "class": "Fixtures$Point", "fields": {"x": 1, "y": 2}},
            {"kind": "object", "class": "Fixtures$Point", "fields": {"x": 3, "y": 4}},
        ], "note": "reset 后 Point 的 classDesc 会重新出现一次"},
    ),
    dict(
        id="object/36", file="fixtures/object/36-misc-java-types.ser",
        case="Object[]{Date(0), BigInteger(30 位), BigDecimal(\"1.50\"), LocalDate(2024-02-29)}",
        features=["内置类", "writeReplace（java.time 走 java.time.Ser 代理）"],
        expect={"kind": "array", "type": "java.lang.Object", "values": [
            {"type": "java.util.Date", "epoch_millis": 0},
            {"type": "java.math.BigInteger", "decimal": "123456789012345678901234567890"},
            {"type": "java.math.BigDecimal", "decimal": "1.50", "scale": 2},
            {"type": "java.time.Ser", "semantic": "2024-02-29"},
        ]},
    ),
    dict(
        id="object/37", file="fixtures/object/37-object-with-transient.ser",
        case="含 transient 字段的 POJO",
        features=["transient 字段不写入流"],
        expect={"kind": "object", "class": "Fixtures$WithTransient",
                "fields": {"a": 1, "s": "s"}, "absent_fields": ["t"]},
    ),
    dict(
        id="object/38", file="fixtures/object/38-collection-nested-map.ser",
        case="嵌套 Map：{\"outer\": {\"k\": 7}}",
        features=["嵌套集合", "多级 MAP 头部"],
        expect={"kind": "map", "class": "java.util.HashMap",
                "entries": {"outer": {"k": 7}}},
    ),
    dict(
        id="object/39", file="fixtures/object/39-suid-computed.ser",
        case="未声明 serialVersionUID 的类（由 JDK 计算）",
        features=["computeDefaultSUID", "SUID 对拍"],
        expect={"kind": "object", "class": "Fixtures$NoSuid",
                "suid": -3559046683250708543, "fields": {"x": 1, "y": "y"},
                "note": "Rust 侧 computeDefaultSUID 必须算出同一值"},
    ),
    dict(
        id="object/40", file="fixtures/object/40-java-time.ser",
        case="Object[]{Duration, Instant, LocalDate, LocalDateTime, ZonedDateTime}",
        features=["java.time.Ser", "Duration/Instant/LocalDate/LocalDateTime/ZonedDateTime"],
        expect={"kind": "array", "type": "java.lang.Object", "values": [
            {"type": "java.time.Duration", "seconds": 123, "nanos": 456789000},
            {"type": "java.time.Instant", "epoch_second": 1700000000, "nano": 123456789},
            {"type": "java.time.LocalDate", "semantic": "2024-02-29"},
            {"type": "java.time.LocalDateTime", "semantic": "2024-02-29T12:34:56.123456789"},
            {"type": "java.time.ZonedDateTime", "semantic": "2024-02-29T12:34:56.123456789+08:00[Asia/Shanghai]"},
        ]},
    ),
    dict(
        id="object/41", file="fixtures/object/41-nested-objects.ser",
        case="三层对象嵌套：Outer -> Middle -> Leaf",
        features=["多层对象嵌套", "classdata 中的对象字段", "TC_OBJECT 递归"],
        expect={"kind": "object", "class": "Fixtures$Outer", "fields": {
            "middle": {"class": "Fixtures$Middle", "fields": {
                "name": "mid",
                "leaf": {"class": "Fixtures$Leaf", "fields": {"value": 42}},
            }},
        }},
    ),
    dict(
        id="object/42", file="fixtures/object/42-java-time-more.ser",
        case="Object[]{Year, YearMonth, MonthDay, Period}",
        features=["java.time.Ser", "Year/YearMonth/MonthDay/Period"],
        expect={"kind": "array", "type": "java.lang.Object", "values": [
            {"type": "java.time.Year", "year": 2024},
            {"type": "java.time.YearMonth", "year": 2024, "month": 2},
            {"type": "java.time.MonthDay", "month": 2, "day": 29},
            {"type": "java.time.Period", "years": 1, "months": 2, "days": 3},
        ]},
    ),
    dict(
        id="object/43", file="fixtures/object/43-java-time-zone.ser",
        case="Object[]{ZoneId, ZoneOffset, OffsetTime, OffsetDateTime}",
        features=["java.time.Ser", "ZoneId/ZoneOffset/OffsetTime/OffsetDateTime"],
        expect={"kind": "array", "type": "java.lang.Object", "values": [
            {"type": "java.time.ZoneId", "id": "Asia/Shanghai"},
            {"type": "java.time.ZoneOffset", "total_seconds": 28800},
            {"type": "java.time.OffsetTime", "semantic": "12:34:56.123456789+08:00"},
            {"type": "java.time.OffsetDateTime", "semantic": "2024-02-29T12:34:56.123456789+08:00"},
        ]},
    ),
    dict(
        id="object/44", file="fixtures/object/44-hybrid-nested.ser",
        case="Hybrid：Map/数组/自定义类 + 共享引用 + 自环",
        features=["混合嵌套", "HashMap 值含 Point/int[]/String/ArrayList", "Object[]",
                  "共享引用", "自环"],
        expect={"kind": "object", "class": "Fixtures$Hybrid", "fields": {
            "array": {"kind": "array", "type": "java.lang.Object", "length": 3},
            "map": {"kind": "map", "type": "java.util.HashMap",
                    "keys": ["point", "ints", "text", "list"]},
            "self": {"kind": "reference", "class": "Fixtures$Hybrid"},
            "shared": {"class": "Fixtures$Point", "fields": {"x": 7, "y": 8}},
        }},
    ),
    dict(
        id="object/45", file="fixtures/object/45-value-classes.ser",
        case="Object[]{UUID, Currency, Locale, BitSet, StringBuilder, StringBuffer, Pattern, sql.Date/Time/Timestamp}",
        features=["UUID/Currency/Locale", "BitSet long[]", "StringBuilder/StringBuffer",
                  "Pattern", "java.sql 日期时间"],
        expect={"kind": "array", "type": "java.lang.Object", "values": [
            {"type": "java.util.UUID", "most_sig_bits": 81985529216486895,
             "least_sig_bits": -81985529216486896},
            {"type": "java.util.Currency", "currency_code": "USD"},
            {"type": "java.util.Locale", "language": "en", "country": "US"},
            {"type": "java.util.BitSet", "words": [10, 4294967295]},
            {"type": "java.lang.StringBuilder", "value": "builder"},
            {"type": "java.lang.StringBuffer", "value": "buffer"},
            {"type": "java.util.regex.Pattern", "pattern": "a+b", "flags": 0},
            {"type": "java.sql.Date", "epoch_millis": 0},
            {"type": "java.sql.Time", "epoch_millis": 0},
            {"type": "java.sql.Timestamp", "epoch_millis": 0, "nanos": 123456789},
        ]},
    ),
    dict(
        id="object/46", file="fixtures/object/46-containers.ser",
        case="Object[]{LinkedHashSet, TreeMap, TreeSet, ArrayDeque, PriorityQueue, Vector, Stack, Hashtable, Properties, UnmodifiableList, Arrays$ArrayList}",
        features=["更多容器布局", "TreeMap/TreeSet 自定义数据", "Hashtable/Properties",
                  "Vector/Stack 字段", "Collections 包装", "Arrays$ArrayList"],
        expect={"kind": "array", "type": "java.lang.Object", "values": [
            {"type": "java.util.LinkedHashSet", "size": 2},
            {"type": "java.util.TreeMap", "size": 2},
            {"type": "java.util.TreeSet", "size": 2},
            {"type": "java.util.ArrayDeque", "size": 2},
            {"type": "java.util.PriorityQueue", "size": 3},
            {"type": "java.util.Vector", "size": 2},
            {"type": "java.util.Stack", "size": 2},
            {"type": "java.util.Hashtable", "size": 2},
            {"type": "java.util.Properties", "size": 1},
            {"type": "java.util.Collections$UnmodifiableList", "size": 2},
            {"type": "java.util.Arrays$ArrayList", "size": 2},
        ]},
    ),
    dict(
        id="object/47", file="fixtures/object/47-class-objects.ser",
        case="Object[]{String.class, String.class, Color.class, Mixed.class}",
        features=["TC_CLASS", "类对象共享（第二个 String.class 为 TC_REFERENCE）", "自定义类 Class"],
        expect={"kind": "array", "type": "java.lang.Object", "values": [
            {"type": "java.lang.Class", "class": "java.lang.String"},
            {"type": "java.lang.Class", "class": "java.lang.String"},
            {"type": "java.lang.Class", "class": "Fixtures$Color"},
            {"type": "java.lang.Class", "class": "Fixtures$Mixed"},
        ]},
    ),
    dict(
        id="object/48", file="fixtures/object/48-dynamic-proxy.ser",
        case="动态代理：Greeter 接口 + GreeterHandler",
        features=["TC_PROXYCLASSDESC", "代理接口列表", "InvocationHandler 字段"],
        expect={"kind": "proxy", "interfaces": ["Fixtures$Greeter"],
                "handler": "Fixtures$GreeterHandler"},
    ),
    dict(
        id="object/49", file="fixtures/object/49-enum-containers.ser",
        case="Object[]{EnumSet.of(RED), EnumMap(RED -> red)}",
        features=["EnumSet$SerializationProxy", "EnumMap keyType + custom entries"],
        expect={"kind": "array", "type": "java.lang.Object", "values": [
            {"type": "java.util.EnumSet", "values": ["RED"]},
            {"type": "java.util.EnumMap", "entries": {"RED": "red"}},
        ]},
    ),    dict(
        id="object/50", file="fixtures/object/50-tc-exception.ser",
        case="TC_EXCEPTION -> java.io.IOException(\"exception-message\")",
        features=["TC_EXCEPTION", "fatal exception object", "java.io.IOException + Throwable fields",
                  "empty stack trace（合成 fatal 流）"],
        expect={"kind": "exception", "class": "java.io.IOException",
                "message": "exception-message"},
    ),
]


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def build() -> dict:
    cases = []
    total = 0
    for c in CASES:
        path = ROOT / c["file"]
        if not path.is_file():
            raise SystemExit(f"缺少文件：{c['file']}")
        size = path.stat().st_size
        total += size
        cases.append({**c, "size": size, "sha256": sha256(path)})
    return {
        "schema": "jserde-fixtures/1",
        "jdk": os.environ.get("JDK_VERSION", "unknown"),
        "generator": "gen/Fixtures.java",
        "cross_check": "gen/build_codec_fixtures.py",
        "case_count": len(cases),
        "total_bytes": total,
        "cases": cases,
    }


def main() -> int:
    check = "--check" in sys.argv[1:]
    manifest = build()

    # 反向校验：fixtures/ 下不能有未登记的 .ser
    listed = {c["file"] for c in CASES}
    stray = sorted(
        p.relative_to(ROOT).as_posix()
        for p in (ROOT / "fixtures").rglob("*.ser")
        if p.relative_to(ROOT).as_posix() not in listed
    )
    if stray:
        print("未登记的文件：", *stray, sep="\n  ")
        return 1

    text = json.dumps(manifest, ensure_ascii=False, indent=2) + "\n"
    if check:
        if not MANIFEST.is_file():
            print("manifest.json 不存在")
            return 1
        if MANIFEST.read_text(encoding="utf-8") != text:
            print("manifest.json 与 fixtures 不一致（文件变化或未重新生成）")
            return 1
        print(f"manifest 校验通过：{manifest['case_count']} 个用例，"
              f"{manifest['total_bytes']} 字节")
        return 0

    MANIFEST.write_text(text, encoding="utf-8", newline="\n")
    print(f"已写入 {MANIFEST.relative_to(ROOT)}：{manifest['case_count']} 个用例，"
          f"{manifest['total_bytes']} 字节")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
