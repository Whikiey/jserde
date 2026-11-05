# 0006 · Rust 实现 Java 序列化（与 ObjectOutputStream / ObjectInputStream 互通）

- 状态：Accepted（2026-11-04；§8 问题已全部定案）
- 关联想法：`ideas.md` #6「Rust Java 序列化」
- 需求基线：[requirements.md](requirements.md)（验收以其中的 `AC-*` 为准，本文只回答"怎么做"）
- 实现计划：[implementation-plan.md](implementation-plan.md)（任务拆分、阶段门禁、缺口定案）
- 日期：2026-11-03
- 最近修订：2026-11-05 同步 P8/P9（primitive raw、嵌套 derive/serde、TC_CLASS/proxy 保真、扩展容器/值类）
- 预期语言：Rust（`serde` 可选）+ Java（仅用于生成测试基准，不进入交付物）

---

## 1. 问题定义

Java 的序列化格式（Java Object Serialization Stream Protocol）是**自描述**的：流里带着类名、字段表、类型码，因此理论上可以在没有 Java 类定义的情况下解析。我们要在 Rust 侧提供两个方向的互操作：

- **读（decode）**：拿到 Java `ObjectOutputStream` 写出的字节流，在 Rust 中还原成可用的数据结构（`HashMap`、`Vec`、或任意对象的动态结构）。
- **写（encode）**：Rust 侧把数据写成字节流，Java 用 `ObjectInputStream` 能正确还原成对应的对象/集合。

原始想法里的「字典到字典的转换（Java Map ↔ Rust Map）」是这个能力的**最小可用切片**，所以 MVP 就围绕 Map / 集合 / 基本类型 / POJO 展开。

关键约束：格式不是"我们和 Java 各写一遍"，而是**必须逐字节符合 JDK 的实现**（`ObjectStreamClass` / `ObjectOutputStream` 的行为），否则 Java 端会抛 `StreamCorruptedException` / `InvalidClassException`。

## 2. 目标与非目标

### v0.1 目标（MVP）

1. 流头 `0xACED 0x0005` 的读写，句柄表、`TC_RESET`、块数据缓冲。
2. 基本类型、`String`、一维与多维数组（基本类型 + 对象数组）。
3. 常用集合：`HashMap`、`LinkedHashMap`、`HashSet`、`ArrayList`、`LinkedList`。
4. 任意 POJO 的**动态**读写（不知道 Rust 侧结构也能读；写时由 schema 提供类名/字段）。
5. 循环引用与对象共享（同一对象多次出现只写一次，后续写回指）。
6. Java ↔ Rust 互通测试：JDK 生成的基准字节流做 fixture，双向验证。
7. 解码结果保留 `classDesc`、块边界与元素顺序，**未修改的流**能原样回写（`encode_stream(decode_stream(x)) == x`）。这是 P3 的回归门禁，不是验收标准（验收仍以 `AC-02` 的 Java 读回为准），边界见 §5.1。

### v0.1 非目标（明确不做）

| 不做 | 原因 |
|---|---|
| `Externalizable` 的通用 `writeExternal/readExternal` 语义 | 自定义协议对端逻辑不可知，只能当不透明块处理；`java.time.Ser` 等已知特例走注册表，见 §4.2 第 16 条 |
| 动态代理的**调用语义**（`TC_PROXYCLASSDESC`） | P9-2 做接口列表/classdata 的字节保真回放；`InvocationHandler` 调用语义仍不做 |
| `TC_CLASS` 的**类加载语义** | P9-1 做 `Class` 对象的字节/句柄保真回放；Rust 侧没有类加载概念 |
| RMI / JNDI 传输层 | 与序列化格式无关 |
| 任意 Java 类 → Rust 结构体的自动映射 | 需要类元数据，改为"动态结构 + 可选 schema" |
| 反序列化安全防护（gadget chain 拦截） | 只提供 allowlist 钩子并在文档中警告，不做完整沙箱 |

## 3. 方案选型（关键决策）

### D1：数据模型 = 句柄化动态图 + arena；schema 映射在后

Java 侧的类是开放的（任何 `Serializable` 类都可能出现在流里），Rust 编译期无法穷举。核心模型必须是 **wire 无损的动态图**：引用型值（`String`、数组、对象、enum）一律进 arena，`JValue` 只携带句柄；标量、块数据内联。

```rust
pub enum JValue {
    Null,
    Bool(bool), Byte(i8), Char(u16), Short(i16), Int(i32), Long(i64), Float(f32), Double(f64),
    Str(Handle), Arr(Handle), Obj(Handle), Enum(Handle),  // 引用型统一走句柄
    Block(BlockData),                                    // 顶层/自定义块，保留块边界
    Reset,                                               // TC_RESET：只能作为流/自定义段元素，不能作字段值
}

/// arena 索引，整条流单调递增；**不是** wire handle
pub struct Handle(pub u32);

pub struct Graph {
    slots: Vec<Slot>,       // index = Handle
    items: Vec<JValue>,     // 顶层元素，按出现顺序（可含 Reset）
}

pub enum Slot {
    Desc(ClassDesc),                         // classDesc 也占句柄（wire 表里同样占）
    Str { value: String, long: bool },        // long = 原流用 TC_LONGSTRING 头
    Arr { desc: Handle, data: ArrayData },     // 对象数组 = Vec<JValue>；primitive = wire 大端字节
    Obj(JObject),
    Enum { desc: Handle, constant: Handle },     // constant → Slot::Str（枚举名是 TC_STRING，占句柄）
}

pub struct ClassDesc {
    pub name: String,                        // 点号形式；数组也写点号（如 [Ljava.lang.String;）
    pub suid: i64,
    pub flags: u8,                           // SC_* 原样保留
    pub fields: Vec<FieldDesc>,              // 描述符顺序
    pub super_desc: Option<Handle>,
    pub annotations: Vec<JValue>,            // classAnnotation 内容，通常为空
}

pub struct FieldDesc {
    pub name: String,
    pub typecode: FieldType,                 // B/C/D/F/I/J/S/Z 或 Object/Array
    pub class_name1: Option<String>,         // L...; 或 [I，原样保留
    pub class_name_handle: Option<Handle>,   // 解码时的 className1 字符串槽，回放用；新建为 None
}

pub struct JObject { pub classes: Vec<ClassData> }   // 父类 → 子类，与 classdata 顺序一致

pub struct ClassData {
    pub desc: Handle,                        // 该层 classDesc
    pub fields: Vec<(String, JValue)>,       // 该层 classdata，按描述符顺序
    pub custom: Option<Vec<JValue>>,         // SC_WRITE_METHOD / SC_BLOCK_DATA；可含 Reset/Block；Some(vec![]) 表示需写 TC_ENDBLOCKDATA
}

pub struct BlockData { pub bytes: Vec<u8>, pub long: bool }  // long = 用 TC_BLOCKDATALONG 头
```

