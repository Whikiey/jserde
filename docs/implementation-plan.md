# 0006 · 实现计划（Implementation Plan）

- 状态：Draft（第 1 节假设与第 3 节设计缺口待确认后转 Accepted）
- 上游：[requirements.md](requirements.md)（`AC-*` 是验收依据）、[design.md](design.md)（`D1–D5` 是技术路线）
- 工作规则：[AGENT.md](../AGENT.md)（事实来源优先级、命令、提交前检查）
- 基准数据：[testdata/](testdata/)（32 例，JDK 17.0.20.1）
- 日期：2026-11-03
- 范围：v0.1（设计 §2 的 MVP + M4/M5/M6 收尾），不含 `OUT-01`～`OUT-06`

本文回答"**按什么顺序做、每步产出什么、怎么证明做对了**"。需求与协议细节不在这里重复，冲突时以 `requirements.md` / `testdata/` 为准。

---

## 0. 交付目标与总策略

v0.1 的完成定义：**32 个 fixture 全部解码正确；Rust 写出的流能被 JDK 读回；`Map`/`List`/`Set`/POJO/数组/环/`reset` 双向互通；解码不 panic、零 JVM 依赖。**

总策略四条：

1. **先读后写**：解码器是理解协议的过程，写方向复用同一套 `ClassDesc`/句柄模型，避免"两边各写一遍"。
2. **fixture 驱动**：每一小步都对应一个已知字节序列；不靠文档记忆写协议。
3. **可重放**：解码结果保留 `classDesc` 元数据、字段顺序、自定义块与句柄，使 `encode(decode(bytes)) == bytes` 成为 M2 的硬门禁（见 §7）。这条比"能读"更强，能提前暴露模型缺口。
4. **先骨架后优化**：M6 之前不做零拷贝/并行优化，只保证大端读写无多余分配。

---

## 1. 计划假设（需确认，否则按此执行）

| # | 假设 | 理由 / 影响 |
|---|---|---|
| A-1 | 目标 **Java 17**（fixture 为 17.0.20.1） | wire 格式自 1.2 未变；17 决定 `java.time`/`BigInteger` 的字段集合 |
| A-2 | **双向**实现（读 + 写） | `FR-02` 为 `MUST`，`S-2` 明确双向 |
| A-3 | 默认 **严格 allowlist**（`java.lang.*`、`java.util.*` + 显式放行） | `NFR-04` 的安全默认；放宽需显式配置并在文档警告 |
| A-4 | Rust `edition 2021`、`rust-version = 1.75` | 保守、无 nightly 依赖；可在 P0 调整 |
| A-5 | 首版**不发布** crates.io | 设计 §8 未决；不影响代码结构，只影响文档/CI 投入 |
| A-6 | 单人开发估算，人日为 1 个熟练 Rust 工程师 | 并行化建议见 §2 |
| A-7 | 本机可能无 JDK，CI 必须有 JDK 17 | 无 JDK 时 interop 测试 `#[ignore]`，fixture 校验在 CI 跑 |

---

## 2. 里程碑总览

阶段编号 `P*` 是本计划的执行单元，与设计里程碑 `M*` 的映射如下：

| 阶段 | 对应 | 内容 | 主要产出 | 出口条件 | 人日 |
|---|---|---|---|---|---|
| P0 | M0 前 | 工程骨架、错误模型、manifest 测试夹具、CI | workspace、`cargo test` 可跑、CI 绿 | 骨架编译、空测试通过、fixture 校验接入 CI | 1 |
| P1 | M0 收尾 | codec 层：大端原语、MUTF-8、块缓冲、句柄表、顶层分发 | `wire/` + codec 12 例全绿 | 12 个 codec fixture 语义通过（逐字节重放留到 P3） | 2–3 |
| P2 | M1 | 解码：`classDesc`、对象、继承、数组、enum、集合、环 | `desc/`、`value/`、`builtin/`（只读） | 32 例全部可解码，语义断言通过（36 仅结构） | 5–7 |
| P3 | M2 | 编码 + 回放门禁 + 双向对拍 | `wire/writer`、`desc` 生成、Java `Verify` | 未修改的 32 例 `encode_stream(decode_stream(x)) == x`（例外登记）；mutation/构造流 Java 读回 | 5–7 |
| P4 | M3 | 动态 POJO 收口、typed API、未知类回写、环/reset | 对外 API、文档示例 | `AC-03`/`AC-05` 通过 | 2–3 |
| P5 | M4 | `JavaSerializable` + derive + SUID | `jser-derive`、SUID 工具 | `NoSuid` 的 SUID 与 JDK 一致；derive 互通 | 4–5 |
| P6 | M5 | 内置类 + `java.time` + serde 桥 | `builtin/` 完整、`jser-serde` | 每类双向对拍；serde 越界明确报错 | 4–6 |
| P7 | M6 | 加固、fuzz、bench、文档、发布准备 | fuzz 目标、bench、README/SECURITY | `AC-04`/`AC-06`；`cargo-deny`/审计通过 | 4–5 |

