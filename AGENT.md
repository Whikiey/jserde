# AGENT.md — 本仓库的编码须知

给在本仓库工作的 AI 编码 Agent（以及人类贡献者）的操作说明。**开始任何改动前先读本文件**；它与 `docs/` 冲突时，以第 2 节的事实来源顺序为准。

一句话：用**纯 Rust** 实现 Java 对象序列化协议，与 `ObjectOutputStream` / `ObjectInputStream` 双向字节级互通，运行时不依赖 JVM。

---

## 1. 当前状态（2026-11-05）

| 项 | 状态 |
|---|---|
| 文档 | `requirements.md` / `design.md` / `implementation-plan.md` 已同步到 P9；剩余项见计划 §20/§21 |
| 实现计划 | `docs/implementation-plan.md`：P0–P9 已完成主要功能；P8-1 的 30 分钟 fuzz 暂缓；剩余 P9 扩展见 §20/§21 |
| 基准数据 | **已完成**：43 个用例、79031 字节（P6/P7/P8/P9 新增 object/40–50），JDK 17.0.20.1 生成，见 `docs/testdata/` |
| Rust 代码 | **P0–P9 主要功能已完成**：编解码、typed、allowlist、derive/SUID、内置类语义、嵌套 serde、primitive raw 数组、`TC_CLASS`/代理保真回放、扩展容器、CI/bench/panic 审计；**183 项测试全绿**。剩余 P9-5（暂缓）/P9-12（部分） 见 §20/§21 |
| 版本控制 | 已初始化 git（`main`）；文档、testdata 与 P0–P9 代码均已提交，**之后每次改动都必须提交**（见 §10.1） |

所以：P0–P9 主要功能已完成；剩余项见 `implementation-plan.md` §20/§21：P8-1 的 30 分钟 fuzz（AC-04，暂缓）、P8-4 的 OUT 负例、P8-6 的并发/深链、P9-5 零拷贝（架构级暂缓）、P9-12 的完整环（仅非环 `Shared<T>` 已完成）。发布仍按 A-5 暂缓。

---

## 2. 事实来源与优先级（冲突时严格按此顺序）

1. `docs/testdata/fixtures/**/*.ser` 的**字节本身** —— 唯一事实来源。
2. `docs/testdata/manifest.json` —— 用例编号、文件、覆盖特性、期望语义值、size、sha256。
3. `docs/requirements.md` 的 `FR-*` / `NFR-*` / `AC-*` —— 验收以 `AC-*` 为准。
4. `docs/design.md` 的协议说明（§4.2、§4.3）—— 实现路径。
5. 其它一切（JDK 文档记忆、同类库、本文件）。

规则：

- **文档与字节冲突 → 改文档，不改 fixture。** 在 `design.md` §4.3 增加一行"事实 + 依据文件"，并在 `docs/devlog.md` 与 plan §19 变更记录里各记一笔。
- **fixture 只能通过 `docs/testdata/gen/gen.sh` 重新生成**，禁止手改 `.ser`、手改 `manifest.json` 里的 size/sha256。
- 实现写不通时，先问"我对协议的理解是否错了"，再问"fixture 是否错了"。历史经验：两次理解错误都是靠 fixture 抓出来的（`design.md` §4.2 第 2、6 条）。

---

## 3. 硬约束（违反即返工）

| 来源 | 约束 |
|---|---|
| S-1 | 纯 Rust，运行时**零 JVM**：不做 JNI、不调外部 `java` 进程。JDK 只用于生成/校验基准数据。 |
| S-2 | **双向**互通：Java 写 → Rust 读、Rust 写 → Java 读，都要正确。 |
| S-3 | Java `Map` ↔ Rust `Map` 是**必须项**，是 MVP 核心。 |
| NFR-01 | 解码任意输入**不得 panic**、不使用 `unsafe`；截断/随机字节只返回 `Err`。 |
| NFR-03 | 交付物运行时不依赖 JDK/JRE。 |
| NFR-05 | 可移植：Linux/macOS/Windows 行为一致，核心库不引入平台相关依赖。 |
| NFR-06 | 测试离线可跑：基准数据随仓库提交，不联网。 |
| §5 OUT | 不做 `Externalizable` 语义、动态代理调用语义、`TC_CLASS` 类加载语义、RMI/JNDI、完整 gadget 沙箱；P9-1/2 仅做 `TC_CLASS`/`TC_PROXYCLASSDESC` 的字节与句柄保真回放。 |

