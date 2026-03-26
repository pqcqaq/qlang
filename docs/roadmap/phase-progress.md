# P1-P4 阶段总览

> 最后同步时间：2026-03-26

这份文档不是路线图，而是对已经完成的 P1-P4 开发工作的阶段性归档。目标是回答三个问题：

1. 每个阶段原本要解决什么问题
2. 现在到底已经做到了什么
3. 还有哪些边界是刻意保留、暂时没有继续往下做

如果需要看更细的设计拆解，请继续阅读：

- [开发计划](/roadmap/development-plan)
- [编译器流水线](/architecture/compiler-pipeline)
- [工具链设计](/architecture/toolchain)
- `docs/plans/` 下的 P3 / P4 设计稿

## 总体结论

当前仓库已经不再是“只有语言设计文档”的预研空壳，而是形成了四层真实地基：

- P1 建立了可解析、可格式化、可回归测试的前端最小闭环
- P2 建立了 HIR、名称解析、first-pass typing、统一诊断和最小 LSP/查询闭环
- P3 建立了结构化 MIR、ownership facts、cleanup-aware 分析和 closure escape groundwork
- P4 建立了 LLVM 文本后端、`ql build`、对象文件/可执行文件/静态库产物路径，以及第一版 codegen golden harness

这四期的核心价值不是“功能很多”，而是已经把前端、语义、中端、后端、CLI、LSP、测试、文档这几个大边界稳定下来。后续继续开发时，重点应该是沿着这些边界扩展，而不是回头推翻它们。

## P1: 前端最小闭环

### 阶段目标

P1 的任务是把 Qlang 从纯设计稿推进到“能读、能写、能稳定回归”的前端工程基线。重点不是语义正确性，而是：

- Rust workspace 和 crate 拆分
- lexer / parser / AST
- `ql check` 与 `qfmt`
- 可维护的 parser 测试与 formatter 稳定性测试

### 切片进度

#### P1.1 Workspace 与基础前端骨架

已完成：

- 建立 Rust workspace
- 新增 `ql-span`、`ql-ast`、`ql-lexer`、`ql-parser`、`ql-fmt`、`ql-cli`
- 建立 `ql check` / `ql fmt` 的最小 CLI 入口

这一刀解决的是“项目能不能开始长期演进”的问题，而不是语法覆盖率问题。

#### P1.2 基础声明、类型与表达式

已完成：

- package / use
- const / static / type / opaque type
- fn / trait / impl / extend / extern
- struct / data struct / enum
- generics、`where`、泛型类型应用、callable type、tuple return
- 闭包、结构体字面量、基础运算表达式
- `unsafe fn` 和 `unsafe { ... }`

这一步把语言草案里的核心声明面做成了可解析的真实前端。

#### P1.3 控制流、模式匹配与 formatter 稳定化

已完成：

- `if` / `match` expression
- `while` / `loop` / `for` / `for await`
- richer pattern：路径模式、tuple-struct 模式、struct 模式、字面量模式
- parser fixture 覆盖控制流和声明切片
- formatter 对当前语法面的稳定 round-trip

这一刀的重点是防止 parser 变成“一堆 declaration rule 的堆叠”，让控制流和模式系统尽早进入回归面。

#### P1.4 顶层声明补全与错误恢复增强

已完成：

- parser 从单文件拆成 `item` / `expr` / `pattern` / `stmt`
- 统一函数签名模型，覆盖 free function、trait method、extern function
- 修复控制流头部与 struct literal 歧义
- 加固错误恢复与 fixture 回归

这一步的价值是把前端从“能跑”提升到“后面还能继续维护”。

#### P1.5 为 HIR lowering 预留前端边界

已完成：

- AST 节点级 span
- 精确 identifier / receiver span
- 单元素 tuple type / expr 稳定 round-trip
- `*const` 指针类型、转义标识符、下划线前缀绑定、`pub extern` block round-trip
- `ql check` 根目录扫描过滤 fixture、工具输出目录和用户 scratch 目录

这一步本质上是在为 P2 的 HIR / diagnostics / query 做准备，避免后面被迫大改 AST。

### 已交付能力

- `ql check` 能稳定解析和报告 span 级语法错误
- `ql fmt` 能对当前前端语法集工作
- parser / formatter regression 基础已经建立

### 关键 crate

- `crates/ql-span`
- `crates/ql-ast`
- `crates/ql-lexer`
- `crates/ql-parser`
- `crates/ql-fmt`
- `crates/ql-cli`

### 当前边界

