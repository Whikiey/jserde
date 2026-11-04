# Benchmark（P7-2 / P8-2）

- 日期：2026-11-04
- 机器/工具链：P7 基线为 Rust 1.98.1 / Linux x86_64；P8-2 数组数据在本机 Windows 上测得（Rust 1.99.1），release。

## 合成块流（P7-2）

```bash
cargo run --release -p jser --example bench -- 100
```

Linux 基线输出：

```text
string-70k      70013 B     2235.80 MB/s
point              51 B      116.59 MB/s
hashmap           199 B      139.79 MB/s
block-100MB   104857609 B     4787.37 MB/s  items=1
peak RSS: 207568 kB
```

- 100 MB 级输入：解码吞吐约 **4.8 GB/s**，峰值 RSS 约 **203 MiB**（约输入 100 MiB 的 2.0×）。
- `block-100MB` 是合成的 `TC_BLOCKDATALONG` 顶层块，用于测大输入；数组/对象的内存开销见下。

## 真实 primitive 数组（P8-2 / AC-06）

```bash
cargo run --release -p jser --example bench -- array 25000000 300
```

本机（Windows）输出：

```text
int-array-25000000  100000027 B     3598.53 MB/s  repeats=300 items=1 len=25000000
```

- 输入 **100,000,027 B**；`int[]` 以 4 B/元素（wire 大端字节）存储，不再展开为 32 B/`JValue`。
- 300 次重复解码，`Get-Process` 以 5 ms 间隔采样 488 次，峰值工作集 **203,378,688 B ≈ 194 MiB**，约 **2.03×** 输入，满足 `< 3×`。
- Linux 上 bench 末尾会打印 `/proc/self/status` 的 `VmHWM`，以该值为最终复核；采样方法不同会有小幅差异。
- 单次吞吐约 3.5–5.9 GB/s（随重复次数与页缓存波动）；小对象吞吐受固定开销影响，不代表大对象吞吐。
- 该 bench 不依赖 criterion，可离线运行。