关键不变量（均有 fixture 依据）：

- **字符串也走句柄**：`08` 第二次写同一字符串是 `TC_REFERENCE`；若 `JValue::Str` 内联字符串，回指无法表达，重放会多写一份。enum 的常量名也是 `TC_STRING`（`24`），同样必须占句柄。
- **字符串头也保留**：`Slot::Str { value, long }` 的 `long` 记录原文是 `TC_LONGSTRING` 还是 `TC_STRING`。Java 按长度选头，但非规范/手写流可能短串用 long；不保留该位就无法逐字节回放。新建字符串时按**修改版 UTF-8 字节长度**（不是 Rust `String::len()`）选头：超过 65535 用 `TC_LONGSTRING`。
- **classDesc 也进 arena**：`29` 中后续 `Integer` 的 classDesc 用 `TC_REFERENCE` 回指；重放要按原顺序写描述符或回指。
- **className1 也保留句柄**：字段的 `className1` 是 `TC_STRING`/`TC_REFERENCE`，同样占 wire 句柄；`FieldDesc.class_name_handle` 记录它，重放时才能保持句柄编号与回指。没有该字段时（新建描述符）编码器按 MUTF-8 长度重新写一个字符串。
- **内部句柄 ≠ wire 句柄**：wire 句柄从 `0x7E0000` 起、`TC_RESET` 后重用；内部 `Handle` 整条流单调递增。`Reader` 维护 `wire -> Handle` 映射，`reset` 只清映射、不回收 arena。否则 `35` 的两个 `Point` 会撞到同一索引。
- **先占位、后填充**：对象在读完 `classDesc` 后、读 classdata 前分配句柄（`34` 中 `b.next` 先回指 `a`）；数组同样先分配句柄再读元素。
- **块边界保留**：`11` 是 `LONG 1024 + LONG 976`（976 > 255，仍用 long 头），`23` 是单块 `77 0C`；重放必须逐块写回。
- **顶层是多元素流**：`08`、`09`、`10`、`11`、`33`、`35` 都不是单根；`Reset` 作为流元素出现，编码时原样重放（`writeObject` 内部调用 `reset()` 时也可能出现在自定义段）。
- **primitive 数组用 raw bytes（P8-2）**：`int[]` 等按 wire 大端字节存 `ArrayData::Primitive`；`Graph::as_array` 只返回对象数组，primitive 用 `Graph::as_primitive_array` → `PrimitiveArray`（`len/get/bytes/iter`）。25M `int[]` 从 `JValue` 32 B/元素（约 800 MB）降到 4 B/元素（约 100 MB）；typed/derive/serde 走 raw 路径，回放仍逐字节一致。

- **为何不用 `Rc<RefCell<..>>` 直接建环**：`Rc` 环会泄漏，`RefCell` 与后续 serde 桥、并发使用都冲突；`Handle(u32)` + arena 让"先创建再填充"的合法做法（Java 流正是这样写的）变成自然实现。
- **代价**：所有访问都要过 arena，构造值要先 `Graph::insert_*`。对外提供 `Graph::resolve` 与 `as_str/as_map/as_list/as_object` 等视图降低使用成本。

### D2：写出一侧用 `JavaSchema` 描述符，而不是裸用 serde derive

serde 的数据模型**表达不了** Java 流的这些必需信息：字段类型码（`I`/`J`/`L...;`/`[I`）、`serialVersionUID`、`classDescFlags`（`SC_WRITE_METHOD` 等）、`HashMap` 的 capacity 头部、字段顺序。

因此主路径是自定义 trait。注意签名必须带 `Graph`：构造字符串/对象/数组时要在 arena 里分配句柄：

```rust
pub struct JavaField {
    pub name: &'static str,
    pub typecode: FieldType,
    pub class_name1: Option<&'static str>,
}

pub trait JavaSerializable: Sized {
    const CLASS: &'static str;                     // "com.example.Foo"
    const SUID: i64;                               // 显式给出，或编解码期用工具计算
    const DESCRIPTOR: &'static str;                // "Lcom/example/Foo;"，嵌套字段用
    fn schema() -> &'static [JavaField];           // 名、类型码、顺序
    fn to_jvalue(&self, g: &mut Graph) -> Result<JValue>;
    fn from_jvalue(v: &JValue, g: &Graph) -> Result<Self>;
}
```