范围外的请求不要"顺手实现"；需要扩展时先改 `requirements.md` 并让用户确认。

---

## 4. 目录地图

```
AGENT.md                     # 本文件（仓库根）
README.md / SECURITY.md      # 快速开始、安全边界
CHANGELOG.md                 # 版本变更
fuzz/                        # cargo-fuzz 目标（独立 workspace）
deny.toml / .github/         # 依赖审计与 CI matrix/nightly
dsh-session-*/               # 本地 DSH 日志（已 ignore，不入库）
docs/
├── requirements.md          # 要做什么：FR/NFR/AC/范围外/已定案问题
├── design.md                # 怎么做：决策 D1–D5、协议要点、模块、里程碑、风险
├── implementation-plan.md   # P0–P9 任务拆分、门禁、§20/§21 缺口 backlog
├── bench.md / bench-baseline.json / bench_gate.py  # 性能基线
├── devlog.md                # 每次提交的开发日志（§10.2）
└── testdata/
    ├── README.md            # 基准数据的用途、用法、覆盖表
    ├── manifest.json        # 机读契约（43 例，含期望语义值 + sha256）
    ├── fixtures/
    │   ├── codec/           # 01–12：无类描述符的流原语
    │   └── object/          # 20–49：类描述符/SUID/继承/enum/数组/集合/环/reset/扩展类型
    ├── gen/
    │   ├── Fixtures.java              # 权威生成器（JDK 17+）
    │   ├── build_codec_fixtures.py    # codec 层独立再推导，用于对拍
    │   ├── build_manifest.py          # 生成/校验 manifest
    │   ├── Verify.java                # Rust 写 → Java 读回校验
    │   ├── interop.sh                 # Rust ↔ Java 互操作检查
    │   └── gen.sh                     # 编排入口
    └── build/               # 临时产物，已在 .gitignore，不要提交
```

---

## 5. 环境与验证命令

依赖：**JDK 17+**（仅 `docs/testdata/gen/` 需要）、`python3`、Rust 工具链（`cargo`）。

```bash
# 校验基准数据没被改动 / 生成器与 JDK 输出一致（CI 同款，只读）
cd docs/testdata && ./gen/gen.sh --check

# 重新生成 fixtures 与 manifest（仅当确实要新增/修改用例）
cd docs/testdata && ./gen/gen.sh
```

Rust 侧命令（骨架建好后必须保持可用）：

```bash
cargo test --workspace                 # 全部测试，含 fixture 驱动用例
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check                       # 依赖/许可证/来源审计（deny.toml）
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps   # 文档无警告
cargo fmt --all -- --check
cargo test -p jser --test codec        # 只跑 codec 12 例
cargo test -p jser --test reader       # StreamReader 单元契约
cargo test -p jser --test mutf8        # 修改版 UTF-8 契约
cargo test -p jser --test roundtrip_bytes # 42 例逐字节回放（50 除外）
cargo test -p jser --test typed       # typed 转换
cargo test -p jser --test allowlist   # 默认严格 allowlist
cargo test -p jser --test derive      # derive 互通
cargo test -p jser --test mutation    # 7 例变异/构造流（标量/Map/数组）
cargo test -p jser --test property    # 8 例属性测试（固定种子 xorshift）
cargo test -p jser --test builtin     # Date/BigInteger/java.time 语义
cargo test -p jser-serde              # serde 桥
cargo test -p jser --test nested      # 三层嵌套对象
cargo test -p jser --test exception   # TC_EXCEPTION 结构化错误
cargo test -p jser --test fuzz_smoke  # 离线 fuzz 冒烟
cargo +nightly fuzz run decode -- -max_total_time=1800  # 30 分钟 fuzz（需 cargo-fuzz；nightly CI 自动执行）
cargo run --release -p jser --example bench -- 100 # 100MB 级 bench
cargo run --release -p jser --example bench -- array 25000000  # 25M int[] 内存/吞吐（AC-06）
python3 docs/bench_gate.py             # bench 回归门禁（CI nightly 同款）
cargo run -p jser --bin suid -- path/to/Foo.class # 默认 SUID 计算
cargo test -p jser --doc              # rustdoc 示例
cargo run -p jser --example emit       # 生成 build/rust/*.ser
cargo run --release -p jser --example bench -- array 25000000 300
cargo deny check                       # 依赖审计（需安装 cargo-deny）
docs/testdata/gen/interop.sh           # Rust 写 → Java 读回（需 JDK）
```

