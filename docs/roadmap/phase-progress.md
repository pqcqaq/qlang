# P1-P7 阶段总览

> 最后同步时间：2026-03-29

这份文档不是路线图，而是对当前已经完成与正在推进的 P1-P7 开发工作的阶段性归档。目标是回答三个问题：

1. 每个阶段原本要解决什么问题
2. 现在到底已经做到了什么
3. 还有哪些边界是刻意保留、暂时没有继续往下做

如果需要看更细的设计拆解，请继续阅读：

- [开发计划](/roadmap/development-plan)
- [编译器流水线](/architecture/compiler-pipeline)
- [工具链设计](/architecture/toolchain)
- [`/plans`](/plans/) 下的合并设计稿与归档切片稿

## 总体结论

当前仓库已经不再是“只有语言设计文档”的预研空壳，而是形成了六层真实地基：

- P1 建立了可解析、可格式化、可回归测试的前端最小闭环
- P2 建立了 HIR、名称解析、first-pass typing、统一诊断和最小 LSP/查询闭环
- P3 建立了结构化 MIR、ownership facts、cleanup-aware 分析和 closure escape groundwork
- P4 建立了 LLVM 文本后端、`ql build`、对象文件/可执行文件/静态库产物路径，以及第一版 codegen golden harness
- P5 建立了最小 C ABI 互操作闭环，包括导出、导入、头文件生成、静态库/动态库集成和 sidecar header
- P6 建立了 same-file editor semantics 强化层，把 query / rename / completion / semantic tokens / LSP bridge 做到更稳定的一致性

这几期的核心价值不是“功能很多”，而是已经把前端、语义、中端、后端、FFI、CLI、LSP、测试、文档这几个大边界稳定下来。后续继续开发时，重点应该是沿着这些边界扩展，而不是回头推翻它们。

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
- source-level fixed array type expr `[T; N]`
- homogeneous array literal inference 与 expected fixed-array context 下的 array item type checking
- 保守 tuple / array indexing：array element projection、支持 lexer-style integer literal 的 constant tuple indexing、array index type checking、tuple out-of-bounds diagnostics
- same-file local import alias value/callable typing（function / const / static）
- comparison operand compatible-numeric checking
- bare mutable binding assignment diagnostics（`var` local / `var self`）
- const / static / function / import assignment-target diagnostics
- explicit unsupported member / index assignment-target diagnostics
- ambiguous method member diagnostics
- invalid projection receiver diagnostics
- invalid struct-literal root diagnostics
- invalid pattern-root shape diagnostics
- invalid path-pattern root diagnostics
- unsupported const/static path-pattern diagnostics
- positional-after-named diagnostics
- equality operand compatibility
- struct member existence checking
- pattern root / literal compatibility
- calling non-callable values

这一步已经足够支撑“中小样例的可信类型诊断”，但还不是完整类型系统。

当前仍刻意保留的边界：

