# 编译器流水线

## 实现语言选择

推荐使用 Rust 实现 Qlang 工具链。不是因为“Rust 最潮”，而是因为它在这件事上工程收益最高：

- 内存安全和并发安全更适合长期维护大型编译器
- 生态中已有较成熟的 parsing、diagnostic、arena、interner、LSP、LLVM 绑定方案
- 对构建 CLI、增量缓存和测试工具也足够友好

## 总体流水线

```text
source
  -> lexer
  -> parser
  -> AST
  -> HIR lowering
  -> name resolution
  -> type / trait / effect checking
  -> MIR
  -> ownership / borrow / escape analysis
  -> monomorphization
  -> LLVM IR
  -> object / library / executable
```

如果要看“当前代码到底按什么算法工作、每层输入输出是什么、未来应把新能力接到哪一层”，直接看：

- [实现算法与分层边界](/architecture/implementation-algorithms)

## 分层原则

### AST

- 保留接近源码的结构
- 方便格式化器和语法级诊断
- 不承载复杂语义
- `item` / `type` / `stmt` / `pattern` / `expr` 节点都应直接携带 span，避免后续 diagnostics 与 LSP 反向逼迫 AST 重构
- 控制流头部表达式与普通表达式要允许有不同解析约束，例如 `if` / `while` / `for` / `match` 头部不能被结构体字面量歧义污染

当前前端实现已经按职责拆开 parser 入口：

- `item` 负责顶层声明
- `expr` 负责表达式和 postfix 规则
- `pattern` 负责绑定与 `match` 模式
- `stmt` 负责 block 和控制流语句
- 函数签名抽象可同时服务普通函数、trait item 和 extern item

这个边界要继续保持，避免未来把 parser 重新写回一个巨型文件。

### HIR

- 提供语义导向、可稳定引用的中间前端表示
- 作为 LSP 语义查询的重要基础
- 对上层工具最友好

截至 2026-03-25，HIR 的第一层地基已经实际落地，而不是停留在文档里：

- 新增 `ql-hir` crate，负责 AST 到 HIR 的 lowering
- HIR 中的 `item` / `type` / `block` / `stmt` / `pattern` / `expr` / `local` 全部进入独立 arena
- 从第一天开始引入稳定 ID，而不是后续再为 LSP 和增量分析返工
- pattern 里的绑定名在 lowering 时就被转成 `LocalId`
- AST 现在额外保留 declaration name、generic param、regular param、pattern field、struct literal field、named call arg、closure param 的精确 name span，避免语义诊断继续依赖粗粒度 fallback
- HIR 会在 lowering 时主动正规化 surface shorthand：`Point { x }` 模式字段会变成真实 binding pattern，`Point { x }` 结构体字面量字段会变成真实 `Name("x")` 表达式
- HIR 仍然保留 span，供 diagnostics、name resolution 和 IDE 查询复用

这一版 HIR 仍然是“语义前置层”，不会把名称解析、约束求解和查询系统硬塞进同一层，避免在 P2 初期把抽象提前做死。

### Name Resolution

- 在 HIR 之上单独建层，而不是把作用域和名称查找散落进 type checker
- 负责构造 lexical scope graph
- 负责建立值命名空间和类型命名空间的第一层引用关系
- 作为未来类型约束、LSP 查询和 go-to-definition 的共用基础

截至 2026-03-25，名称解析的第一层实现也已经实际落地：

- 新增 `ql-resolve` crate
- `ql check` 流水线现在是 parser -> HIR lowering -> resolve -> semantic checks
- scope graph 已覆盖 module、callable、block、closure、match arm、for-loop binding scope
- 当前已支持 best-effort 解析：local binding、regular param、receiver `self`、generic param、top-level item、import alias、builtin type、struct literal root、pattern path root
- 当前 diagnostics 采取保守策略，只落地绝对可靠的一条语义错误：method receiver 作用域外非法使用 `self`
- 对 unresolved global / unresolved type 的系统性报错刻意延后，直到 import / module / prelude 语义稳定，否则会把现有 fixture 误判成失败

### Type Checking