`#[derive(JavaSerializable)]` 生成上述实现（`jser-derive` proc-macro crate）。支持的属性/字段：`#[java(class = "...", suid = ...)]`、`#[java(rename = "...")]`、`#[java(transient)]`、`#[java(flatten)]`；字段类型支持 Java 基本类型、`String`、`Option<T>`、`HashMap`/`HashSet`、`Vec<T>`（primitive/String/自定义元素数组；自定义数组的类 SUID 由默认算法按数组类名计算）、组合字段 `Option<Vec<T>>`/`Option<HashMap>`/`Vec<Option<T>>`，以及**自定义 `JavaSerializable` 类型**（嵌套对象，`Option<Custom>` 也可）：宏从子类 `class` 属性生成 `DESCRIPTOR`/`ARRAY_DESCRIPTOR`。继承通过 `#[java(flatten)]` 父类字段支持（类型安全，生成 `super_desc` 与父类 classdata 层）。unit-only enum 也可 derive：variant 默认转 UPPER_SNAKE 常量名（`rename` 覆盖），flags `0x12`、SUID 0、super `java.lang.Enum`。derive 会按“基本类型按名 → 对象/数组按名”生成 schema，并要求类型实现 `Clone`；`from_jvalue` 先经 `JavaSerializable::validate_decoded` 校验类名/SUID（P9-19），SUID 不匹配的错误包含期望值与实际值。

### D3：serde 只做可选的"便利层"，不进核心

`jser-serde` 是独立 crate（不污染核心）。serde 无法表达 Java 类名/SUID/类型码，因此由调用方提供 `JavaSchema`（class/suid/fields）：typed 路径用 `direct` 模块的 `Serializer`/`Deserializer` 直接读写 `JValue`（无 `serde_json::Value` 中间层）；`from_slice_roots` 是显式 JSON 视图（返回 `Value`）。支持**单根 POJO 的递归嵌套**：字段可为标量/String/Option/数组、嵌套 POJO（`SchemaField::nested`）、`HashMap`/`LinkedHashMap`（`SchemaField::map`）、`HashSet`（`SchemaField::set`）与枚举（`SchemaField::enumeration`）；Map/Set/枚举根通过 `to_vec_map`/`from_slice_map`、`to_vec_set`/`from_slice_set`、`to_vec_enum`/`from_slice_enum` 提供；多根流（含 `TC_RESET`）用 `MultiRootWriter`/`from_slice_multi`。serde 值是树：共享引用解码为重复子树，环无法用有限 schema 表达（真实身份/环用动态 `Graph`）。仍拒绝对象数组与未知类。

### D4：内置类用「注册表 + 特例编解码器」

`HashMap`/`HashSet`/`Date`/`BigInteger`/`BigDecimal`/`java.time.*` 等的 wire 形态不符合"字段直读"规则（它们有 `writeObject/readObject` 自定义逻辑），必须逐个实现：

```rust
pub struct Registry; // builtin::Registry
impl Registry {
    pub fn is_known(&self, class: &str) -> bool;
    pub fn decode(&self, graph: &Graph, value: &JValue) -> Result<Option<BuiltinValue>>;
}
```

解码器仍保留通用 `JValue::Obj`（字段、描述符、自定义块原样，可再编码回去）；`Registry`/`Graph::builtin` 在需要时做语义分派。未知类保持 `JValue::Obj`，但受 allowlist 门禁。

对 `Externalizable`（`OUT-01`）的边界也要写清楚：

- **未知类 + `SC_EXTERNALIZABLE|SC_BLOCK_DATA`**：按不透明块读到 `TC_ENDBLOCKDATA`，保留字节，可原样回写；不提供字段语义。
- **未知类 + 旧式 `Externalizable`（无 `SC_BLOCK_DATA`）**：无法安全跳过对端自定义字节，返回 `UnsupportedClass`（带类名），不猜测。
- **已知类**（如 `java.time.Ser`）走注册表特例，见 §4.2 第 16 条。

### D5：错误模型

`thiserror` 枚举，区分：协议错误（`BadMagic`/`BadTag`/`BadHandle`/`MalformedString`）、不支持的类（`UnsupportedClass(String)`，带上类名便于报错）、schema 不匹配（`FieldMismatch`）。decode 绝不 panic。

## 4. 协议要点（wire format 清单）

> ⚠️ 以下细节以 JDK 实现为准。标注"已核实"的条目由 `testdata/` 验证；其余条目实现前必须用 golden fixture 核对，不要凭本表直接写死。

### 4.1 常量

```
STREAM_MAGIC 0xACED   STREAM_VERSION 0x0005   baseWireHandle 0x7E0000

TC_NULL 0x70        TC_REFERENCE 0x71     TC_CLASSDESC 0x72    TC_OBJECT 0x73
TC_STRING 0x74      TC_ARRAY 0x75        TC_CLASS 0x76        TC_BLOCKDATA 0x77
TC_ENDBLOCKDATA 0x78 TC_RESET 0x79       TC_BLOCKDATALONG 0x7A TC_EXCEPTION 0x7B
TC_LONGSTRING 0x7C  TC_PROXYCLASSDESC 0x7D  TC_ENUM 0x7E

SC_WRITE_METHOD 0x01  SC_SERIALIZABLE 0x02  SC_EXTERNALIZABLE 0x04
SC_BLOCK_DATA 0x08    SC_ENUM 0x10

字段类型码：B byte  C char  D double  F float  I int  J long  S short  Z boolean
            L 对象（附 JVM 类型串，如 Ljava/lang/String;）  [ 数组
```

### 4.2 必须踩准的实现细节

