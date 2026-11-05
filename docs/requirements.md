# 0006 · 需求基线（Requirements）

- 状态：Accepted（2026-11-04；§6 已定案）
- 关联想法：`ideas.md` #6
- 下游设计：[design.md](design.md)
- 测试基准：[testdata/](testdata/)
- 日期：2026-11-03
- 最近修订：2026-11-05

需求文档回答"**要做什么**"，设计文档回答"**怎么做**"。两者分工明确：验收以本文的 `AC-*` 为准，实现路径以设计文档为准。

---

## 0. 原始需求（逐字引用）

> **Rust Java 序列化**: 用 Rust 实现 Java 序列化（与 Java 的 ObjectOutputStream/ObjectInputStream 兼容），需包含字典到字典的转换（Java Map ↔ Rust 字典/Map）。

从这句话可以拆出**三条硬性要求**，后续所有需求都必须能追溯到它们：

| 编号 | 硬性要求 | 含义 |
|---|---|---|
| S-1 | 「用 Rust 实现」 | 纯 Rust 实现，**运行时不依赖 JVM**；不是 JNI 绑定、不是调外部进程 |
| S-2 | 「与 ObjectOutputStream/ObjectInputStream 兼容」 | **双向字节级互通**：Java 写→Rust 读、Rust 写→Java 读都能正确还原 |
| S-3 | 「包含字典到字典的转换」 | Java `Map` ↔ Rust `Map` 是**必须项**，不是可选项；这也是 MVP 的验收核心 |

---

## 1. 编号规则

- `FR-xx` 功能需求，`NFR-xx` 非功能需求，`AC-xx` 验收标准，`OUT-xx` 明确的范围外。
- 每条需求标注**来源**：`原文` = 直接来自 `ideas.md` #6；`推导` = 为可用性/可测性补充，**需要你确认后才算基线**。
- 优先级：`MUST` / `SHOULD` / `MAY`。

---

## 2. 功能需求

| 编号 | 需求 | 优先级 | 来源 |
|---|---|---|---|
| FR-01 | **解码**：读取 `ObjectOutputStream` 产生的字节流（含 `AC ED 00 05` 流头校验），还原成 Rust 侧的数据结构 | MUST | 原文（S-2） |
| FR-02 | **编码**：Rust 侧数据写成字节流，Java `ObjectInputStream` 能无损还原 | MUST | 原文（S-2） |
| FR-03 | **标量与字符串**：`boolean/byte/char/short/int/long/float/double`、`String`（含空串、`null`、中文、emoji 等补充平面字符、超 65535 字节）；含孤立 UTF-16 代理项的 `String` 无法表示为 Rust `String`，解码返回错误（不静默替换） | MUST | 原文（S-2） |
| FR-04 | **数组**：基本类型数组、对象数组、多维数组；元素可为 `null` | MUST | 推导 |
| FR-05 | **对象**：任意 `Serializable` 类（类名 + 字段表自描述），含继承链、`transient` 字段跳过、自定义 `writeObject`（`SC_WRITE_METHOD`）、`enum` | MUST | 原文（S-2） |
| FR-06 | **字典映射**：Java `Map` ↔ Rust `Map`（`HashMap`/`LinkedHashMap` 等），键值任意可序列化类型 | MUST | **原文（S-3）** |
| FR-07 | **集合映射**：Java `List` ↔ Rust `Vec`、Java `Set` ↔ Rust `HashSet` | MUST | 推导（由 S-3 的"字典"扩展而来） |
| FR-08 | **引用语义**：同一对象多次出现只写一次（`TC_REFERENCE` 回指）；循环引用（自引用链表、双向图）可读写且不丢结构 | MUST | 推导 |
| FR-09 | **未知类降级**：Rust 没有对应定义的 Java 类，不应导致整体解码失败，而是降级为通用对象结构（保留类名与字段原样），并能**原样再编码回去** | SHOULD | 推导 |
| FR-10 | **错误诊断**：区分"协议错误 / 不支持的类 / 结构不匹配"三类；错误信息包含类名，便于定位 | SHOULD | 推导 |
| FR-11 | **资源上限**：可配置最大嵌套深度、最大元素数、最大流大小，防止恶意流导致栈溢出/内存耗尽 | SHOULD | 推导 |
| FR-12 | **`null` 与缺省**：Java `null` ↔ Rust `Option::None`；无此字段时给出明确错误而非静默丢数据 | MUST | 推导 |
| FR-13 | **内置常见类**（可选扩展）：`java.util.Date`、`BigInteger`、`BigDecimal`、`java.time.*`（`java.util.Optional` 不是 `Serializable`，不纳入） | MAY | 推导 |
| FR-14 | **派生映射**（可选扩展）：用 derive 宏把 Rust 结构体绑定到 Java 类名/字段/SUID，免去手写映射 | MAY | 推导 |
| FR-15 | **serde 集成**（可选扩展）：`Serialize`/`Deserialize` 直接产出/消费 Java 流 | MAY | 推导 |

