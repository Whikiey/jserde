# 开发日志（Dev Log）

记录每次提交的工作内容、验证结果与 TDD 红绿过程。规则见 [AGENT.md §10.2](../AGENT.md)。

- 每次 `git commit` 前追加一条，与改动同一个提交。
- 日志条目不写自身 hash（避免自指）；历史条目已回填 hash，当前条目用"本次"，完整 hash 以 `git log` 为准。
- TDD 条目写清 Red（失败测试）与 Green（最小实现）。

---

## 2026-11-04 16:20 · test: 混合嵌套 fixture（P9-33）

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - `Fixtures.java` 新增 `Hybrid`（`Object[] array`、`HashMap<String,Object> map`、`Hybrid self`、`Point shared`）与 `object/44-hybrid-nested.ser`（486 B）：map 含 Point/int[]/String/ArrayList，数组含同一 Point，`shared` 与 `self` 构成共享与自环。
  - `build_manifest.py` 登记 object/44；manifest 重新生成为 **37 例 / 75710 字节**（原有 36 例逐字节不变）。
  - `tests/nesting.rs` 新增 `decodes_hybrid_fixture_with_map_array_shared_and_cycle`：Map/数组/共享句柄/自环/ArrayList/primitive 数组断言 + 逐字节回放；原有 Rust 构造型混合嵌套用例保留。
- **验证**：`tests/nesting.rs` 3 passed；回放门禁 37 例通过；manifest `--check` 通过；全仓 **149 passed**；clippy/fmt 通过。

---

## 2026-11-04 16:05 · docs: 文档收口与 §8 定案（P9-32）

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - `jser-serde` crate 文档增加可编译示例（`Point` schema → `to_vec`/`from_slice`）。
  - `design.md` §8、`implementation-plan.md` §18、`requirements.md` §6 全部转为“已定案”：Java 17 基线（JDK 25 重生成逐字节一致）、schema 驱动 serde、暂不发布 crates.io、性能 ≥1 GB/s 且 RSS <3×（nightly 35% 门禁）。
  - `NFR-02` 数值更新为 ≥1 GB/s；修正“34 例回放”为 36 例。
  - CI lint 增加 `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps`。
- **验证**：`cargo doc` 无警告；`cargo test --workspace` **148 passed**（新增 1 doctest）；clippy/fmt 通过。

---

## 2026-11-04 15:50 · test: mutation 扩展与图修改 API（P9-31）

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - `Graph::set_map_entry`：按值比较 key，替换已有条目或追加；primitive key/value 自动经 `box_primitive` 装箱；`insert_map_entry` 同样装箱。
  - `Graph::set_array_element`：对象数组按下标替换；primitive 数组提示重建。
  - `tests/mutation.rs` 新增：全部标量字段（byte/short/int/long/float/double/char/bool/String）修改、Map 替换与追加（含装箱校验）、`[Ljava.lang.String;` 元素替换；嵌套叶子修改已在 `tests/nested.rs` 覆盖。
  - `tests/interop.rs` 增加 Mixed 的 `i/s/b` 修改后 Java `Verify` 读回（i=42、s=changed、b=false）。
- **验证**：`cargo test -p jser --test mutation` 7 passed；interop 通过；全仓 **147 passed**；clippy/fmt 通过。
- **TDD**：先加 Map 替换/数组元素用例（API 缺失、primitive 值编码损坏），再实现 API 与装箱后全绿。

---

## 2026-11-04 15:35 · perf: nightly bench 门禁（P9-30）

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - 新增 `docs/bench-baseline.json`（观测吞吐、35% 阈值、`int[]` RSS <3×）与 `docs/bench_gate.py`（运行 `bench`、解析 MB/s 与 `VmHWM`、失败退出非零）。
  - nightly workflow 增加 `bench-gate` job（stable Rust，`python3 docs/bench_gate.py`）。
  - `docs/bench.md` 增加门禁说明与本地命令。
- **验证**：本机 `python3 docs/bench_gate.py` 通过（block 6033 MB/s、int[] 3803 MB/s；Windows 无 `VmHWM` 时跳过 RSS）；全仓 144 passed、clippy/fmt 通过。

---

## 2026-11-04 15:20 · ci: matrix/MSRV/i686 与 cargo-deny（P9-28/29）

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - `ci.yml`：`test` 改为 Linux/macOS/Windows matrix；新增 `msrv`（1.77.0，`cargo check --all-targets --locked`）、`i686`（`cargo check --target i686-unknown-linux-gnu --locked`）与 `deny`（`EmbarkStudios/cargo-deny-action@v2`）job。
  - `rust-version` 从 1.75 校准为 **1.77**（锁定依赖 `thiserror 2.0.21` 声明 1.77）；workspace 与三个 crate 增加 `license = "MIT OR Apache-2.0"`。
  - 新增 `deny.toml`：licenses allowlist（MIT/Apache/BSD/ISC/Unicode/Unlicense/Zlib）、`bans`（wildcards deny、multiple-versions warn）、`sources`（仅 crates.io）、`advisories`（yanked deny）。
- **验证**：`cargo test --workspace` 144 passed；clippy/fmt 通过；`py -c "tomllib..."` 校验 `deny.toml` 语法；本机无 cargo-deny/YAML 解析器，CI job 会在 GitHub 上执行 `cargo deny check`（YAML 结构按 GitHub Actions 语法手检）。

---

## 2026-11-04 15:00 · perf: serde 直连 JValue（P9-13）

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - 新增 `crates/jser-serde/src/direct.rs`：`Serializer` 将 schema 类型值直接写入 `Graph`；`Deserializer` 按 schema 读取 `JValue`（标量/String/Option/数组/嵌套 POJO/Map/Set/枚举/多根）。
  - `to_vec`/`from_slice`、`to_vec_map`/`from_slice_map`、`to_vec_set`/`from_slice_set`、`to_vec_enum`/`from_slice_enum`、`MultiRootWriter`/`from_slice_multi` 全部走直连；`from_slice_roots` 保留 JSON 视图（返回 `Value`）。
  - 错误仍带 schema 类名/SUID 校验；serde 错误经 `SerdeError` 转回 `jser::Error`。