**合计约 27–37 人日（中位 32，约 6.5 周）**，不含需求返工与 JDK 环境问题。P3 的 5–7 人日已包含回放门禁、mutation/构造流测试与例外处理（约 +0.5–1 人日）。

依赖关系（→ 表示必须在前者之后）：

```
P0 ──► P1 ──► P2 ──► P3 ──► P4
                 │      │
                 │      ├──► P5（derive 依赖 desc 生成，可与 P4 并行）
                 │      └──► P6（内置类编码依赖 P3，可与 P5 并行）
                 └──► 扩充 fixture（P5/P6 需要时）
P2 ─────────────────────────► P7 的 fuzz 目标（可提前起草）
```

关键路径：`P1 → P2 → P3`。P2 的 `classDesc`/句柄模型一旦定错，P3 的逐字节门禁会全面返工，因此 §3 的模型缺口必须在 P2 开工前定案。

---

## 3. 设计缺口与定案（已回写 `design.md`）

以下缺口是写计划时对照 fixture 发现的。**2026-11-03 已全部定案并回写设计文档**（G-8 的门禁边界见 `design.md` §5.1）；实现以 `design.md` 为准，本节只保留决策摘要，便于 review。

| # | 缺口 | 定案（回写位置） |
|---|---|---|
| G-1 | `JObject { class, fields }` 不足以表达继承链与每层自定义块 | `JObject { classes: Vec<ClassData> }`，父→子；每层含 `desc` 句柄、该层字段、`custom: Option<Vec<JValue>>`（空也要写 `TC_ENDBLOCKDATA`）。`design.md` D1 |
| G-2 | 只存类名字符串不够 | `Graph` 的 `Slot::Desc(ClassDesc)` 保存 `name/suid/flags/fields/super_desc`，描述符也占句柄；重编码原样写回。`design.md` D1 |
| G-3 | 顶层流是**多元素**的，单根 API 不够 | `Graph { slots, items }`，items 可含 `JValue::Reset`；`decode_stream` 读全流，`decode` 只接受单元素。`design.md` D1/§5 |
| G-4 | `TC_RESET`、顶层块数据没有对应值 | `JValue::Reset`（流/自定义段元素）、`JValue::Block(BlockData { bytes, long })`；保留块边界与头部类型（`11` 的 1024+976）。`design.md` D1 |
| G-5 | `java.time.Ser` flags `0x0C`（Externalizable + block） | 作为**已知内置特例**；未知 Externalizable 带 block 时按不透明块保留，旧式无 block 的报 `UnsupportedClass`。`design.md` D4/§4.2 第 16 条 |
| G-6 | `java.util.Optional` **不是** `Serializable` | 从 FR-13 与 M5 移除，并在文档说明 Java 端不会产出该流。`requirements.md` FR-13、`design.md` M5 |
| G-7 | 未配对 UTF-16 代理项的表示 | 解码返回 `MalformedString`（不 panic、不做有损替换）；`char` 字段仍用 `JValue::Char(u16)`。`design.md` §4.2 第 3 条 |
| G-8 | 逐字节重放是强门禁，可能有个别非规范输出 | **已定案**：作为 P3 回归门禁，不写入 AC；只适用于"未修改的 32 例回放"；例外登记"文件+原因+负责人+到期"；配 mutation/构造流测试防"原样回显"。`design.md` §5.1、本计划 P3-6/P3-7 |
| G-9 | `JavaSerializable::to_jvalue` 无 `Graph`，无法分配字符串/对象句柄 | 签名改为 `to_jvalue(&self, &mut Graph)` / `from_jvalue(&JValue, &Graph)`。`design.md` D2 |
| G-10 | `classDesc` 的收尾标记位置未写明 | `classAnnotation` 的 `TC_ENDBLOCKDATA` 位于 `superClassDesc` **之前**（`20` 的 `78 70`）。`design.md` §4.2 第 5 条、§4.3 |
| G-11 | `TC_LONGSTRING` 头部选择未保存，非规范流的短串用 long 时无法回放 | `Slot::Str { value, long }`，回放沿用 `long`；新建字符串按修改版 UTF-8 字节长度（非 Rust `len()`）选头。`design.md` D1/§4.2 第 3 条 |

已同步：`design.md`（D1–D5、§4.2、§4.3、§5、§5.1 回放门禁边界、M1/M2/M3/M5、§7、§8）、`requirements.md`（FR-03、FR-13、类型映射表、AC 说明）、本计划（P3 门禁与 mutation 测试、§12、§16、§18）。

---

## 4. P0 · 工程骨架（1 人日）

