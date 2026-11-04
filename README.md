# jser

纯 Rust 实现的 Java 对象序列化协议，与 JDK 的 `ObjectOutputStream` / `ObjectInputStream` 双向互通，**运行时零 JVM 依赖**。

当前版本：`0.1.0`（P0–P7 开发线；尚未发布 crates.io）。

## 当前进度

- P0–P7 已实现：36 个 fixture / 75224 字节、147 项测试通过、36 例 Java 读回 + SUID 对拍；CI 覆盖 Linux/macOS/Windows、MSRV 1.77、i686 与 `cargo-deny`，nightly 含 bench 回归门禁。
- 已覆盖：动态编解码、集合、环/共享、typed、derive（含 enum）、SUID、内置类语义（含 Year/YearMonth/MonthDay/Period、ZoneId/ZoneOffset/OffsetTime/OffsetDateTime）、allowlist、serde 桥（直连 `JValue`，嵌套 POJO/Map/Set/枚举/多根）、三层嵌套对象。
- P8-2：primitive 数组改为紧凑 raw 字节存储，25M `int[]` 本地工作集约 2.03×（AC-06）。
- 未收口：30 分钟 fuzz 暂缓（AC-04，workflow 已就绪、等功能完成后再执行）；嵌套 derive/serde、`Year` 等内置类、跨平台 CI。
- 缺口与优先级见 [implementation-plan.md §20](docs/implementation-plan.md#20-当前进度与后续缺口2026-11-04)（近期）；全部纯 Rust 可实现的扩展候选见 [§21](docs/implementation-plan.md#21-扩展-backlog可行性登记)。

## 能力

| 能力 | 状态 |
|---|---|
| 流头、`TC_STRING`/`TC_LONGSTRING`、修改版 UTF-8、块数据、句柄、`TC_RESET` | ✅ |
| `classDesc`、对象、继承、自定义 `writeObject`、数组、enum | ✅ |
| `HashMap`/`LinkedHashMap`/`HashSet`/`ArrayList`/`LinkedList` | ✅ |
| 共享引用、循环引用、多元素流 | ✅ |
| 动态 POJO、未知类原样回写 | ✅ |
| `decode_typed`/`encode_typed`、`FromJValue`/`IntoJValue` | ✅ |
| `#[derive(JavaSerializable)]`、`computeDefaultSUID`/`suid` 工具 | ✅ |
| `Date`、`BigInteger`、`BigDecimal`、`java.time` 语义视图 | ✅ |
| 严格默认 allowlist | ✅ |
| schema 驱动 serde 桥（单根扁平 POJO） | ✅（限制见下） |
| 34 个 JDK 生成 fixture 逐字节回放 | ✅ |
| Rust 写 → Java 读回 | ✅（Point/Mixed/Map/环/enum/serde） |

## 快速开始

```rust
use jser::{decode_stream_with_allowlist, Allowlist};

fn main() -> jser::Result<()> {
    let bytes = std::fs::read("stream.ser").map_err(|error| jser::Error::FieldMismatch {
        message: error.to_string(),
    })?;
    let allowlist = Allowlist::strict().allow("com.example.*")?;
    let graph = decode_stream_with_allowlist(&bytes, &allowlist)?;

    for item in graph.items() {
        if let Some(map) = graph.as_map(item)? {
            for (key, value) in map.entries {
                println!("{:?} => {:?}", key, value);
            }
        }
    }
    Ok(())
}
```

写一个 Map：

```rust
use jser::{builtin::new_hash_map, encode_stream, Graph, JValue};

fn main() -> jser::Result<()> {
    let mut graph = Graph::new();
    let map = new_hash_map(&mut graph, &[("a", "one")])?;
    graph.add_item(JValue::Obj(map));
    let bytes = encode_stream(&graph)?;
    Ok(())
}
```

derive：

```rust
use jser_derive::JavaSerializable;

#[derive(JavaSerializable, Clone)]
#[java(class = "com.example.Point", suid = 1)]
struct Point {
    x: i32,
    y: i32,
}
```

## 安全默认值

`decode_*` 默认**严格 allowlist**：只放行 `java.lang.*`、`java.util.*`。其它类必须通过 `Allowlist` 显式放行；`fixture` 测试放行 `Fixtures$*`、`java.math.*`、`java.time.*`。

**allowlist 不是 gadget 沙箱**。放行的类仍可能执行危险的 `readObject`。详见 [SECURITY.md](SECURITY.md)。

## 构建与验证

```bash
cargo test --workspace          # 113 passed
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

cd docs/testdata
./gen/gen.sh --check       # 校验 JDK 生成的 36 个 fixture / 75224 字节
./gen/interop.sh           # Rust 写 → Java 读回 + SUID 对拍（需 JDK 17）

# 可选
cargo run --release -p jser --example bench -- 16
cargo test -p jser --test fuzz_smoke
```

## 文档

- [需求](docs/requirements.md)
- [设计](docs/design.md)
- [实现计划](docs/implementation-plan.md)
- [测试基准](docs/testdata/README.md)
- [Benchmark](docs/bench.md)
- [编码须知](AGENT.md)

## 已知限制

- `Externalizable` 通用语义、动态代理、`TC_CLASS` 完整语义、RMI/JNDI 不在范围。
- 动态 `JValue` 层支持任意深度嵌套（`object/41` 验证 3 层）；derive 支持嵌套自定义字段、`Option<Custom>`、`Vec<Custom>`（数组类 SUID 自动计算）、继承（`#[java(flatten)]`）与 unit-only enum（默认 UPPER_SNAKE 常量名）；serde 通过递归 `SchemaField::nested` 支持多层 POJO。
- serde 桥不支持对象数组；共享引用按值展开、环不在树模型内（真实身份/环用动态 `Graph`）；多根与 `TC_RESET` 经 `MultiRootWriter` 支持。
- `java.util.Optional` 不是 `Serializable`，不纳入。
- primitive 数组已改紧凑 raw 存储（4 B/`int`）；25M `int[]` 本地工作集采样约 2.03× 输入，Linux 上可用 `bench -- array` 的 `VmHWM` 复核；对象数组仍按 `JValue` 展开。
- 默认 allowlist 不能替代完整反序列化沙箱。
- 完整扩展 backlog（Map/Set serde、`TC_CLASS`/proxy 保真回放、嵌套 derive、更多内置类等）见 [implementation-plan.md §21](docs/implementation-plan.md#21-扩展-backlog可行性登记)。
