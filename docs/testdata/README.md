# testdata/ — 基准数据与生成器

**当前状态**：已产出。36 个用例、75224 字节，由 JDK 17.0.20.1 生成；codec 层的手写推导与 JDK 输出**逐字节一致**。

这批数据是协议层的唯一事实来源：`requirements.md` 的 **AC-01** 就是"全部用例的解码结果与 `manifest.json` 的期望值一致"，AC-02 反向要求 Rust 写出的字节能被 Java 读回。

设计/计划另用这些 fixture 做"未修改流"的逐字节回放门禁（`design.md` §5.1）：它是内部回归门禁，**不替代 AC-02**。

## 目录

| 路径 | 说明 | 入库 |
|---|---|---|
| `manifest.json` | **机读契约**：36 个用例的编号、文件、覆盖特性、期望语义值、size、sha256 | 是 |
| `fixtures/codec/` | codec 层：无类描述符的流原语（字符串/空/块数据/句柄回指/reset） | 是 |
| `fixtures/object/` | object 层：类描述符、SUID、继承、自定义 `writeObject`、enum、数组、集合、循环引用、三层对象嵌套 | 是 |
| `gen/Fixtures.java` | **权威生成器**（JDK 17+），同时把各类的 `serialVersionUID` 打到 stdout | 是 |
| `gen/build_codec_fixtures.py` | codec 层的**独立再推导**：按规范字面规则重算同一批字节，用于对拍 | 是 |
| `gen/build_manifest.py` | 扫描 `fixtures/`、计算 size/sha256、生成 `manifest.json`；`--check` 只校验 | 是 |
| `gen/Verify.java` | Rust 写出的 `.ser` 用 `ObjectInputStream` 读回并断言类名/字段/Map 条目 | 是 |
| `gen/interop.sh` | Rust → Java 互操作检查（gen --check、emit、javac、Verify） | 是 |
| `gen/gen.sh` | 编排：编译 → 生成 → 对拍 → 生成/校验 manifest | 是 |
| `build/` | 临时产物（class 文件、对拍中间目录、SUID 表、日志） | 否（`.gitignore`） |

## 为什么要两个生成器

只依赖 JDK，就无法区分"协议理解错了"和"JDK 就是这么写的"；`build_codec_fixtures.py` 把规范规则（修改版 UTF-8、块缓冲的 flush 时机、句柄回指）显式写一遍再算一次。两边输出必须逐字节相同——不一致时以 JDK 为准，并回头修正 Python 侧的推导规则。

这套对拍在 2026-11-03 已经抓到两处原先写错的理解（见 `design.md` §4.2 第 2、6 条：classdata 的基本类型字段是裸字节、不走块缓冲；字段排序是"基本类型按名 → 对象按名"分组排序而非全局排序）。

## 用法

```bash
cd testdata
./gen/gen.sh           # 重新生成 fixtures/ 与 manifest.json
./gen/gen.sh --check   # CI：重新生成到 build/，与已提交内容逐字节比对
./gen/interop.sh       # Rust 写 → Java 读回（需 JDK；写 build/rust/*.ser）
```

`--check` 会依次验证：fixtures 与重新生成结果一致 → codec 手写推导一致 → manifest 的 size/sha256 一致 → 没有未登记的 `.ser` 文件。

## 用例覆盖

| 层 | 编号 | 覆盖点 |
|---|---|---|
| codec | 01–06 | 修改版 UTF-8（空串、`U+0000`、CJK、代理对 6 字节、>65535 走 `TC_LONGSTRING`） |
| codec | 07–08 | `TC_NULL`、句柄回指 `TC_REFERENCE` |
| codec | 09–11 | 块缓冲合并、`TC_RESET`、1024 字节块边界（1024 与 976 两块都 >255，均用 `TC_BLOCKDATALONG`） |
| codec | 12 | 1/2/3 字节宽度混合的 UTF 编码 |
| object | 20–22 | 简单 POJO、10 字段混排（字段排序规则）、继承链 |
| object | 23–24 | 自定义 `writeObject`（`SC_WRITE_METHOD` + `TC_ENDBLOCKDATA`）、enum |
| object | 25–27 | 基本类型数组、含 `null` 的对象数组、二维数组 |
| object | 28–32 | `ArrayList`/`HashMap`/`LinkedHashMap`/`HashSet`/`LinkedList` 的自定义布局 |
| object | 33–35 | 对象共享、循环引用、流中 `reset()` |
| object | 36–40 | `Date`/`BigInteger`/`BigDecimal`/`java.time.Ser`、`transient`、JDK 计算的 SUID、java.time 多类型 |
| object | 41 | 三层对象嵌套 `Outer → Middle → Leaf`（对象字段递归） |
| object | 42 | `Year`/`YearMonth`/`MonthDay`/`Period`（java.time.Ser 余项） |
| object | 43 | `ZoneId`/`ZoneOffset`/`OffsetTime`/`OffsetDateTime` |

## SUID 表

`./gen/gen.sh` 会把各测试类的 `serialVersionUID` 写到 `build/suid.txt`，供 Rust 侧 `computeDefaultSUID` 实现对照：

```
Fixtures$Point	1
Fixtures$Color	0
Fixtures$NoSuid	-3559046683250708543
```

除 `NoSuid` 外都是显式声明 `1L`，保证字节跨 JDK 版本稳定；`NoSuid` 专门用来校验"默认 SUID 计算"是否与 JDK 一致（对应 `39-suid-computed.ser`）。

```bash
# 与 build/suid.txt 的 Fixtures$NoSuid 对拍（interop.sh 也会执行）
cargo run -p jser --bin suid -- 'build/classes/Fixtures$NoSuid.class'
```