| ID | 任务 | 产出 | 验证 |
|---|---|---|---|
| P0-1 | 初始化版本控制并提交当前文档基线（**已完成**：`main` @ `ad46c43`） | `.git/`、首次提交 | `git log` 可见文档快照 |
| P0-2 | ✅ workspace 骨架 | `Cargo.toml`（`members = ["crates/*"]`、`resolver = "2"`、`rust-version = "1.75"`）、`rust-toolchain.toml`（可选） | `cargo metadata` 通过 |
| P0-3 | ✅ 核心 crate 空壳 | `crates/jser/{Cargo.toml,src/lib.rs}`，`#![forbid(unsafe_code)]`，模块占位 `wire/ desc/ value/ builtin/` | `cargo check` |
| P0-4 | ✅ derive / serde 占位 crate | `crates/jser-derive`（`proc-macro = true`）、`crates/jser-serde`（独立占位，P6 再接） | `cargo check --workspace` |
| P0-5 | ✅ 错误模型 | `error.rs`：`BadMagic`、`Truncated`、`BadTag`、`BadHandle`、`MalformedString`、`UnsupportedClass(String)`、`FieldMismatch`、`LimitExceeded` | 测试构造每种错误 |
| P0-6 | ✅ manifest 测试夹具 | `crates/jser/tests/common/mod.rs`：按 `CARGO_MANIFEST_DIR` + `../../docs/testdata/manifest.json` 定位，反序列化为 `Case { id, file, features, expect, size, sha256 }` | 用例数 = 32 的断言 |
| P0-7 | ✅ CI 骨架（已添加，待远端首次运行） | `.github/workflows/ci.yml`：`fmt` / `clippy -D warnings` / `test`；`fixtures` job 装 JDK 17 跑 `docs/testdata/gen/gen.sh --check` | CI 三个 job 通过（待远端） |
| P0-8 | ✅ 依赖白名单 | 运行时仅 `thiserror`；dev 为 `serde`/`serde_json`（读 manifest）；`proptest` 在 P4 引入 | `cargo tree` 复核 |