1. **基本类型一律大端**（`writeInt/writeLong` 的语义），`u8` 例外。
2. **两条不同的原始字节路径（已核实，原文档写错过）**：
   - **classdata 里的基本类型字段不经过块缓冲**：`defaultWriteFields` 把该类所有基本类型字段按描述符顺序拼成一段**裸字节**整体写出，前面**没有** `TC_BLOCKDATA` 头。例：`Point` 的 `x,y` 直接是 8 个字节。
   - **自定义 `writeObject` 里的 `writeInt`/`writeUTF` 等才走块缓冲**：先进 1024 字节块，遇到对象标签或 `TC_ENDBLOCKDATA` 前 flush 成 `TC_BLOCKDATA <u8 长度> <数据>`；长度 > 255 用 `TC_BLOCKDATALONG <i32 长度>`。
   - 依据：`fixtures/object/20-object-point.ser`、`21-object-mixed-fields.ser`、`23-object-custom-write-method.ser`。
3. **字符串是"修改版 UTF-8"**（CESU-8 风格）：`NUL` 写成 `C0 80`；补充平面字符写成两个三字节代理对（`😀` 是 6 字节，而标准 UTF-8 是 4 字节）；`TC_STRING` 用 2 字节长度（上限 65535），超限用 `TC_LONGSTRING` + 8 字节长度。头部类型记入 `Slot::Str.long`，回放沿用（非规范流可能短串用 long）；新建字符串按**修改版 UTF-8 字节长度**（非 Rust `len()`）选头，>65535 才用 `TC_LONGSTRING`。**孤立代理项**（Java `String` 允许、Rust `String` 不允许）解码时返回 `MalformedString`，不 panic、不做有损替换；`char` 字段走 `JValue::Char(u16)`，保留原始 UTF-16 码元。
4. **类名字符串占句柄**：每个 `TC_CLASSDESC`、`TC_STRING`、数组、对象、enum 都依次在 wire 句柄表中拿一个新句柄；`TC_CLASSDESC` 自身的句柄在读完类名/SUID/flags 后、读字段描述符**之前**分配（`34`），对象的句柄在**读完 `classDesc` 之后、读 classdata 之前**分配。内部 `Handle` 与 wire 句柄分离（见 D1），`Reader` 维护 `wire -> Handle` 映射。
5. **类描述符只写一次**：同一类在同一条流里重复出现时，后续用 `TC_REFERENCE` + 句柄回指（`writeClassDesc` 查表）。`classDesc` 的 `classAnnotation` 以 `TC_ENDBLOCKDATA` 收尾，且该标记位于 `superClassDesc` **之前**（`20`：字段后是 `78 70`，再进入 classdata）。
6. **字段排序规则（已核实）**：`classDesc` 里的字段顺序 = **先基本类型字段（按字段名升序），再对象/数组字段（按字段名升序）**。注意这是"分组内按名字"，不是全局按名字：`Mixed` 里 `a`（int[]）字母序最前，却排在最后。classdata 的写出顺序与描述符顺序严格一致。
7. **类名两种写法并存**：`classDesc.className` 用点号（`java.util.HashMap`），而对象/数组字段的 `className1` 用 JVM 描述符（`Ljava/lang/String;`、`[I`）。解析器要分别处理，别统一替换。
8. **数组类名与 SUID**：类名是 `[I`、`[Ljava.lang.String;`（数组用点号，与第 7 条的字段描述符不同）；**数组类的 SUID 不为 0**，是算出来的（`[I` = `0x4DBA602676EAB2A5`），不能假设为 0。自定义元素数组（`[Lcom.example.Foo;`）同样用默认算法按数组类名计算（`PUBLIC|FINAL|ABSTRACT`、无接口/成员，P9-14）。基本类型数组按元素宽度连续写裸字节：`B=1, C=2, D=8, F=4, I=4, J=8, S=2, Z=1`。
9. **`SC_WRITE_METHOD` 的类**：先按 schema 写字段，再写自定义数据，最后 `TC_ENDBLOCKDATA`。
10. **`TC_RESET`**：重置 wire 句柄表（对应 Java 的 `ObjectOutputStream.reset()`），reader 必须支持；reset 后类描述符与对象句柄都从 `0x7E0000` 重新分配。内部 arena 不回收（D1），重放时按原顺序重新分配即可保持字节一致。
11. **`TC_EXCEPTION`**：写出侧异常时产生的元素；P9-3 起解析异常对象并返回 `JavaException`（class/message/字段摘要），异常类解析使用临时宽松 allowlist，不进入 `Graph` 回放。
12. **`HashMap` 的 wire 形态（已核实，JDK 17）**：classdata 是 `loadFactor`(float) + `threshold`(int) 两个基本类型字段（裸字节 8 字节，按名字升序即 loadFactor 在前），随后自定义部分是 `TC_BLOCKDATA 08` + `capacity`(int) + `size`(int) + 逐条 `key,value`，最后 `TC_ENDBLOCKDATA`。`readObject` 读但**忽略** capacity，`putMapEntries` 会重算 threshold，所以从 Rust 写出时这两个值按 `size` 反推合理值即可（Java 自己写的是 `table.length` 与 `capacity*loadFactor`）。依据：`fixtures/object/29-collection-hashmap.ser`。
13. **深度与长度上限**：解析必须设最大嵌套深度与最大元素数，否则恶意流可导致栈溢出/OOM。
14. **集合与内置类的已知布局（已核实，JDK 17）**：classdata 与自定义块不同，不能统一按"字段直读"处理。

    | 类 | flags | classdata（裸字节） | 自定义块（`TC_BLOCKDATA`） | 依据 |
    |---|---|---|---|---|
    | `ArrayList` | `0x03` | `size:int` | `size:int` + 逐元素 | `28` |
    | `LinkedList` | `0x03` | 无 | `size:int` + 逐元素 | `32` |
    | `HashMap` | `0x03` | `loadFactor:float`、`threshold:int` | `capacity:int`、`size:int`、key/value... | `29` |
    | `LinkedHashMap` | `0x02` | `accessOrder:boolean` + 父类 `HashMap` 同左 | 同 `HashMap` | `30` |
    | `HashSet` | `0x03` | 无 | `capacity:int`、`loadFactor:float`、`size:int`、元素 | `31` |
    | `Date` | `0x03` | 无 | `long`（epoch millis） | `36` |
    | `BigInteger` | `0x03` | `bitCount`、`bitLength`、`firstNonzeroByteNum`、`lowestSetBit`、`signum`（int）+ `magnitude:[B` | 空（仅有收尾 `TC_ENDBLOCKDATA`） | `36` |
    | `BigDecimal` | `0x03` | `scale:int` + `intVal:BigInteger` | 空（仅有收尾 `TC_ENDBLOCKDATA`） | `36` |