P1 已完成。当前不再把“补更多 parser 规则”视为主线目标，后续前端变化应服务于 HIR、typeck、MIR 和后端需求。

## P2: 语义分析与类型检查地基

### 阶段目标

P2 的目标不是一步做完整类型系统，而是建立统一语义边界，让 CLI、LSP 和后续中端都能共享同一份真相源：

- HIR
- 名称解析
- first-pass typing
- 统一 diagnostics
- 最小 position-based semantic query
- 最小 LSP

### 切片进度

#### P2.1 HIR 与统一诊断边界

已完成：

- 新增 `ql-diagnostics`
- 新增 `ql-analysis`
- 新增 `ql-hir`
- `ql check` 切到 parser -> HIR lowering -> resolve -> semantic/type diagnostics 的统一流水线
- HIR arenas、稳定 ID、local binding lowering

这一刀把“前端产物”从 AST 推进到真正能被语义阶段消费的 HIR。

#### P2.2 名称解析与作用域图

已完成：

- 新增 `ql-resolve`
- lexical scope graph 覆盖 module / callable / block / closure / match arm / for-loop
- best-effort resolution 覆盖 local、param、receiver `self`、generic、import alias、builtin type、struct literal root、pattern path root
- 记录 item scope / function scope，给 query / LSP 提供稳定锚点

当前策略是故意保守：先解析可靠根绑定，再逐步把 struct field / unique method / enum variant token 纳入 query；但仍不冒进宣称 full module-path 语义已经成立。

#### P2.3 First-pass typing

已完成：

- return-value type mismatch
- `if` / `while` / match guard 的 `Bool` 条件检查
- callable arity / argument typing
- top-level `const` / `static` value typing 传播
- tuple-based multi-return destructuring
- expected callable type 下的 direct closure checking
- struct literal field / missing-field / type checking
- positional-after-named diagnostics
- equality operand compatibility
- struct member existence checking
- pattern root / literal compatibility
- calling non-callable values

这一步已经足够支撑“中小样例的可信类型诊断”，但还不是完整类型系统。

#### P2.4 Duplicate / semantic diagnostics 硬化

已完成：

- duplicate top-level definition
- duplicate generic parameter
- duplicate function parameter
- duplicate enum variant
- duplicate method in trait / impl / extend
- duplicate pattern binding
- duplicate struct / struct-pattern / struct-literal field
- duplicate named call argument
- positional-after-named
- invalid `self` outside method receiver scope

同时，identifier / receiver span 已经从 AST 打通到 diagnostics，避免误锚到整个函数或整个语句。

#### P2.5 Position-based query 与最小 LSP

已完成：

- `ql-analysis` 暴露 `symbol_at` / `hover_at` / `definition_at`
- `ql-analysis` 暴露 `references_at`，通过稳定 symbol identity 聚合同文件 occurrence
- `ql-analysis` 暴露 `prepare_rename_at` / `rename_at`，基于同一份 `QueryIndex` 产出同文件 rename edits
- 查询覆盖 item / local / param / generic / receiver `self` / enum variant token / struct field member / explicit struct field label / unique method member / named type root / pattern path root / struct literal root
- import alias 现在是 source-backed symbol：支持 hover / definition / 同文件 references / 同文件 rename；builtin type 仍只提供 hover 级语义信息
- method declaration span 现在会精确保留到 HIR，同一 impl 里的多个方法会记录不同 function scope
- named path segment span 现在也会精确保留，enum variant 的 pattern / constructor token 可以稳定参与 query 与 LSP
- explicit struct literal / struct pattern 字段标签现在也会进入 field query surface，但 shorthand `Point { x }` token 仍刻意保守，继续落在 local/binding 语义；当从 source-backed field symbol 发起 rename 时，这些 shorthand site 会被自动扩写成显式标签
- same-file rename 当前只开放 function / const / static / struct / enum / variant / trait / type alias / local / parameter / generic / import / field；method / receiver / builtin type / 从 shorthand field token 本身发起的 field rename 与 cross-file rename 仍然刻意不开放
- 新增 `ql-lsp`
- `qlsp` 支持 open/change/close、live diagnostics、hover、go to definition、same-file find references、same-file prepare rename / rename
- LSP bridge 完成 UTF-16 position、span/range、compiler diagnostic -> LSP diagnostic，以及 analysis rename -> `WorkspaceEdit` 的边界转换

### 已交付能力

- `ql check` 已经是 parser + semantic + type 的统一入口
- `ql-analysis` 成为 CLI / LSP / 未来 IDE 能力的共享分析层
- `qlsp` 最小可用
- UI diagnostics snapshot harness 已建立

