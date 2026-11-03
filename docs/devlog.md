# 开发日志（Dev Log）

记录每次提交的工作内容、验证结果与 TDD 红绿过程。规则见 [AGENT.md §10.2](../AGENT.md)。

- 每次 `git commit` 前追加一条，与改动同一个提交。
- 日志条目不写自身 hash（避免自指）；历史条目已回填 hash，当前条目用"本次"，完整 hash 以 `git log` 为准。
- TDD 条目写清 Red（失败测试）与 Green（最小实现）。

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