- **验证**：13 个 serde 测试全绿；全仓 **144 passed**；clippy `-D warnings`、fmt 通过。
- **TDD/重构**：以现有 13 个 serde 用例作为回归门禁，直连实现逐项对齐 JSON 路径语义（含共享引用按值展开、多根/`TC_RESET`）。

---

## 2026-11-04 14:30 · test: AC-02 全 fixture Java 读回（P9-27）

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - `Verify.java` 新增 `--read`（读到 EOF，支持多元素/`TC_RESET`）、`--sequence`、`--primitives`、`--blocks`、`--array` 模式。
  - `emit.rs` 将全部 fixture 解码→重编码到 `build/rust/all/`（可用 `JSER_EMIT_DIR` 覆盖；本机沙箱下用 `target/rust-emit` 验证），并给关键 I/O 加路径上下文。
  - `interop.sh` 对全部 object-like 文件执行 `--read`，09/11 用 `--primitives`/`--blocks`。
  - `tests/interop.rs` 同样逐例重编码并 `--read`、补 primitives/blocks 与 36/40/42/43 的 `--array` 元素类断言。
- **验证**：`cargo test -p jser --test interop` 通过（36 例 + 各模式）；本机 `JSER_EMIT_DIR=target/rust-emit` 生成 36 个文件，Java 读回 `point`/`09`/`11`/`35`/`43` 全部 OK；`cargo test --workspace` **144 passed**；clippy/fmt 通过。
- **说明**：`interop.sh` 需要 bash + `python3`，本机无法直接运行；其逻辑与 `tests/interop.rs` 一致，CI 执行。

---

## 2026-11-04 14:15 · feat: java.time zone 类型（P9-21，TDD）

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - `Fixtures.java` 新增 `object/43-java-time-zone.ser`（`ZoneId.of("Asia/Shanghai")`、`ZoneOffset.ofHours(8)`、`OffsetTime`、`OffsetDateTime`，146 字节）；manifest 登记期望语义并重新生成（36 例 / 75224 字节）。
  - `builtin::semantic` 新增 `ZoneId`/`ZoneOffset`/`OffsetTime`/`OffsetDateTime` 与对应 `BuiltinValue`；java.time.Ser 类型码 7/8/9/10，复用 offset/region 读取逻辑。
  - 测试：`tests/builtin.rs` 断言 object/43 四个语义值；`tests/manifest.rs` 计数更新为 36。
- **验证**：本机 JDK 25 重新生成与已提交 35 例逐字节一致后仅新增 object/43；`py gen/build_manifest.py --check` 通过；`cargo test --workspace` **144 passed**；clippy/fmt 通过。
- **TDD**：Red：先加 object/43 语义断言（类型码/布局未实现）；Green：按 fixture 字节实现 7/8/9/10 后全绿。

---

## 2026-11-04 14:00 · feat: serde 多根流/TC_RESET/共享引用（P9-10/11/12，TDD）

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - 新增 `MultiRootWriter`（`push`/`reset`/`finish`）：逐根序列化，`reset()` 插入 `TC_RESET`；`from_slice_roots`/`from_slice_multi` 按 schema 列表解码并跳过 Reset 标记。
  - P9-12 语义定界：serde 是树模型，共享引用解码为重复子树；环无法用有限 schema 表达，真实身份/环继续用动态 `Graph`（未做指针级 serde 支持）。
  - 测试：多根（含 `TC_RESET`，断言 stream items = 3）与无 reset 多根往返；构造 `SharedRoot{a,b}` 指向同一对象（`TC_REFERENCE`）验证按值展开。
- **验证**：`cargo test --workspace` **143 passed**；clippy/fmt 通过。
- **TDD**：Red：先加 `MultiRootWriter`/`from_slice_multi` 与共享引用用例（API 不存在）；Green：实现后全绿。

---

## 2026-11-04 13:45 · feat: serde 枚举字段与根（P9-9，TDD）

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - `jser-serde` 新增 `EnumConstant`/`EnumSchema`（serde 名 ↔ Java 常量名映射），`SchemaField` 增加 `enumeration`；编码生成 `TC_ENUM`（flags 0x12、SUID 0、super `java.lang.Enum`），解码校验类名并按映射回 serde 字符串。
  - 新增根入口 `to_vec_enum`/`from_slice_enum`；`validate_schema` 校验枚举非空、常量唯一、与 `class_name1` 描述符一致。
  - 测试：`Palette{name, color}` 枚举字段往返与图断言（类名/常量/flags/SUID/super）、`Color` 枚举根往返。
- **验证**：`cargo test --workspace` **141 passed**；clippy/fmt 通过。
- **TDD**：Red：先加枚举字段/根用例（`SchemaField` 无 `enumeration`、`to_vec_enum` 不存在）；Green：实现 schema 与编解码后全绿。

---

## 2026-11-04 13:30 · feat: java.time 余项 Year/YearMonth/MonthDay/Period（P8-5，TDD）

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - `Fixtures.java` 新增 `object/42-java-time-more.ser`（`Object[]{Year.of(2024), YearMonth.of(2024,2), MonthDay.of(2,29), Period.of(1,2,3)}`，131 字节）；`build_manifest.py` 登记期望语义；manifest 重新生成为 **35 例 / 75078 字节**。
  - `builtin::semantic` 新增 `Year`/`YearMonth`/`MonthDay`/`Period` 与对应 `BuiltinValue` 变体；`java.time.Ser` 类型码按 fixture 实测为 11/12/13/14。
  - `build_manifest.py` 的未登记文件比较改用 `as_posix()`，修复 Windows 反斜杠导致的误报（此前只能靠 Linux CI 跑 `--check`）。
  - 测试：`tests/builtin.rs` 新增 object/42 语义断言；`tests/manifest.rs` 计数更新为 35。
- **验证**：用本机 JDK 25 重新生成全部 fixture，与已提交的 34 例**逐字节一致**后，仅新增 object/42；`py gen/build_manifest.py --check` 通过；`cargo test --workspace` **139 passed**；clippy/fmt 通过。
- **TDD**：Red：先加 object/42 语义断言（类型码按记忆写成 11=Period 等导致 `Truncated`）；Green：按 fixture 字节修正为 11=Year/12=YearMonth/13=MonthDay/14=Period 后全绿。

---