### 关键 crate

- `crates/ql-diagnostics`
- `crates/ql-analysis`
- `crates/ql-hir`
- `crates/ql-resolve`
- `crates/ql-typeck`
- `crates/ql-lsp`

### 当前边界

P2 已经完成“语义地基”，但以下部分仍刻意未完成：

- unresolved global / unresolved type 的激进报错
- module-path / ambiguous member 查询
- completion / semantic tokens
- method rename、从 shorthand field token 本身发起的 field rename，以及 cross-file rename
- 更完整的类型推断、trait solving、effect checking、flow-sensitive narrowing
- 默认参数进入 AST / HIR / typeck

## P3: MIR 与所有权分析地基

### 阶段目标

P3 的目标是让所有权、cleanup、drop、escape 这类后续高复杂度能力有稳定的中间表示和分析落点，而不是继续在 HIR/AST 上硬推。

### 切片进度

#### P3.1 Structural MIR Foundation

已完成：

- 新增 `ql-mir`
- HIR -> MIR lowering
- function body 的 stable local / block / statement / scope / cleanup ID
- `if` / block tail / `while` / `loop` / `break` / `continue` 的 CFG lowering
- `defer` 的 register / run-cleanup 显式表达
- `match` / `for` / `for await` 保持结构化 terminator
- `ql mir <file>` 可以直接渲染当前 MIR

这一刀的核心是建立“可解释、可扩展、可测试”的中端结构，而不是急着把所有控制流压扁。

#### P3.2 Ownership Facts 与显式 `move self`

已完成：

- 新增 `ql-borrowck`
- MIR local 的 `Unavailable` / `Available` / `Moved(certainty, origins)` 状态
- block entry / exit merge
- read / write / consume 事件记录
- direct local receiver 调用唯一匹配的 `move self` method 时，产出 use-after-move / maybe-moved diagnostics

这一步没有假装“borrow checker 已经完成”，而是先把 ownership facts 和第一类可信诊断做出来。

#### P3.3 Cleanup-aware ownership

已完成：

- `RunCleanup` 真实参与 ownership analysis
- deferred cleanup 按 LIFO 顺序执行
- cleanup 中的 root local reassignment 可重新建立 `Available`
- `move self` receiver 的消费时机调整到参数求值之后

这一步的价值是把 `defer` 从语法糖推进到真正影响 ownership 结果的分析面。

#### P3.3b Move closure capture ownership

已完成：

- `move` closure 创建时消费当前 body 中被捕获的 direct local
- 普通 closure capture 作为真实 read 进入 ownership facts
- `ql ownership` 可以展示 move closure capture consume 事件

#### P3.3c Explicit MIR closure capture facts

已完成：

- closure capture facts materialize 到 `Rvalue::Closure`
- ownership / debug 不再需要回头遍历 HIR 临时收集 capture 列表

#### P3.3d Closure escape groundwork

已完成：

- MIR closure 稳定 identity
- `ql ownership` 渲染 conservative may-escape facts
- 当前 escape kind 覆盖 `return` / `call-arg` / `call-callee` / `captured-by-cl*`

### 已交付能力

- `ql-analysis` 已聚合 MIR 和 borrowck 结果
- `ql mir` / `ql ownership` 已成为可用的调试入口
- ownership diagnostics 已经有第一类真实用户可见结果

### 关键 crate

- `crates/ql-mir`
- `crates/ql-borrowck`
- `crates/ql-analysis`

### 当前边界

P3 当前刻意未完成：

- 通用 call consume contract
- place-sensitive move analysis
- path-sensitive borrow / escape analysis
- cleanup closure capture / nested defer runtime modeling
- 完整 closure environment / escape graph
- drop elaboration
- `match` / `for` 更低层 elaboration
- 完整 ownership diagnostics 体系

换句话说，P3 已经把“可继续做所有权系统”的地基打好，但还没有宣称“所有权系统完成”。

## P4: LLVM 后端与原生产物地基

### 阶段目标

P4 的目标是把 MIR 真正接到原生产物链路上，同时把 driver / backend / toolchain / diagnostics / 测试的边界做稳。

### 切片进度

#### P4.1 Backend foundation

已完成：

- 新增 `ql-driver`
- 新增 `ql-codegen-llvm`
- `ql build` 接入统一 build 路径
- 默认输出 LLVM IR：`target/ql/<profile>/<stem>.ll`
- program mode / library mode 入口模型拆开
- program mode 用户 `main` lower 成内部 Qlang entry + host `main` wrapper