15. **引用型值统一走句柄、顶层可多元素**：`TC_REFERENCE` 返回同一个 `Handle`；编码时同一句柄首次写实体、后续写回指。顶层流按出现顺序产生 `JValue`，其中 `JValue::Reset` 表示 `TC_RESET`；`decode` 单元素便捷函数遇到多元素或 `Reset` 必须报错并提示用 `decode_stream`。依据：`08`、`33`（共享）、`34`（环）、`35`（reset）、`09`/`11`（块元素）。
16. **`java.time.Ser` 是已知的 Externalizable 特例**：`java.time.*` 经 `writeReplace` 变成 `java.time.Ser`（`36`）。它的 classDesc flags 是 `0x0C`（`SC_EXTERNALIZABLE|SC_BLOCK_DATA`）、0 个字段，数据全部在块里：`LocalDate` 为 `type=3` + `year:int` + `month:byte` + `day:byte`（`36` 的块是 `77 07 03 00 00 07 E8 02 1D`）。不要把它当普通 POJO，也不要因此实现通用 `Externalizable`（`OUT-01` 仍有效）。

> 第 2、6、12、14、16 条以及数组 SUID 曾是我的记忆推断，**已被基准数据证实或修正**——这正是 `testdata/` 存在的意义。基准数据的字节是唯一事实来源，本文与它冲突时以它为准。

### 4.3 已由基准数据核实的事实（`testdata/`，JDK 17.0.20.1）

| 事实 | 依据 |
|---|---|
| classdata 的基本类型字段是**裸字节**，无 `TC_BLOCKDATA` 头 | `20-object-point.ser`、`21-object-mixed-fields.ser` |
| 自定义 `writeObject` 里的 `writeInt`/`writeUTF` **走**块缓冲（`77 0C` 合并成一个块） | `23-object-custom-write-method.ser` |
| 字段顺序 = 基本类型（按名升序）→ 对象/数组（按名升序） | `21-object-mixed-fields.ser`（`a` 排最后） |
| `HashMap` 的 classdata 含 `loadFactor` + `threshold`，其后才是 `capacity`+`size` | `29-collection-hashmap.ser` |
| 数组类 SUID 非零（`[I` = `0x4DBA602676EAB2A5`） | `25-array-int.ser` |
| `enum` 的 flags 是 `0x12`（`SC_SERIALIZABLE` + `SC_ENUM`）、SUID 为 0、常量名以 `TC_STRING` 结尾 | `24-object-enum.ser` |
| `Integer` 走标准 classDesc（字段 `value` + 父类 `java.lang.Number`），后续同类实例用 `TC_REFERENCE` 回指 classDesc | `29-collection-hashmap.ser` |
| `java.time.*` 经 `writeReplace` 变成 `java.time.Ser` 代理 | `36-misc-java-types.ser` |
| `transient` 字段不出现在 classDesc 中 | `37-object-with-transient.ser` |
| 修改版 UTF-8：`U+0000` → `C0 80`；`U+1F600` → 6 字节代理对 | `03-string-nul.ser`、`05-string-surrogate-pair.ser` |
| 块缓冲：1024 字节块与余量 976 字节都 >255，均用 `TC_BLOCKDATALONG` | `11-blockdata-long.ser` |
| `classDesc` 的 `classAnnotation` 以 `TC_ENDBLOCKDATA` 收尾，且位于 `superClassDesc` 之前（`78 70`） | `20-object-point.ser`、`22-object-inheritance.ser` |
| `classDesc` 的 wire 句柄在写字段（`className1` 字符串）**之前**分配；因此 Node 描述符是 `0x7E0000`，两个 `className1` 字符串是 `0x7E0001/2`，对象是 `0x7E0003` | `34-ref-cycle.ser`（`b.next` 回指 `0x7E0003`） |
| classdata 按**父 → 子**顺序；每层字段后紧跟该层的自定义块。`LinkedHashMap` 的 `accessOrder` 排在父类 `HashMap` 的 entries 块之后 | `22-object-inheritance.ser`、`30-collection-linkedhashmap.ser` |
| `java.time.Ser` 的 flags 为 `0x0C`、0 字段；`LocalDate` 的块为 `type=3` + year/month/day | `36-misc-java-types.ser` |
| `ArrayList`/`LinkedList`/`HashSet`/`LinkedHashMap` 的 classdata 与块布局（见 §4.2 第 14 条） | `28`–`32` |
| `Date` 的自定义块是 `long`；`BigInteger` 有 6 个描述符字段（含 `[B magnitude`）；`BigDecimal` 是 `scale` + `intVal` | `36-misc-java-types.ser` |
| 未声明的 SUID 由 JDK 算出（`Fixtures$NoSuid` = `-3559046683250708543`） | `39-suid-computed.ser` + `build/suid.txt` |

## 5. 模块划分与 API 草图