要求：

- 提交前跑 `docs/testdata/gen/gen.sh --check` + `cargo test`；两者都必须全绿。
- 测试**从 `manifest.json` 数据驱动**（路径基于 `CARGO_MANIFEST_DIR` 定位 `docs/testdata`），不要把期望值复制进 Rust 代码。
- 不新增需要网络的测试或依赖。

---

## 6. 计划中的 Rust 结构（`design.md` §5）

```
jser/
├── Cargo.toml                # workspace
├── crates/
│   ├── jser/                 # 核心库：wire 读写 + JValue + Registry
│   │   ├── src/wire/         # StreamReader/Writer、BlockBuffer、Mutf8、HandleTable
│   │   ├── src/desc/         # ClassDesc 解析/生成、SUID 计算
│   │   ├── src/value/        # JValue、JObject、Graph、Rust 类型转换
│   │   ├── src/typed.rs      # FromJValue/IntoJValue、typed 编解码
│   │   ├── src/java.rs       # JavaSerializable/JavaField
│   │   ├── src/suid.rs       # computeDefaultSUID + .class 解析
│   │   ├── src/bin/suid.rs   # serialver 对拍工具
│   │   ├── src/allowlist.rs  # 默认严格类白名单
│   │   ├── src/builtin/      # 集合布局 + semantic.rs
│   │   └── examples/         # emit / bench
│   ├── jser-derive/          # #[derive(JavaSerializable)]（proc-macro）
│   └── jser-serde/           # schema 驱动 serde 桥
└── tests/                    # 对拍、fuzz 目标、bench
```

设计约束：

- 核心数据模型是**句柄化动态图 + arena 句柄表**（`JValue` + `Handle(u32)` + `Graph`）：引用型值（`String`/数组/对象/enum）统一走 `Handle`，`classDesc` 也占句柄；primitive 数组按 wire 大端字节存 raw（`PrimitiveArray`），`as_array` 只返回对象数组；内部 `Handle` 单调递增，`TC_RESET` 只清 wire 映射。不要用 `Rc<RefCell<..>>` 建环（泄漏且与 serde/并发冲突）。
- 写出走 `JavaSerializable` trait + `JavaSchema`，**不要**试图只用 serde derive 表达 `serialVersionUID` / 字段类型码 / `classDescFlags` / 集合头部。derive 支持嵌套自定义字段（`DESCRIPTOR`）、`Option<Custom>`、`Vec<Custom>`（`ARRAY_DESCRIPTOR`，数组 SUID 自动计算）、继承（`#[java(flatten)]`）与 unit-only enum（`TC_ENUM`、flags 0x12、SUID 0）。
- serde 只做单根/多根、无环、无回指、已知 POJO 的便利层；typed 路径由 `direct` 模块直连 `JValue`（无 JSON 中间层，`from_slice_roots` 除外，它按设计返回 JSON）；字段可经 `SchemaField::nested` 递归嵌套，并支持 Map/Set/枚举字段与 Map/Set/枚举根；多根与 `TC_RESET` 用 `MultiRootWriter`/`from_slice_multi`；共享引用按值展开、环不在树模型内（真实身份/环用动态 `Graph`）；仍不支持对象数组与未知类，超范围直接报错。
- 已知类走注册表 + 特例 codec；未知类在 allowlist 放行时降级为 `JValue::Obj` 并支持原样回写。
- 错误用 `thiserror` 枚举，至少区分：协议错误 / 不支持（**带类名**）/ 结构不匹配。
- 解析器用迭代式而非深递归，并显式限制最大深度、最大元素数、最大流大小。

---

## 7. 协议踩坑清单（实现前必读）