- assignment target 现在仍然只在 bare binding 级别开放真实写入语义；但 `const` / `static` / function / import binding 已有显式不可赋值诊断，member/index place assignment 也已改成显式 unsupported 诊断，避免静默漏诊
- ambiguous method 现在已经会给出显式 type diagnostics，但 same-file query / completion / rename 仍然只接受唯一 candidate，不会伪造 ambiguous member truth surface
- invalid projection receiver diagnostics 现在也只覆盖已知必错的类型；generic、unresolved 与 deeper import-module case 仍刻意保守
- invalid deeper path-like call 现在也有显式回归覆盖：当 receiver 已知必错时，不会继续偷用 root function/import signature，因此 `ping.scope(true)` 这类 case 不会再额外冒出伪造的 call-argument mismatch
- invalid struct-literal root diagnostics 现在也覆盖 builtin / generic root、root 已解析成功且明确不支持 struct-style 字段构造的 case，以及 same-file 已解析二段 enum variant path 上的 unknown variant；deeper module-path 仍刻意保守
- unsupported 或仍 deferred 的 struct literal root 现在也会回退成 `unknown`，避免再泄漏伪造的具体 item type 并触发误导性的后续 return/assignment mismatch
- deferred multi-segment type path 现在也会保持 source-backed `Named` 形态，不再把 same-file local item / import alias 过早 canonicalize 成具体 item type
- deferred multi-segment `impl` / `extend` target 现在也有显式回归覆盖：它们不会再被错误投影到 concrete local receiver surface 上，因此 `Counter.Scope.Config` 这类 deferred path 的方法不会伪装成 same-file `Counter` 的成员
- invalid pattern-root shape diagnostics 现在也只覆盖 pattern root 已解析成功且明确不支持当前 struct/tuple 构造形状的 case，以及 same-file 已解析二段 enum variant path 上的 unknown variant；path pattern shape 与 deeper module-path 仍刻意保守
- invalid path-pattern root diagnostics 现在也只覆盖 path root 已解析成功且明确不支持 bare path 形状的 case；unit variant 仍允许，同文件已解析二段 enum variant path 的 unknown variant 现在也会显式报错
- const/static bare path pattern 现在也已改成显式 unsupported diagnostics，但仍只覆盖 same-file root 与 same-file local import alias；cross-file / deeper module-path constant pattern 语义仍刻意保守

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
- `ql-analysis` 暴露 `completions_at`，基于 resolver lexical scope 与同一份 symbol identity 产出同文件 completion 候选
- `ql-analysis` 暴露 `semantic_tokens()`，基于同一份 `QueryIndex` occurrence 产出 source-backed semantic token occurrence
- 查询覆盖 item / local / param / generic / receiver `self` / enum variant token / struct field member / explicit struct field label / unique method member / named type root / pattern path root / struct literal root
- import alias 现在是 source-backed symbol：支持 hover / definition / 同文件 references / 同文件 rename / semantic tokens；builtin type 则继续作为非 source-backed stable symbol 参与 hover / references / semantic tokens，但不提供 definition / rename
- `ql-typeck` 现在还会把同文件单段 local import alias 规范化回本地 item，用于 struct literal 字段检查、struct / enum pattern root 检查，以及同文件 function / const / static item alias 的 value typing / callable signature
- `ql-typeck` 现在还会把 enum struct-variant literal 的字段检查接到同一条路径上，same-file local import alias -> local enum item 也会复用这条 canonicalization；这仍是 typing 能力，不代表 query surface 已经支持 variant field symbols
- `ql-typeck` 现在还会对 struct pattern 的未知字段报错，这条校验同样会复用同文件 local import alias -> local item 的 canonicalization
- method declaration span 现在会精确保留到 HIR，同一 impl 里的多个方法会记录不同 function scope
- named path segment span 现在也会精确保留，enum variant 的 pattern / constructor token 可以稳定参与 query 与 LSP
- explicit struct literal / struct pattern 字段标签现在也会进入 field query surface，但 shorthand `Point { x }` token 仍刻意保守，继续落在 local/binding 语义；当从 source-backed field symbol 发起 rename，或从该 shorthand token 上发起 renameable binding symbol 的 rename 时，这些 shorthand site 都会被自动扩写成显式标签
- 这条 shorthand binding rename 回归现在还额外锁住了 const / static item，避免字段标签在 item-value shorthand 场景下被误改
- function shorthand-binding rename parity 现在也有显式回归覆盖，锁住了 `Ops { add_one }` 这类 shorthand token 在 analysis / LSP 两层的 same-file prepare-rename / rename 一致性，保证 field label 保留且 rename 只落到 function declaration / use
- type-namespace item same-file rename 现在也有显式回归覆盖，锁住了 `type` / `opaque type` / `struct` / `enum` / `trait` 在 analysis / LSP 两层的共享 item identity
- type-namespace item same-file references / semantic tokens 现在也有显式回归覆盖，锁住了 `type` / `opaque type` / `struct` / `enum` / `trait` 的 query / highlighting 一致性
- type-namespace item same-file hover / definition 现在也有显式回归覆盖，锁住了 `type` / `opaque type` / `struct` / `enum` / `trait` 的导航与悬浮一致性
- type-namespace item aggregate parity 现在也有显式回归覆盖，锁住了 `type` / `opaque type` / `struct` / `enum` / `trait` 这组 same-file item surface 在 analysis / LSP 两层的聚合 hover / definition / references / semantic-token 一致性
- global value item same-file query 现在也有显式回归覆盖，锁住了 `const` / `static` 在 analysis / LSP 两层的 hover / definition / references / semantic-token 一致性
- extern callable same-file parity 现在也有显式回归覆盖，锁住了 `extern` block function declaration、顶层 `extern "c"` declaration，以及顶层 `extern "c"` function definition / call site 在 analysis / LSP 两层的 hover / definition / references / rename / semantic-token 一致性
- extern callable value completion 现在也有显式回归覆盖，锁住了 `extern` block function declaration、顶层 `extern "c"` declaration，以及顶层 `extern "c"` function definition 在 analysis / LSP 两层的 `FUNCTION` completion item 映射、detail 与 text-edit 一致性
- free function query parity 现在也有显式回归覆盖，锁住了 ordinary free function direct call site 在 analysis / LSP 两层的 hover / definition / references 一致性，而不是只靠 completion / rename 或聚合 root-binding 测试间接覆盖
- free function semantic-token parity 现在也有显式回归覆盖，锁住了 ordinary free function declaration / direct call site 在 analysis / LSP 两层的 semantic-token 一致性，而不是只靠聚合 semantic-token 快照间接覆盖
- callable surface aggregate parity 现在也有显式回归覆盖，锁住了 `extern` block callable、顶层 `extern "c"` 声明、顶层 `extern "c"` 定义与 ordinary free function 这组 same-file callable surface 在 analysis / LSP 两层的聚合 hover / definition / references / semantic-token 一致性
- plain import alias same-file parity 现在也有显式回归覆盖，锁住了 `import` binding 在 analysis / LSP 两层的 hover / definition / references / semantic-token 一致性
- plain import alias completion parity 现在也有显式回归覆盖，锁住了 `import` binding 在 analysis / LSP 两层的 type-context completion 与 `MODULE` completion item 映射
- free function completion parity 现在也有显式回归覆盖，锁住了 lexical value completion 中的 free-function candidate 以及 LSP `FUNCTION` completion item 映射
- plain import alias value completion parity 现在也有显式回归覆盖，锁住了 lexical value completion 中的 source-backed `import` candidate 以及 LSP `MODULE` completion item 映射
- builtin / struct type completion parity 现在也有显式回归覆盖，锁住了 type-context completion 中的 builtin type / local struct candidate 以及 LSP `CLASS` / `STRUCT` completion item 映射
- type alias completion parity 现在也有显式回归覆盖，锁住了 same-file type-context completion 中的 `type alias` candidate 以及 LSP `CLASS` completion item 映射
- opaque type completion parity 现在也有显式回归覆盖，锁住了 same-file type-context completion 中的 `opaque type` candidate 以及带 `opaque type ...` detail 的 LSP `CLASS` completion item 映射
- generic completion parity 现在也有显式回归覆盖，锁住了 same-file type-context completion 中的 `generic` candidate 以及 LSP `TYPE_PARAMETER` completion item 映射
- enum completion parity 现在也有显式回归覆盖，锁住了 same-file type-context completion 中的 `enum` candidate 以及 LSP `ENUM` completion item 映射
- trait completion parity 现在也有显式回归覆盖，锁住了 same-file type-context completion 中的 `trait` candidate 以及 LSP `INTERFACE` completion item 映射
- field completion parity 现在也有显式回归覆盖，锁住了 stable receiver member completion 中的 `field` candidate 以及 LSP `FIELD` completion item 映射
- unique method completion parity 现在也有显式回归覆盖，锁住了 stable receiver member completion 中的唯一 `method` candidate 以及 LSP `FUNCTION` completion item 映射
- const / static completion parity 现在也有显式回归覆盖，锁住了 same-file lexical value completion 中的 `const` / `static` candidate 以及 LSP `CONSTANT` completion item 映射
- local value completion parity 现在也有显式回归覆盖，锁住了 same-file lexical value completion 中的 `local` candidate 以及 LSP `VARIABLE` completion item 映射
- parameter completion parity 现在也有显式回归覆盖，锁住了 same-file lexical value completion 中的 `parameter` candidate 以及 LSP `VARIABLE` completion item 映射
- lexical value candidate-list parity 现在也有显式回归覆盖，锁住了 import / const / static / extern callable / free function / local / parameter 这些 same-file value candidate 的完整有序列表、detail 渲染与 replacement text-edit 投影
- enum variant completion parity 现在也有显式回归覆盖，锁住了 parsed enum path completion 中的 `variant` candidate 以及 LSP `ENUM_MEMBER` completion item 映射
- import alias variant completion parity 现在也有显式回归覆盖，锁住了 local import alias -> same-file enum item 这条 parsed variant-path completion 中的 `variant` candidate 以及 LSP `ENUM_MEMBER` completion item 映射
- import alias struct-variant completion parity 现在也有显式回归覆盖，锁住了 local import alias -> same-file enum item 这条 struct-literal variant-path completion 中的 `variant` candidate 以及 LSP `ENUM_MEMBER` completion item 映射
- remaining variant-path completion parity 现在也有显式回归覆盖，锁住了 direct struct-literal path 与 direct/local-import-alias pattern path 上既有的 `variant` candidate 以及 LSP `ENUM_MEMBER` completion item 映射
- variant-path candidate-list parity 现在也有显式回归覆盖，锁住了 enum-root / struct-literal / pattern path 及其 same-file import-alias 镜像上下文的完整有序 `variant` candidate 列表、detail 渲染与 replacement text-edit 投影
- deeper variant-like member chain 现在也有显式回归覆盖，锁住了只有 root enum item / same-file import alias 的第一段 variant tail 才能复用 variant truth；`Command.Retry.more` / `Cmd.Retry.more` 不会再伪造同文件 query identity 或 `ENUM_MEMBER` completion
- deeper variant-like member chain 的关闭边界现在也被显式锁到 rename / semantic-token 两层，因此这类更深 member chain 既不能被错误 rename，也不会再被投影成 enum-member semantic token
- deeper struct-literal / pattern variant-like path 现在也有显式回归覆盖，锁住了只有严格两段 `Root.Variant` path 才能复用 variant truth；`Command.Scope.Config { ... }` / `Cmd.Scope.Retry(...)` 不会再伪造同文件 query / rename / semantic-token identity 或 `ENUM_MEMBER` completion
- deeper struct-like path 的 field truth 现在也有显式回归覆盖，锁住了只有严格 root struct path 才能复用 field truth；`Point.Scope.Config { x: ... }` / `P.Scope.Config { x: ... }` 不会再伪造同文件 field query / rename / semantic-token identity
- deeper struct-like shorthand token 的 lexical fallback 现在也有显式回归覆盖，锁住了 `Point.Scope.Config { x }` / `Point.Scope.Config { source }` 这类 token 继续落在 local / binding / import surface；references 与 semantic tokens 也沿用这条 lexical truth，同文件 rename 会保持 raw binding edit，不会伪造 field-label expansion
- completion filtering parity 现在也有显式回归覆盖，锁住了 lexical value visibility/shadowing 与 impl-preferred member filtering 这两条 already-supported same-file completion boundary 在 analysis / LSP 两层的一致性；其中 lexical value visibility 的聚合回归现在也已经显式覆盖 import / function / local 的 detail 与 text-edit 投影，而 impl-preferred member 聚合回归现在也已经显式覆盖 surviving candidate count 以及稳定 detail / text-edit 投影
- completion candidate-list parity 现在也有显式回归覆盖，锁住了 type-context 与 stable-member completion 的完整候选列表、排序与命名空间边界在 analysis / LSP 两层的一致性；其中 type-context 总表现在已经显式覆盖 builtin / import / struct / `type` / `opaque type` / `enum` / `trait` / generic，而 stable-member 总表现在也已经显式覆盖 method / field 的 detail 与 text-edit 投影
- shorthand query boundary parity 现在也有显式回归覆盖，锁住了 shorthand struct field token 继续落在 local/binding surface 而不是 field surface 的 same-file 查询边界在 analysis / LSP 两层的一致性
- direct query parity 现在也有显式回归覆盖，锁住了 direct enum variant token 与 direct explicit struct field label 的 same-file definition / references 在 analysis / LSP 两层的一致性
- direct semantic-token parity 现在也有显式回归覆盖，锁住了 direct enum variant token 与 direct explicit struct field label 的 same-file semantic-token/highlighting 在 analysis / LSP 两层的一致性
- direct symbol surface aggregate parity 现在也有显式回归覆盖，锁住了 direct enum variant token 与 direct explicit struct field label 这组 same-file direct-symbol surface 在 analysis / LSP 两层的聚合 hover / definition / references / semantic-token 一致性
- direct member query parity 现在也有显式回归覆盖，锁住了 direct field member 与唯一 method member 的 same-file hover / definition / references 在 analysis / LSP 两层的一致性
- direct member semantic-token parity 现在也有显式回归覆盖，锁住了 direct field member 与唯一 method member 的 same-file semantic-token/highlighting 在 analysis / LSP 两层的一致性
- direct member surface aggregate parity 现在也有显式回归覆盖，锁住了 direct field member 与唯一 method member 这组 same-file direct-member surface 在 analysis / LSP 两层的聚合 hover / definition / references / semantic-token 一致性
- impl-preferred member query parity 现在也有显式回归覆盖，锁住了 direct member query 中 `impl` 优先于 `extend` 的 same-file hover / definition / references 在 analysis / LSP 两层的一致性
- lexical semantic symbol same-file parity 现在也有显式回归覆盖，锁住了 `generic` / `parameter` / `local` / `receiver self` / `builtin type` 在 analysis / LSP 两层的 hover / definition / references / semantic-token 一致性；builtin type 仍无 definition / rename
- lexical rename parity 现在也有显式回归覆盖，锁住了 `generic` / `parameter` / `local` 在 analysis / LSP 两层的 same-file rename 行为，并继续保持 `receiver self` / `builtin type` 的 rename surface 关闭
- root value-item rename parity 现在也有显式回归覆盖，锁住了 `function` / `const` / `static` 在 analysis / LSP 两层的 same-file prepare-rename / rename 一致性，而不是只靠聚合 analysis 测试或 shorthand-binding 回归间接覆盖
- 同文件 local import alias -> local struct item 现在也会进入这条 field query / references / rename / semantic-token surface，显式字段标签和 field-driven shorthand rewrite 会继续映射回原 struct field symbol
- same-file rename 当前只开放 function / const / static / struct / enum / variant / trait / type alias / import / field / method（仅唯一 candidate）/ local / parameter / generic；ambiguous method / receiver / builtin type / 从 shorthand field token 本身发起的 field-symbol rename 与 cross-file rename 仍然刻意不开放
- same-file completion 当前会复用 `ql-resolve` 的 scope graph 和 `ql-analysis` 的 symbol data，已覆盖 lexical scope 的 value/type completion、稳定 receiver type 的 parsed member token completion、same-file parsed enum variant path completion，以及 local import alias -> local enum item 的 variant follow-through；同一条 follow-through 也已经进入 same-file query / rename / semantic token surface；completion 候选现在还会区分语义 label 与源码 insert text，因此 keyword-named escaped identifier 会继续生成合法编辑；ambiguous member completion、parse-error tolerant dot-trigger completion、import-graph/module-path deeper completion、foreign import alias variant semantics 与 cross-file/project-indexed completion 仍刻意不开放
- same-file semantic tokens 当前会复用 `ql-analysis` 的 source-backed occurrence 与 `SymbolKind`，已覆盖统一 query surface 中的稳定语义 token；ambiguous / unresolved / parse-error token 与跨文件 semantic classification 仍刻意不开放
- resolver 现在也补上了保守 unresolved diagnostics：bare value name、bare named type、single-segment pattern root 与 struct literal root 会报 unresolved，而 multi-segment module/import path 仍刻意不报
- 新增 `ql-lsp`
- `qlsp` 支持 open/change/close、live diagnostics、hover、go to definition、same-file find references、same-file lexical-scope completion、same-file parsed member-token completion、same-file parsed enum variant-path completion、local import alias -> local enum item 的 variant-path query / completion / rename / semantic-token follow-through、local import alias -> local struct item 的 struct-field query / references / rename / semantic-token follow-through、same-file semantic tokens、same-file prepare rename / rename
- `qlsp` 现在还支持 `textDocument/semanticTokens/full`，直接桥接 analysis semantic token occurrence
- LSP bridge 完成 UTF-16 position、span/range、compiler diagnostic -> LSP diagnostic，以及 analysis completion / semantic tokens / rename -> LSP 响应的边界转换
- diagnostics bridge parity 现在也有显式回归覆盖，锁住了 compiler `Warning` / `Note` 到 LSP severity 的映射、无 label diagnostics 回退到 `0:0` range 且 `related_information = None`、无 primary label 时使用第一条 label span、仅 primary label 时 `related_information = None`，以及 secondary label 的 related-information 顺序与 message 回退（默认 `related span`）和 `source = ql` 协议行为
- UTF-16 / CRLF position-range bridge parity 现在也有显式回归覆盖，锁住了 surrogate-pair 中间位置返回空、CRLF 行尾不接受越界 character、空尾行 `Position` 可回落到 EOF，以及跨 CRLF 多行 `Span -> Range` 投影与上层 hover / definition / references / completion / rename 在非法位置上的安全退空行为
- semantic-token UTF-16 / CRLF parity 现在也有显式回归覆盖，锁住了 CRLF 文本中“同一行前缀含 emoji”场景的 token 列号按 UTF-16 code unit 计数，不会按 UTF-8 字节偏移漂移
- references include-declaration parity 现在也有显式回归覆盖，锁住了 `include_declaration = false` 且仅存在定义位点时返回 `Some(empty)`（而不是 `None`）的桥接契约

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