采用 workspace，但只有 proc-macro 必须独立成 crate：

```
仓库根（workspace）
├── Cargo.toml            # workspace
├── crates/
│   ├── jser/             # 核心：wire 读写 + JValue + Registry（可再拆 core/value）
│   │   ├── src/wire/     # StreamReader / StreamWriter / BlockBuffer / Mutf8 / HandleTable
│   │   ├── src/desc/     # ClassDesc 解析/写回、字段排序、SUID 计算工具
│   │   ├── src/value/    # JValue / Slot / Graph / ClassDesc / 视图与 Rust 类型转换
│   │   ├── src/builtin/  # HashMap / HashSet / ArrayList / Date / BigInteger / java.time.Ser ...
│   │   └── tests/        # 集成测试；用 CARGO_MANIFEST_DIR 定位 ../../docs/testdata（不复制）
│   ├── jser-derive/      # #[derive(JavaSerializable)]
│   └── jser-serde/       # 可选桥（feature 默认关闭）
└── docs/testdata/        # JDK 生成的 .ser + 生成脚本（唯一事实来源）
```

API 草图：

```rust
// 读：整条流（可多元素、可含 reset）
let stream = jser::decode_stream(bytes)?;
for item in stream.items() {
    match item {
        JValue::Reset => { /* wire 句柄表已重置 */ }
        v => {
            if let Some(map) = stream.graph().as_map(v)? { /* ... */ }
        }
    }
}

// 读：primitive 数组用紧凑视图（不展开 Vec<JValue>）
if let Some(array) = stream.graph().as_primitive_array(v)? {
    assert_eq!(array.get(0), Some(JValue::Int(1)));
}

// 读：单元素便捷（多元素流会报错，提示用 decode_stream）
let pojo: MyPojo = jser::decode_typed(bytes)?;

// 写：原样重放（byte-exact 回归门禁用这个）
let out = jser::encode_stream(&stream)?;

// 写：构造新流（引用型值先入 arena 拿 Handle）
let mut g = jser::Graph::new();
let s = g.new_string("hello");
let one = g.new_int(1);
let m = g.new_map(MapKind::HashMap, [(s, one)])?;
let out = jser::encode(&g, &[JValue::Obj(m)])?;

// 安全：allowlist（默认严格：java.lang.*、java.util.*）
let allowlist = jser::Allowlist::strict().allow("com.example.*")?;
let mut r = jser::Reader::with_allowlist(bytes, Limits::default(), allowlist)?;
```

### 5.1 M2 回放门禁的边界（2026-11-03 定案）

"逐字节回放"只用于**未修改的 fixture 流**，是 P3 的回归门禁，不是验收标准（验收仍是 `AC-02` 的 Java 读回）。

- **定义**：`encode_stream(decode_stream(x)) == x`。
- **适用**：`testdata/fixtures/**` 的 43 个已提交用例；decode 后不改动 `Graph`。
- **不适用**：新建的图、修改过字段/集合/引用的图；这些只要求"Java 读回语义相等"。
- **例外**：必须登记"文件 + 原因 + 负责人 + 到期"，默认不允许；例外不放宽全局断言。
- **当前例外**：`object/50`（`TC_EXCEPTION`）按设计没有可重放的 `Graph`，由 `tests/exception.rs` 断言结构化错误；原因：fatal 流。
- **防作弊**：mutation test（解码 → 改字段/Map 条目/数组元素 → 编码 → Java 读到新值）与构造流测试（手工构造 Map/POJO/数组 → Java 读回）；`encode` 必须走 `Graph`，不得缓存原始流直接回显。
- **分级**：P1 只要求 12 个 codec 用例语义正确；P3 起要求全部常规 fixture 回放 + Java 读回（当前 42 个常规用例；`object/50` 为错误路径例外）。

### 5.2 Allowlist（2026-11-04 定案）

- **默认严格**：`decode_stream` / `Reader::new` / `decode_typed` 使用 `Allowlist::strict()`，只放行 `java.lang.*`、`java.util.*`；fixture 测试额外放行 `Fixtures$*`、`java.math.*`、`java.time.*`、`java.sql.*`、`java.util.regex.*`。
- **匹配规则**：精确类名、`prefix.*`、`Fixtures$*`（以 `$` 结尾的命名空间前缀）；裸 `*` / `Allowlist::all()` 表示全放行。非法模式返回 `InvalidSchema`。
- **检查时机**：每读到一个 `TC_CLASSDESC`（含父类链）立刻检查；数组类名 `[Lcom.example.Foo;` 提取元素类后再检查，基本类型数组直接放行。
- **错误**：未放行时返回 `UnsupportedClass(<class name>)`，不会先破坏性读取 classdata。
- **边界**：allowlist 只是入口门禁，**不是 gadget 沙箱**；放行的类仍可能执行危险的 `readObject`。默认严格 + 文档警告，完整沙箱仍是 OUT-06。
- **API**：`Allowlist::{strict,all,new,add,allow,check,is_allowed}`、`Reader::with_allowlist`、`decode_stream_with_allowlist`、`decode_typed_with_allowlist`。

### 5.3 SUID 工具（P5）

- `jser::suid::compute_default_suid(&ClassSignature) -> Result<i64>` 移植 `ObjectStreamClass.computeDefaultSUID`：类名、修饰符、排序后的接口/字段/构造器/非私有方法 → SHA-1；摘要前 8 字节**从末尾向前**拼成 i64。
- `jser::suid::parse_class_file(&[u8])` 读取 `.class` 的最小元数据（常量池、字段、方法、`<clinit>`）。
- `cargo run -p jser --bin suid -- path/to/Foo.class` 输出现值；`interop.sh` 用 `build/suid.txt` 的 `Fixtures$NoSuid = -3559046683250708543` 与之对拍。
- 限制：这是默认算法值，**优先显式声明 `suid`**；Java 版本/类成员变化会改变计算值，文档不承诺跨版本稳定。