以下每一条都有 fixture 证据，不要在实现里"按直觉"改掉：

| # | 事实 | 依据 |
|---|---|---|
| 1 | 基本类型一律**大端**。 | 协议 |
| 2 | classdata 里的基本类型字段是**裸字节**，前面没有 `TC_BLOCKDATA` 头；只有自定义 `writeObject` 里的 `writeInt/writeUTF` 才进块缓冲。 | `20`、`21`、`23` |
| 3 | 字段顺序 = **先基本类型（按名升序），再对象/数组（按名升序）**；是分组内排序，不是全局排序。 | `21`（`a` 排最后） |
| 4 | 字符串是**修改版 UTF-8**：`U+0000` → `C0 80`；补充平面字符写成 6 字节代理对；≤65535 用 `TC_STRING`，超出用 `TC_LONGSTRING` + 8 字节长度。 | `01`–`06`、`12` |
| 5 | 每个 `TC_CLASSDESC`/`TC_STRING`/数组/对象依次占一个句柄；对象句柄在读完 `classDesc`、读 classdata **之前**分配。 | 协议 + `29` |
| 6 | 同类 `classDesc` 只写一次，之后用 `TC_REFERENCE` 回指。 | `29` |
| 7 | 类名两种写法并存：`classDesc.className` 用点号；字段的 `className1` 用 JVM 描述符（`Ljava/lang/String;`、`[I`）。 | 协议 |
| 8 | 数组类名用点号（`[Ljava.lang.String;`），且**数组 SUID 非 0**（`[I` = `0x4DBA602676EAB2A5`），不能假设为 0。 | `25` |
| 9 | `SC_WRITE_METHOD`：先写字段，再写自定义数据，最后 `TC_ENDBLOCKDATA`。 | `23` |
| 10 | `TC_RESET` 重置句柄表，之后句柄从 `0x7E0000` 重新分配，类描述符也要重写。 | `10`、`35` |
| 11 | `HashMap` classdata = `loadFactor` + `threshold`（裸字节），随后 `TC_BLOCKDATA 08` + `capacity` + `size` + 逐条 key/value + `TC_ENDBLOCKDATA`；`HashSet` 是 capacity + loadFactor + size + 元素。 | `29`、`31` |
| 12 | `transient` 字段不出现在 classDesc；`enum` flags = `0x12`、SUID = 0，常量名以 `TC_STRING` 结尾。 | `37`、`24` |
| 13 | 未声明 SUID 由 JDK 的 `computeDefaultSUID` 算出（`Fixtures$NoSuid` = `-3559046683250708543`），实现须与 `build/suid.txt` 对拍。 | `39` |
| 14 | `java.time.*` 经 `writeReplace` 变成 `java.time.Ser` 代理（flags `0x0C` = Externalizable + block，0 字段，数据在块里）；`Date`/`BigInteger`/`BigDecimal` 有自定义布局。 | `36` |
| 15 | `TC_STRING` 与 `TC_LONGSTRING` 的头部选择要保留（非规范流可能短串用 long）；`Slot::Str.long` 记录，新建字符串按修改版 UTF-8 字节长度（非 Rust `len()`）选头。 | `06`、`design.md` §4.2 第 3 条 |
| 16 | `TC_CLASSDESC` 的句柄在写字段描述符（`className1` 字符串）**之前**分配，故 `34` 的 Node 描述符是 `0x7E0000`，两个 `className1` 是 `0x7E0001/2`，对象是 `0x7E0003`。 | `34` |
| 17 | classdata 按**父 → 子**顺序；每层字段后紧跟该层自定义块（`LinkedHashMap.accessOrder` 在 HashMap entries 块之后）。 | `22`、`30` |
| 18 | 字段的 `className1` 本身占 wire 句柄；重放必须沿用解码时的字符串句柄（`FieldDesc.class_name_handle`），否则句柄编号漂移。 | `21`、`34` |
| 19 | primitive 数组按 wire 大端字节存储（4 B/`int`），`as_array` 只返回对象数组；`as_primitive_array` 提供 `PrimitiveArray` 视图，typed/derive/serde 走 raw 路径。 | `25`、`27` |