- 在 `ql-resolve` 之后消费 scope graph 和 name resolution 结果
- 先做 first-pass typing，而不是一开始就做完整 Hindley-Milner / trait solver / effect system
- 通过 `unknown` 作为受控退化点，避免在未建模模块语义、成员解析和索引协议前制造大面积假阳性

截至 2026-03-25，`ql-typeck` 已经从 duplicate checker 演进为第一版真正的类型检查层：

- duplicate-oriented diagnostics 继续保留
- 新增 return-value 类型检查
- 新增 `if` / `while` / match guard 的 `Bool` 条件检查
- 新增 callable 调用的 arity / argument type checking
- 新增 top-level `const` / `static` 值引用的声明类型传播
- 新增 tuple-based multi-return destructuring 的第一层约束
- 新增 direct closure against expected callable type 的 first-pass checking
- 新增 struct literal 的 field / missing-field 检查
- 新增 equality operand compatibility checking
- 新增 struct member existence checking
- 新增 pattern root / literal compatibility checking

当前依然保守的边界：

- 未解析成员调用、索引协议、import prelude 细节时，表达式类型会主动退化为 `unknown`
- unresolved global / unresolved type diagnostics 仍然延后
- 完整 trait solving、泛型实参推断、effect checking 和 flow-sensitive narrowing 还未开始
- 默认参数仍停留在语言设计稿，没有进入当前 AST / HIR / type checker 的实现边界

### Analysis Boundary

- 新增 `ql-analysis` crate，作为 parse / HIR / resolve / typeck 的统一分析入口
- `ql-analysis::analyze_source` 在 parse 成功后始终返回可查询的 `Analysis`
- 当前 `Analysis` 已暴露：
  - AST / HIR snapshot
  - `ResolutionMap`
  - `TypeckResult`
  - 聚合后的 diagnostics
  - `expr` / `pattern` / `local` 的第一层类型查询
  - 基于源码 offset 的 `symbol_at` / `hover_at` / `definition_at`
- 当前位置语义查询已覆盖：
  - top-level item name
  - local binding
  - regular parameter
  - generic parameter
  - receiver `self`
  - named type root、pattern path root、struct literal root
  - import / builtin type 的 hover 级信息
- `ql-resolve` 现在额外保留 item scope 和 function scope，`ql-analysis` 不再需要靠“重放 resolver 遍历顺序”去回推参数和泛型的声明位置
- receiver parameter 的精确 span 也已从 AST 打通到 HIR，查询和 diagnostics 会锚定 `self` / `var self` / `move self` 本身，而不是整个函数 span
- 这层边界先服务 CLI，并为后续 LSP 的 hover / definition / rename 打稳定地基
- 现在 `ql-lsp` 已经成为这层边界的第一个真实消费者，而不只是“未来会用到”
- 当前仍刻意不宣称完整 member / method / variant / module-path 查询，这部分要等更完整的语义面成熟后再继续扩展

### MIR

- 明确控制流和所有权动作
- 适合 borrow、escape、drop、effect lowering
- 适合后续优化和代码生成准备

当前已经真正落地的 P3.1 切片是：

- 新增 `ql-mir` crate，承接 HIR -> MIR lowering
- MIR body 已包含：
  - stable body / block / statement / local / scope / cleanup IDs
  - basic block + terminator 结构
  - local / temp / return slot / lexical scope
  - `defer` 注册与显式 cleanup 执行步骤
- `if` / block tail / `while` / `loop` / `break` / `continue` 已 lower 到显式 CFG
- `match` 与 `for` / `for await` 暂时保持为“结构化高层 terminator”，而不是现在就硬编码成低层状态机
- `ql-analysis` 已把 MIR 纳入统一分析快照，`ql mir` 可以直接渲染当前 MIR 文本

当前已经继续落地的 P3.2 切片是：

- 新增 `ql-borrowck` crate，明确作为 MIR 之上的独立 ownership facts 分析层
- 当前 analysis 只跟踪 MIR local 的 moved-vs-usable 状态，不假装已经实现完整 borrow checker
- block entry / exit 状态、read / write / consume 事件以及 merge 规则已经固定成可测试输出
- 当前只有一种用户可见消费语义会触发诊断：
  - direct local receiver
  - 唯一匹配的 method candidate
  - receiver 为 `move self`