### 5.4 内置类与 serde 支持矩阵（P6）

| 类型 | 状态 | 入口 |
|---|---|---|
| `java.util.Date` | ✅ epoch millis | `BuiltinValue::Date` |
| `java.math.BigInteger` | ✅ signum + 大端 magnitude，十进制转换 | `BuiltinValue::BigInteger` |
| `java.math.BigDecimal` | ✅ scale + `intVal`，保留 scale | `BuiltinValue::BigDecimal` |
| `java.time.Duration` | ✅ seconds + nanos | `BuiltinValue::Duration` |
| `java.time.Instant` | ✅ epoch second + nano | `BuiltinValue::Instant` |
| `java.time.LocalDate` | ✅ year/month/day | `BuiltinValue::LocalDate` |
| `java.time.LocalTime` | ✅ hour/minute/second/nano | `BuiltinValue::LocalTime` |
| `java.time.LocalDateTime` | ✅ date + time | `BuiltinValue::LocalDateTime` |
| `java.time.ZonedDateTime` | ✅ date/time + offset + zone | `BuiltinValue::ZonedDateTime` |
| `java.time.ZoneId` | ✅ region id | `BuiltinValue::ZoneId` |
| `java.time.ZoneOffset` | ✅ total seconds | `BuiltinValue::ZoneOffset` |
| `java.time.OffsetTime` | ✅ time + offset | `BuiltinValue::OffsetTime` |
| `java.time.OffsetDateTime` | ✅ date-time + offset | `BuiltinValue::OffsetDateTime` |
| `java.time.Year` | ✅ year | `BuiltinValue::Year` |
| `java.time.YearMonth` | ✅ year/month | `BuiltinValue::YearMonth` |
| `java.time.MonthDay` | ✅ month/day | `BuiltinValue::MonthDay` |
| `java.time.Period` | ✅ years/months/days | `BuiltinValue::Period` |
| `java.util.Optional` | ❌ **不是 `Serializable`**，Java 端不会产出 | — |

serde 桥额外限制：单根/多根 POJO；字段可为标量/String/`Option`/数组、递归嵌套 POJO（`SchemaField::nested`）、Map（`SchemaField::map`，HashMap/LinkedHashMap）、Set（`SchemaField::set`，HashSet）与枚举（`SchemaField::enumeration`）；Map/Set/枚举根有独立入口，多根用 `MultiRootWriter`（可插 `TC_RESET`）；共享引用按值展开、环不在树模型内（真实身份用动态 `Graph`）；不支持对象数组；`from_slice*` 必须传入 allowlist。扩展候选（直接 serde 实现）已登记在 `implementation-plan.md` §21.2。`object/46` 不含 `Collections$SynchronizedList`：其默认 SUID 跨 JDK 构建不稳定；解码支持仍在。

typed API 还支持 `&[T]`/`[T; N]` 编码为 Java 数组，以及 `BTreeMap`/`BTreeSet` 与 Java Map/Set 的互转（P9-20）。

嵌套对象：动态 `JValue` 层递归支持任意深度（`object/41` 验证 Outer→Middle→Leaf 三层，受 `max_depth` 限制）；typed 可手写递归实现；derive 支持嵌套自定义字段、`Option<Custom>`、`Vec<Custom>`、继承（`#[java(flatten)]`）与 unit-only enum；serde 通过递归 `SchemaField::nested` 支持多层 POJO。

### 5.5 `TC_EXCEPTION`（P9-3）

- **触发**：`ObjectOutputStream.writeObject` 在顶层写入过程中遇到 `IOException` 时，调用 `writeFatalException` 写入 `TC_EXCEPTION` + 异常对象；流中的对象数据可能已部分写出。
- **解码**：`read_exception` 用临时 `Allowlist::all()` 解析异常对象，仅用于构建错误；随后恢复原 allowlist。这样 `java.io.IOException`（严格默认不放开 `java.io.*`）也能报告类名/消息，且仍受深度/元素/大小限制。
- **错误**：返回 `Error::JavaException(JavaException { class, message, fields })`；`message` 取 `Throwable.detailMessage`，`fields` 是 class 链字段的受限字符串摘要（标量/字符串/对象类名/数组长度等）。
- **不回放**：异常流没有可重建的 `Graph`，`object/50` 只验证错误结构与 `tests/exception.rs`，不进入逐字节回放门禁；`emit`/Java 读回跳过该 fixture。

---

## 6. 里程碑与出口条件

| # | 内容 | 出口条件 |
|---|---|---|
| M0 ✅ | 协议探针：`MUTF-8`、块缓冲、句柄表、`classDesc` 解析；生成 fixture 的 Java 脚本 + CI（GitHub Actions 装 JDK 17） | 43 例/79031 字节；codec 手写推导与 JDK 输出逐字节一致；Rust 侧已能解码并回放全部常规用例；`object/50` 验证异常错误路径 |
| M1 ✅ | 基本类型 / `String` / 数组 / 集合的**读** | fixture 全部可解码；多元素流与 `TC_RESET` 可读 |
| M2 ✅ | **写** + 回放门禁 + 双向对拍 | 42 个常规 fixture 全部逐字节回放；`object/50` 为 fatal 错误路径，不重放；构造/修改流由 Java 读回 |
| M3 ✅ | 动态 POJO（`JValue::Obj`）+ typed API + 循环引用 + `TC_RESET` | 自引用链表/双向图可读写；`ObjectOutputStream.reset()` 的流可读写；未知类可原样回写 |
| M4 ✅ | `#[derive(JavaSerializable)]` + SUID 工具（含与 JDK `serialver` 的对拍） | derive 的结构体与等价 Java 类互通；SUID 计算与 JDK 一致 |
| M5 ✅ | 内置类补齐（`Date`、`BigInteger`、`BigDecimal`、`java.time.*`）+ `jser-serde` | 每类双向对拍；`java.time` 余项与常见值类/扩展容器已补；serde 支持递归嵌套/Map/Set/枚举/多根，树模型限制已文档化 |
| M6 ◐ | 加固：深度/长度限制、fuzz（`cargo-fuzz` 跑 30 分钟无 panic）、bench | 合成 100 MB 与 25M `int[]` bench 达标（约 2.0× RSS）；fuzz 目标/冒烟完成；30 分钟完整 fuzz 暂缓 |