### 2.1 类型映射表（需求级约定）

| Java | Rust | 备注 |
|---|---|---|
| `boolean` / `Boolean` | `bool` | |
| `byte` / `Byte` | `i8` | 无符号语义需显式转换 |
| `char` / `Character` | `char`（或 `u16`） | Java `char` 是 UTF-16 码元，代理项非合法 `char`，需保留 `u16` 通路 |
| `short` / `Short` | `i16` | |
| `int` / `Integer` | `i32` | |
| `long` / `Long` | `i64` | |
| `float` / `Float` | `f32` | |
| `double` / `Double` | `f64` | |
| `String` | `String` | 需处理修改版 UTF-8；含孤立代理项的字符串解码报错 |
| `null` | `Option::None` / `JValue::Null` | |
| `Map<K,V>` | `HashMap<K,V>` / `BTreeMap<K,V>` | 迭代顺序无关，写回时不保证与 Java 原序一致 |
| `List<T>` | `Vec<T>` | |
| `Set<T>` | `HashSet<T>` | |
| `T[]` | `Vec<T>` / `[T; N]` | |
| 任意 POJO | 结构体（derive）或 `JValue::Obj` | 未知类走 `JValue::Obj` |

> `java.util.Optional` 未实现 `Serializable`，Java 端不会产出它的流；不作为内置类支持。

---

## 3. 非功能需求

| 编号 | 需求 | 优先级 | 来源 |
|---|---|---|---|
| NFR-01 | **健壮性**：任意输入不得 panic；解码路径不使用 `unsafe`；恶意/截断/随机字节只返回错误 | MUST | 推导 |
| NFR-02 | **性能**：大块/primitive 数组解码吞吐 ≥ **1 GB/s**（release 实测约 3.5–6 GB/s），单流 ≥ 100 MB 可用，内存峰值不超过输入规模的 3 倍 | SHOULD | 推导 |
| NFR-03 | **零 JVM 依赖**：产出库在运行时不需要 JDK/JRE；JDK 仅用于生成测试基准与互通校验 | MUST | 原文（S-1） |
| NFR-04 | **默认安全**：解码默认只放行白名单类；放宽需显式配置，并在文档中警告 gadget 风险 | SHOULD | 推导 |
| NFR-05 | **可移植**：Linux/macOS/Windows 行为一致；核心库不引入平台相关依赖 | MUST | 推导 |
| NFR-06 | **可复现测试**：测试基准数据随仓库提交，离线即可跑全部用例 | SHOULD | 推导 |

---

## 4. 验收标准

| 编号 | 验收标准 | 对应需求 |
|---|---|---|
| AC-01 | 全部基准数据（`testdata/fixtures/**`）解码结果与 `testdata/manifest.json` 中的期望值一致 | FR-01/03/04/05/06/07 |
| AC-02 | Rust 写出的字节流被官方 Java 校验程序读回，语义与原值等价（双向） | FR-02 |
| AC-03 | 循环引用、对象共享的用例往返后结构不变（不膨胀、不丢引用） | FR-08 |
| AC-04 | 用随机/截断/变异字节 fuzz 30 分钟无 panic、无 OOM | NFR-01 |
| AC-05 | 未知类的用例不整体失败，且能原样再编码 | FR-09 |
| AC-06 | 100 MB 级输入在配置上限内成功解码，内存峰值 < 3× 输入 | NFR-02 |