## 2026-11-04 13:10 · feat: derive Java enum（P9-16，TDD）

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - `jser-derive` 支持 unit-only enum：生成 `TC_ENUM` 值，flags `0x12`、SUID 0、super `java.lang.Enum`；常量名默认由 variant 转 UPPER_SNAKE（`Green`→`GREEN`、`XmlHttp`→`XML_HTTP`），`#[java(rename)]` 覆盖。
  - 校验：enum 的 `suid` 必须为 0（可省略）；variant 必须是 unit，仅支持 `rename`；空 enum 报错。
  - 测试：`Fixtures$Color` 解码（GREEN）、三个 variant 往返、图断言（SUID/flags/super）；宏单测（非 unit、suid≠0、默认 SUID 0）。
- **验证**：`cargo test --workspace` **138 passed**；clippy/fmt 通过。
- **TDD**：Red：先加 enum 用例（宏报 “only derived for structs”）；Green：实现 enum 分支与常量名映射后全绿。

---

## 2026-11-04 12:55 · feat: serde Map/Set 字段与根（P9-7/8，TDD）

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - `jser-serde`：新增 `SchemaValue`/`MapSchema`，`SchemaField` 增加 `map`/`set`；递归编码/解码 Map（HashMap/LinkedHashMap）与 Set（HashSet），Map 值可嵌套 POJO；Map 键限标量/String（JSON object 键）。
  - 新增根入口 `to_vec_map`/`from_slice_map`（`linked` 参数）与 `to_vec_set`/`from_slice_set`。
  - `builtin::new_linked_hash_map_entries`：HashMap 父层 + LinkedHashMap `accessOrder` 子层。
  - 测试：Map/LinkedHashMap/HashSet 字段往返与 graph 断言、Map 值嵌套 POJO、Map/Set 根（含 `HashMap<i32, String>` 键）。
- **验证**：`cargo test --workspace` **134 passed**；clippy/fmt 通过。
- **TDD**：Red：先加 3 个 serde 用例（`SchemaField` 无 `map`/`set`、`to_vec_map` 不存在）；Green：实现 schema 扩展与递归编解码后全绿。

---

## 2026-11-04 12:45 · test: interop 临时目录改用 CARGO_TARGET_TMPDIR

- **提交**：本次（hash 见 `git log`）
- **改动**：`tests/interop.rs` 的编译/校验目录从系统 `temp_dir()/jser-interop-<pid>` 改为 `env!("CARGO_TARGET_TMPDIR")/jser-interop`。本机系统临时目录出现异常 ACL，`create_dir_all` 返回 `PermissionDenied`（os error 5）；Cargo 的 target 临时目录随构建产物管理，始终可写。
- **验证**：`cargo test -p jser --test interop` 通过；Java 读回 14 项断言不变。
- **TDD**：不适用（测试环境修复）。

---

## 2026-11-04 12:35 · chore: 重命名 jser-serde emit 示例

- **提交**：本次（hash 见 `git log`）
- **改动**：`crates/jser-serde/examples/emit.rs` → `serde_emit.rs`，`docs/testdata/gen/interop.sh` 同步 `--example serde_emit`。`jser` 与 `jser-serde` 原来都产出 `target/debug/examples/emit.exe`，并行链接时报 `LNK1104`，阻塞 `cargo test --workspace`（示例编译）。
- **验证**：`cargo test --workspace` 不再出现示例输出名冲突；`cargo fmt --check` 通过。
- **TDD**：不适用（构建配置）。

---

## 2026-11-04 12:20 · feat: P9-15 derive 继承（TDD）

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - `jser-derive` 新增 `#[java(flatten)]` 父类字段：编码时先调用父类 `to_jvalue`，克隆其 `ClassData` 层，子类 `ClassDesc.super_desc` 指向父类最派生描述符，`classdata` 按父→子写入；解码时父类 `from_jvalue` 自动跨层 `require_field`。
  - 约束：最多一个 `flatten` 字段；必须是非 primitive/String/集合的 `JavaSerializable` 类型；不可与 `transient`/`rename` 组合。
  - 测试：`Base <- Inherited` 往返与类层次断言（2 层、`super_desc`、字段 a/b）；用 flatten struct 解码 Java fixture `object/22` 并再往返；宏单测 `flatten_requires_a_custom_type`。
- **验证**：`cargo test --workspace` **131 passed**；clippy/fmt 通过。
- **TDD**：Red：先加 flatten 用例（宏报 `flatten` 未知/无 `super_desc`）；Green：实现父类层合并与 `super_desc` 后全绿。

---

## 2026-11-04 12:05 · feat: P9-14 对象数组 Vec<Custom>（TDD）

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - `suid::array_class_suid`：用默认 SUID 算法计算任意数组类（`PUBLIC|FINAL|ABSTRACT`、无接口/成员），用 10 个已知数组 SUID 对拍（`[Z`…`[D`、`String[]`、`Object[]`）全部一致。
  - `builtin::new_java_array` 去掉固定 SUID 表，改走 `array_class_suid`，支持自定义对象数组类（`[Lcom.example.Foo;`）。
  - `JavaSerializable::ARRAY_DESCRIPTOR`（derive 由 `class` 生成，`.`→`/`）；derive 支持 `Vec<Custom>`，编码时用子类 `CLASS` 构造数组类名；`Vec<Vec<T>>` 仍拒绝。
  - 测试：SUID 已知值表 + `Bag{items: Vec<Inner>}` 编解码（空数组/多元素、类名 `[LFixtures$Inner;`）。
- **验证**：`cargo test --workspace` **128 passed**；clippy/fmt 通过；无新增 Java fixture（自定义数组仅 Rust 往返，Java 读回待 P9-27/interop 扩展）。
- **TDD**：Red：先加数组 SUID 已知值测试（算法初版不匹配，经暴力枚举定位 JDK 规则）与 `Vec<Custom>` derive 用例；Green：实现 `array_class_suid`/`ARRAY_DESCRIPTOR` 后全绿。

---