## 7. 风险与缓解

| 风险 | 影响 | 缓解 |
|---|---|---|
| 协议细节靠文档记忆写错（字段顺序、块缓冲时机） | Java 端直接拒绝流 | golden fixture 驱动开发；每个细节一条对拍用例 |
| `serialVersionUID` 计算（`computeDefaultSUID`：SHA-1 over 类签名）移植不精确 | Java 端 `InvalidClassException` | 优先要求显式声明 SUID；提供计算工具并与 JDK `serialver` 对拍；文档说明"计算值仅供参考" |
| serde 数据模型无法表达环/共享 | 半成品体验 | D3 已隔离：serde 桥只做无环 POJO，超范围明确报错 |
| `TC_EXCEPTION` / 自定义 `writeObject` 语义 | 读到一半失败 | 已知异常解析为 `JavaException`；未知类降级为 `JValue::Obj`，无法降级时给出带类名的精确错误 |
| 大对象/深递归导致栈溢出 | 崩溃 | 迭代式解析器 + 显式深度/数量上限 |
| 内部句柄与 wire 句柄混淆 | reset 后引用错位、重放字节不一致 | D1 已分离：arena 单调、wire 映射随 reset 清空；`35` 专项用例 |
| 顶层多元素流被当成单根 | 只读到第一个元素/丢失 reset | `decode_stream` + `JValue::Reset`；`08`/`09`/`11`/`33`/`35` 专项用例 |
| 孤立代理项被静默替换 | 数据损坏、回写不再等价 | 解码返回 `MalformedString`；文档写明该限制 |
| `java.time.Ser` 无公开 wire 文档 | 只能靠源码/字节猜 | 以 `36` + OpenJDK 源码为准，先只保证 `LocalDate`；其余标注实验性 |
| 回放门禁误报（语义等价但字节不同） | 阻塞 P3、误伤正确实现 | 门禁只对未修改 fixture（§5.1）；构造/修改图改验 Java 读回；例外需评审登记 |
| 回放被"原样回显"绕过 | 编码器未真正实现 | mutation test + 构造流测试；`encode` 必须走 `Graph`，不缓存原始流 |
| 反序列化 gadget 风险 | 安全 | 默认 allowlist（默认只放行 `java.util.*`、`java.lang.*`、基本类型）；文档醒目警告 |
| 本机无 JDK，测试环境依赖外部 | 开发体验 | fixture 提交进仓库；CI 负责重新生成与校验 |

## 8. 已定案问题（2026-11-04 收口）

| # | 问题 | 决定 |
|---|---|---|
| 1 | 方向 | **双向**（FR-02 MUST）：读与写均已实现并通过 P3/P6/P7 回归。 |
| 2 | 目标 Java 版本 | **Java 17** 为基线：fixture 由 17.0.20.1 生成；wire 协议自 1.2 未变，JDK 25 重新生成已提交用例逐字节一致。`java.time` 等 Java 8+ 类按需支持；对受支持类，JDK 8–25 的流预期可读，但不作为 CI 门禁。 |
| 3 | serde 集成 | **已实现**为独立 `jser-serde`：schema 驱动、直连 `JValue`；自定义 derive 仍独立（`jser-derive`）。不做“无 schema 的 `Serialize` 直出”。 |
| 4 | 发布到 crates.io | **暂不发布**：0.1.0 保持 unreleased，保留 `jser`/`jser-derive`/`jser-serde` 三 crate 拆分；API 冻结与 semver 在首次发布前再评审。 |
| 5 | 性能目标 | 大块/primitive 数组解码 **≥1 GB/s**、内存 **<3×** 输入；实测 `int[]` 约 3.5–6 GB/s、100 MB 块约 4.8–6 GB/s、`int[]` RSS 约 2.03×；nightly bench 门禁按观测值 35% 兜底（见 `docs/bench.md`）。 |
| 6 | 安全默认值 | **已定案（2026-11-04）**：默认严格 allowlist（`java.lang.*`、`java.util.*`）；`Allowlist::all()` 为显式放宽，文档警告 gadget 风险。 |

> 回放门禁的边界（适用范围、例外、防作弊）已在 §5.1 定案；验收仍以 `AC-02` 为准。

## 9. 参考资料

- Java Object Serialization Specification（Java SE 17）：`java.io` 包文档中的 "Java Object Serialization Specification"，尤其 **6.4 Stream Elements** 与附录 "Object Serialization Stream Protocol"。
- JDK 源码：`ObjectOutputStream`、`ObjectInputStream`、`ObjectStreamClass`（字段排序、SUID、`BlockDataOutputStream` 的缓冲策略均在其中）。
- 同类实现可参考（用于对照行为，不复用代码）：Go 的 `github.com/tardisx/java-serialization`、Python 的 `javaobj` 系列库。
- 修改版 UTF-8：JDK `DataOutput.writeUTF` 的 javadoc。