> 逐字节回放（`encode_stream(decode_stream(x)) == x`，限 fixture 未修改流（当前 43 例；`object/50` 按错误处理））是 `design.md` §5.1 / 计划 P3 的**内部回归门禁**，不替代 `AC-02`，也不作为验收标准；构造或修改过的流只按 `AC-02` 验收。

### 4.1 当前实现状态（2026-11-05）

| AC | 状态 | 证据/缺口 |
|---|---|---|
| AC-01 | ✅ | 43 个 fixture 全部可处理（42 个正常解码，`50` 返回结构化异常）；`36`/`40`/`42`/`43` 语义、`41` 三层嵌套、`44` 混合/共享/环、`45` 常见值类、`46` 扩展容器、`47` Class 对象、`48` 动态代理、`49` EnumSet/EnumMap、`50` TC_EXCEPTION 由 `tests/builtin.rs`、`tests/nested.rs`、`tests/nesting.rs`、`tests/containers.rs`、`tests/class_objects.rs`、`tests/proxy.rs`、`tests/exception.rs` 覆盖 |
| AC-02 | ✅（常规 fixture） | `tests/interop.rs` + `Verify.java`：42 个常规 fixture 全部由 Rust 重编码后经 Java `ObjectInputStream` 读回（`--read`/`--sequence`/`--primitives`/`--blocks`/`--array`）；`50` 是 fatal 流，不重编码；构造/typed/derive/serde 代表用例断言字段值；范围外类型（`Externalizable` 语义、代理语义等）仍不覆盖 |
| AC-03 | ✅ | `33`/`34`/`35` 读→写→读；`tests/roundtrip_semantics.rs` |
| AC-04 | ✅ | `fuzz/` 三目标 + `tests/fuzz_smoke.rs` + CI smoke；2026-11-06 decode 1801 s / 5,000,272 runs / peak 476 MB，roundtrip 301 s，mutf8 301 s，均 exit 0；nightly schedule 已启用 |
| AC-05 | ✅ | `23`/`36`/`39` 原样回写；`tests/unknown.rs` |
| AC-06 | ✅（本地实测） | 合成 100 MB 块流约 4.8 GB/s、2.0× RSS；25M `int[]` raw 存储本地约 **2.03×**（Windows；Linux `bench -- array` 可复核） |

> 剩余缺口与处理优先级见 `implementation-plan.md` §20。

---

## 5. 明确的范围外（OUT）

| 编号 | 不做 | 原因 |
|---|---|---|
| OUT-01 | `Externalizable` 的 `readExternal/writeExternal` 语义 | 对端逻辑不可知，只能当不透明块；`java.time.Ser` 等已知内置特例走注册表，不属于本项 |
| OUT-02 | 动态代理的调用语义（`InvocationHandler`） | P9-2 已实现 `TC_PROXYCLASSDESC` 字节/句柄保真回放，不做代理调用语义 |
| OUT-03 | RMI / JNDI 传输层 | 与序列化格式无关 |
| OUT-04 | `Class` 对象的类加载语义（`TC_CLASS`） | P9-1 已实现字节/句柄保真回放；Rust 无类加载概念 |
| OUT-05 | 运行时调用 JVM 做转换 | 违反 S-1（零 JVM 依赖） |
| OUT-06 | 完整的反序列化 gadget 沙箱 | 只做白名单 + 文档警告 |

---

## 6. 已定案问题（2026-11-04 收口）

1. **FR-02（写方向）**：双向已交付并通过 P3/P6/P7 回归。
2. **目标 Java 版本**：**Java 17** 基线（fixture 17.0.20.1；wire 自 1.2 未变；JDK 25 重生成逐字节一致）。Java 8+ 类按需支持，旧 JDK 流对受支持类预期可读但非 CI 门禁。
3. **NFR-02 的数值**：大块/primitive 数组解码 ≥1 GB/s、内存 <3×；实测约 3.5–6 GB/s、`int[]` RSS 约 2.03×；nightly bench 门禁按观测值 35% 兜底。
4. **NFR-04 的默认策略**：严格默认，只放行 `java.lang.*`、`java.util.*`；放宽用 `Allowlist::all()` 并警告 gadget 风险。
5. **FR-13/14/15**：typed API、derive、schema 驱动 serde 桥（直连 `JValue`）均已交付；无 schema 的 serde 直出不在范围。
6. **发布**：0.1.0 保持 unreleased，暂不发布 crates.io；首次发布前再做 API 冻结与 semver 评审。

