# jserde

[English](README.en.md) | 简体中文

纯 Rust 实现的 Java 对象序列化协议，与 JDK 的 `ObjectOutputStream` / `ObjectInputStream` 双向互通，**运行时零 JVM 依赖**。

当前版本：`0.1.0`（2026-11-06 已发布到 crates.io）。

## 安装

```bash
# 核心库
cargo add jserde

# 可选：derive
cargo add jserde-derive

# 可选：serde 桥
cargo add jserde-serde
```

GitHub：[Whikiey/jserde](https://github.com/Whikiey/jserde)

crates.io：[jserde](https://crates.io/crates/jserde)、
[jserde-derive](https://crates.io/crates/jserde-derive)、
[jserde-serde](https://crates.io/crates/jserde-serde)。文档见
[docs.rs/jserde](https://docs.rs/jserde)。

版本与发布说明见 [release notes](docs/release-notes-0.1.0.md) 和
[RELEASING.md](RELEASING.md)。

## 兼容性

| 项 | 值 |
|---|---|
| Java 基线 | Java 17 fixture；wire 格式兼容 Java 1.2+（受支持类） |
| Rust MSRV | 1.77 |
| 平台 | Linux / macOS / Windows（CI matrix） |
| 运行时 | 无 JVM、无 `unsafe` |
| License | MIT OR Apache-2.0 |

`0.x` 阶段允许 breaking change；1.0 前不承诺 semver 兼容。详细发布流程见
[RELEASING.md](RELEASING.md)。

## 能力

| 能力 | 状态 |
|---|---|
| 流头、`TC_STRING`/`TC_LONGSTRING`、修改版 UTF-8、块数据、句柄、`TC_RESET` | ✅ |
| `classDesc`、对象、继承、自定义 `writeObject`、数组、enum | ✅ |
| `HashMap`/`LinkedHashMap`/`HashSet`/`ArrayList`/`LinkedList` | ✅（`HashMapLayout` 可指定精确 capacity/threshold） |
| 共享引用、循环引用、多元素流 | ✅ |
| 动态 POJO、未知类原样回写 | ✅ |
| `decode_typed`/`encode_typed`、`FromJValue`/`IntoJValue`（含 slice/数组/BTree/组合字段） | ✅ |
| 流式 `decode_from<R: Read>` / `encode_to<W: Write>` | ✅（解码增量缓冲） |
| `#[derive(JavaSerializable)]`、`computeDefaultSUID`/`suid` 工具 | ✅（泛型结构体支持） |
| `Date`、`BigInteger`、`BigDecimal`、`java.time` 语义视图 | ✅ |
| 严格默认 allowlist | ✅ |
| schema 驱动 serde 桥（递归嵌套/Map/Set/枚举/多根） | ✅（限制见下；`Shared<T>` 保留非环共享身份） |
| JDK fixture 逐字节回放 + `TC_EXCEPTION` 结构化错误 | ✅ |
| Rust 写 → Java 读回 | ✅（fixture + 构造/typed/derive/serde） |
| `TC_EXCEPTION` 结构化错误（`JavaException`） | ✅（`object/50`，不重编码） |

## 快速开始

```rust
use jserde::{decode_stream_with_allowlist, Allowlist};

fn main() -> jserde::Result<()> {
    let bytes = std::fs::read("stream.ser").map_err(|error| jserde::Error::FieldMismatch {
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
use jserde::{builtin::new_hash_map, encode_stream, Graph, JValue};

fn main() -> jserde::Result<()> {
    let mut graph = Graph::new();
    let map = new_hash_map(&mut graph, &[("a", "one")])?;
    graph.add_item(JValue::Obj(map));
    let bytes = encode_stream(&graph)?;
    Ok(())
}
```

derive：

```rust
use jserde_derive::JavaSerializable;

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
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

cd docs/testdata
./gen/gen.sh --check       # 校验 JDK 生成的 fixture
./gen/interop.sh           # Rust 写 → Java 读回 + SUID 对拍（需 JDK 17）

# 可选
cargo run --release -p jserde --example bench -- 16
cargo test -p jserde --test fuzz_smoke
```

## 文档

- [需求](docs/requirements.md)
- [设计](docs/design.md)
- [实现计划](docs/implementation-plan.md)
- [测试基准](docs/testdata/README.md)
- [Benchmark](docs/bench.md)
- [API 稳定性](docs/api-stability.md)
- [发布流程](RELEASING.md)
- [贡献指南](CONTRIBUTING.md)
- [0.1.0 release notes](docs/release-notes-0.1.0.md)
- [编码须知](AGENT.md)

## 已知限制

- `Externalizable` 通用语义、动态代理调用语义、`TC_CLASS` 类加载语义、RMI/JNDI 不在范围；`TC_CLASS`/`TC_PROXYCLASSDESC` 仅做字节与句柄保真回放。
- 动态 `JValue` 层支持任意深度嵌套（`object/41` 验证 3 层）；derive 支持嵌套自定义字段、`Option<Custom>`、`Vec<Custom>`、继承（`#[java(flatten)]`）与 unit-only enum；serde 通过递归 `SchemaField` 支持多层 POJO/Map/Set/枚举/多根。
- serde 默认仍是树模型：普通共享引用按值展开；`jserde_serde::Shared<T>` 在单次调用内保留**非环**共享身份；完整环用动态 `Graph`，serde 环检测返回错误；不支持对象数组字段。
- `java.util.Optional` 不是 `Serializable`，不纳入。
- primitive 数组使用紧凑 raw 存储（4 B/`int`）；25M `int[]` 本地工作集约 2.03× 输入，Linux 可用 `bench -- array` 的 `VmHWM` 复核。
- 输入零拷贝（`Graph` 生命周期改造）与完整 serde 环不做；`Shared<T>` 仅覆盖非环共享。
- 默认 allowlist 不能替代完整反序列化沙箱。

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