## 2026-11-04 11:45 · feat: P8-3 嵌套 derive/serde（TDD）

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - `JavaSerializable` 新增 `DESCRIPTOR: &'static str`；derive 从 `class` 属性生成（`.`→`/`），手写实现同步补常量。
  - `jser-derive`：未知字段类型按嵌套 `JavaSerializable` 处理（`class_name1` 引用子类 `DESCRIPTOR`），支持 `Option<Custom>`；`Vec<Custom>` 明确拒绝并指向 P9-14；`u8/u32/u64/...` 给出明确错误。
  - `jser-serde`：`SchemaField` 新增 `nested: Option<Box<JavaSchema>>`；`to_vec`/`from_slice` 重构为递归 `encode_object`/`decode_object`，逐层校验类名与 SUID；`validate_schema` 拒绝「nested 但 typecode 非 Object」。
  - 测试：derive 嵌套 `Outer{Inner}`、`Option<Inner>`、点号类名 DESCRIPTOR；serde 两层 `Outer{Inner}` 往返；更新既有 `SchemaField` 字面量与 derive 单测。
- **验证**：`cargo test --workspace` **126 passed**；clippy/fmt 通过；无新增 fixture（Rust 构造用例，Java fixture 待 P9-33/P8-5）。
- **TDD**：Red：先加嵌套测试，derive 报 `unsupported ... Inner`、serde 报 `SchemaField has no field nested`；Green：实现 `DESCRIPTOR`、宏自定义字段与递归 schema 后全绿。

---

## 2026-11-04 11:30 · test: 混合嵌套（Map × 数组 × 自定义类）回归

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - 新增 `crates/jser/tests/nesting.rs`（2 项）：Rust 构造 `HashMap<String, Object[]>`，数组内含自定义 `Widget`（字段为 `int[]` 与 `HashMap`），并让同一 `HashMap` 在数组中出现两次以验证 `TC_REFERENCE`；断言解码结构、共享句柄与构造流的逐字节回放。
  - 另加 typed `HashMap<String, Vec<Point>>` 往返用例。
  - `implementation-plan.md` §21 新增 P9-33（混合嵌套 fixture 矩阵），§19/§20 计数同步。
- **验证**：`cargo test --workspace` **122 passed**；clippy/fmt 通过。
- **TDD**：动态层已支持该嵌套，本用例先写断言再构造图；同时记录了 `new_java_array` 目前只支持固定数组类（自定义对象数组的 SUID 待 P9-14/P8-3 扩展）。

---

## 2026-11-04 11:15 · feat: P8-2 primitive 数组 raw 存储（TDD）

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - `value`：`Slot::Arr` 改为 `ArrayData`（`Values`/`Primitive`）；新增 `PrimitiveArray`（`len/get/bytes/iter`）、`Graph::as_primitive_array`、`Graph::new_primitive_array`；`as_array` 仅返回对象数组；`Ref::PrimitiveArr`。
  - `reader`：primitive 数组一次性读 `length*width` 字节；`writer`：raw 字节直写并校验长度/类型；`new_array` 自动压缩 primitive 元素。
  - `typed`：`FromJValue::from_primitive_element` 默认方法 + 整数/浮点/bool/char/u16/`JValue` 实现；`Vec<T>` 走 raw 快路径。
  - `builtin::semantic`（BigInteger magnitude）与 `jser-serde` 数组字段改走 raw 视图。
  - 新增 `tests/primitive_arrays.rs`（7 项）；`object_containers` 使用统一断言；`bench -- array [elements] [repeats]`。
- **验证**：`cargo test --workspace` **120 passed**；clippy/fmt 通过；`bench -- array 25000000 300` 约 3.6 GB/s、峰值工作集 203,378,688 B ≈ **2.03×**（Windows 采样）；Linux `VmHWM` 复核见 `docs/bench.md`。
- **TDD**：Red：先加 `tests/primitive_arrays.rs`（`as_primitive_array` 不存在、`as_array` 语义变化导致失败）；Green：实现 `ArrayData`/视图/typed 快路径后全绿；34 例逐字节回放门禁保持通过。

---

## 2026-11-04 11:00 · chore: 暂缓 fuzz（P8-1），停用 nightly schedule

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - `.github/workflows/nightly.yml`：停用 `schedule`、保留 `workflow_dispatch`（恢复时取消注释）。
  - `fuzz/README.md` 标注暂缓；`requirements.md` §4.1/§8、`implementation-plan.md` §11/§20、`README.md`、`SECURITY.md`、`CHANGELOG.md`、`AGENT.md` §1 同步为“暂缓，等功能完成后再执行”。
- **验证**：仅配置/文档改动；`cargo test --workspace --offline` 维持 113 passed。
- **TDD**：不适用（配置与文档）。

---

## 2026-11-04 10:55 · ci: 接入 nightly 30 分钟 fuzz（P8-1）

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - 新增 `.github/workflows/nightly.yml`：每日 18:00 UTC（Asia/Shanghai 02:00）+ 手动触发；nightly 工具链、corpus 缓存与 fixture 播种、decode `-max_total_time=1800`（AC-04）、roundtrip/mutf8 各 300s、`-rss_limit_mb=2048` / `-timeout=30`，失败上传 `fuzz/artifacts` 与 `fuzz/corpus`。
  - `fuzz/README.md`：本地 seed/运行命令、nightly 说明与 AC-04 口径。
  - 状态同步：requirements §4.1/§8、implementation-plan §11/§19/§20、README、SECURITY、CHANGELOG、AGENT §1/§5。
- **验证**：本机无 `cargo-fuzz` 且离线，无法执行 30 分钟实跑；workflow 结构与现有 `ci.yml` 一致，待远端首次运行；`cargo test --workspace` 维持 113 passed（本次未改 Rust 代码）。
- **TDD**：不适用（CI 配置与文档）。

---

## 2026-11-04 10:45 · chore: 忽略 Visual Studio 的 .vs 目录

- **提交**：本次（hash 见 `git log`）
- **改动**：`.gitignore` 增加 `.vs/`，避免 Visual Studio 的本地元数据出现在 `git status`。
- **验证**：`git check-ignore -v .vs` 命中；`git status --short` 不再显示 `?? .vs/`。
- **TDD**：不适用（仓库配置）。

---

