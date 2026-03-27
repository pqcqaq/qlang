# Phase 2 语义与类型检查地基

## 目标

Phase 2 的目标不是一次做完完整类型系统，而是建立统一语义边界，让 CLI、LSP、后续 MIR 和 borrowck 都共享同一份可测试的真相源。

核心交付：

- HIR
- 名称解析
- first-pass typing
- 统一 diagnostics
- position-based semantic query
- 最小 LSP

## 合并后的切片结论

### 1. HIR 与统一分析边界

这一组切片把前端产物从 AST 推进到真正可消费的语义层：

- `ql-hir` 建立 arena、稳定 ID、semantic normalization
- `ql-analysis` 成为 parser -> HIR -> resolve -> typeck 的统一入口
- `ql-diagnostics` 统一 parser / semantic / type diagnostics 渲染

关键设计：

- HIR 才是后续语义与查询的稳定输入
- shorthand surface 在 lowering 时就正规化，避免 resolver/typeck 再临时猜
- span 必须从 AST 打通到 HIR，而不是靠后期补救

### 2. 名称解析与作用域图

这一组切片把 lexical truth surface 做稳：

- module / item / function / block / closure / match arm / for-loop scope graph
- best-effort value/type resolution
- item scope / function scope 显式记录
- bare single-segment unresolved diagnostics

关键设计：

- resolver 只负责当前已经稳定的根绑定
- multi-segment module/import/prelude 语义继续保守
- query/LSP 不再回放 resolver 遍历顺序去找声明点

### 3. First-pass typing

这一组切片把“可用类型错误”推进到真实水平：

- return mismatch
- bool condition checks
- callable arity / argument typing
- tuple destructuring
- expected callable closure checking
- struct / enum struct-variant literal checking
- fixed array type syntax `[T; N]`
- array literal inference 与 expected fixed-array context 收紧
- tuple / array indexing
- equality / comparison operand compatibility
- member existence
- invalid pattern-root shape diagnostics
- invalid path-pattern root diagnostics
- unsupported const/static path-pattern diagnostics
- pattern root / literal compatibility
- bare mutable binding assignment diagnostics

关键设计：

- `Ty::Unknown` 是明确的退化阀门，不是“暂时没做”
- source text 与 semantic normalized value 分层保存
- 只在已有稳定语义身份处收紧规则，不提前宣称完整 place/index protocol
- pattern root shape 也只在“已解析且已知必错”时显式报错；same-file 已解析二段 enum variant path 的 unknown variant 现在也显式报错，path-pattern semantics / deeper module-path 继续保守
- bare path pattern 也只在“已解析且已知 bare path 形状必错”时显式报错；unit variant 保持允许
- const/static bare path pattern 现在也显式报 unsupported，但只限 same-file root / same-file local import alias；更广义 constant-pattern 语义继续保守
- unsupported 或仍 deferred 的 struct literal root 现在也回退成 `unknown`，避免继续制造级联 type mismatch
- deferred multi-segment type path 现在也保持 source-backed `Named`，避免 same-file local item / import alias 的首段解析结果继续污染 type/query truth surface
- deferred multi-segment `impl` / `extend` target 也不会被投影到 concrete local receiver surface；同文件 concrete item 的 member typing/completion 只继续接真实 receiver 自己的字段/方法，不把 `Counter.Scope.Config` 这类 deferred path 的方法伪装成 `Counter` 的能力
- deeper path-like call 在 receiver 已知必错时也不会继续复用 root callable signature；像 `ping.scope(true)` 这类 case 现在会停在 projection error，而不是再额外制造伪造的 call-argument mismatch

### 4. 同文件查询与最小 LSP

这一组切片把 analysis 与 LSP 绑定成同一套 truth surface：

- `symbol_at`
- `hover_at`
- `definition_at`
- `references_at`
- `prepare_rename_at`
- `rename_at`
- `completions_at`
- `semantic_tokens()`

已经进入稳定 surface 的对象包括：

- item
- local / parameter / generic / receiver `self`
- import alias
- struct field
- unique method
- enum variant
- named type root / pattern root / struct literal root

### 5. same-file editor semantics 收口

后续大量 P6 风格切片其实是建立在 P2 的 query boundary 之上：

- shorthand rewrite 规则
- import alias -> same-file item canonicalization
- escaped identifier completion insert text 保真
- occurrence-based semantic tokens
- LSP bridge 只做协议映射，不复制语义遍历

## 当前架构收益

现在 P2 的价值已经很明确：

- `ql check` 不再只是 parser 工具
- `ql-analysis` 已成为 CLI / LSP / future tooling 共用入口
- HIR / resolve / typeck / query 已经组成第一条稳定主干

## 当前仍刻意保留的边界

- default parameter 仍未实现
- full module graph / import / prelude 语义仍未完成
- field/index assignment 仍未进入完整 place-sensitive 语义
- cross-file/project-wide editor semantics 仍未开始
- 泛型推断、trait solving、flow-sensitive narrowing 仍未进入完整实现

## 归档

本阶段原始切片稿已归档到 [`/plans/archive/phase-2`](/plans/archive/index)。
