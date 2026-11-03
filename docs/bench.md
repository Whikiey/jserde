# Benchmark（P7-2）

- 日期：2026-11-04
- 机器/工具链：Rust 1.98.1，Linux x86_64，release
- 命令：

```bash
cargo run --release -p jser --example bench -- 100
```

输出（本机）：

```text
string-70k      70013 B     2235.80 MB/s
point              51 B      116.59 MB/s
hashmap           199 B      139.79 MB/s
block-100MB   104857609 B     4787.37 MB/s  items=1
peak RSS: 207568 kB
```

- 100 MB 级输入：解码吞吐约 **4.8 GB/s**，峰值 RSS 约 **203 MiB**（约输入 100 MiB 的 2.0×），满足“< 3× 输入”的目标。
- `block-100MB` 是合成的 `TC_BLOCKDATALONG` 顶层块，用于测大输入；它不覆盖 2500 万个元素的数组/对象内存开销，后者需在真实负载上复测。
- 小对象吞吐受固定开销影响，不代表大对象吞吐。
- 该 bench 不依赖 criterion，可离线运行；`/proc/self/status` 仅用于 Linux 峰值 RSS。