## 2026-11-04 10:40 · docs: 登记全部纯 Rust 可实现的扩展 backlog

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - `docs/implementation-plan.md`：§20 增加近期/全量指引；新增 §21「扩展 backlog（可行性登记）」32 项（P9-1～P9-32）——`TC_CLASS`/proxy 保真回放、`TC_EXCEPTION` 语义化、流式 IO、零拷贝、serde Map/Set/枚举/多根/`TC_RESET`/共享、derive 对象数组/继承/enum/泛型/SUID 校验、java.time 与 java.util 内置类、写侧构造器、property 测试、AC-02 全类型、CI matrix、cargo-deny、bench 门禁、文档收口；§21.6 单列不可实现项及理由；§19 变更记录。
  - `README.md`、`AGENT.md` §1、`docs/design.md` §5.4：同步 §21 指引；明确 D3 边界在扩展落地前不变。
- **验证**：仅文档改动，无代码/fixture 变更；不触碰 `.ser` 与 manifest。
- **TDD**：不适用（计划与文档登记）。

---

## 2026-11-04 03:10 · docs: 同步当前进度与缺口

- **提交**：本次（hash 见 `git log`）
- **改动**：统一更新 README/SECURITY/CHANGELOG、AGENT、requirements、design、implementation-plan、testdata README、bench：
  - 当前基线：**34 个 fixture / 74947 字节，113 项测试通过，Java/SUID 互操作 14 项**。
  - P0–P7 状态；P7-1 的 30 分钟 fuzz 标注为待夜间。
  - 新增 `implementation-plan.md` §20“当前进度与后续缺口”，按 P8-1～P8-6 列出 AC-04、AC-06 真实 primitive 数组、嵌套 derive/serde、OUT 负例、Year 等、跨平台。
  - 明确 100 MB bench 是合成块流，真实 2500 万 `int[]` 受 `JValue` 32 B/元素影响尚未达标。
- **验证**：文档变更，无代码改动；当前 `cargo test --workspace` 113 passed、clippy/fmt/gen/interop 维持通过。
- **TDD**：不适用（文档同步）。

---

## 2026-11-04 03:00 · test: 增加三层嵌套对象 fixture（object/41）

- **提交**：本次（hash 见 `git log`）
- **改动**：`Fixtures.java` 新增 `Leaf`/`Middle`/`Outer` 与 `object/41-nested-objects.ser`（Outer → Middle → Leaf，197 字节）；`build_manifest.py` 登记嵌套语义；重新生成 manifest（34 例、74947 字节）；新增 `tests/nested.rs` 验证三层解码与深层字段修改后重编码；同步 `tests/manifest.rs`、README、AGENT、design/plan 的 fixture 计数。
- **验证**：`cargo test --workspace` **113 passed**；clippy/fmt/`gen.sh --check`/`interop.sh` 通过。
- **TDD**：先加入 fixture/期望与失败测试，再确认动态对象递归解码/编码通过；该用例补上了此前仅靠 `36`/`38` 间接覆盖的多层 POJO 缺口。
- **覆盖边界**：动态 `JValue` 层支持任意深度对象；derive 仍只支持扁平字段，`jser-serde` 明确拒绝嵌套对象字段，手写 typed 结构可自行递归实现。

---

## 2026-11-04 02:35 · docs: 更新 P7 完成状态

- **提交**：本次（hash 见 `git log`）
- **改动**：`AGENT.md` §1/§5/目录；`docs/implementation-plan.md` P7-1～P7-6、P7 状态与 §19；`docs/requirements.md` 变更记录；本条目。
- **验证**：`cargo test --workspace` **111 passed**；clippy/fmt/gen/interop/package 检查通过。
- **TDD**：不适用（状态与文档收口）。

---

## 2026-11-04 02:30 · feat: P7 加固与发布准备（TDD）

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - panic 审计：`jser` 增加 `#![deny(clippy::unwrap_used, clippy::expect_used)]`；移除 allowlist/builtin/semantic 中的 `expect`；`Reader` 批量解码改为 move 进 arena，避免 100 MB 顶层块多一次复制。
  - `fuzz/`：decode/roundtrip/mutf8 三个 libFuzzer 目标、README、seed 说明；新增离线 `tests/fuzz_smoke.rs`，CI 增加 `fuzz-smoke` job。
  - `crates/jser/examples/bench.rs` + `docs/bench.md`：100 MB 块流实测约 4.8 GB/s、峰值约 2.0× 输入。
  - `README.md`、`SECURITY.md`、`CHANGELOG.md`；CI 继续覆盖 lint/test/fixtures/interop。
- **验证**：`cargo test --workspace` **111 passed**；clippy/fmt/`gen.sh --check`/`interop.sh` 通过；`cargo package --list -p jser` 通过（无发布元数据，按 A-5 暂不发布）。
- **TDD**：Red：新增 `fuzz_smoke` 与 bench 流程先运行；Green：修复 panic 路径与批量解码复制后通过。
- **AC 状态**：`AC-06` 的合成 100 MB 块负载满足吞吐/内存目标；`AC-04` 已有 fuzz 目标 + 冒烟，但 30 分钟完整运行待夜间/手工执行。

---

## 2026-11-04 02:15 · docs: 更新 P6 完成状态

- **提交**：本次（hash 见 `git log`）
- **改动**：`AGENT.md` §1/§5/§6；`docs/implementation-plan.md` P6-1～P6-6、P6 状态与 §19；`docs/requirements.md` 变更记录；本条目。
- **验证**：`cargo test --workspace` **110 passed**；clippy/fmt/`gen.sh --check`/`interop.sh` 通过（14 项 Java/SUID 校验）。
- **TDD**：不适用（状态与文档收口）。

---

## 2026-11-04 02:10 · feat: 实现 P6 内置类语义与 serde 桥（TDD）

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - `crates/jser/src/builtin/semantic.rs`：`JavaDate`、`BigInteger`（十进制度转换）、`BigDecimal`（保留 scale）、`Duration`/`Instant`/`LocalDate`/`LocalTime`/`LocalDateTime`/`ZonedDateTime`；`BuiltinValue` 与 `Registry`（按类名分派，未知类返回 `None`）。
  - `Graph::builtin` 便捷入口；`crates/jser/tests/builtin.rs` 覆盖 `36` 与新增 `40` 的语义，未知类不误判。
  - `crates/jser-serde`：schema 驱动 `to_vec`/`from_slice`（JavaSchema/SchemaField），支持单根扁平 POJO、标量/String/Option/数组；拒绝多根、未知类/schema 不匹配、嵌套对象；`tests/serde.rs` 5 例。
  - `examples/emit.rs` + `interop.sh`：serde 输出的 `point-serde.ser` 由 Java 读回；`docs/design.md` §5.4 支持矩阵与 serde 限制。