`TC_EXCEPTION` 解析异常对象并返回 `Error::JavaException`（class/message/字段摘要）；异常类解析临时放宽 allowlist，深度/长度上限始终生效。

---

## 8. 开发流程（每个任务照此执行）

1. **定位需求**：找到对应的 `FR-*` / `AC-*` 与里程碑（`design.md` §6），确认不在 OUT 范围内。
2. **先写失败测试**：从 `manifest.json` 取用例，或为尚未覆盖的协议细节新增一个 fixture + 期望值。新增 fixture 必须同步：`Fixtures.java` → `docs/testdata/gen/gen.sh`（会重算 manifest 与 size/sha256）。
3. **最小实现**：按 §6 的结构落位；改协议行为时同步更新 `design.md` §4.2/§4.3。
4. **验证**：`docs/testdata/gen/gen.sh --check`、`cargo test --workspace`、`cargo clippy -D warnings`、`cargo fmt --check`。
5. **自检**：解码路径有没有 `unwrap`/`expect`/索引越界/`unsafe`？新代码是否引入了平台相关或需要网络的依赖？
6. **提交**：先在 `docs/devlog.md` 追加本次条目（§10.2）→ `git status --short` 确认范围 → 按路径 `git add`（含日志）→ `git commit -m "<type>: <简述>"`。一个逻辑变更一个提交；自己负责的改动必须已提交。
7. **报账**：说明改了哪些文件、哪几条 `AC-*` 现在满足、哪些仍未覆盖，以及本次提交 hash。

里程碑顺序：M1 读通 fixture → M2 写 + 双向对拍 → M3 动态 POJO/环/`TC_RESET` → M4 derive + SUID → M5 内置类/serde → M6 加固（fuzz、bench）。**不要跳过 M1 直接做 M5。**

P3 的逐字节回放门禁（design §5.1）**只对未修改的 fixture 流（当前 42 例可回放，`object/50` 除外）**；修改/构造过的图必须用 Java 读回验证，别把回放门禁当成 `AC-02`。回放门禁的例外要登记"文件+原因+负责人+到期"。

### 8.1 TDD 循环（强制）

每个行为按 **Red → Green → Refactor** 推进：

1. **Red**：先写会失败的测试。测试从 `manifest.json`/fixture 或公开 API 契约出发，不照抄实现；运行 `cargo test` 确认失败原因正确。
2. **Green**：写最小实现让测试通过；不提前加入没有被测试覆盖的功能。
3. **Refactor**：在测试全绿下整理结构、命名与错误信息，然后重跑 §5 的全部检查。
4. **提交**：测试与实现放在同一个提交（或先提交已通过的测试基线，再提交实现）；**不要在 `main` 上留红的提交**。提交信息用 `test:`/`feat:` 并注明 TDD。
5. **修 bug**：先补一个能复现的失败测试，再修；禁止只改实现不加回归测试。

---

## 9. 测试要求（对应 `AC-*`）

| AC | 测试形态 |
|---|---|
| AC-01 | 数据驱动：遍历 `manifest.json`，逐个 fixture 断言解码结果与 `expect` 一致 |
| AC-02 | Rust 编码 → JDK 校验程序读回（脚本在 `docs/testdata/gen/` 或 CI 中；本机无 JDK 时该测试可 `#[ignore]`，CI 必须跑） |
| AC-03 | 共享/循环用例往返后结构不变（按句柄判等，不按展开值） |
| AC-04 | fuzz 30 分钟无 panic/OOM（`cargo-fuzz` 目标纳入仓库） |
| AC-05 | 未知类解码不整体失败，且再编码字节等价 |
| AC-06 | 大输入（100 MB 级）解码成功，内存峰值 < 3× 输入（bench/回归） |

单元测试之外，**每个协议常量/边界至少一条用例**；新增字节布局用例必须进 `manifest.json`（由生成器写，不手写）。

---

## 10. 文档与代码约定

- 文档用**中文**；标识符、错误信息用**英文**（错误信息里必须带 Java 类名，便于定位）。
- 代码注释解释"为什么"（尤其与 JDK 行为对齐的原因），不写"是什么"的废话。
- 公开 API 加 rustdoc；crate 级文档说明它与 Java 的映射关系与安全默认值。
- 改动需求或协议事实时，同步更新：`requirements.md` 的追踪矩阵/变更记录、`design.md` 的对应章节、`testdata/README.md` 的覆盖表（若新增用例）。
- 不提交 `docs/testdata/build/`、`target/`；不提交任何自动生成的 `.class`。

