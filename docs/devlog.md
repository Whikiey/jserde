# 开发日志（Dev Log）

记录每次提交的工作内容、验证结果与 TDD 红绿过程。规则见 [AGENT.md §10.2](../AGENT.md)。

- 每次 `git commit` 前追加一条，与改动同一个提交。
- 日志条目不写自身 hash（避免自指）；历史条目已回填 hash，当前条目用"本次"，完整 hash 以 `git log` 为准。
- TDD 条目写清 Red（失败测试）与 Green（最小实现）。

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