- **验证**：`cargo test --workspace` **110 passed**；clippy/fmt/`gen.sh --check`/`interop.sh` 通过（14 项 Java/SUID 校验）。
- **TDD**：Red：先加 `tests/builtin.rs`/serde 测试，缺少 `Registry`/`BuiltinValue`/`to_vec` 编译失败；Green：实现语义解析与 schema 桥后逐项通过。
- **AC 状态**：`AC-01` 的 `36` 语义补齐，新增 `40` 通过；`AC-02` 的 serde 输出增加 Java 读回；`AC-04`/`AC-06` 仍未覆盖。

---

## 2026-11-04 02:00 · test: 扩充 java.time fixture（object/40）

- **提交**：本次（hash 见 `git log`）
- **改动**：`gen/Fixtures.java` 新增 `object/40-java-time.ser`（Object[]：Duration、Instant、LocalDate、LocalDateTime、ZonedDateTime/Asia/Shanghai）；`build_manifest.py` 登记期望语义；重新生成 `manifest.json`（33 例、74750 字节）；`testdata/README.md` 覆盖表与 `tests/manifest.rs` 计数同步。
- **验证**：`cd docs/testdata && ./gen/gen.sh --check` 通过；`cargo test --workspace` 通过（33 例全量解码/回放）。
- **TDD**：先登记 fixture 与期望值，再实现语义解析（见下一条 feat）。

---

## 2026-11-04 01:45 · docs: 更新 P5 完成状态

- **提交**：本次（hash 见 `git log`）
- **改动**：`AGENT.md` §1/§5/§6；`docs/implementation-plan.md` P5-1～P5-6、P5 状态与 §19；`docs/requirements.md` 变更记录；本条目。
- **验证**：`cargo test --workspace` **102 passed**；clippy/fmt/`gen.sh --check`/`interop.sh` 通过（13 项 OK + SUID）。
- **TDD**：不适用（状态与文档收口）。

---

## 2026-11-04 01:40 · feat: 实现 P5 derive 与 SUID（TDD）

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - `crates/jser/src/java.rs`：`JavaSerializable`/`JavaField`（CLASS/SUID/schema/to_jvalue/from_jvalue）。
  - `crates/jser-derive`：`#[derive(JavaSerializable)]`，支持 `#[java(class/suid)]`、`#[java(rename)]`、`#[java(transient)]`；字段排序、基本类型/String/Option/Vec（Java 数组）/HashMap/HashSet；生成 `FromJValue`/`IntoJValue`。
  - `crates/jser/src/builtin/mod.rs`：`new_java_array`（常用一维数组类与 SUID）。
  - `crates/jser/src/suid.rs`：`computeDefaultSUID` 移植（SHA-1 摘要前 8 字节反向折叠）、`.class` 最小解析；`src/bin/suid.rs` 输出计算值。
  - 测试：`tests/derive.rs`（7）、`tests/java_trait.rs`（1）、`tests/suid.rs`（1）、derive crate 编译错误单测（3）。
  - `examples/emit.rs`/`interop.sh`：增加 derive Point 的 Java 读回与 `Fixtures$NoSuid` SUID 对拍。
- **验证**：`cargo test --workspace` **102 passed**；clippy/fmt/`gen.sh --check`/`interop.sh` 全通过（含 SUID `-3559046683250708543`）。
- **TDD**：Red：先写 derive/SUID 测试，宏未实现、`encode_typed` 缺实现导致编译/断言失败；Green：实现 trait/macro/SUID 后逐步修到全绿；derive 生成代码曾因 `vec![...]` 同时可变借用 `graph` 失败，改为顺序 `push`。
- **P5-2 说明**：离线环境未引入 `trybuild`，用 derive crate 内的 `expand` 单元测试覆盖缺 class/不支持类型/元组结构体的编译错误路径。
- **AC 状态**：P5 不新增 AC；`39` 的 SUID 一致，derive Point 已 Java 读回；Mixed 以 Rust 往返 + schema 断言验证。

---

## 2026-11-04 01:18 · docs: 记录 allowlist 定案

- **提交**：本次（hash 见 `git log`）
- **改动**：`requirements.md` §6/§8、`design.md` §5.2/§8、`implementation-plan.md` A-3/P6-3/§19、`AGENT.md` §1/§5/§6/§11；本条目。
- **验证**：`cargo test --workspace` **90 passed**；clippy/fmt/gen.sh --check/interop.sh 通过。
- **TDD**：不适用（决策与文档收口）。

---

## 2026-11-04 01:15 · feat: 实现默认严格 allowlist（P6-3 部分）

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - 新增 `crates/jser/src/allowlist.rs`：`Allowlist` 支持精确名、`prefix.*`、`Fixtures$*`、裸 `*`/`Allowlist::all()`；默认 `strict()` 放行 `java.lang.*` + `java.util.*`；数组类名按元素类检查；`check` 返回带类名的 `UnsupportedClass`。
  - `crates/jser/src/reader.rs`：`Reader::with_allowlist`，`TC_CLASSDESC` 解析时检查父类链；`decode_stream_with_allowlist`/`decode_stream_with_options`/`decode_with_allowlist`。
  - `crates/jser/src/typed.rs`：`decode_typed_with_allowlist`。
  - 测试：`tests/allowlist.rs`（7）；fixture 测试统一通过 `common::fixture_allowlist()` 显式放行 `Fixtures$*`、`java.math.*`、`java.time.*`；Java interop/emit 同步。
  - `crates/jser/src/lib.rs`：安全说明更新，新增 allowlist rustdoc 示例。
- **验证**：`cargo test --workspace` **90 passed**；clippy/fmt/`gen.sh --check`/`interop.sh` 全通过。
- **TDD**：Red：先写 `tests/allowlist.rs`，缺少 `Allowlist`/`decode_stream_with_allowlist` 编译失败；Green：实现 allowlist 与 Reader 集成后 7 例通过，再修 fixture 测试的显式放行。
- **行为变化**：`decode_stream`/`Reader::new`/`decode_typed` 默认严格；未放行类（如 `Fixtures$Point`、`java.math.BigInteger`）返回 `UnsupportedClass`。这是有意的安全默认。