**状态（2026-11-03）**：P0-2～P0-8 已完成。`cargo test --workspace` 通过 11 项 P0 测试（manifest 7 + error 4），`cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`docs/testdata/gen/gen.sh --check` 均通过；CI 配置待远端首次运行验证。

出口：`cargo test --workspace` 通过；CI 能跑 `gen.sh --check`；没有业务逻辑。

---

## 5. P1 · codec 层：原语、MUTF-8、块缓冲、句柄（2–3 人日）

目标：把"无类描述符的流"读对，并建立后续复用的 `StreamReader`。这是 M0 的剩余部分。

| ID | 任务 | 要点 | fixture |
|---|---|---|---|
| P1-1 | ✅ `wire/reader.rs`：有界游标 | 大端 `u16/u32/u64/i*/f32/f64`、`read_exact`、剩余长度、`max_stream_size`；任何越界返回 `Truncated`，禁止索引越界 panic | 单测 + fuzz 初版 |
| P1-2 | ✅ `wire/mutf8.rs`：解码/编码 | `C0 80` ↔ `U+0000`；2/3 字节 BMP；补充平面 = 两个 3 字节代理项（6 字节）；拒绝非法/超长（`C0 80` 除外）；解码 `TC_STRING`/`TC_LONGSTRING` 并保留头部类型；编码与解码互为逆 | `01–06`、`12` |
| P1-3 | ✅ 顶层块读取 | `TC_BLOCKDATA`（u8 长度）/`TC_BLOCKDATALONG`（i32 长度），保留块边界与头部类型；自定义 `writeObject` 的块序列在 P2-4 复用同一解析循环 | `09`（13 字节合并）、`11`（两个 long 块） |
| P1-4 | ✅ `wire/handle.rs`：wire 句柄表 | 从 `0x7E0000` 递增分配 wire 值；维护 `wire -> 内部 Handle` 映射；`get`/`assign`/`reset`；`TC_REFERENCE` 越界报 `BadHandle`；reset 只清映射，内部 `Handle` 继续单调；P1 只映射 String | `07`、`08`、`10` |
| P1-5 | ✅ 顶层分发（P1 子集） | `TC_NULL`/`TC_STRING`/`TC_LONGSTRING`/`TC_REFERENCE`/`TC_BLOCKDATA*`/`TC_RESET`；返回 `JValue`（`TC_RESET` → `JValue::Reset`）；`reset` 清 wire 映射；对象/数组/enum 留给 P2 | `08`、`10` |
| P1-6 | ✅ codec 测试 | 数据驱动：`kind = string/null/sequence/blockdata/primitive_stream/reset_stream` 的语义断言；字符串长度上限分支（`06` 70000 字节） | 12 例全绿 |

**状态（2026-11-03，TDD）**：P1 主体完成，codec 12 例全绿；P1 新增 21 项测试（codec 10 个测试函数覆盖 12 例 + MUTF-8 6 + reader 5），加上 P0 的 11 项共 32 项，`clippy`/`fmt`/`gen.sh --check` 均通过。P1-3 的自定义 `writeObject` 块上下文并入 P2-4。

出口：`cargo test -p jser --test codec` 12 例通过；`cargo fuzz run decode` 初版 10 分钟无 panic（fuzz 目标可延后到 P7，但 reader 必须从第一天就是有界的）。

---

## 6. P2 · 解码：描述符、对象、数组、集合（5–7 人日，M1）

目标：32 个 fixture **全部能解码**并给出正确语义。先不做写方向。

| ID | 任务 | 要点 | fixture |
|---|---|---|---|
| P2-1 | `desc/classdesc.rs` 解析 | `TC_CLASSDESC`：类名（MUTF-8）、`suid: i64`、`flags: u8`、字段数、字段（类型码 + 名 + `L`/`[` 的 `className1`）；`superClassDesc` 链；每遇到新描述符分配句柄，重复出现走 `TC_REFERENCE` | `20`、`22`、`29`、`39` |
| P2-2 | 类层次与字段布局 | 字段顺序 = 基本类型（按名升序）→ 对象/数组（按名升序）；classdata 顺序与描述符严格一致；`transient` 不出现在描述符；父类字段先于子类字段 | `21`（断言顺序 `b,by,c,d,f,i,l,sh,a,s`）、`37` |
| P2-3 | `value/` 图模型（按 design D1） | 引用型值（String/数组/对象/enum）统一 `Handle`；`Slot::Desc` 存完整 `ClassDesc`；`JObject { classes }` 分层；对象/数组先占位再填充；`Graph::resolve` 不递归展开环 | `33`、`34` |
| P2-4 | 对象解码 | `TC_OBJECT`：读 `classDesc` → 分配对象句柄 → 逐层 classdata；基本类型字段读**裸字节**；对象字段递归；`SC_WRITE_METHOD` 读自定义块到 `TC_ENDBLOCKDATA` 并保留原始字节；`SC_EXTERNALIZABLE` + `SC_BLOCK_DATA`（`java.time.Ser`）先按不透明块读取并保留字节，语义留 P6；未知类降级为 `JValue::Obj` | `20–23`、`36`（结构） |
| P2-5 | 数组 | `TC_ARRAY`：读描述符与长度；基本类型数组按元素宽度连续读裸字节；对象数组逐元素；支持嵌套数组；数组类名两种写法（点号 vs JVM 描述符）；不假设数组 SUID 为 0 | `25–27` |
| P2-6 | enum | `TC_ENUM`：描述符（flags `0x12`、SUID 0）→ 常量名 `TC_STRING`（占句柄，存为 `Slot::Str`，`Slot::Enum.constant` 指向它） | `24` |
| P2-7 | 集合解码（注册表） | 见下方布局表；`Integer`/`Number` 等无自定义逻辑的走通用对象路径 | `28–32`、`38` |
| P2-8 | 上限与健壮性 | `Limits { max_depth, max_elements, max_stream_size }`；`classDesc` 链用迭代；对象/数组嵌套用显式栈或受控深度；默认值写入文档 | fuzz 冒烟 |
| P2-9 | 读 API 与语义断言 | `decode_stream -> Graph`、`decode`（单元素）、`Reader`、`Graph::items()`；`as_map/as_list/as_set/as_object/as_scalar`；按 `manifest.expect.kind` 写比较器 | 全部 32 例 |

集合/内置类的 wire 布局（**以 fixture 字节为准**，实现前用 `xxd` 复核）：

| 类 | flags | classdata | 自定义块 | 依据 |
|---|---|---|---|---|
| `ArrayList` | `0x03` | `size: int` | `size: int` + 逐元素 | `28` |
| `HashMap` | `0x03` | `loadFactor: float` + `threshold: int`（裸字节） | `capacity: int` + `size: int` + 逐条 key/value | `29` |
| `LinkedHashMap` | `0x02` | `accessOrder: bool` + 父类 `HashMap` 字段 | 同 `HashMap` | `30` |
| `HashSet` | `0x03` | 无 | `capacity: int` + `loadFactor: float` + `size: int` + 元素 | `31` |
| `LinkedList` | `0x03` | 无 | `size: int` + 元素 | `32` |
| `Date` | `0x03` | 无 | `long`（epoch millis） | `36` |
| `java.time.Ser` | `0x0C` | 无 | 类型字节 + 字段（`LocalDate` 为 type=3, year, month, day） | `36` |

注意：`BigInteger` 在 JDK 17 的描述符含 `bitCount/bitLength/firstNonzeroByteNum/lowestSetBit/signum` 等字段与 `[B magnitude`，与旧版记忆不同；**不要按记忆写字段表**。语义支持放到 P6。

出口：`AC-01`（解码部分）通过；36 先按"结构可解码"验收，语义在 P6 补齐。此时允许编码尚未实现。

---

## 7. P3 · 编码 + 双向对拍（5–7 人日，M2）

目标：Rust 写出的字节被 Java 读回；未修改的流能**逐字节回放**，构造/修改的流由 Java 读回验证。

| ID | 任务 | 要点 | 验证 |
|---|---|---|---|
| P3-1 | `wire/writer.rs` | 流头 `AC ED 00 05`；大端写入；wire 句柄分配与 `TC_REFERENCE`；`JValue::Reset` 清 wire 映射后重新写描述符 | codec 层字节一致 |
| P3-2 | `wire/block.rs` 写侧 | 1024 字节分块、`u8` vs `i32` 长度、flush 时机（写对象标签/`TC_ENDBLOCKDATA` 前）；顶层 `close` 时 flush | `09`、`11` |
| P3-3 | `desc/write.rs` | 由 `ClassDesc` 原样写回；新描述符按字段排序规则生成（基本类型按名 → 对象按名）；同类去重；`suid` 来自 schema/解码值 | `20–24`、`39` |
| P3-4 | `Graph` → 字节 | 按 `items` 顺序编码；同一 `Handle` 首次实体、后续 `TC_REFERENCE`；`Block` 保留头部类型；字符串按 `Str.long` 选 `TC_STRING`/`TC_LONGSTRING`（新建时按 MUTF-8 字节长度，非 Rust `len()`；含合成短串-long 用例）；`Reset` 清表；`SC_WRITE_METHOD` 先字段后自定义块再 `TC_ENDBLOCKDATA`；未知类按保留的元数据回写 | `20–39` |
| P3-5 | 集合编码 | 按上表布局；`HashMap` 的 `capacity`/`threshold` 由 `size` 反推合理值（Java `readObject` 读但忽略 capacity；阈值为 `table_size * loadFactor` 的整数语义）；`LinkedHashMap` 保持条目顺序 | `28–32`、`38` |
| P3-6 | **回放门禁（未修改流）** | `encode_stream(decode_stream(bytes)) == bytes` 覆盖 32 例；范围仅限 decode 后不改图；例外登记"文件+原因+负责人+到期"，默认不允许；**不写入 AC** | `tests/roundtrip_bytes.rs` |
| P3-7 | **防作弊 / mutation 测试** | 解码 fixture → 改一个标量或字段 → 编码 → Java 读到新值；手工构造 `HashMap`/POJO → 编码 → Java 读回；确认 `encode` 不是原始字节回显 | `tests/mutation.rs` + Java `Verify` |
| P3-8 | Java 校验程序 | `docs/testdata/gen/Verify.java`：读 Rust 产出的 `.ser`，断言类名/字段/集合内容；`interop.sh` 编排；不修改现有 fixture 与 manifest | 手工 + CI |
| P3-9 | 双向对拍测试 | 解码语义 → 再编码 → 再解码，结构等价（环按句柄判等）；Java 读回作为最终裁决 | `AC-02` |
| P3-10 | CI interop job | 装 JDK 17；`cargo run --example emit` 写出 `build/rust/*.ser` → `java Verify`；无 JDK 时本地跳过 | CI 绿 |

回放门禁的意义：它同时校验字段顺序、块边界、句柄编号、`classDesc` 去重、`reset` 时机。**它不替代 `AC-02`**：门禁只证明"未修改的流能原样写回"，Java 读回与 mutation 测试才证明编码器对构造/修改后的图也正确。若某例无法重放，先怀疑模型缺信息（§3），再按 §5.1 登记例外。

出口：`AC-01` 全量 + `AC-02` + `AC-03`（不含 typed API）+ mutation/构造流 Java 读回。

---

## 8. P4 · 动态 POJO 与 typed API（2–3 人日，M3）

| ID | 任务 | 要点 | 验证 |
|---|---|---|---|
| P4-1 | typed 转换 | `decode_typed::<T>`、`FromJValue`/`IntoJValue`；标量、`Option`、`Vec`、`HashMap`、`HashSet`、嵌套结构；字段缺失报 `FieldMismatch` 而非静默丢 | 单元 + `20–32` |
| P4-2 | 未知类回写 | 无 Rust 定义时保留类名/字段/自定义块，`encode` 后字节等价；`AC-05` | `23`、`39`、36 |
| P4-3 | 环/共享 API | `Graph::resolve`、`Handle` 比较；`34` 往返后 `a.next.next == a`（按句柄）；`33` 第二次出现是同一 `Handle`（回指） | `33`、`34` |
| P4-4 | `reset` 双向 | 解码时清表；编码时可主动 `reset()`；`35` 重放时描述符重新写出 | `35` |
| P4-5 | API 文档示例 | `docs/` 或 crate rustdoc：读 Map、读 POJO、写 Map、allowlist | `cargo test --doc` |

出口：`AC-03`、`AC-05` 通过；`Graph` 对用户可用（示例可编译）。

---

## 9. P5 · derive 与 SUID（4–5 人日，M4）

| ID | 任务 | 要点 | 验证 |
|---|---|---|---|
| P5-1 | `JavaSerializable` trait | `CLASS`、`SUID`、`schema()`、`to_jvalue`/`from_jvalue`；与 `Graph` 解耦 | 手写实现先跑通 |
| P5-2 | `#[derive(JavaSerializable)]` | 属性：`#[java(class = "...", suid = ...)]`、`#[java(rename = "...")]`、`#[java(transient)]`；字段顺序由宏按规则排序；编译期错误信息清晰 | `trybuild` 编译失败用例 |
| P5-3 | `computeDefaultSUID` | 按 JDK `ObjectStreamClass` 算法：类名、修饰符、接口、字段（名/修饰符/描述符）、构造器、方法 → SHA-1；再按 `i = 7..0` 逐字节左移拼入摘要（**摘要字节的拼接方向与直觉相反**，易错点） | `39` → `-3559046683250708543` |
| P5-4 | `serialver` 对拍工具 | `cargo run --bin suid -- <class>` 与 JDK `serialver` 输出比对；CI 装 JDK 时执行 | `build/suid.txt` + CI |
| P5-5 | derive 互通测试 | 定义与 `Fixtures$Point`/`Mixed` 等价的 Rust 结构体，编码后 Java 读回；Java 写出的流 typed 解码 | `20`、`21` |
| P5-6 | SUID 文档 | 明确"计算值仅供参考"、优先显式声明；不匹配的错误信息含两个 SUID | 文档评审 |

出口：derive 的结构体与等价 Java 类双向互通；`39` 的 SUID 一致。

---

## 10. P6 · 内置类与 serde 桥（4–6 人日，M5）

| ID | 任务 | 要点 | 验证 |
|---|---|---|---|
| P6-1 | `Date`/`BigInteger`/`BigDecimal` 语义 | 以 fixture 字段为准；`Date` 自定义块 `long`；`BigDecimal` 的 `scale`+`intVal`；`BigInteger` 的 6 字段 + `[B` | `36` |
| P6-2 | `java.time.Ser` | 按类型字节实现 `LocalDate`（`36`），再扩 `Instant`/`LocalDateTime`/`Duration`/`ZonedDateTime`；新增 fixture 覆盖 | 新增用例 + 双向 |
| P6-3 | 注册表与 allowlist | `Registry` 按类名分派 codec；未知类降级；allowlist 支持精确名 + `prefix.*`；默认严格；越界返回带类名的错误 | 单元 + 文档 |
| P6-4 | `jser-serde` | feature 默认关闭；只支持单根、无环、无回指、已知 POJO；遇 `Reset`/多元素/回指/未知类明确报错 | serde 测试 + 负例 |
| P6-5 | 扩充 fixture 流程 | 改 `gen/Fixtures.java` + `gen/build_manifest.py` 的 `CASES` → `./gen/gen.sh` → `--check`；同步 `testdata/README.md` 覆盖表 | `gen.sh --check` |
| P6-6 | 文档收口 | 内置类支持矩阵、`Optional` 不支持说明（非 `Serializable`）、serde 限制 | 文档评审 |

出口：`36` 的四个值语义正确；每个新增内置类有双向用例；serde 越界行为有测试。

---

## 11. P7 · 加固与发布准备（4–5 人日，M6）

| ID | 任务 | 要点 | 验证 |
|---|---|---|---|
| P7-1 | fuzz | `fuzz/` + `libfuzzer-sys`；目标：任意字节解码、解码→编码→解码、`MUTF-8`；语料从 32 例 seed；CI 冒烟，夜间 30 分钟 | 30 分钟无 panic/OOM（`AC-04`） |
| P7-2 | 性能 | `criterion` bench：字符串、POJO、大 Map、100 MB 级流；目标 ≥ 50 MB/s、内存峰值 < 3×；不达标先 profile 再优化 | `AC-06` + bench 报告 |
| P7-3 | panic 审计 | `#![forbid(unsafe_code)]`；库内禁用 `unwrap`/`expect`（clippy `unwrap_used` 或评审）；所有切片用 `get`/游标 | `cargo clippy` |
| P7-4 | 文档 | README（能力矩阵、快速开始、安全警告）、SECURITY.md（gadget 风险与 allowlist）、CHANGELOG、rustdoc 示例 | `cargo test --doc` |
| P7-5 | 发布准备 | `cargo package --list`、版本号、`docs.rs`/feature 说明（若确认发布） | 打包通过 |
| P7-6 | 全量回归 | 32 例 + 新增用例；`gen.sh --check`；interop；fuzz 冒烟；bench 基线归档 | 全绿 |

出口：`AC-04`、`AC-06`；`cargo test --workspace`、`clippy -D warnings`、`fmt --check`、`gen.sh --check` 全部通过；v0.1.0 可交付。

---

## 12. 测试矩阵

| 测试层 | 覆盖 | 位置 | 何时 |
|---|---|---|---|
| 单元 | MUTF-8 表驱动、字符串头 long/short、大端读写、块边界、句柄表、字段排序、SUID | `crates/*/src/**` 内 `#[cfg(test)]` | P1 起持续 |
| golden 语义 | 32 例 → `manifest.expect` 比较器 | `tests/golden.rs` | P2 起 |
| 回放门禁（未修改流） | `encode_stream(decode_stream(x)) == x`，32 例，例外登记 | `tests/roundtrip_bytes.rs` | P3 起 |
| mutation / 防作弊 | 解码→改字段→编码→Java 读到新值；证明不是原样回显 | `tests/mutation.rs` + Java `Verify` | P3 起 |
| 构造流 Java | 手工构造 Map/POJO → 编码 → Java 读回 | `tests/interop.rs` | P3 起 |
| 双向对拍（Java） | Rust 写 → `Verify` 读，语义相等 | `tests/interop.rs`（`#[ignore]` 无 JDK） | P3 起 |
| typed/derive | `decode_typed`、derive 互通 | `tests/typed.rs`、`tests/derive.rs` | P4/P5 |
| property | 随机 `JValue` 树/图往返；随机字节不 panic | `tests/property.rs` | P4 起 |
| fuzz | 任意字节、round-trip | `fuzz/fuzz_targets/` | P7（PR 冒烟） |
| bench | 吞吐/内存 | `benches/` | P7 |

fixture → 阶段映射（用于排期与验收）：

| 阶段 | fixture | 数量 |
|---|---|---|
| P1 | codec `01–12` | 12 |
| P2 | object `20–39`（36 仅结构） | 20 |
| P3 | 全部 32 的写方向 + 未修改回放 | 32 |
| P4 | `33`、`34`、`35`、`39` 的 API 层 | 4 |
| P5 | `20`、`21`、`22`、`39` 的 typed/derive | 4 |
| P6 | `36` + 新增 `java.time` 用例 | 1+ |

ac 对应：`AC-01` → P1/P2/P3；`AC-02` → P3；`AC-03` → P3/P4；`AC-04` → P7；`AC-05` → P2/P4；`AC-06` → P7。

> "32 例回放"是 P3 回归门禁，**不是 AC**；验收仍以 `AC-02` 的 Java 读回为准（`design.md` §5.1）。

---

## 13. CI 门禁

| Job | 触发 | 命令 | 失败判定 |
|---|---|---|---|
| lint | PR | `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings` | 任一告警 |
| test | PR | `cargo test --workspace`（离线，不装 JDK） | 失败 |
| fixtures | PR | JDK 17 + `cd docs/testdata && ./gen/gen.sh --check` | 字节/hash/manifest 漂移 |
| interop | PR | 编译 `Verify`；`cargo run --example emit` → Java 断言 | 任一用例读回失败 |
| fuzz-smoke | PR | `cargo +nightly fuzz run decode -- -runs=5000`（可选 job） | panic |
| nightly | 定时 | fuzz 30 分钟、bench 对比 | 回归/panic |

CI 安装 JDK 17 与 `python3`；Rust 用 stable（MSRV job 可选）。缓存 `~/.cargo`、`target/`。

---

## 14. 依赖与工具

| 类别 | 选型 | 理由 |
|---|---|---|
| 运行时 | `thiserror` | 错误建模；不引入 `anyhow` |
| 运行时 | `sha1` | `computeDefaultSUID`（纯 Rust） |
| 运行时（可选） | `serde`（`feature = "serde"`，经 `jser-serde`） | 桥接层隔离，核心不依赖 |
| dev | `serde`、`serde_json` | 读 `manifest.json` |
| dev | `proptest` | 随机图/字符串往返 |
| dev | `criterion` | bench |
| fuzz | `libfuzzer-sys`、`arbitrary` | 独立 fuzz crate，不进发布物 |
| 不引入 | `byteorder`、async runtime、平台相关 crate | 手写大端即可，保持可移植 |

工具链：`cargo fmt`、`clippy`、`cargo-fuzz`、JDK 17、`python3`。

---

## 15. 风险登记（实现级，补充设计 §7）

| 风险 | 触发信号 | 缓解 |
|---|---|---|
| `JValue` 模型缺元数据，P3 逐字节门禁大面积失败 | P2 中只能存类名/字段 | §3 G-1/G-2 在 P2 前定案；先做 `23`（自定义块）与 `22`（继承）验证模型 |
| 孤立代理项导致字符串语义争议 | 遇到 `ED A0 xx` 类字节 | 严格报错 + 文档；不急于引入 `u16` 字符串 |
| 递归过深导致栈溢出 | 深嵌套数组/超长继承链 | 迭代 `classDesc` 链、显式栈、默认深度上限（建议 1024，可配置） |
| SUID 算法移植偏差 | `39` 对不上 | 逐项对照 JDK 源码；CI 用 `serialver` 对拍；文档降低预期 |
| `HashMap` 写侧阈值不合理 | Java 读回后容量异常/阈值不一致 | capacity 取 ≥ size 的 2 的幂，threshold = capacity × loadFactor 的 Java 语义；对拍验证 |
| `java.time.Ser` 格式无公开文档 | 只能读到部分类型 | 以 JDK 源码 + 新增 fixture 为准；先支持 `36` 需要的 `LocalDate`，其余标注"实验性" |
| 无本地 JDK，写方向开发受阻 | `java`/`javac` 不在 PATH | interop 测试 `#[ignore]`；CI 负责最终裁决；必要时你提供 JDK 环境 |
| serde 桥侵蚀核心模型 | 为支持环/共享而加特例 | 维持 `D3`：serde 只做无环 POJO，越界报错 |
| allowlist 给人"安全"错觉 | 文档不足 | SECURITY.md 明确：allowlist 降低风险不等于沙箱；默认严格 |
| 回放门禁误报/例外滥用 | 阻塞 P3 或掩盖回归 | 只对未修改 fixture（design §5.1）；构造/修改图改验 Java 读回；例外必须登记并有到期 |
| 估算偏乐观（尤其 P2/P3） | 回放门禁失败次数 | 每阶段留 20% 缓冲；P2 出口不达标不进入 P3 |

---

## 16. 每阶段 Definition of Done

通用（每阶段结束都要满足）：

- [ ] 新增行为有测试，测试数据来自 `manifest.json` 或新增 fixture，不复制期望值
- [ ] 解码路径无 `unwrap`/`expect`/越界索引，无 `unsafe`
- [ ] `cargo test --workspace`、`cargo clippy -D warnings`、`cargo fmt --check` 通过
- [ ] 改了 fixture/协议事实 → 同步 `design.md` §4.3、`testdata/README.md`、`requirements.md` 追踪矩阵
- [ ] 报告写明满足的 `AC-*` 与遗留风险
- [ ] P3 起：32 例未修改回放通过；mutation 与构造流 Java 读回通过；若有例外，已登记"文件+原因+负责人+到期"

阶段专属出口见 §2 表格；**P2 未让 32 例可解码之前，不开 P3 的编码实现**（可先写 `ClassDesc` 写侧的数据结构）。

---

## 17. 开工清单（按顺序执行）

1. ✅ 按 §1 的 A-1～A-7 默认值执行；§3 的 G-1～G-11 已定案。
2. ✅ 确认 git 基线（P0-1，`ad46c43`）。
3. ✅ 搭 workspace + 空 crate + 错误模型（P0-2～P0-5）。
4. ✅ 写 manifest 测试夹具，断言 32 例（P0-6）。
5. ✅ 接 CI 的 `gen.sh --check`（P0-7），确认基线数据未漂移。
6. ✅ 实现有界 `StreamReader` + MUTF-8（P1-1/P1-2）。
7. ✅ 实现顶层块读取 + 句柄表（P1-3/P1-4），codec 12 例全绿。
8. 实现 `classDesc` 解析 + 单层对象（P2-1/P2-4），先过 `20`。
9. 加继承、字段排序、`SC_WRITE_METHOD`（P2-2），过 `21–23`。
10. 加数组/enum/集合/环/reset（P2-5～P2-8），过 `24–39`。

完成第 10 步即可演示"Java 写 → Rust 读"的完整 MVP（`AC-01`），之后按 P3 补写方向。

---

## 18. 待确认（影响本计划）

1. §1 的假设是否成立（Java 17、双向、严格 allowlist、edition/MSRV、暂不发布）。
2. 新增 fixture（P5/P6 需要）由谁执行：本机有 JDK 则可直接跑 `gen.sh`；否则需你在有 JDK 的环境生成后提交。
3. 是否需要在 v0.1 就产出 typed API 与 serde 桥，还是先交付 `JValue` 动态层（P4/P6 可后移）。

> §3 的 11 项设计缺口已定案并回写 `design.md`；回放门禁的范围/例外/防作弊见 `design.md` §5.1，不再作为开放问题。若 review 后要改模型（尤其 D1 的句柄化），需在 P2 开工前提出。

---

## 19. 变更记录

| 日期 | 变更 | 来源 |
|---|---|---|
| 2026-11-03 | 初版：P0–P7 阶段拆分、任务表、测试矩阵、CI 门禁、风险与开工清单；识别 8 项设计缺口（§3） | 本轮 |
| 2026-11-03 | 按 §3 回写 `design.md`：句柄化 D1、D2 签名、多元素 API、java.time.Ser 特例、`Optional` 移出、代理项策略；同步 P1/P2/P6 任务与 §18/§19 | 补缺口 |
| 2026-11-03 | 完善回放门禁：限定"未修改的 32 例"、例外登记、mutation/构造流防作弊；补 `Slot::Str.long`（G-11）；明确门禁不是 AC | 拍板 1 |
| 2026-11-03 | P0-1 完成：git 初始化并提交文档/testdata（`ad46c43`） | 版本控制 |
| 2026-11-03 | P0 完成：workspace/错误模型/manifest 夹具/CI；P0-2～P0-8 落地，6 项测试通过 | P0 |
| 2026-11-03 | P1 主体完成（TDD）：StreamReader/MUTF-8/块/句柄/顶层分发，codec 12 例与 27 项测试全绿 | P1 |
| 2026-11-03 | 修正 `11` 号夹具语义预期：976 字节块 >255，JDK 也使用 `TC_BLOCKDATALONG`；同步 manifest/文档 | TDD 发现 |
| 2026-11-03 | 补齐 P0 契约测试：manifest 结构/未登记 fixture/错误 Send+Sync/Result 别名；全仓 32 项通过 | P0 TDD |