- multi-segment unresolved global / unresolved type 的激进报错
- module-path / ambiguous member 查询
- ambiguous member completion
- ambiguous method rename、从 shorthand field token 本身发起的 field-symbol rename，以及 cross-file rename
- 通用索引协议与更宽的 indexable type surface
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
- deferred multi-segment source-backed type path 现在也有黑盒回归覆盖：backend unsupported diagnostics 会继续保留 `Cmd.Scope.Config` 这类源码路径文本，而不是把它误折叠成伪 concrete type
- cleanup/defer lowering 仍未进入 P4 支持矩阵，但完全重复的 backend unsupported diagnostics 现在会被稳定去重；backend、driver 与 CLI 也已经补上这条 rejection contract 的显式回归

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

## P5: C FFI 与宿主互操作地基

### 阶段目标

P5 的目标不是一次做完所有 FFI，而是先建立“Qlang <-> C 宿主”的最小可维护闭环，让导入、导出、头文件和真实宿主集成都有统一真相源。

### 切片进度

#### P5.1 Top-level `extern "c"` definition export

已完成：

- 顶层 `extern "c"` 函数定义允许保留 body
- codegen 会为它们生成稳定导出符号
- `staticlib` 已可直接承载这类导出

这一步把“语言能被 C 调用”从设计稿推进到真实产物层。

#### P5.2 Dynamic library emission

已完成：

- `ql build --emit dylib`
- shared-library 构建路径复用 library mode
- 当前要求模块至少有一个 public `extern "c"` 顶层定义，明确收敛在可解释的 C ABI surface 上
- Windows 下显式补 `/EXPORT:<symbol>`，避免生成“看起来成功、实际上没有导出”的 DLL

#### P5.3 Minimal C header generation

已完成：

- `ql ffi header <file>`
- exported surface header 生成
- 默认输出 `target/ql/ffi/<stem>.h`
- 当前支持标量与指针的最小 C 类型映射
- header 生成已经进入真实 FFI 集成链路，而不是停留在独立脚本
- deferred multi-segment source-backed type path 现在也有显式回归覆盖：unsupported header-signature diagnostics 会继续保留源码路径文本，而不是把 `Cmd.Scope.Config` 误写成 same-file concrete type

#### P5.4 Import / both surface projection

已完成：

- `ql ffi header --surface exports|imports|both`
- import surface / combined surface 默认输出命名已稳定
- extern block 成员和顶层 extern 声明可一起进入同一套头文件投影

#### P5.5 Build-side sidecar header

已完成：

- `ql build --emit staticlib|dylib --header`
- `--header-surface`
- `--header-output`
- sidecar header 和主产物使用同一份分析结果
- 若 sidecar 生成失败，会回滚刚生成的库产物，避免半成功状态

#### P5.6 Real C-host integration harness

已完成：

- `crates/ql-cli/tests/ffi.rs`
- static library linking smoke test
- dynamic library runtime loading smoke test
- imported-host callback harness
- Rust static-link harness（经由稳定 C ABI 直接调用导出符号）
- Rust static-link harness 也已覆盖最小宿主 callback 导入路径
- Cargo-based Rust host smoke test（临时生成最小 Cargo 工程并链接 Qlang `staticlib`）
- committed `examples/ffi-rust` 示例工程与其自动化回归
- header-surface metadata 驱动的 FFI fixture

### 已交付能力

- Qlang 已经能通过最小 C ABI surface 被宿主调用
- Qlang 也已经能声明和调用外部 C ABI 函数
- `ql ffi header` 与 `ql build --header` 已经建立真实工具链闭环
- 静态库 / 动态库 + 头文件 + C 宿主测试 已经形成可信最小链路

### 关键 crate

- `crates/ql-driver`
- `crates/ql-cli`
- `crates/ql-codegen-llvm`

### 当前边界

P5 当前仍刻意未完成：

- struct / tuple / enum 的稳定 C ABI 建模
- 更完整的布局诊断与 ABI 兼容诊断
- C++ 直接绑定生成
- Rust-specific wrapper 生成
- 自动安全包装和 richer FFI ergonomics
- 复杂 runtime / ownership 穿边界语义

## P6: LSP 与编辑器语义收口

### 阶段目标

P6 的目标不是把 IDE 能力一次做成“跨项目完整版”，而是在既有 same-file 分析边界上，把 query、rename、completion、semantic tokens 和 LSP bridge 收敛成一套稳定 truth surface。

### 切片进度

#### P6.1 Query truth surface 加固

已完成：

- same-file references / hover / definition 的真相源固定到 `QueryIndex`
- direct symbol surface、direct member surface、callable surface、type-namespace surface 都补上显式 parity 回归
- import alias、same-file local import alias -> local struct/enum item 的 follow-through 被统一收敛到 analysis/query 边界

#### P6.2 Same-file rename 收口

已完成：

- `prepare_rename_at`
- `rename_at`
- LSP `prepareRename` / `rename`
- 同文件 rename 现在覆盖 function / const / static / struct / enum / trait / type alias / import / field / method（唯一候选）/ local / parameter / generic / variant
- shorthand binding / shorthand field 的 rewrite 规则已进入稳定回归

#### P6.3 Completion 收口

已完成：

- lexical value completion
- type-context completion
- stable receiver member completion
- parsed enum variant-path completion
- import alias variant / struct-variant completion follow-through
- escaped identifier completion insert text 保真
- candidate-list parity 与 filtering parity 已锁定

#### P6.4 Semantic tokens 与桥接一致性

已完成：

- same-file semantic tokens
- analysis occurrence -> LSP legend 的稳定映射
- direct symbol/member、callable、type-namespace、import alias、variant 等 surface 都补上 semantic-token parity 回归

#### P6.5 Editor-facing aggregate hardening

已完成：

- callable / direct symbol / direct member / type namespace / value candidate / variant candidate 等 aggregate surface 回归
- impl-preferred member filtering
- lexical visibility / shadowing consistency
- analysis / LSP 两层结果顺序、detail、text edit 投影一致性

### 已交付能力

- `qlsp` 现在已经具备可信的 same-file diagnostics / hover / definition / references / completion / semantic tokens / prepare-rename / rename
- `ql-analysis` 已经成为 CLI 和 LSP 共用的 editor semantics 边界
- import alias、field、variant、method、type namespace 等易漂移 surface 已经被系统性回归锁住

### 关键 crate

- `crates/ql-analysis`
- `crates/ql-lsp`
- `crates/ql-resolve`
- `crates/ql-typeck`

### 当前边界

P6 当前仍刻意未完成：

- cross-file / workspace 索引
- parse-error tolerant dot-trigger completion
- module graph deeper completion / navigation
- ambiguous member completion 和 ambiguous method rename
- 从 shorthand field token 本身发起 field-symbol rename 的完整 editor semantics
- code actions / inlay hints / call hierarchy / project-wide rename