- `ql-analysis` 已把这层结果纳入统一分析快照，`ql ownership` 可以直接渲染当前 ownership facts
- 当前已经继续推进的 P3.3 子切片是 cleanup-aware ownership：
  - `RunCleanup` 会真实驱动 deferred expr 的 local read / consume / root-write
  - deferred cleanup 的 LIFO 运行顺序现在会影响 ownership diagnostics
  - `move self` 的消费时机已经调整为参数求值之后，避免把错误调用顺序固化进分析层
- 当前同一阶段还补上了 move closure capture ownership：
  - `move` closure 创建时会消费当前 body 的 direct-local captures
  - 普通 closure capture 也会作为一次真实 read 进入 ownership facts
  - closure capture facts 现在直接 materialize 到 `Rvalue::Closure`，后续 ownership / escape / drop 工作不需要再从 HIR 临时回推 capture 列表
  - closure 现在还拥有稳定 MIR identity，`ql ownership` 已能渲染第一版 conservative may-escape facts（`return` / `call-arg` / `call-callee` / `captured-by-cl*`）

这意味着 P3 的下一步应继续建立在这层 MIR + ownership facts 之上做更一般的 call contract、borrow / escape 分析和 drop elaboration，而不是重新回头改 HIR。

这种分层能避免“一套 AST 硬扛一切”导致的后期崩塌。

## 查询系统

编译器、LSP 和文档生成都应该建立在统一查询系统上。推荐引入增量查询架构，核心思想：

- 每个分析结果都是可缓存 query
- 输入变更后只重新计算受影响节点
- 编译器命令行与 LSP 共用数据库

这样做的好处非常直接：

- `ql check` 和 `qlsp` 不会各自维护一套语义逻辑
- IDE 响应速度更可控
- 后续重构和诊断增强成本更低

## 诊断架构

诊断系统应单独成层，而不是散落在各模块里。建议包含：

- span 与文件映射
- 错误码
- 主错误与附加说明
- fix-it 建议
- 机器可读 JSON 输出

当前实现已经完成第一层抽离：

- 新增 `ql-diagnostics` crate，统一承载 `Diagnostic` / `Label` / renderer
- `ql check` 已不再只会打印 parser 错误，而是统一输出 parser 与 semantic diagnostics
- duplicate diagnostics 现在区分 primary / secondary label，header 会稳定锚定在真正的 duplicate 位置，而不是偶然取第一个 label
- 当前 duplicate 语义检查已经覆盖 top-level definition、generic parameter、function parameter、enum variant、trait/impl/extend method、pattern binding、struct field、struct-pattern field、struct-literal field、named call arg
- 当前调用参数结构检查还会拒绝“命名参数开始之后又出现位置参数”的调用形式，和语法设计文档保持一致
- 现阶段先覆盖文本输出；错误码、fix-it 和 JSON 输出留给后续切片

## LLVM 后端边界

不要让 LLVM 污染整个编译器架构。建议：

- 语义分析和中间表示与 LLVM 解耦
- LLVM 只在 codegen 层出现
- 为未来可能的解释器、WASM 或自定义后端保留空间

截至 2026-03-26，这条边界已经开始进入真实实现，而不再只停留在原则层：