### 10.1 Git 提交规则（强制）

- **每次改动都必须提交**：完成一个任务或一个逻辑变更后立即 `git commit`；任务结束时，自己产生的改动不得留在工作区，`git status --short` 不应列出它们。
- **一个逻辑变更一个提交**：不要把无关的文档、代码、fixture 混在同一个提交里；大改动拆成多个可回滚的提交。
- **提交前先验证**：按 §5 跑对应检查。改了 `docs/testdata/` 必须按 §5 跑 `docs/testdata/gen/gen.sh --check`；改了代码必须 `cargo test`、`cargo clippy`、`cargo fmt --check`。
- **提交信息**：`<type>: <简述>`，`type` 用 `docs` / `feat` / `fix` / `test` / `refactor` / `chore`；正文可写原因与验证结果。
- **只提交自己的改动**：工作区若有无关的未提交文件，不要 `git add -A` / `git add .`，按路径显式添加；无法判断时先问。
- **fixture 变更要成组提交**：`.ser`、`manifest.json`、`Fixtures.java` / 生成脚本、`testdata/README.md` 同一个提交，保证可复现。
- **不要改写已汇报的历史**：不 `git commit --amend` 已汇报的提交、不 force-push；修正用新提交。
- **收尾要报 hash**：在最终答复里给出本次提交的 hash 与工作区状态。

### 10.2 提交日志（强制）

每次 `git commit` 前，在 `docs/devlog.md` 追加一条记录：

- 日期、类型、提交标题（与 commit message 一致）
- 主要改动（文件/模块）
- 验证命令与结果（测试数、clippy/fmt/gen.sh）
- TDD 的 Red / Green 内容（若适用）

日志与本次改动放在**同一个提交**里。日志条目不写自身 hash（避免自指）：历史条目可写 hash，当前条目写“本次”，完整 hash 以 `git log` 为准。顺序是：先更新日志，再 `git add` 日志和改动，最后提交。

---

## 11. 明确不要做

- 不引入 JNI、`java` 子进程、或任何运行时 JVM 依赖。
- 不为了"跑通"而放宽断言、跳过 fixture、手改 `.ser`/sha256。
- 不实现 OUT 表里的特性（`Externalizable` 语义、代理、`TC_CLASS`、RMI/JNDI、gadget 沙箱）。
- 不把 serde 做成能处理环/共享的"半成品"。
- 不添加未在 §5 声明的重量级依赖（async runtime、平台特定库）；新增依赖先说明理由。
- 不在没有 fixture 证据的情况下"修正"协议细节。
- `requirements.md` §6 / `design.md` §8 的目标 Java 版本、serde 形式、发布与性能数值**已定案（2026-11-04）**：Java 17 基线、schema 驱动 serde、暂不发布、≥1 GB/s 且 <3× RSS（nightly bench 门禁）。**allowlist 默认严格**：`java.lang.*`、`java.util.*`；放宽用 `Allowlist::all()` 并警告。写方向是基线 `FR-02 MUST`。

---

## 12. 完成任务前的检查清单

- [ ] 改动落在正确的 crate/模块，未新建未声明的结构
- [ ] `docs/testdata/gen/gen.sh --check` 通过（改了用例则已重新生成并提交 manifest）
- [ ] 新增行为先有失败测试（或 bug 的回归测试），实现后 `cargo test --workspace` 全绿
- [ ] `cargo clippy -D warnings`、`cargo fmt --check` 通过
- [ ] 解码路径无 panic 风险，无 `unsafe`，深度/长度上限生效
- [ ] 文档（`design.md` / `requirements.md` / `testdata/README.md`）按需同步
- [ ] `docs/devlog.md` 已追加本次提交条目，并与改动一起提交
- [ ] 已提交（一个逻辑变更一个 commit），`git status --short` 干净，报告含 commit hash
- [ ] 报告里写明满足/未满足的 `AC-*` 与遗留风险