## P7: 并发、异步与 Rust 互操作（进行中）

### 当前范围

- 先收口语义层与诊断层，不直接跳到 runtime/codegen 大改
- 保持 conservative 策略，避免过早承诺完整 effect/Future 类型系统

### 本轮已完成

- `ql-typeck` 已新增函数级 async 上下文
- `await` / `spawn` 在非 `async fn` 内使用会给出显式 diagnostics
- 新增 `crates/ql-typeck/tests/async_typing.rs`，锁住边界行为
- `ql-resolve` 新增 async 语义查询契约：`expr_is_in_async_function` / `scope_is_in_async_function`
- `ql-analysis` 新增 `async_context_at`（`await` / `spawn` / `for await` -> 是否处于 async 函数上下文）
- 新增 `ql-analysis` async 查询回归测试，锁住查询语义
- `ql-lsp` bridge 新增 `async_context_for_analysis` 只读桥接（位置 -> async 运算符上下文）
- 新增 `ql-lsp` async 桥接回归测试，锁住 bridge 行为
- `ql-analysis` / `ql-lsp` async 查询桥接已覆盖 `for await` 运算符上下文（当前锚定 `await` 关键字 span）
- `ql-resolve` / `ql-typeck` / `ql-analysis` / `ql-lsp` 新增 `trait` / `impl` / `extend` 方法面的 async 回归：锁住方法体内 `await` / `spawn` / `for await` 的边界与查询语义
- `ql-typeck` 新增 `for await` 的 async 上下文约束（非 `async fn` 显式诊断）
- 新增 `ql-typeck` 的 `for await` 边界回归测试
- `ql-typeck` 新增 `await` / `spawn` 操作数形态约束：当前要求操作数必须是 call expression
- 新增 `ql-typeck` 的 `await` / `spawn` 非调用操作数回归测试
- `ql-typeck` 进一步收紧 `await` / `spawn` 的调用目标约束：当前不仅要求 operand 是 call expression，还要求被调用目标来自 `async fn`；同步函数、sync 方法和普通 closure/callable 值调用都会给出显式诊断
- 新增 `ql-typeck` 的 async-call-target 回归测试，覆盖 sync function / async function / method / closure callable 这几类 operand
- `ql-resolve` / `ql-typeck` / `ql-analysis` / `ql-lsp` 新增 closure async 边界回归：closure body 当前不会继承外层 `async fn` 语义上下文
- `ql-typeck` 新增 closure block 显式 `return` 回归：当 closure 有期望 callable 返回类型时，显式 `return` 会按 callable 签名检查；内层 nested closure 的 `return` 不会污染外层 closure 返回推断
- `ql-typeck` 新增保守 return-path 收口：函数与 closure body 现在会拒绝“部分路径 `return`、部分路径 fallthrough”的情形；`if` 与最小穷尽性 `match`（`_`、`Bool true/false`、enum 全 variant）已进入 all-path return 推断；带 guard 的 arm 默认仍保守，只有显式字面量 `true` guard 会计入覆盖
- `ql-typeck` 已把显式常量条件的 `if` 纳入 must-return 收口：`if true { return ... }`、`if false { ... } else { return ... }` 与 closure 中同构写法现在会被接受；`if false { return ... }` 仍不会被误判成保证返回
- `ql-typeck` 已把显式字面量 `Bool` scrutinee 的 `match` 纳入 must-return 收口：`match true/false` 会按 arm 顺序和字面量 guard 做保守裁剪；无可达 arm 或被字面量 `false` guard 挡住的唯一 arm 仍不会被误判成保证返回
- `ql-typeck` 已把非字面量 `Bool` / enum `match` 的字面量 guard 收口到有序 arm 流分析：`true if true`、`false if true`、`_ if true` 和 enum variant `if true` 现在会参与穷尽性与 must-return 推断；未知 guard 仍保持保守，不会提前裁掉后续 arm
- `ql-typeck` 新增 loop-control 语义约束：`break` / `continue` 在非 loop body 中会给出显式诊断；closure body 不会继承外层 loop-control 上下文
- `ql-typeck` 已把 must-return 收口提升到有序控制流摘要：`loop { return ... }` 现在会被接受；`break; return ...` 与“无 break 的 loop 之后再写 return”这类不可达路径不会再被误判成保证返回；更深层表达式子节点也按求值顺序参与保守 return 分析
- `ql-typeck` 已把显式常量条件的 `while` 纳入 must-return 收口：`while true { return ... }` 与 closure 中同构写法现在会被接受；`while true` 中的 `break; return ...` 和 `while false { return ... }` 仍不会被误判成保证返回
- `ql-analysis` / `ql-lsp` 已补上 loop-control 查询桥接：`break` / `continue` 现在可以像 async operator 一样通过只读分析接口查询当前位置是否位于 loop body，closure body 仍会重置外层 loop 上下文
- `ql-driver` 新增 async backend 拒绝路径回归：语义层通过后，codegen 仍会稳定给出 `async fn` unsupported 诊断
- `ql-cli` codegen 黑盒快照新增 `unsupported_async_fn_build`：锁住终端侧 async backend 拒绝输出
- `ql-driver` / `ql-cli` 已把 `dylib` 打开首个 async library 子集：带内部 async helper 的动态库现在可以在存在同步 `extern "c"` 顶层导出时通过，而公开 C header surface 仍只暴露同步导出，不泄露 `worker` / `helper` 这类 async implementation details
- `ql-driver` 新增 cleanup + async 混合诊断回归：同一文件里同时出现 `defer` 和 `async fn` 时，cleanup lowering 失败与 `async fn` unsupported 现在都会各自稳定只出现一次，不再互相吞掉或额外放大
- `ql-driver` / `ql-codegen-llvm` 新增 cleanup + `for await` 混合诊断回归：同一文件里同时出现 `defer` 与 `for await` 时，cleanup lowering 失败与 `for await` unsupported 现在都会各自稳定只出现一次，不再互相吞掉或额外放大
- `ql-driver` / `ql-codegen-llvm` 新增 cleanup + `for` 混合诊断回归：同一文件里同时出现 `defer` 与 `for` 时，cleanup lowering 失败与 `for` unsupported 现在都会各自稳定只出现一次，不再互相吞掉或额外放大
- `ql-driver` / `ql-codegen-llvm` / `ql-cli` 新增 cleanup + `for` CLI fail snapshot：用户可见 stderr 现在也稳定只出现 cleanup lowering 失败与 `for` unsupported 两条主诊断，不再额外级联 backend noise
- `ql-driver` / `ql-codegen-llvm` / `ql-cli` 新增 cleanup + `match` 混合诊断回归：同一文件里同时出现 `defer` 与 `match` 时，cleanup lowering 失败与 `match` unsupported 以及现有的 match elaboration / pattern diagnostics 现在都会稳定只出现一次，不再额外放大 backend noise
- `ql-driver` / `ql-codegen-llvm` / `ql-cli` 新增 `match` + `?` 混合诊断回归：同一文件里同时出现 helper `match` 与 main `?` 时，match lowering、pattern diagnostics 与 `?` unsupported 现在都会稳定只出现一次，不再额外放大 backend noise
- `ql-driver` / `ql-codegen-llvm` / `ql-cli` 新增 cleanup + `?` 混合诊断回归：同一文件里同时出现 `defer` 与 `?` 时，cleanup lowering 失败与 `?` unsupported 现在都会稳定只出现一次，不再额外级联 MIR elaboration noise
- `ql-driver` / `ql-codegen-llvm` / `ql-cli` 新增 cleanup + closure value 混合诊断回归：同一文件里同时出现 `defer` 与 closure 值时，cleanup lowering 失败与 closure value unsupported 现在都会稳定只出现一次，不再额外级联 backend noise
- `ql-driver` / `ql-cli` 新增 `async + generic` 并存回归：锁住多条 backend unsupported 诊断的聚合稳定性
- `ql-driver` / `ql-cli` 新增 `async + unsafe fn body` 并存回归：锁住签名级多条 backend unsupported 诊断的聚合稳定性与终端输出
- `ql-codegen-llvm` / `ql-driver` / `ql-cli` 新增结构化 MIR terminator 的 backend 拒绝回归：`match` lowering unsupported 与 `for` lowering unsupported 现在都有后端单测、driver 回归和 CLI 失败快照覆盖
- `ql-codegen-llvm` / `ql-driver` / `ql-cli` 进一步清理 backend 失败合同噪声：当 closure value、`match` lowering、`for` lowering 已经明确 unsupported 时，不再继续级联产出 `_t0` local-type 推导噪声，CLI 快照现在更接近真实主失败原因
- `ql-mir` 新增 async operator lowering 回归：`await` / `spawn` 当前会作为显式 unary rvalue 保留在 MIR 中，并消费前面物化出来的 call 结果；same-file import alias 的 async call 也会继续保留 `Import` callee，而不是退化成 opaque/unresolved operand
- `ql-cli` FFI 集成测试已补上最小 Rust host 静态链接回归：Rust harness 现在既可以直接链接 Qlang `staticlib` 调用导出函数，也可以为 Qlang 的 `extern "c"` import 提供 callback，实现最保守的双向互操作
- `ql-cli` FFI 集成测试进一步补上 Cargo-based Rust host smoke test：测试会临时生成最小 Cargo 工程并通过 `build.rs` 链接 Qlang `staticlib`，让当前 Rust 混编路径更接近真实项目工作流
- 仓库已提交 `examples/ffi-rust`：真实 Cargo host 通过 `build.rs` 编译 sibling Qlang 源码并链接 `staticlib`，同时 `ql-cli` FFI 集成测试也已回归锁住该示例的可运行性
- 新增 `crates/ql-runtime`：当前仓库已有最小 runtime/executor 抽象地基，提供 `Task` / `JoinHandle` / `Executor` trait 和单线程 `InlineExecutor`
- `crates/ql-runtime/tests/executor.rs` 已锁住 run-to-completion、`spawn` + `join`、`block_on` 与单线程执行顺序
- `crates/ql-runtime` 已固定第一批稳定 capability 名称：`async-function-bodies`、`task-spawn`、`task-await`、`async-iteration`
- `crates/ql-runtime` 已起草第一版共享 runtime hook ABI skeleton：当前固定 `async-frame-alloc`、`async-task-create`、`executor-spawn`、`task-await`、`task-result-release`、`async-iter-next` 及对应稳定符号名，并给出统一 `ccc` + opaque `ptr` 的第一版 LLVM-facing contract string
- `ql-analysis` 已暴露 `runtime_requirements()`，按源码顺序枚举当前 async surface 对应的 runtime 需求，并补上 operator span / declaration-vs-definition 边界回归
- `ql-cli` 已扩展 `ql runtime <file>`，现在会同时输出 runtime capability 需求和 dedupe 后的 runtime hook 计划，作为后续 runtime/codegen 接线前的开发者可见检查面
- `ql-driver` 已开始保守消费这份 runtime requirement surface：当前会把 `async-function-bodies`、`task-spawn`、`task-await`、`async-iteration` 映射成稳定的 build-time unsupported 诊断，并与 backend 同类 diagnostics 去重，锁住 driver/codegen 边界的拒绝合同
- `ql-codegen-llvm` 已开始直接消费共享 runtime hook ABI signatures：后端输入现在可携带 dedupe 后的 hook 列表，并渲染稳定的 LLVM `declare` 语句，避免 backend 自己复制 hook 名称或 ABI 字符串
- `ql-codegen-llvm` 现已把 body-bearing `async fn` 推进一步：backend 会统一生成 `ptr frame` 形态的真实 body symbol，并在 wrapper 中为带参数的 `async fn` 通过 `qlrt_async_frame_alloc` materialize 最小 heap frame，再交给 `qlrt_async_task_create`，先冻结 async body / wrapper / frame hydration 三层结构
- `ql-codegen-llvm` / `ql-driver` / `ql-cli` 已把 `for await` 打开首个 library-mode lowering 竖切片：`staticlib` async library body 内对 fixed array iterable 的 `for await` 现在会稳定进入 backend，并通过 index-slot + array-element load 的最小 IR 形态驱动 loop item binding；非 array iterable 与非 `staticlib` build surface 仍保持稳定 unsupported
- `ql-runtime` / `ql-cli` / `ql-codegen-llvm` 已补上 task-result transport 的第一条 ABI skeleton：`task-await` capability 现在会同时暴露 `qlrt_task_await` 与 `qlrt_task_result_release`，先把“等待得到 opaque result ptr”与“释放 result payload”这两个动作拆开冻结合同，再决定后续 typed extraction lowering
- `ql-codegen-llvm` 已补上 `AsyncTaskResultLayout` 内部抽象：当前 async 结果已接受 `Void`、scalar builtin，以及递归可加载的 tuple / fixed array / 非泛型 struct，并在 signature 阶段锁定 payload 的 LLVM type/size/align，避免后续 `await` lowering 再反过来重写 async wrapper/result 合同
- `ql-codegen-llvm` 已把 parameterized `async fn` wrapper 的 frame layout 扩到递归可加载 aggregate 参数：tuple / fixed array / 非泛型 struct 参数现在可被写入 heap frame，并在 `__async_body` 内按相同布局回读
- `ql-codegen-llvm` 已打开首个真实 `await` lowering：当前在 backend 内支持 `Void` / scalar builtin / recursively loadable aggregate async 结果，并把 `await` 降成 `qlrt_task_await` + load payload + `qlrt_task_result_release` 的最小链路
- `ql-codegen-llvm` 已打开首个真实 `spawn` lowering 子集：当前 backend 支持把 task-handle operand 降成 `qlrt_executor_spawn(ptr null, task)`，并返回可继续 `await` 的 task handle；当前已覆盖 direct async call、局部绑定 handle 与 sync helper 返回 handle，statement-position fire-and-forget 只是丢弃该返回句柄的特例
- `ql-codegen-llvm` 已把最小 place projection lowering 扩到写路径：当前嵌套 struct field read、constant tuple index read、array index read 继续走统一 place/type 推导与 LLVM GEP 链 lowering，而 struct-field write、constant tuple-index write，以及非 `Task[...]` 元素的 dynamic fixed-array index write 也已复用同一条 lowering；`Task[...]` 动态数组元素赋值仍刻意保持关闭
- `ql-codegen-llvm` / `ql-driver` / `ql-cli` 已打开 projected task-handle operand 支持：当投影结果本身是 `Task[T]` 时，`await pair[0]`、`spawn pair[0]`、`await tasks[0]`、`spawn tasks[0]`、`await pair.task` 与 `spawn pair.task` 现在都能稳定进入 codegen 与 `staticlib` 路径；这条支持目前已有 tuple、fixed-array literal index 与 struct-field 的端到端回归覆盖
- `ql-typeck` / `ql-borrowck` / `ql-codegen-llvm` / `ql-driver` 已打开最小 projection write/reinit 切片：mutable root 下的 `pair[0] = ...`、`pair.left = ...` 与 `tasks[0] = ...`（当前 task-handle 仍仅 fixed-array literal index）现在会在 typeck 层稳定放行、在 borrowck 层按 projection path 清除已消费的 task-handle move 记录、在 codegen 层生成写入 projection pointer 的 store；同时非 `Task[...]` 元素的 dynamic array assignment 也已进入这条最小写路径，补齐普通数组 `values[index] = ...` 与 nested `matrix[row][col] = ...` 的端到端能力，并在 `ql-driver` 内部单测与 `ql-cli` 黑盒 fixture 层锁定；`Task[...]` 动态数组元素赋值仍保持显式 unsupported，且 `ql-driver` 内部回归（`build_file_surfaces_dynamic_task_array_index_assignment_diagnostic_once`）与 `ql-cli` 黑盒 fail fixture 均已补齐
- `ql-typeck` / `ql-borrowck` / `ql-driver` / `ql-cli` 现也已补上 fixed-array literal-index projected task-handle reinit 的 branch-join 正向回归：`if flag { let first = await tasks[0]; tasks[0] = worker() } return await tasks[0]` 这类条件性重初始化路径现在在前端、borrowck、driver 与 CLI 黑盒层都有定向覆盖，避免这条刚开放的 write/reinit 子集在后续收口里静默回退
- `ql-codegen-llvm` 已补上 empty-array expected-context lowering：当前会把具体 `[T; N]` 期望类型保守回传到 direct temp locals 与 tuple / array / struct 聚合字面量内部，因此 `return []`、`take([])`、`([], 1)`、`Wrap { values: [] }` 与 `[[]]` 这类已有 `[T; 0]` 上下文的路径现在都可稳定出 IR；没有期望数组类型的裸 `[]` 仍保持显式拒绝
- `ql-codegen-llvm` / `ql-driver` / `ql-cli` 已锁住 zero-sized async parameter 回归：`[Int; 0]`、`Wrap { values: [Int; 0] }` 与 `[[Int; 0]; 1]` 这类 zero-sized aggregate 参数现在稳定走递归可加载 frame lowering，`await` 与 `staticlib` 路径都已被定向测试覆盖
- `ql-codegen-llvm` / `ql-driver` / `ql-cli` 已锁住 zero-sized async result 回归：`[Int; 0]` 与只包含 zero-sized fixed-array 字段的递归 aggregate 现在仍按 loadable async result 处理，direct `await`、direct/bound `spawn` 后再 `await`、helper direct/bound `await`、helper local-`return task` / helper-forward `Task[Wrap]` 流动，以及 `spawn schedule()` / `let task = schedule(); let running = spawn task; await running` / `spawn forward(task)` 这类 helper task-handle 路径都已被定向测试覆盖
- `ql-typeck` / `ql-borrowck` / `ql-codegen-llvm` / `ql-driver` / `ql-cli` 已把嵌套 task-result payload 收进口径：`async fn outer() -> Task[Int]` 与 `async fn outer() -> Task[Wrap]`（其中 `Wrap` 仅含 zero-sized fixed-array 字段）现在都可稳定走 `let next = await outer(); await next` 这条 chained-await 路径，类型层会把第一次 `await` 结果继续视为 fresh task handle，borrowck 会继续对该新句柄做 consume/use-after-move 跟踪，`staticlib` codegen/driver/CLI 也已有端到端回归锁定
- `ql-typeck` / `ql-borrowck` / `ql-codegen-llvm` / `ql-driver` / `ql-cli` 现也已锁住 aggregate-carried task-handle payload：`async fn outer() -> (Task[Int], Task[Int])`、`async fn outer() -> [Task[Int]; 2]`、带 `Task[Wrap]` 字段的 struct 结果，以及 `[Pending; 2]` 且 `Pending { task: Task[Int], value: Int }` 这类递归 nested fixed-shape aggregate 结果，现在都可先 `await outer()` 得到 aggregate，再通过 `await pair[0]` / `await pair[1]`、`await tasks[0]` / `await tasks[1]`、`await pending.first` / `await pending.second`、`await pending[0].task` / `await pending[1].task` 继续消费内部 task handle；这说明当前 loadable result contract 已覆盖“递归 nested fixed-shape aggregate payload 内继续携带 task handle 并走后续 projection await”这条受控子集
- `ql-typeck` / `ql-borrowck` 现也已显式镜像 zero-sized helper `Task[Wrap]` 流动回归：`await schedule()`、`spawn schedule()`、local-return helper、bound `spawn` 后再 `await`、helper `Task[Wrap]` 参数传递、conditional helper return、conditional maybe-moved 分支、branch-join maybe-moved 分支、branch-join helper consume/reinit 分支、branch-join spawned reinit 分支、reverse-branch spawned reinit 分支、helper 内条件性 spawn-return 分支、helper-consumed 后重赋值可恢复的路径、deferred cleanup 重初始化后继续读取的路径、deferred cleanup helper-consume use-after-move、conditional cleanup maybe-moved 分支、conditional cleanup helper-consume maybe-moved 分支、conditional cleanup 重初始化路径、conditional cleanup 重初始化并消费路径、conditional cleanup 重初始化并 helper-consume use-after-move 路径、reverse-branch conditional cleanup helper-consume/reinit 路径、以及对应的 borrowck debug render 事实，现在都有前端层专门测试，不再只依赖 backend/staticlib 黑盒覆盖；当前新增的 helper 条件分支回归专门锁住的是 async helper 内条件性 `spawn` 后再 `await` 返回的 zero-sized `Task[Wrap]` 结果流，以及直接 async 调用条件分支与 branch-join spawn reinit 的同类 flow，并且这些路径现在也已在 codegen / driver 的 staticlib 路径上补齐端到端回归
- `ql-typeck` / `ql-borrowck` 还补上了无 cleanup 的 branch-join helper consume/reinit 回归：`if flag { forward(task) } else { task = fresh_worker() }` 这类 zero-sized `Task[Wrap]` flow 现在会在 typeck 层稳定放行、在 borrowck 层稳定报出 `local \`task\` may have been moved on another control-flow path`；`ql-driver` 也已补上对应的稳定失败合同，继续补齐 task-handle 的控制流边界
- `ql-driver` 已开放第一条 public async build 子集：`staticlib` 现在允许已被 backend 支持的 async library body、scalar/task-handle/tuple/array/struct/void `await`、以及可绑定后再 `await` 的 `spawn` task handle 通过；fixed array iterable 的 `for await` 也已进入这条受控 library-mode 子集，而 broader async iteration surface 仍保持关闭
- `ql-driver` / `ql-cli` 已开放最小 async `dylib` 子集：只要模块仍提供同步 `extern "c"` 顶层导出，内部 async helper、`await`、已支持的 task-handle lowering，以及 fixed-array iterable 的 `for await` 现在都可进入 `dylib` 构建；非数组 iterable 与更广义 async surface 仍保持显式 unsupported
- `ql-resolve` / `ql-typeck` 已开放首个显式 task-handle 类型面：`Task[T]` 现在作为保留类型根被接受，不再触发 unresolved-type 诊断，并映射到内部 task-handle 语义；direct async call、spawned task、局部绑定 handle 与 sync helper 返回值现在都复用同一条句柄值模型
- `ql-typeck` 已移除 direct async call 的“必须立刻 `await` / `spawn`”限制：`let task = worker(); await task`、helper 参数传递、`return worker()` 到 `Task[T]` wrapper 等路径现在都可保守通过；当 direct async call 最终流入非 `Task[T]` 上下文时，会自然退化成普通类型不匹配，而不再依赖单独的特判诊断
- `ql-typeck` 已把 `spawn` 消费模型对齐到 task-handle 语义：`spawn task` 与 `spawn schedule()`（其中 `schedule() -> Task[T]`）现在都可保守通过；非 task operand 会给出稳定诊断，而不再把 `spawn` 限死在“直接 async call”形态
- `ql-borrowck` 已把 direct-local task handle 的 `await` / `spawn`、静态可判定的 helper `Task[T]` 参数传递与 direct-local `return task` 接进当前 ownership facts：这些路径现在会显式 consume 任务句柄，本地句柄在消费后复用会得到稳定的 use-after-move / maybe-moved 诊断，而重赋值仍可恢复 local 可用性；对 tuple index / fixed-array literal index / struct-field 的只读 projected task-handle operand，borrowck 现在也会把 consume 精确记到 projection path，而不是直接退化成 base local，因此 `await pair[0]; await pair[1]`、`await tasks[0]; await tasks[1]`、条件性消费后继续使用 sibling projection、以及读取 sibling 非 task 字段这类路径都已有回归覆盖，而同一 projection 的二次消费仍会稳定报 use-after-move / maybe-moved
- `ql-codegen-llvm` / `ql-driver` / `ql-cli` 已补充 task-handle helper 回归：`fn schedule() -> Task[Int] { return worker() }` 加 `await schedule()` 的 staticlib 路径已被单测、driver 回归和黑盒快照锁住
- `ql-codegen-llvm` / `ql-driver` / `ql-cli` 已补充 helper-argument task-handle 回归：`let task = worker(); let forwarded = forward(task); await forwarded` 与 `let running = spawn forward(task); await running` 这两条组合路径已被单测、driver staticlib 回归和 CLI 黑盒 fixture 锁住
- `ql-driver` / `ql-cli` 已补上 async staticlib mixed-surface 回归：带内部 async helper 的库现在也锁住了 `extern "c"` export header sidecar 路径，确保公开 C header surface 不会被 async implementation details 污染
- 当前 runtime crate 仍刻意不承诺 polling、cancellation、scheduler hints 或 Rust `Future` 绑定，只固定最小执行器接口
- 当前共享 hook ABI 已冻结第一版 LLVM-facing contract string，但真实内存布局、结果传递协议和更细粒度调用约定仍未冻结
- 当前 backend/driver 虽已具备 declaration + async body wrapper/frame scaffold，并已打通 scalar/task-handle/递归可加载 aggregate frame/result lowering（含递归 nested aggregate-carried task-handle payload）、`spawn` task-handle lowering、tuple/struct-field/fixed-array projection lowering、非 `Task[...]` 元素 dynamic array assignment lowering、expected-context empty-array lowering、zero-sized async parameter/result 稳定性、fixed-array `for await` lowering，以及 `staticlib` / 最小 async `dylib` 子集开放，但更广义的 async iteration 协议、任务结果协议、frame 生命周期管理、更广义的布局协议、`Task[...]` 动态数组元素写入 / 更广 place 的 projection write 语义与无期望类型的裸 `[]` 仍未开放
- 当前 `async-iteration` 已不再只是纯失败合同：library-mode async body 内的 fixed array `for await` 已走通最小 lowering；但共享 `qlrt_async_iter_next` hook 仍主要承担 capability/ABI 预留语义，还不代表通用 async iterator runtime 协议已经冻结
- 当前 `Task[T]` 虽已进入显式类型面，但仍是保守的句柄抽象：还没有 cancellation、polling、scheduler hint、auto-drop 语义或更一般的 async effect/type inference 设计
- 当前 borrowck 已对 direct-local task handle、tuple/struct-field projected task-handle 的 consume/write-reinit，以及 fixed-array literal index projected task-handle 的只读 consume/write-reinit 建立最小合同；`await` / `spawn`、静态可判定的 helper `Task[T]` 参数传递、direct-local `return task`、以及 sibling projection 的控制流分支都已接入，且 tuple/struct-field 与 fixed-array literal index 投影重赋值现在都会按 path 恢复对应 task-handle 的可用性；普通非 `Task[...]` 数组的 dynamic assignment 已可通过现有保守 borrowck 路径，但 `Task[...]` 动态索引写入、动态 index 的更精细 place-sensitive handle lifecycle、以及更广义 drop/submission 协议仍待后续切片