- 新增 `ql-driver`，负责 build 请求、分析调用和产物输出
- 新增 `ql-codegen-llvm`，只消费 `HIR` / `resolve` / `typeck` / `MIR`
- 当前 `ql build` 已经能把受控 MIR 子集 lower 成文本 LLVM IR，并继续经 compiler/archive toolchain 产出 `.obj` / `.o`、基础 `.exe`、受约束的 `.dll` / `.so` / `.dylib`，以及 `.lib` / `.a`
- `ql-driver` 现在还额外承接了最小 C API 头文件投影：`ql ffi header` 会复用 analysis 结果筛选 public 顶层 `extern "c"` 定义、顶层 `extern "c"` 声明和 `extern "c"` block 声明，并按 export/import/both surface 输出确定性的 `.h` 文件
- `ql-driver` 现在还会把这套 header 投影逻辑挂到 library build 路径上：`ql build --emit dylib|staticlib --header*` 会在主 artifact 成功后直接生成 sidecar header，并把它作为 build artifact 的一部分返回给 CLI
- `ql-codegen-llvm` 现在区分 program mode 与 library mode：前者会把用户态 `main` lower 成内部符号并补宿主 wrapper，后者则直接导出 free function 集合
- 当前还新增了一层 callable identity：顶层函数与 `extern` block 声明会统一走 `FunctionRef`，因此 extern C direct call 不再是 parser-only 语法，而是能进入 typeck / MIR / LLVM IR 的真实后端路径
- 顶层 `extern "c"` 函数定义现在也已经进入真实后端路径，并会使用稳定 C 符号名而不是内部 mangling
- `ql-driver` 现在还会在 `dylib` 请求里先投影 exported C symbol 列表，再把这份符号集传给 toolchain；Windows 下会显式把它们转成 `/EXPORT:<symbol>` linker 参数
- build-side header sidecar 只允许用于 `dylib` / `staticlib`，会提前拒绝与主 artifact 路径冲突的 `--header-output`，并在 sidecar 失败时回收刚生成的 library artifact，避免 pipeline 暴露半成功状态
- 当前仍然故意不把独立 linker family discovery、runtime startup object、任意 shared-library surface 和更完整平台差异一次性揉进同一刀实现里

也就是说，P4 先固定“后端放在哪一层、如何失败、如何测试、如何贯通 artifact pipeline”，再继续补更完整的链接与运行时能力。

## 测试策略

编译器项目不能只靠单元测试。必须同时具备：

- parser fixture / snapshot tests
- typecheck tests
- UI diagnostics tests
- codegen golden tests
- 运行时集成测试
- FFI 集成测试

当前状态：

- parser fixture tests 已稳定
- `ql-analysis` unit tests 已覆盖：
  - parse-success + semantic-error analysis snapshots
  - resolution diagnostics aggregation
  - `expr` / `local` type queries
- HIR lowering tests 已拆到 `crates/ql-hir/tests/`，并覆盖 shorthand normalization、closure param span、named call arg span
- name resolution tests 已拆到 `crates/ql-resolve/tests/`，并按 value / type / scopes / rendering 分组
- semantic tests 已拆到 `crates/ql-typeck/tests/`，并按 duplicates / typing / rendering 分组
- 精确 name span 与 shorthand lowering 回归已建立
- 黑盒 UI diagnostics snapshot harness 已建立：
  - fixture 位于仓库根 `tests/ui/`
  - `crates/ql-cli/tests/ui.rs` 会驱动真实 `ql` 二进制
  - 当前已锁住 parser / resolve / duplicate-semantic / type diagnostics 的最终 stderr 输出
- 黑盒 FFI header snapshot harness 已建立：
  - `crates/ql-cli/tests/ffi_header.rs`
  - `tests/codegen/pass/extern_c_export.h`
  - `tests/codegen/pass/extern_c_surface.imports.h`
  - `tests/codegen/pass/extern_c_surface.ffi.h`
- 黑盒 `ql build` snapshot 现在还额外锁住 library build sidecar header：
  - `tests/codegen/pass/extern_c_export.h`
  - `tests/codegen/pass/extern_c_library.imports.h`
- 黑盒 `ql build` snapshot 现在也锁住 build-side `both` header 在 imported-host library 上的输出：
  - `tests/codegen/pass/extern_c_import_top_level.ffi.h`
- 真实 FFI smoke harness 现在不再手写 prototype，并且已经改成在同一次 `ql build --header-output` 中同时拿到 library artifact 与 C header，再让宿主消费生成的 header
- 真实 FFI smoke harness 现在还覆盖“Qlang 导出函数体内调用宿主提供的 imported C symbol”，并且同时验证 extern block 与 top-level extern 两种导入语法

这也是目录结构设计要前置考虑的原因。
