# jser

纯 Rust 实现的 Java 对象序列化协议，与 JDK 的 `ObjectOutputStream` / `ObjectInputStream` 双向互通，**运行时零 JVM 依赖**。

当前版本：`0.1.0`（P0–P7 开发线；尚未发布 crates.io）。

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
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

cd docs/testdata
./gen/gen.sh --check       # 校验 JDK 生成的 34 个 fixture / 74947 字节
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
- serde 桥只支持单根扁平 POJO、标量/String/Option/数组；不支持嵌套对象、Map/Set 字段、环/共享、`TC_RESET`、多元素流。
- `Year`/`YearMonth`/`MonthDay`/`Period` 暂无语义视图。
- `java.util.Optional` 不是 `Serializable`，不纳入。
- 默认 allowlist 不能替代完整反序列化沙箱。