### 下一步（P7.3 / P7.4 方向）

P7.2 两个主线任务与 P7.4 的首个 program-entry 切片均已完成（2026-03-29/30）。

**P7.4 Task 1（已完成）：开放 `async fn main` 的 executable 程序入口**

- `crates/ql-codegen-llvm/src/lib.rs`：移除对 `async fn main` 的统一拒绝；在 program-mode host `@main` wrapper 中补齐 `task_create -> executor_spawn -> task_await -> result_load -> task_result_release -> trunc/ret` 生命周期 lowering，并为 void-return 路径保留 `ret i32 0`
- `crates/ql-driver/src/build.rs`：允许 `BuildEmit::Executable` 通过 async runtime capability gate；新增 driver 单测 `build_file_writes_async_main_executable_with_mock_toolchain`
- `crates/ql-cli/tests/codegen.rs` + `fixtures/codegen/pass/async_program_main.ql`：补齐 CLI 黑盒回归，锁定 async program build 端到端成功路径
- 当前边界仍保持保守：`llvm-ir` / `object` emit 仍通过 driver capability gate 拒绝 async runtime surface；只有 `BuildEmit::Executable` 会在 program-mode host entry 上额外注入 `async fn main` 所需 hook，并开放最小程序入口生命周期。更广义的 program bootstrap、async dylib surface 与 task result transport 仍未开放

