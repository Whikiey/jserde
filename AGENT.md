# AGENT.md — 本仓库的编码须知

给在本仓库工作的 AI 编码 Agent（以及人类贡献者）的操作说明。**开始任何改动前先读本文件**；它与 `docs/` 冲突时，以第 2 节的事实来源顺序为准。

一句话：用**纯 Rust** 实现 Java 对象序列化协议，与 `ObjectOutputStream` / `ObjectInputStream` 双向字节级互通，运行时不依赖 JVM。

---

## 1. 当前状态（2026-11-03）

| 项 | 状态 |
|---|---|
| 文档 | 已有 `docs/requirements.md`（需求基线，Draft）、`docs/design.md`（设计，Draft） |
| 实现计划 | 已有初版 `docs/implementation-plan.md`（P0–P7 任务拆分、测试矩阵、CI 门禁；计划假设待确认，设计缺口已定案） |
| 基准数据 | **已完成**：32 个用例、74559 字节，JDK 17.0.20.1 生成，见 `docs/testdata/` |
| Rust 代码 | **尚不存在**：没有 `Cargo.toml`、没有 `src/`、没有 CI 配置 |
| 版本控制 | 当前目录**不是 git 仓库** |

所以：第一步是搭 workspace 骨架并让解码器读通 fixture（里程碑 M1），不是"重构现有代码"。

---

## 2. 事实来源与优先级（冲突时严格按此顺序）

1. `docs/testdata/fixtures/**/*.ser` 的**字节本身** —— 唯一事实来源。
2. `docs/testdata/manifest.json` —— 用例编号、文件、覆盖特性、期望语义值、size、sha256。
3. `docs/requirements.md` 的 `FR-*` / `NFR-*` / `AC-*` —— 验收以 `AC-*` 为准。
4. `docs/design.md` 的协议说明（§4.2、§4.3）—— 实现路径。
5. 其它一切（JDK 文档记忆、同类库、本文件）。

规则：

- **文档与字节冲突 → 改文档，不改 fixture。** 在 `design.md` §4.3 增加一行"事实 + 依据文件"，并在 §8 变更记录里记一笔。
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
| §5 OUT | 不做 `Externalizable` 语义、动态代理（`TC_PROXYCLASSDESC`）、`TC_CLASS` 完整语义、RMI/JNDI、完整 gadget 沙箱。 |

范围外的请求不要"顺手实现"；需要扩展时先改 `requirements.md` 并让用户确认。

---

## 4. 目录地图

```
docs/
├── requirements.md          # 要做什么：FR/NFR/AC/范围外/待确认问题
├── design.md                # 怎么做：决策 D1–D5、协议要点、模块、里程碑、风险
├── AGENT.md                 # 本文件
└── testdata/
    ├── README.md            # 基准数据的用途、用法、覆盖表
    ├── manifest.json        # 机读契约（32 例，含期望语义值 + sha256）
    ├── fixtures/
    │   ├── codec/           # 01–12：无类描述符的流原语
    │   └── object/          # 20–39：类描述符/SUID/继承/enum/数组/集合/环/reset
    ├── gen/
    │   ├── Fixtures.java              # 权威生成器（JDK 17+）
    │   ├── build_codec_fixtures.py    # codec 层独立再推导，用于对拍
    │   ├── build_manifest.py          # 生成/校验 manifest
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
cargo fmt --all -- --check
cargo test -p jser --test decode       # 只跑解码对拍（测试文件建好后）
```

要求：