---

## 2026-11-04 01:02 · docs: 更新 P4 完成状态

- **提交**：本次（hash 见 `git log`）
- **改动**：`AGENT.md` §1/§5/§6；`docs/implementation-plan.md` P4-1～P4-5 标记、P4 状态与 §19 变更记录；本条目。
- **验证**：`cargo test --workspace` **82 passed**；clippy/fmt/gen.sh --check/interop.sh 通过。
- **TDD**：不适用（状态与文档收口）。

---

## 2026-11-04 00:58 · feat: 实现 P4 typed 转换与动态 POJO 收口（TDD）

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - 新增 `crates/jser/src/typed.rs`：`FromJValue`/`IntoJValue`、`decode_typed`/`encode_typed`；覆盖标量、`Option`、`Vec`、`HashMap`、`HashSet`、手写 POJO，集合元素自动装箱。
  - `crates/jser/src/builtin/mod.rs`：`new_array_list`、`new_hash_set`、`new_hash_map_entries`；`box_primitive` 支持 `java.lang.*` 包装类（Integer/Long/Boolean/Character 等）。
  - `crates/jser/src/value/mod.rs`：`require_field`、`add_reset`、构造描述符去重。
  - `crates/jser/src/lib.rs`：crate rustdoc 示例（读 Map、typed Map、手写 POJO）与安全警告。
  - 新测试：`tests/typed.rs`（7）、`tests/unknown.rs`（1，AC-05）、reset 插入回归（1）；`tests/interop.rs` 增加 typed `HashMap<String,i32>` 与 `ArrayList<Integer>` 的 Java 读回。
  - `docs/testdata/gen/Verify.java` 支持 `List`；`examples/emit.rs` 增加 typed map/list 输出。
- **验证**：`cargo test --workspace` **82 passed**；`cargo clippy -D warnings`、`cargo fmt --check`、`gen.sh --check`、`interop.sh` 全部通过。
- **TDD**：Red：先写 `tests/typed.rs`，因缺少 `decode_typed`/`FromJValue` 编译失败；Green：实现 typed 层后逐项通过，再补 Java interop 的 typed Map/List 校验。
- **AC 状态**：`AC-03`（33/34/35 读→写→读）与 `AC-05`（23/36/39 原样回写）通过；`AC-01`/`AC-02` 维持 P3 状态；allowlist 仍留 P6。

---

## 2026-11-04 00:50 · docs: 更新 P3 完成状态

- **提交**：本次（hash 见 `git log`）
- **改动**：`AGENT.md` §1 状态与命令、协议踩坑 #16–18；`docs/implementation-plan.md` P3-1～P3-10 标记、P3 状态与 §19 变更记录；本条目。
- **验证**：`cargo test --workspace` **70 passed**；`cargo clippy -D warnings`、`cargo fmt --check`、`docs/testdata/gen/gen.sh --check`、`docs/testdata/gen/interop.sh` 全部通过。
- **TDD**：不适用（状态与文档收口）。

---

## 2026-11-04 00:45 · feat: 实现 P3 编码层（TDD）

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - `crates/jser/src/writer.rs`：`encode_stream`/`encode`；按 `items` 顺序写顶层元素；wire 句柄分配/`TC_REFERENCE`/`TC_RESET`；字符串按 `Str.long` 选头；ClassDesc 去重与字段/`className1` 写回；对象、数组、enum、块、自定义数据。
  - `crates/jser/src/wire/writer.rs`：大端 `StreamWriter`；`crates/jser/src/wire/block.rs`：1024 字节分块与 `u8`/`i32` 长度选择。
  - `crates/jser/src/value/mod.rs`：`FieldDesc.class_name_handle`（回放 className1 句柄）、`Graph::new_string/add_item/add_class_desc/new_object/new_array/new_enum/set_field/insert_map_entry` 与编码侧槽位访问器。
  - `crates/jser/src/builtin/mod.rs`：`new_hash_map` 从零构造 HashMap（capacity/threshold/自定义块）。
  - 新测试：`tests/roundtrip_bytes.rs`（1，32 例逐字节回放）、`tests/roundtrip_semantics.rs`（4）、`tests/mutation.rs`（4，字段/Map 条目/构造 POJO/HashMap）、`tests/interop.rs`（1，无 JDK 跳过）、`wire::block` 单测（2）。
  - `crates/jser/examples/emit.rs`、`docs/testdata/gen/Verify.java`、`docs/testdata/gen/interop.sh`、CI `interop` job。
  - `docs/design.md` D1：补 `ClassDesc.annotations`、`FieldDesc.class_name_handle` 与 className1 句柄不变量。
- **验证**：`cargo test --workspace` **70 passed**；`cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`cd docs/testdata && ./gen/gen.sh --check` 全通过；`docs/testdata/gen/interop.sh` 的 9 个 Java 校验全部 OK。
- **TDD**：
  - Red：先加 `tests/roundtrip_bytes.rs`，因缺少 `encode_stream` 编译失败。
  - Green：实现 writer 后 32 例逐字节回放一次通过；继续补 block writer、mutation、构造 Map/POJO 与 Java interop，逐项到全绿。
- **AC 状态**：`AC-01` 读方向 32 例（36 语义留 P6）；`AC-02` 由 Java `Verify` 覆盖 Point/Mixed/HashMap（回放、mutation、构造）/环/enum 代表用例；`AC-03` 的 33/34/35 读→写→读通过；`AC-04`/`AC-06` 未覆盖。

---

## 2026-11-04 00:40 · chore: 收口 P2 状态并补充句柄视图

- **提交**：本次（hash 见 `git log`）
- **改动**：`crates/jser/src/value/mod.rs` 增加 `Ref` 与 `Graph::resolve`（按句柄借用槽位，不展开环）；`crates/jser/src/lib.rs` 导出；`tests/object.rs` 增加 `resolves_a_handle_without_expanding_the_cycle`；`AGENT.md`、`implementation-plan.md` 标记 P2 完成。
- **验证**：`cargo test --workspace` **58 passed**；`cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`docs/testdata/gen/gen.sh --check` 通过。
- **TDD**：不适用（P2-3 辅助视图收口；回归测试与实现同提交）。

---

## 2026-11-04 00:32 · feat: 实现 P2 解码层（TDD）