**P7.4 Task 2（已完成）：锁定 `async fn main` + fixed-array `for await` 的 executable 闭环**

- `crates/ql-codegen-llvm/src/lib.rs`：新增 program-mode 单测 `emits_async_main_entry_lifecycle_with_fixed_array_for_await_in_program_mode`，锁定 async main host entry lifecycle 与 fixed-array `for await` lowering 在同一模块内共存
- `crates/ql-driver/src/build.rs`：新增 executable 成功回归 `build_file_writes_executable_with_async_main_fixed_array_for_await`
- `crates/ql-cli/tests/codegen.rs` + `fixtures/codegen/pass/async_program_main_for_await_array.ql`：补齐 CLI 黑盒 pass fixture，锁定 `ql build --emit exe` 的组合成功路径
- 当前边界保持不变：仅 fixed-array iterable 被开放；non-array iterable、`llvm-ir` / `object` async surface 与更广 program bootstrap 仍关闭

**P7.4 Task 3（进行中）：扩大 executable async payload 的 regression-locked 子集**

- `crates/ql-codegen-llvm/src/lib.rs`：新增 program-mode 单测 `emits_async_main_entry_lifecycle_with_tuple_task_handle_payload_in_program_mode`、`emits_async_main_entry_lifecycle_with_array_task_handle_payload_in_program_mode`、`emits_async_main_entry_lifecycle_with_nested_aggregate_task_handle_payload_in_program_mode`
- `crates/ql-driver/src/build.rs`：新增 executable 成功回归，覆盖 tuple / fixed-array / nested aggregate task-handle payload 在 `async fn main` 内的成功构建路径
- `crates/ql-cli/tests/codegen.rs` + `fixtures/codegen/pass/async_program_main_task_handle_tuple_payload.ql`、`async_program_main_task_handle_array_payload.ql`、`async_program_main_nested_aggregate_task_handle_payload.ql`：补齐 CLI 黑盒 fixture，把这三类 executable program-mode payload 路径正式纳入 pass matrix
- `crates/ql-codegen-llvm/src/lib.rs` / `crates/ql-driver/src/build.rs` / `crates/ql-cli/tests/codegen.rs` + `fixtures/codegen/pass/async_program_main_helper_task_handle_flows.ql`：新增 program-mode helper task-handle 综合回归，锁住 `await schedule()`、bound helper handle、`spawn schedule()`、forwarded helper handle 与 `spawn forward(task)` 这五条 executable 路径
- `crates/ql-codegen-llvm/src/lib.rs` / `crates/ql-driver/src/build.rs` / `crates/ql-cli/tests/codegen.rs` + `fixtures/codegen/pass/async_program_main_local_return_task_handle.ql`、`async_program_main_local_return_zero_sized_task_handle.ql`：新增 program-mode local-return helper task-handle 回归，锁住 `fn schedule() -> Task[...] { let task = worker(); return task }` 这类 helper 在 executable 下经由 `await schedule()` 的 regular / zero-sized 两条成功构建路径
- `crates/ql-codegen-llvm/src/lib.rs` / `crates/ql-driver/src/build.rs` / `crates/ql-cli/tests/codegen.rs` + `fixtures/codegen/pass/async_program_main_zero_sized_helper_task_handle_flows.ql`：新增 program-mode zero-sized helper task-handle 综合回归，锁住 `Task[Wrap]`（其中 `Wrap` 仅含 zero-sized fixed-array 字段）经由 direct helper await、bound helper handle、`spawn schedule()`、forwarded helper handle 与 `spawn forward(task)` 这五条 executable 路径
- `crates/ql-codegen-llvm/src/lib.rs` / `crates/ql-driver/src/build.rs` / `crates/ql-cli/tests/codegen.rs` + `fixtures/codegen/pass/async_program_main_aggregate_results.ql`：新增 program-mode 非零尺寸 aggregate-result 回归，锁住 tuple / fixed-array / struct 三种 direct `await` 结果布局在 executable 下的组合成功构建路径
- `crates/ql-codegen-llvm/src/lib.rs` / `crates/ql-driver/src/build.rs` / `crates/ql-cli/tests/codegen.rs` + `fixtures/codegen/pass/async_program_main_spawned_aggregate_results.ql`：新增 program-mode spawned 非零尺寸 aggregate-result 回归，锁住 tuple / fixed-array / struct 三种结果布局在 executable 下经由 `spawn -> await` 的组合成功构建路径
- `crates/ql-codegen-llvm/src/lib.rs` / `crates/ql-driver/src/build.rs` / `crates/ql-cli/tests/codegen.rs` + `fixtures/codegen/pass/async_program_main_recursive_aggregate_results.ql`：新增 program-mode 递归 aggregate-result 回归，锁住 `(Pair, [Int; 2])` 这类 nested loadable aggregate 在 executable 下经由 direct `await` 的成功构建路径
- `crates/ql-codegen-llvm/src/lib.rs` / `crates/ql-driver/src/build.rs` / `crates/ql-cli/tests/codegen.rs` + `fixtures/codegen/pass/async_program_main_spawned_recursive_aggregate_results.ql`：新增 program-mode spawned 递归 aggregate-result 回归，锁住 `(Pair, [Int; 2])` 这类 nested loadable aggregate 在 executable 下经由 `spawn -> await` 的成功构建路径
- `crates/ql-codegen-llvm/src/lib.rs` / `crates/ql-driver/src/build.rs` / `crates/ql-cli/tests/codegen.rs` + `fixtures/codegen/pass/async_program_main_zero_sized_aggregate_results.ql`、`async_program_main_spawn_zero_sized_aggregate_result.ql`：新增 program-mode zero-sized aggregate-result 回归，锁住 `[Int; 0]` / `Wrap { values: [Int; 0] }` 这类 zero-sized loadable result 在 executable 下经由 direct `await` 与 direct `spawn worker()` 两条成功构建路径
- `crates/ql-codegen-llvm/src/lib.rs` / `crates/ql-driver/src/build.rs` / `crates/ql-cli/tests/codegen.rs` + `fixtures/codegen/pass/async_program_main_recursive_aggregate_params.ql`、`async_program_main_zero_sized_aggregate_params.ql`：新增 program-mode aggregate-parameter 回归，锁住 recursive aggregate 参数与 zero-sized aggregate 参数在 executable 下经由 direct `await` 的成功构建路径
- `crates/ql-codegen-llvm/src/lib.rs` / `crates/ql-driver/src/build.rs` / `crates/ql-cli/tests/codegen.rs` + `fixtures/codegen/pass/async_program_main_spawned_recursive_aggregate_params.ql`：新增 program-mode spawned 递归 aggregate-parameter 回归，锁住 `spawn worker(Pair { left: 1, right: 2 }, [3, 4])` 这类 nested fixed-shape aggregate 参数在 executable 下经由 `spawn -> await` 的成功构建路径
- `crates/ql-codegen-llvm/src/lib.rs` / `crates/ql-driver/src/build.rs` / `crates/ql-cli/tests/codegen.rs` + `fixtures/codegen/pass/async_program_main_spawned_zero_sized_aggregate_params.ql`：新增 program-mode spawned zero-sized aggregate-parameter 回归，锁住 `spawn worker([], Wrap { values: [] }, [[]])` 这类 zero-sized fixed-shape aggregate 参数在 executable 下经由 `spawn -> await` 的成功构建路径
- `crates/ql-codegen-llvm/src/lib.rs` / `crates/ql-driver/src/build.rs` / `crates/ql-cli/tests/codegen.rs` + `fixtures/codegen/pass/async_program_main_projected_task_handle_awaits.ql`：新增 program-mode projected task-handle await 回归，锁住 local tuple index、fixed-array literal index 与 struct-field projection 三种 `Task[Int]` await 路径在 executable 下的连续成功构建
- `crates/ql-codegen-llvm/src/lib.rs` / `crates/ql-driver/src/build.rs` / `crates/ql-cli/tests/codegen.rs` + `fixtures/codegen/pass/async_program_main_projected_task_handle_spawns.ql`：新增 program-mode projected task-handle spawn 回归，锁住 local tuple index、fixed-array literal index 与 struct-field projection 三种 `Task[Int]` 先 `spawn` 再 `await running` 路径在 executable 下的连续成功构建
- `crates/ql-codegen-llvm/src/lib.rs` / `crates/ql-driver/src/build.rs` / `crates/ql-cli/tests/codegen.rs` + `fixtures/codegen/pass/async_program_main_zero_sized_nested_task_handle.ql`、`async_program_main_zero_sized_struct_task_handle_payload.ql`：新增 program-mode zero-sized nested / struct aggregate task-handle 回归，锁住 `Task[Wrap]` 的 chained-await 路径，以及 `Pending { first: Task[Wrap], second: Task[Wrap] }` 这类 aggregate-carried task-handle 在 executable 下的连续 await 路径
- `crates/ql-codegen-llvm/src/lib.rs` / `crates/ql-driver/src/build.rs` / `crates/ql-cli/tests/codegen.rs` + `fixtures/codegen/pass/async_program_main_zero_sized_projected_task_handle_awaits.ql`：新增 program-mode zero-sized projected task-handle await 回归，锁住 local tuple index、fixed-array literal index 与 struct-field projection 三种 `Task[Wrap]` await 路径在 executable 下的连续成功构建
- `crates/ql-codegen-llvm/src/lib.rs` / `crates/ql-driver/src/build.rs` / `crates/ql-cli/tests/codegen.rs` + `fixtures/codegen/pass/async_program_main_zero_sized_projected_task_handle_spawns.ql`：新增 program-mode zero-sized projected task-handle spawn 回归，锁住 local tuple index、fixed-array literal index 与 struct-field projection 三种 `Task[Wrap]` 先 `spawn` 再 `await running` 路径在 executable 下的连续成功构建
- `crates/ql-codegen-llvm/src/lib.rs` / `crates/ql-driver/src/build.rs` / `crates/ql-cli/tests/codegen.rs` + `fixtures/codegen/pass/async_program_main_zero_sized_projected_task_handle_reinit.ql`：新增 program-mode zero-sized projected task-handle direct reinit 回归，锁住 local tuple index、fixed-array literal index 与 struct-field projection 三种 `Task[Wrap]` 在 `await` 后重赋值、再二次 `await` 的 executable 成功构建路径
- `crates/ql-codegen-llvm/src/lib.rs` / `crates/ql-driver/src/build.rs` / `crates/ql-cli/tests/codegen.rs` + `fixtures/codegen/pass/async_program_main_zero_sized_projected_task_handle_conditional_reinit.ql`：新增 program-mode zero-sized projected task-handle conditional reinit 回归，锁住 fixed-array literal index `Task[Wrap]` 在 `if flag { await tasks[0]; tasks[0] = worker() } await tasks[0]` 这类 branch-join 路径下的 executable 成功构建
- `crates/ql-codegen-llvm/src/lib.rs` / `crates/ql-driver/src/build.rs` / `crates/ql-cli/tests/codegen.rs` + `fixtures/codegen/pass/async_program_main_zero_sized_branch_spawned_reinit.ql`：新增 program-mode zero-sized direct-local branch spawned reinit 回归，锁住 `var task = worker(); if flag { let running = spawn task; task = fresh_worker(); return await running } else { task = fresh_worker() } return await task` 这类 executable 成功构建路径
- `crates/ql-codegen-llvm/src/lib.rs` / `crates/ql-driver/src/build.rs` / `crates/ql-cli/tests/codegen.rs` + `fixtures/codegen/pass/async_program_main_zero_sized_reverse_branch_spawned_reinit.ql`：新增 program-mode zero-sized direct-local reverse-branch spawned reinit 回归，锁住 `var task = worker(); if flag { task = fresh_worker() } else { let running = spawn task; task = fresh_worker(); return await running } return await task` 这类 executable 成功构建路径
- `crates/ql-codegen-llvm/src/lib.rs` / `crates/ql-driver/src/build.rs` / `crates/ql-cli/tests/codegen.rs` + `fixtures/codegen/pass/async_program_main_zero_sized_conditional_async_call_spawns.ql`：新增 program-mode zero-sized conditional async-call spawn 回归，锁住 `async fn choose(flag: Bool) -> Wrap { if flag { let running = spawn worker(); return await running } return await worker() }` 及其 reverse-branch 组合在 executable 下的连续成功构建路径
- `crates/ql-codegen-llvm/src/lib.rs` / `crates/ql-driver/src/build.rs` / `crates/ql-cli/tests/codegen.rs` + `fixtures/codegen/pass/async_program_main_zero_sized_conditional_helper_task_handle_spawns.ql`：新增 program-mode zero-sized conditional helper-task-handle spawn 回归，锁住 `async fn choose(flag: Bool, task: Task[Wrap]) -> Wrap { if flag { let running = spawn task; return await running } return await task }` 及其 reverse-branch helper 组合在 executable 下的连续成功构建路径
- 这次实现层没有新增 ABI 或 lowering 分支；新回归证明的是 program-mode async body 已经复用既有 fixed-shape aggregate/task-handle lowering、projection write/reinit lowering、fixed-array literal-index branch-join reinit lowering，以及 direct-local task-handle consume/reinit lowering，只是此前缺少显式 coverage