---

## 7. 追踪矩阵（需求 → 设计）

| 需求 | 设计文档章节 | 里程碑 |
|---|---|---|
| FR-01 / FR-03 | §4 协议要点、§5 模块 | M0–M1 |
| FR-02 | §3 D2、§5 | M2 |
| FR-04 / FR-05 | §3 D1、§4.2 第 8 条 | M1、M3 |
| FR-06 / FR-07 | §3 D4（内置类注册表） | M1、M5 |
| FR-08 | §3 D1（句柄表 / arena） | M3 |
| FR-09 | §3 D4（未知类降级） | M3 |
| FR-10 / FR-11 | §3 D5、§4.2 第 13 条 | M6 |
| FR-13 / FR-14 / FR-15 | §3 D2、D3 | M4–M5 |
| NFR-01 | §6 M6（fuzz） | M6 |
| NFR-02 | §6 M6（bench） | M6 |
| NFR-04 | §7 风险表 | M1 起 |
| NFR-06 | `testdata/` | M0 |

---

## 8. 变更记录

| 日期 | 变更 | 来源 |
|---|---|---|
| 2026-11-03 | 建立需求基线：从 `ideas.md` #6 拆出 S-1/S-2/S-3，导出 FR-01～FR-15、NFR-01～NFR-06、AC-01～AC-06 | 本轮 |
| 2026-11-03 | 收口设计缺口相关需求：`Optional` 非 `Serializable` 移出 FR-13；孤立代理项字符串明确报错（FR-03/类型映射表） | implementation-plan §3 |
| 2026-11-03 | 明确逐字节回放是内部回归门禁（design §5.1），不替代 AC-02；OUT-01 补充 `java.time.Ser` 已知特例 | 拍板 1 |
| 2026-11-04 | NFR-04 定案：默认严格 allowlist（`java.lang.*`、`java.util.*`），支持精确名与 `prefix.*`/`Fixtures$*`；数组按元素类检查；默认 `decode_stream` 拒绝未放行类 | 用户拍板 |
| 2026-11-04 | FR-14（derive）落地：`JavaSerializable`/`#[derive(JavaSerializable)]`、typed 互通；同时实现 `computeDefaultSUID` 与 `suid` 工具，`Fixtures$NoSuid` 与 JDK 一致 | P5 |
| 2026-11-04 | FR-13/FR-15 部分落地：`Date`/`BigInteger`/`BigDecimal`/`java.time` 语义；新增 `object/40`（33 例）；`jserde-serde` schema 驱动单根扁平桥，明确拒绝嵌套/环/reset/多根；`Optional` 非 Serializable 不纳入 | P6 |
| 2026-11-04 | P7 加固：panic 审计（核心库禁 unwrap/expect）、fuzz 目标与离线冒烟、100 MB 合成块 bench（约 4.7 GB/s、2.0× RSS）、README/SECURITY/CHANGELOG；AC-06 合成负载通过，AC-04 的 30 分钟 fuzz 待夜间执行 | P7 |
| 2026-11-04 | P8-1：新增 nightly fuzz workflow（decode 1800s、roundtrip/mutf8 300s、corpus 取 fixture、失败上传 artifacts）；AC-04 待首次绿色运行 | P8-1 |
| 2026-11-04 | 暂缓 30 分钟 fuzz：nightly `schedule` 停用、保留手动触发；等功能完成后恢复 | 用户决定 |
| 2026-11-04 | P8-2：primitive 数组改紧凑 raw bytes 存储（`PrimitiveArray` 视图），`as_array` 仅对象数组；typed/derive/serde 走 raw；25M `int[]` 本地工作集采样约 2.03× | P8-2 |
| 2026-11-04 | P8-3：`JavaSerializable::DESCRIPTOR`；derive 支持嵌套自定义字段与 `Option<Custom>`；serde 递归 `SchemaField::nested`；`Vec<Custom>`/继承留待 P9-14/15 | P8-3 |
| 2026-11-04 | P9-14：数组类 SUID 改由默认算法计算（含自定义元素数组），`new_java_array` 支持任意对象数组；`ARRAY_DESCRIPTOR` + derive `Vec<Custom>`；继承留待 P9-15 | P9-14 |
| 2026-11-04 | P9-15：`#[java(flatten)]` 父类字段生成 `super_desc` 与父类 classdata 层；derive 继承与 `object/22` 互通 | P9-15 |
| 2026-11-04 | P9-7/8：serde 支持 Map/Set 字段（HashMap/LinkedHashMap/HashSet）与 Map/Set 根；新增 LinkedHashMap 写侧构造器；仍不支持对象数组/环/TC_RESET/多根 | P9-7/8 |
| 2026-11-04 | P9-16：unit-only enum derive（`TC_ENUM`、flags 0x12、SUID 0、super `java.lang.Enum`），常量名默认 UPPER_SNAKE、支持 `rename`；`object/24` 互通 | P9-16 |
| 2026-11-04 | P8-5：新增 `object/42`（Year/YearMonth/MonthDay/Period）与对应 `BuiltinValue` 语义；manifest 35 例/75078 字节；`build_manifest.py` 支持 Windows 路径 | P8-5 |
| 2026-11-04 | P9-9：serde 支持枚举字段（serde↔Java 常量映射）与枚举根（`to_vec_enum`/`from_slice_enum`） | P9-9 |
| 2026-11-04 | P9-10/11：serde 支持多根流与 `TC_RESET`（`MultiRootWriter`/`from_slice_multi`）；P9-12 定界：共享引用按值展开，环用动态 Graph | P9-10/11 |
| 2026-11-04 | P9-21：新增 `object/43`（ZoneId/ZoneOffset/OffsetTime/OffsetDateTime）与对应 `BuiltinValue` 语义；manifest 36 例/75224 字节 | P9-21 |
| 2026-11-04 | P9-27：`Verify.java` 增加 `--read/--sequence/--primitives/--blocks/--array`；emit 重编码全部 fixture；`tests/interop.rs`/`interop.sh` 逐例 Java 读回（AC-02 全 fixture） | P9-27 |
| 2026-11-04 | P9-13：serde typed 路径改为直连 `JValue`（`direct.rs`），去掉 JSON 中间层；`from_slice_roots` 保留 JSON 视图 | P9-13 |
| 2026-11-04 | P9-28/29：CI 增加 Linux/macOS/Windows matrix、MSRV 1.77、i686 check；`rust-version` 校准为 1.77；新增 `deny.toml` 与 cargo-deny job | P9-28/29 |
| 2026-11-04 | P9-30：新增 bench 基线 JSON 与 `docs/bench_gate.py`，nightly `bench-gate` 校验吞吐 ≥35% 基线、`int[]` RSS <3× | P9-30 |
| 2026-11-04 | P9-31：新增 `set_map_entry`/`set_array_element`；mutation 覆盖全部标量、Map 替换/追加、对象数组；Java 读回 Mixed 三字段；全仓 147 passed | P9-31 |
| 2026-11-04 | P9-32：serde crate 增加 doctest；§6/design §8/plan §18 定案（Java 17、不发布、性能门禁）；CI 增加 `cargo doc -D warnings`；全仓 148 passed | P9-32 |
| 2026-11-04 | P9-33：新增 `object/44`（Map×数组×自定义类、共享引用、自环），manifest 37 例/75710 字节；`tests/nesting.rs` 增加解码/共享/环/回放断言；全仓 149 passed | P9-33 |
| 2026-11-04 | P9-26：新增无依赖固定种子 property 测试（字符串/长字符串/数组/Map/重编码/随机字节/截断/变异）；全仓 157 passed | P9-26 |
| 2026-11-04 | P9-22：新增 `object/45`（UUID/Currency/Locale/BitSet/StringBuilder/StringBuffer/Pattern/java.sql）与 10 种 `BuiltinValue`；manifest 38 例/76637 字节；全仓 158 passed | P9-22 |
| 2026-11-04 | P9-23/24/25：新增 `object/46`（12 种容器，EnumSet/EnumMap 需 TC_CLASS 暂排除）；解码视图 + 11 个构造器；`tests/containers.rs`；manifest 39 例/78106 字节；全仓 160 passed | P9-23/24/25 |
| 2026-11-04 | P9-1：`TC_CLASS` 保真回放（`JValue::Class`/`Slot::Class`，含共享类对象与 `new_class`）；`object/47` + `tests/class_objects.rs`；manifest 40 例/78347 字节；全仓 162 passed | P9-1 |
| 2026-11-04 | P9-2：`TC_PROXYCLASSDESC` 保真回放（接口列表 + super Proxy + `h` 字段）；`object/48` + `tests/proxy.rs`；manifest 41 例/78499 字节；全仓 163 passed | P9-2 |
| 2026-11-04 | P9-23 补齐：EnumSet/EnumMap 解码视图 + `new_enum_set`/`new_enum_map`；`object/49`；manifest 42 例/78843 字节；全仓 164 passed | P9-23 |
| 2026-11-05 | 进度同步：P8/P9 主要功能完成；当前 42 例/78640 字节、164 项测试、42 例 Java 读回；剩余 P9-3/4/5/6/12/17/18/19/20 与 P8-1 fuzz | 跨机同步 |
| 2026-11-05 | `object/46` 移除 `Collections$SynchronizedList`（跨 JDK 构建 SUID 不稳定）并重新生成 manifest；fixture 42 例/78640 字节 | 跨平台修复 |
| 2026-11-05 | P9-19：derive `from_jvalue` 校验类名/SUID；类名或 SUID 不匹配返回 `FieldMismatch`（含两个 SUID）；覆盖 struct/enum/继承回归，全仓 167 项通过 | P9-19 |
| 2026-11-05 | P9-3：新增 `object/50`（TC_EXCEPTION + IOException）；`Error::JavaException` 携带 class/message/字段摘要；异常流的类解析使用临时宽松策略，不回放；全仓 168 项通过 | P9-3 |
| 2026-11-05 | P9-18/P9-20：derive 组合字段 `Option<Vec<T>>`/`Option<HashMap>`/`Vec<Option<T>>`（含自定义数组）；typed `IntoJValue` 支持 `&[T]`/`[T;N]`/`BTreeMap`/`BTreeSet`；全仓 173 项通过 | P9-18/20 |
| 2026-11-05 | P9-4：`Reader<I: Input>`、`ReadStream<R>`、`decode_from`/`encode_to`；解码增量缓冲、写侧暂缓冲整图；全仓 178 项通过。P9-5 零拷贝因 D1 生命周期改造暂缓 | P9-4/5 |
| 2026-11-05 | P9-6：新增 `HashMapLayout` 与 `new_hash_map_entries_with_layout`/`new_hash_map_with_layout`，可精确写 capacity/threshold/loadFactor；`MapView.threshold` 可读；全仓 180 项通过 | P9-6 |
| 2026-11-05 | P9-12（部分）：新增 `jserde_serde::Shared<T>`（`Rc<T>`），单次调用内保留非环共享身份；环检测返回 `FieldMismatch`，完整环仍由 `Graph` 支持；全仓 181 项通过 | P9-12 |
| 2026-11-05 | P9-17：泛型 `#[derive(JavaSerializable)]`；`schema()` 改为返回 `Vec<JavaField>` 以支持单态化字段描述符；拒绝生命周期/const 泛型与泛型 enum；全仓 183 项通过 | P9-17 |
| 2026-11-06 | 范围关闭：P9-5 零拷贝不做 `Graph<'a>` 生命周期改造；P9-12 关闭完整环范围，仅保留非环 `Shared<T>`，完整环继续用动态 `Graph` | 用户决定 |
| 2026-11-06 | P8-4：补 OUT 负例（旧式 Externalizable、非 Serializable classdesc、TC_CLASS/proxy 非法路径、错误 tag）；全仓 193 项通过 | P8-4 |
| 2026-11-06 | P8-6：300 层对象链、深度上限错误、8 线程独立/只读并发测试；全仓 198 项通过 | P8-6 |
| 2026-11-06 | P8-1 / AC-04：启用 nightly schedule；本地 decode 1801s/5,000,272 runs/476MB，roundtrip 301s，mutf8 301s，均 exit 0 | P8-1 |
