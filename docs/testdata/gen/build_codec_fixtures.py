#!/usr/bin/env python3
"""codec 层基准数据的**独立**再推导实现（不参与交付，仅用于交叉校验）。

为什么要手写一遍：`fixtures/codec/` 的权威字节由 `gen/Fixtures.java` 用 JDK 生成，
但如果只依赖 JDK，就无法发现"我们对协议的理解"与"JDK 的实际输出"在何处分叉。
本脚本按规范文档的字面规则逐字节推导同一批用例，`gen.sh check` 会把它写到
`build/codec/` 并与已提交的 `fixtures/codec/` 逐字节对比：

    python3 gen/build_codec_fixtures.py <output-dir>

注意：
- 只处理"无类描述符"的流原语（字符串 / 空 / 块数据 / 句柄回指 / reset），
  不实现通用编码器；需要 SUID 的用例见 gen/Fixtures.java。
- 若两边不一致，**以 JDK 为准**，并回来修正本文件里的推导规则（`utf()`、
  `BlockBuffer` 的 flush 时机是两处最容易出错的地方）。
"""

from __future__ import annotations

import sys
from pathlib import Path

# ---- 协议常量 ---------------------------------------------------------------
STREAM_MAGIC = b"\xac\xed"
STREAM_VERSION = b"\x00\x05"
HEADER = STREAM_MAGIC + STREAM_VERSION

TC_NULL = 0x70
TC_REFERENCE = 0x71
TC_STRING = 0x74
TC_BLOCKDATA = 0x77
TC_RESET = 0x79
TC_BLOCKDATALONG = 0x7A
TC_LONGSTRING = 0x7C

BASE_WIRE_HANDLE = 0x7E0000


# ---- 修改版 UTF-8（JDK DataOutput.writeUTF 的编码，非标准 UTF-8） -------------
def mutf8(s: str) -> bytes:
    out = bytearray()
    for ch in s:
        cp = ord(ch)
        if 0 < cp < 0x80:                      # 1 字节；U+0000 不走这里
            out.append(cp)
        elif cp < 0x800:                       # 2 字节，U+0000 -> C0 80
            out += bytes((0xC0 | (cp >> 6), 0x80 | (cp & 0x3F)))
        elif cp < 0x10000:                     # 3 字节
            out += bytes((0xE0 | (cp >> 12), 0x80 | ((cp >> 6) & 0x3F), 0x80 | (cp & 0x3F)))
        else:                                  # 补充平面 -> 两个三字节代理
            cp -= 0x10000
            for u in (0xD800 + (cp >> 10), 0xDC00 + (cp & 0x3FF)):
                out += bytes((0xE0 | (u >> 12), 0x80 | ((u >> 6) & 0x3F), 0x80 | (u & 0x3F)))
    return bytes(out)


def tc_string(s: str) -> bytes:
    b = mutf8(s)
    if len(b) > 0xFFFF:                        # 超过 65535 字节改用 TC_LONGSTRING
        return tc_longstring(b)
    return bytes((TC_STRING,)) + len(b).to_bytes(2, "big") + b


def tc_longstring(b: bytes) -> bytes:
    return bytes((TC_LONGSTRING,)) + len(b).to_bytes(8, "big") + b


def tc_reference(handle: int) -> bytes:
    return bytes((TC_REFERENCE,)) + handle.to_bytes(4, "big")


def block(data: bytes) -> bytes:
    """BlockDataOutputStream.drain() 的等价物：<=255 用 TC_BLOCKDATA，否则 LONG。"""
    if len(data) <= 0xFF:
        return bytes((TC_BLOCKDATA, len(data))) + data
    return bytes((TC_BLOCKDATALONG,)) + len(data).to_bytes(4, "big") + data


def buffered(raw: bytes) -> bytes:
    """模拟 1024 字节块缓冲：写满一块立即 drain，余量在流结束时 drain。"""
    out = bytearray()
    pos = 0
    while pos < len(raw):
        n = min(len(raw) - pos, 1024)
        out += block(raw[pos : pos + n])
        pos += n
    return bytes(out)


# ---- 用例 -------------------------------------------------------------------
def case_primitives_block() -> bytes:
    raw = (1).to_bytes(4, "big") + b"\x01" + bytes.fromhex("3ff8000000000000")
    return HEADER + buffered(raw)


CASES = [
    ("01-string-ascii.ser", lambda: HEADER + tc_string("hello")),
    ("02-string-empty.ser", lambda: HEADER + tc_string("")),
    ("03-string-nul.ser", lambda: HEADER + tc_string("a\x00b")),
    ("04-string-cjk.ser", lambda: HEADER + tc_string("中文")),
    ("05-string-surrogate-pair.ser", lambda: HEADER + tc_string("😀")),
    ("06-string-long.ser", lambda: HEADER + tc_longstring(b"x" * 70000)),
    ("07-null.ser", lambda: HEADER + bytes((TC_NULL,))),
    # 同一个 String 实例写两次：第二次是 TC_REFERENCE 回指首个句柄
    ("08-string-shared-ref.ser",
     lambda: HEADER + tc_string("ab") + tc_reference(BASE_WIRE_HANDLE)),
    ("09-primitives-block.ser", case_primitives_block),
    # reset() 清空句柄表，TC_RESET 单独出现（不在块数据里）
    ("10-reset.ser",
     lambda: HEADER + tc_string("a") + bytes((TC_RESET,)) + tc_string("b")),
    # 一次写 2000 字节：先 1024 字节块（LONG），余 976 字节 > 255，同样是 LONG
    ("11-blockdata-long.ser",
     lambda: HEADER + buffered(bytes(i & 0xFF for i in range(2000)))),
    ("12-utf-widths.ser", lambda: HEADER + tc_string("a\u00e9\u4e2d")),
]


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__)
        return 2
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    for name, build in CASES:
        data = build()
        (out / name).write_bytes(data)
        print(f"{name:<32} {len(data):>7} bytes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