这一刀解决的是“Qlang 有没有真正的后端入口”，不是“后端功能是否丰富”。

#### P4.2 Object emission

已完成：

- `ql build --emit obj`
- clang-style compiler toolchain discovery
- compiler toolchain boundary 从 CLI 中抽离到 driver
- toolchain failure 保留 `.codegen.ll`

#### P4.3 Executable emission

已完成：

- `ql build --emit exe`
- link failure 额外保留 `.codegen.obj/.o`
- 基础宿主 `main` wrapper 路径打通

#### P4.4 Static library emission

已完成：

- `ql build --emit staticlib`
- archive tool discovery 与 archiver boundary
- `staticlib` 走 library mode，不再要求单文件库定义顶层 `main`
- mock compiler + mock archiver 路径已经进入测试

#### P4.4b Dynamic library emission

已完成：

- `ql build --emit dylib`
- `dylib` 走 library mode，不再要求单文件库定义顶层 `main`
- 当前要求模块至少存在一个 public 顶层 `extern "c"` 函数定义，明确把 shared-library 输出约束在可解释的 C ABI surface 上
- Windows 下会把这些 exported symbol 显式转成 `/EXPORT:<symbol>` 传给 linker
- 黑盒快照已经覆盖 `dylib` 成功路径与“无导出时拒绝构建”的失败路径

#### P4.5 Extern C direct-call foundation

已完成：

- resolve / typeck / MIR / codegen 共享 callable identity
- extern block member 不再被粗暴折叠成宿主 item
- `extern "c"` direct call 会 lower 成 LLVM `declare @symbol` + `call @symbol`
- program mode 和 library mode 两条路径都已打通
- extern block declaration 和 top-level extern declaration 两种形态都已进入回归测试

这是 P4 当前最关键的一刀，因为它把“语言能否和宿主世界协作”推进到真实后端路径，而不是停留在语义层。

#### P4.6 Codegen golden harness 与失败模型

已完成：

- `crates/ql-cli/tests/codegen.rs`
- `tests/codegen/pass/`
- `tests/codegen/fail/`
- 黑盒快照覆盖 `llvm-ir` / `obj` / `exe` / `dylib` / `staticlib`
- 增加 library-mode extern C direct-call lowering 快照
- unsupported backend features 走结构化 diagnostics，而不是静默跳过
- first-class function value 现在也会返回结构化 diagnostics，而不是 panic backend

### 已交付能力

- `ql build` 已经是可工作的后端入口
- `.ll` / `.obj` / `.exe` / `dylib` / `staticlib` 路径都已经存在
- toolchain failure model 与中间产物保留策略已建立
- codegen regression harness 已建立

### 关键 crate

- `crates/ql-driver`
- `crates/ql-codegen-llvm`
- `crates/ql-cli`

### 当前边界

P4 当前仍刻意未完成：

- 更完整的 LLVM / linker family 组合探测
- runtime startup object / richer ABI glue
- first-class function value lowering
- closure / tuple / struct / cleanup lowering
- 一般化 shared-library surface、`extern "c"` export 的 visibility/linkage 控制与更完整 ABI surface
- 更大规模的 toolchain / lowering / fail snapshot 扩容

## 阶段状态表

| 阶段 | 状态 | 结论 |
| ---- | ---- | ---- |
| P1 | 已完成 | 前端最小闭环已经稳定，后续前端修改应服务语义与中后端 |
| P2 | 已完成基础阶段 | 语义地基、统一诊断、最小查询/LSP 已建立 |
| P3 | 已完成基础阶段 | MIR、ownership facts、cleanup-aware 分析与 closure groundwork 已建立 |
| P4 | 已完成基础阶段 | LLVM backend、artifact pipeline、extern C direct-call foundation 与 codegen harness 已建立 |

## 对后续开发的直接建议

P1-P4 之后，不应再回到“大而化之地继续堆功能”的工作方式。更合理的推进方向是：

1. 在 P3 上继续补一般化 ownership / borrow / drop，而不是回头重做 MIR
2. 在 P4 上继续补 lowering 与 ABI/runtime，而不是推翻 driver/codegen 边界
3. 在 P2 查询层之上继续补 references / rename / completion，而不是重写语义入口
4. 所有新增能力都要沿着现有 crate 边界、失败模型和测试矩阵扩展

这才是当前仓库最重要的阶段性成果：不是某个单点功能，而是已经有了一条后续不需要大规模返工的主干路径。