- **提交**：本次（hash 见 `git log`）
- **改动**：
  - `crates/jser/src/value/mod.rs`：新增 `FieldType`/`FieldDesc`/`ClassDesc`/`ClassData`/`JObject`，`Slot` 扩展为 `Desc/Str/Arr/Obj/Enum`；`Graph` 增加插入/回填 API 与 `as_object/as_array/as_enum_constant/as_map/as_list/as_set/as_scalar`、`field(s)`、`descriptor`、`as_i64/as_i32/as_f64/as_bool/as_char`。
  - `crates/jser/src/reader.rs`：实现 `TC_CLASSDESC`/`TC_OBJECT`/`TC_ARRAY`/`TC_ENUM`、继承链、`SC_WRITE_METHOD`/`SC_BLOCK_DATA` 自定义块、未知类降级、共享/环、`TC_RESET`；新增 `Limits` 与 `Reader::with_limits`。
  - `crates/jser/src/builtin/mod.rs`：解析 `ArrayList`/`LinkedList`/`HashMap`/`LinkedHashMap`/`HashSet` 的自定义块；`crates/jser/src/wire/mod.rs` 增加 `SC_*` 常量。
  - 新测试：`tests/object.rs`（8）、`tests/object_containers.rs`（9）、`tests/limits.rs`（7）、`tests/decode_all.rs`（1）。
  - `docs/design.md` §4.2/§4.3：补充 classDesc 句柄分配时机、classdata 父→子顺序、每层自定义块位置。
- **验证**：`cargo test --workspace` **57 passed**；`cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`cd docs/testdata && ./gen/gen.sh --check` 全通过；32 个 fixture 全部可解码。
- **TDD**：
  - Red：先新增 `tests/object.rs`，因缺少 `JObject`/`as_object`/`field`/`as_i64` 编译失败。
  - Green：实现 classDesc + 对象层后 8 例通过；继续加容器/限制/全量解码测试，逐项修到全绿。过程中由 `object/34` 抓到“classDesc 句柄先于字段描述符分配”的顺序错误。
- **AC 状态**：`AC-01` 的读方向覆盖 codec 12 例与 object 20–39（36 仅结构，语义留 P6）；`AC-03`/`AC-05` 只到读侧结构，写方向未做；`AC-02`/`AC-04`/`AC-06` 未覆盖。

---

## 2026-11-04 00:15 · docs: 增加提交日志规则并补齐历史

- **提交**：本次（hash 见 `git log`）
- **改动**：`AGENT.md` 新增 §10.2、目录地图、开发流程与检查清单；新增本文件并回填此前 12 个提交。
- **验证**：`cargo test --workspace` 32 passed；`cargo clippy -D warnings`、`cargo fmt --check`、`docs/testdata/gen/gen.sh --check` 通过；`git status` 干净。
- **TDD**：不适用（流程与文档）。

---

## 历史提交（回填）

| 提交 | 时间 | 类型 | 摘要 | 主要改动 | 验证 |
|---|---|---|---|---|---|
| `ad46c43` | 2026-11-03 23:54 | docs | 初始化项目文档与 Java 序列化基准数据 | `AGENT.md`、requirements/design/implementation-plan；32 个 `.ser`、`manifest.json`、生成器 | 44 个文件入库；基准数据 32 例 / 74559 字节 |
| `754ddfe` | 2026-11-03 23:57 | docs | 增加每次改动必须提交 git 的规则 | `AGENT.md` §10.1、开发流程、检查清单；plan P0-1 标记 | 文档检查通过 |
| `d9c15a1` | 2026-11-03 23:57 | docs | 修正开工清单中的 git 基线状态 | `implementation-plan.md` §17 | 文档检查通过 |
| `e8150ff` | 2026-11-04 00:03 | feat | 初始化 jser workspace 与错误模型 | workspace、`jser`/`jser-derive`/`jser-serde` 骨架、`Error` 模型、模块占位 | `cargo check` / `cargo build` 通过 |
| `86a6073` | 2026-11-04 00:03 | test | 接入 manifest 驱动的 32 例夹具测试 | `tests/common`、`tests/manifest.rs`、`tests/errors.rs` | `cargo test` 6 passed；32 例、路径、size、流头校验 |
| `aa46873` | 2026-11-04 00:03 | ci | 添加 fmt/clippy/test 与 testdata 校验 | `.github/workflows/ci.yml` 三个 job | 本地等价检查通过；Actions 待远端首次运行 |
| `134a6da` | 2026-11-04 00:04 | docs | 更新 P0 完成状态 | `AGENT.md`、plan P0 表与开工清单 | 文档检查通过 |
| `fa4191c` | 2026-11-04 00:09 | fix | 修正 11 号块数据的 long 头预期 | `manifest.json`、两个 Python 生成器、`design.md`、`testdata/README.md` | TDD 发现：976 > 255 应为 `TC_BLOCKDATALONG`；`gen.sh --check` 通过 |
| `70f58cc` | 2026-11-04 00:09 | feat | 实现 P1 codec 层（TDD） | `StreamReader`、MUTF-8、wire 句柄表、`Reader`/`decode_stream`、P1 值模型；新增 codec/reader/mutf8 测试 | Red：缺模块编译失败；Green：27 passed；codec 12 例全绿；clippy/fmt 通过 |
| `2cbfc2d` | 2026-11-04 00:10 | docs | 记录 TDD 循环并更新 P1 状态 | `AGENT.md` §8.1；plan P1 状态与开工清单 | 文档检查通过 |
| `f580834` | 2026-11-04 00:11 | test | 补齐 P0 的 manifest 与 error 契约测试 | tier 一致性、`expect.kind` 白名单、未登记 `.ser`、错误 Send+Sync、`Result` 别名 | 全仓 32 passed |
| `d221ec2` | 2026-11-04 00:11 | docs | 记录 P0 测试补齐 | `implementation-plan.md` P0/P1 测试计数与变更记录 | 文档检查通过 |

> 回填说明：`11` 号夹具的期望值在 `fa4191c` 中得到修正；fixture 字节未改，manifest/文档是错的。P1 的 `reader`/`mutf8`/`codec` 测试在 `70f58cc` 中与实现同一提交，符合 TDD 的 Red → Green 记录。
