# jser

纯 Rust 实现的 Java 对象序列化协议，与 JDK 的 `ObjectOutputStream` / `ObjectInputStream` 双向互通，**运行时零 JVM 依赖**。

当前版本：`0.1.0`（P0–P9 开发线；尚未发布 crates.io）。

## 当前进度

- P0–P9 主要功能已实现：42 个 fixture / 78640 字节、164 项测试通过、42 例 Java 读回 + SUID 对拍；CI 覆盖 Linux/macOS/Windows、MSRV 1.77、i686 与 `cargo-deny`，nightly 含 bench 回归门禁。
- 已覆盖：动态编解码、集合/扩展容器、环/共享、typed、derive（嵌套/继承/enum）、SUID、内置类语义、allowlist、`TC_CLASS`/动态代理保真回放、serde 桥（递归嵌套 POJO/Map/Set/枚举/多根/`TC_RESET`）。
- P8-2：primitive 数组改为紧凑 raw 字节存储，25M `int[]` 本地约 2.03×（AC-06）；三层/混合嵌套 fixture 已覆盖。
- P8-1：30 分钟 fuzz 暂缓（AC-04，workflow 已就绪，`schedule` 停用，可从 Actions 手动触发）。
- 剩余 P9 扩展：P9-3/4/5/6/12/17/18/19/20（见 [implementation-plan.md §21](docs/implementation-plan.md#21-扩展-backlog可行性登记)）。

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
| schema 驱动 serde 桥（递归嵌套/Map/Set/枚举/多根） | ✅（限制见下） |
| 42 个 JDK 生成 fixture 逐字节回放 | ✅ |
| Rust 写 → Java 读回 | ✅（42 个 fixture + 构造/typed/derive/serde） |

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
cargo test --workspace          # 164 passed
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

cd docs/testdata
./gen/gen.sh --check       # 校验 JDK 生成的 42 个 fixture / 78640 字节
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

- `Externalizable` 通用语义、动态代理调用语义、`TC_CLASS` 类加载语义、RMI/JNDI 不在范围；P9-1/2 仅做 `TC_CLASS`/`TC_PROXYCLASSDESC` 的字节与句柄保真回放。
- 动态 `JValue` 层支持任意深度嵌套（`object/41` 验证 3 层）；derive 支持嵌套自定义字段、`Option<Custom>`、`Vec<Custom>`、继承（`#[java(flatten)]`）与 unit-only enum；serde 通过递归 `SchemaField` 支持多层 POJO/Map/Set/枚举/多根。
- serde 仍是树模型：共享引用按值展开，环不在 serde 内（真实身份/环用动态 `Graph`）；不支持对象数组字段。
- `java.util.Optional` 不是 `Serializable`，不纳入。
- primitive 数组使用紧凑 raw 存储（4 B/`int`）；25M `int[]` 本地工作集约 2.03× 输入，Linux 可用 `bench -- array` 的 `VmHWM` 复核。
- **剩余 P9**：P9-3 `TC_EXCEPTION` 语义化、P9-4 流式 IO、P9-5 零拷贝、P9-6 HashMap 精确容量、P9-12 serde 指针级共享/环、P9-17 泛型 derive、P9-18 组合字段、P9-19 typed/derive 类名/SUID 校验、P9-20 `IntoJValue` 扩展。
- 默认 allowlist 不能替代完整反序列化沙箱。