**P7.4 Task 4（已完成评估）：`for await` iterable surface 扩展边界**

- `docs/plans/2026-03-29-phase-7-p7.2-runtime-and-interop.md`：延后评估区已补充对比矩阵，确认当前 fixed-array `for await` lowering 直接依赖 concrete `[N x T]` layout 与 `array_len` metadata，而不是通用 iterator ABI 的别名
- `docs/plans/phase-7-concurrency-and-rust-interop.md` / `docs/roadmap/development-plan.md`：已同步结论，明确 `qlrt_async_iter_next` 继续保留 capability/ABI placeholder 语义，不在本轮冻结 item release 协议
- 当前判断：dynamic array `for await` 继续 deferred；如果后续要扩面，优先单独设计不新增 runtime hook 的 `Slice[T]` / span-like fixed-shape view，并继续由 compiler 侧 index/load lowering 驱动

**P7.4 Task 5（已完成）：Windows toolchain UX 收口**

- `crates/ql-driver/src/toolchain.rs`：Windows 下的 clang / archiver discover 现在除了 PATH 和显式 `QLANG_CLANG` / `QLANG_AR` 之外，还会 best-effort 探测常见 LLVM 安装位置（Scoop、`%LOCALAPPDATA%\\Programs\\LLVM\\bin`、`%ProgramFiles%\\LLVM\\bin`、`%ProgramFiles(x86)%\\LLVM\\bin`）
- 同一文件的 `ToolchainError::NotFound` hint 现在会给出具体候选路径，并继续提示 `QLANG_AR_STYLE=lib|ar` 的 wrapper 风格固定方式
- `crates/ql-cli/tests/ffi.rs`：FFI 集成测试现已复用 `ql-driver` 的 toolchain discover 结果，不再单独只按 PATH / 环境变量决定是否跳过测试
- 当前边界仍保持保守：这是一条 best-effort LLVM 安装探测与 hint 收口，不代表已经实现完整 linker family discovery 或任意系统工具链枚举