- 提交前跑 `./gen/gen.sh --check` + `cargo test`；两者都必须全绿。
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
│   │   └── src/builtin/      # HashMap/HashSet/ArrayList/Date/BigInteger...
│   ├── jser-derive/          # #[derive(JavaSerializable)]（proc-macro）
│   └── jser-serde/           # 可选桥，feature 默认关闭
└── tests/                    # 对拍、fuzz 目标、bench
```

设计约束：

- 核心数据模型是**句柄化动态图 + arena 句柄表**（`JValue` + `Handle(u32)` + `Graph`）：引用型值（`String`/数组/对象/enum）统一走 `Handle`，`classDesc` 也占句柄；内部 `Handle` 单调递增，`TC_RESET` 只清 wire 映射。不要用 `Rc<RefCell<..>>` 建环（泄漏且与 serde/并发冲突）。
- 写出走 `JavaSerializable` trait + `JavaSchema`，**不要**试图只用 serde derive 表达 `serialVersionUID` / 字段类型码 / `classDescFlags` / 集合头部。
- serde 只做单根、无环、无回指、已知 POJO 的便利层；遇到 `TC_REFERENCE`、`Reset`、多元素流或未知类直接报错，不做半成品支持。
- 已知类走注册表 + 特例 codec；未知类降级为 `JValue::Obj` 并支持原样回写。
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

`TC_EXCEPTION` 读到即报 `Unsupported`，但错误里保留原始类名。深度/长度上限始终生效。

---

## 8. 开发流程（每个任务照此执行）

1. **定位需求**：找到对应的 `FR-*` / `AC-*` 与里程碑（`design.md` §6），确认不在 OUT 范围内。
2. **先写失败测试**：从 `manifest.json` 取用例，或为尚未覆盖的协议细节新增一个 fixture + 期望值。新增 fixture 必须同步：`Fixtures.java` → `./gen/gen.sh`（会重算 manifest 与 size/sha256）。
3. **最小实现**：按 §6 的结构落位；改协议行为时同步更新 `design.md` §4.2/§4.3。
4. **验证**：`./gen/gen.sh --check`、`cargo test --workspace`、`cargo clippy -D warnings`、`cargo fmt --check`。
5. **自检**：解码路径有没有 `unwrap`/`expect`/索引越界/`unsafe`？新代码是否引入了平台相关或需要网络的依赖？
6. **报账**：说明改了哪些文件、哪几条 `AC-*` 现在满足、哪些仍未覆盖。

里程碑顺序：M1 读通 fixture → M2 写 + 双向对拍 → M3 动态 POJO/环/`TC_RESET` → M4 derive + SUID → M5 内置类/serde → M6 加固（fuzz、bench）。**不要跳过 M1 直接做 M5。**

P3 的逐字节回放门禁（design §5.1）**只对未修改的 32 个 fixture 流**；修改/构造过的图必须用 Java 读回验证，别把回放门禁当成 `AC-02`。回放门禁的例外要登记"文件+原因+负责人+到期"。

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

---

## 11. 明确不要做

- 不引入 JNI、`java` 子进程、或任何运行时 JVM 依赖。
- 不为了"跑通"而放宽断言、跳过 fixture、手改 `.ser`/sha256。
- 不实现 OUT 表里的特性（`Externalizable` 语义、代理、`TC_CLASS`、RMI/JNDI、gadget 沙箱）。
- 不把 serde 做成能处理环/共享的"半成品"。
- 不添加未在 §5 声明的重量级依赖（async runtime、平台特定库）；新增依赖先说明理由。
- 不在没有 fixture 证据的情况下"修正"协议细节。
- 不擅自决定 `requirements.md` §6 / `design.md` §8 里仍待用户拍板的问题（目标 Java 版本、serde 形式、crates.io 发布、性能数值、allowlist 默认严格还是宽松）——阻塞时提问，不阻塞时按里程碑 M1→M2 推进。注意：写方向已是基线 `FR-02 MUST`，默认双向实现，不再当开放问题。

---

## 12. 完成任务前的检查清单

- [ ] 改动落在正确的 crate/模块，未新建未声明的结构
- [ ] `./gen/gen.sh --check` 通过（改了用例则已重新生成并提交 manifest）
- [ ] `cargo test --workspace` 全绿，新增行为有用例
- [ ] `cargo clippy -D warnings`、`cargo fmt --check` 通过
- [ ] 解码路径无 panic 风险，无 `unsafe`，深度/长度上限生效
- [ ] 文档（`design.md` / `requirements.md` / `testdata/README.md`）按需同步
- [ ] 报告里写明满足/未满足的 `AC-*` 与遗留风险