**下一步建议（待执行）：**

- P7.4 Task 3：继续放宽更多 `await` / `spawn` payload 路径（当前已额外锁定 executable 下 tuple / fixed-array / nested aggregate task-handle payload regression matrix）
- 若后续重新打开 `for await` iterable surface，只建议从不新增 runtime hook 的 `Slice[T]` / span-like fixed-shape view 设计切入；dynamic array 与通用 iterator 协议继续保持 deferred

详细计划见 [开发计划](/roadmap/development-plan) 的 P7.4 小节。

**P7.2 Task 1（已完成）：runtime hook ABI 合同细化**

- `ql-runtime/src/lib.rs` 补充完整的 hook 生命周期规约注释：enum-level overview 展示两组生命周期（frame/task creation group、spawn/await/release group），每条 variant 补充明确的 caller/callee 约定，`TaskAwait` 明确"backend load assumption"
- `ql-runtime/tests/executor.rs` 新增三项生命周期单测：`hook_lifecycle_create_await_result_load_release_abi_contract`、`hook_lifecycle_full_llvm_declaration_sequence_is_stable`、`async_iter_next_abi_contract_is_stable`（共 14 项单测全通过）
- `ql-codegen-llvm/src/lib.rs` 的 await lowering load 位置补充 INVARIANT 注释，显式引用 `RuntimeHook::TaskAwait` 合同文档

**P7.2 Task 2（已完成）：Rust interop 双向工作流矩阵扩展**

- `examples/ffi-rust/ql/callback_add.ql` 新增 `q_host_multiply` import 与 `q_scale` export，实现两条独立的双向 FFI 路径
- `examples/ffi-rust/host/src/main.rs` 提供两个 Rust 回调（`q_host_add`、`q_host_multiply`），调用两个 Qlang 导出（`q_add_two`、`q_scale`），两条路径均验证结果为 42
- `crates/ql-cli/tests/ffi.rs` 的 `ffi_rust_example_cargo_host_runs` 扩展断言 `q_scale(6, 7) = 42`

**边界约定（保持不变）：**

- must-return 的字面量常量 `if`/`while`/`match` 收口已到位；若需扩到一般常量传播或 branch pruning，应单独设计
- loop-control 查询若需真正暴露到 editor 协议面，应延续当前 read-only bridge 路线
- `Task[...]` 动态数组索引写入仍保持 fail contract 关闭状态；开放时机需评估 place-sensitive lifecycle 的更广义设计

详细任务分解见 [P7.2 Runtime 合同扩展与 Rust 互操作计划](/plans/2026-03-29-phase-7-p7.2-runtime-and-interop)。

## 阶段状态表

| 阶段 | 状态 | 结论 |
| ---- | ---- | ---- |
| P1 | 已完成 | 前端最小闭环已经稳定，后续前端修改应服务语义与中后端 |
| P2 | 已完成基础阶段 | 语义地基、统一诊断、最小查询/LSP 已建立 |
| P3 | 已完成基础阶段 | MIR、ownership facts、cleanup-aware 分析与 closure groundwork 已建立 |
| P4 | 已完成基础阶段 | LLVM backend、artifact pipeline、extern C direct-call foundation 与 codegen harness 已建立 |
| P5 | 已完成基础阶段 | 最小 C ABI 互操作、头文件生成、sidecar header 与真实 C 宿主集成已建立 |
| P6 | 已完成基础阶段 | same-file query / rename / completion / semantic-token / LSP parity 已系统收口 |
| P7 | 进行中（P7.4） | 已形成保守 async library/staticlib/dylib 闭环，并开放 `async fn main`（含 fixed-array `for await`）的 executable 程序入口子集 |

## 对后续开发的直接建议

P1-P7 之后，不应再回到“大而化之地继续堆功能”的工作方式。更合理的推进方向是：

1. 在 P3 上继续补一般化 ownership / borrow / drop，而不是回头重做 MIR
2. 在 P4/P5 上继续补 lowering、ABI 与 runtime，而不是推翻 driver/codegen/ffi 边界
3. 在 P6 之上继续补 cross-file/project-indexed editor semantics，而不是重写 `ql-analysis` / `QueryIndex`
4. 所有新增能力都要沿着现有 crate 边界、失败模型和测试矩阵扩展

这才是当前仓库最重要的阶段性成果：不是某个单点功能，而是已经有了一条后续不需要大规模返工的主干路径。
