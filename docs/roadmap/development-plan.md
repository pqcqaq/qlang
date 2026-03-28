# 开发计划

如果你想先看当前 P1-P6 的实际完成情况，而不是完整路线图，请先阅读：[P1-P6 阶段总览](/roadmap/phase-progress)。

## 总体原则

开发顺序必须围绕“尽早形成真实闭环”展开。对 Qlang 来说，闭环不是只有一个 parser 能跑，而是至少满足：

1. 能写一个最小程序
2. 能编译到原生可执行文件
3. 能跑测试
4. 能给出像样错误信息
5. 能在编辑器里工作
6. 能调用一个 C 函数

## Phase 0: 预研冻结

### 目标

- 固化语言定位
- 固化仓库结构
- 固化 P0/P1/P2 边界
- 明确高风险技术假设

### 交付物

- 本文档站
- 初版功能清单
- 初版路线图
- 初版架构图和 RFC 流程草案

### 出口标准

- 设计原则不再大幅摇摆
- 后续能按 RFC 继续演进

## Phase 1: 编译器前端最小闭环

### 目标

- 建立 Rust workspace
- 实现 lexer、parser、AST
- 支持基础语法和模块系统
- 冻结 P0 语法基线，包括类型实例化、值实例化、解构绑定、可调用类型与元组返回
- 跑通 parser 回归测试

### 交付物

- `ql check` 能解析项目
- 语法错误有稳定 span
- `qfmt` 能对最小语法集工作

### 出口标准

- hello world 级程序可被解析和格式化
- parser / formatter 回归测试体系建立

### 当前切片拆分

为了避免 Phase 1 被做成一个“不断往 parser 里堆规则”的大泥球，前端开发要拆成更小的可验证切片：

1. workspace 与基础前端骨架
2. 基础声明、类型与表达式
3. 控制流、模式匹配与 formatter 稳定化
4. 顶层声明补全与错误恢复增强
5. 为 HIR lowering 预留更清晰的前端边界

### 当前状态（2026-03-26）

已完成：

- Rust workspace 与基础 crate 拆分
- `ql-span` / `ql-ast` / `ql-lexer` / `ql-parser` / `ql-fmt` / `ql-cli`
- package / use / const / static / type / opaque type / fn / trait / impl / extend / extern
- struct / data struct / enum
- 泛型参数、`where`、泛型类型应用、可调用类型
- 闭包、结构体字面量、基础运算表达式
- `unsafe fn` 和 `unsafe { ... }`
- `if` / `match` 表达式
- `while` / `loop` / `for` / `for await`
- richer pattern 支持：路径模式、tuple-struct 模式、struct 模式、字面量模式
- parser 从单文件拆分为 `item` / `expr` / `pattern` / `stmt` 模块
- 函数签名模型已统一，可覆盖 free function、trait method、extern function
- parser fixture 与 formatter 稳定性测试覆盖到控制流和 Phase 1 声明切片
- Phase 1 评审缺口已补齐：`*const` 原始指针类型、转义标识符、下划线前缀绑定、`pub extern` block round-trip、非法数字后缀诊断
- 前端基础抽象已加固：AST 节点级 span、控制流头部表达式歧义隔离、单元素 tuple 类型/表达式 round-trip、`ql check` 目录扫描过滤 fixture 与工具输出目录

当前验证集：

- `cargo fmt`
- `cargo test`
- `cargo run -p ql-cli -- check fixtures/codegen/pass/minimal_build.ql`
- `cargo run -p ql-cli -- fmt fixtures/parser/pass/phase1_declarations.ql`
- `npm run build` in `docs/`

说明：

- `fixtures/parser/pass/` 继续作为 parser / formatter regression surface 存在
- 由于 `ql check` 现在已经接入 resolve/typeck，部分 parser fixture 中保留的占位符符号会触发 unresolved diagnostics，因此不再把它们当作 semantic-clean `ql check` 样例

Phase 1 已完成。

## Phase 2: 语义分析与类型检查

### 目标

- 引入名称解析和 HIR
- 支持基础类型、`struct`、`enum`、`match`
- 支持 `Result` / `Option`
- 支持一等函数、闭包、元组、多返回值与解构的第一版类型检查
- 完成第一版类型检查

### 交付物

- `ql check` 能完成类型检查
- UI diagnostics 测试框架可用
- LSP 可提供 hover / definition / references / diagnostics

### 出口标准

- 中小示例可获得可信的类型错误提示
- HIR 作为语义单一事实源建立

### 当前状态（2026-03-25）

P2 已完成第一切片，也就是“语义地基”而不是“完整类型系统”：

- 新增 `ql-diagnostics`
- 新增 `ql-analysis`
- 新增 `ql-hir`
- 新增 `ql-resolve`
- 新增 `ql-typeck`
- `ql check` 已切到 parser -> HIR lowering -> resolve -> semantic checks 的新流水线
- HIR arenas、稳定 ID、local binding lowering 已建立
- lexical scope graph 已建立，覆盖 module / callable / block / closure / match arm / for-loop binding scope
- best-effort 名称解析已落地，当前覆盖 local、param、receiver `self`、generic、import alias、builtin type、struct literal root、pattern path root
- 精确 identifier name span 已打通到 AST / HIR / diagnostics
- struct pattern / struct literal shorthand 会在 HIR lowering 时正规化成真实语义节点
- 第一批语义诊断已经稳定回归：
  - top-level duplicate definition
  - duplicate generic parameter
  - duplicate function parameter
  - duplicate enum variant
  - duplicate method in trait / impl / extend block
  - duplicate pattern binding
  - duplicate struct / struct-pattern / struct-literal field
  - duplicate named call argument
  - positional argument after named arguments
  - invalid use of `self` outside a method receiver scope
- 第一批 first-pass typing 也已落地：
  - return-value type mismatch
  - `if` / `while` / match guard 的 `Bool` 条件检查
  - callable 调用的参数个数与参数类型检查
  - unique impl / extend method call 的参数类型检查
  - top-level `const` / `static` value 引用会参与后续表达式 typing
  - tuple-based multi-return destructuring 的第一层约束
- direct closure against expected callable type 的 first-pass checking
- struct literal 的 unknown-field / missing-field / field-type 检查
- source-level fixed array type expr `[T; N]`
- homogeneous array literal inference 与 expected fixed-array context 下的 array item type checking
- 保守 tuple / array indexing：array element projection、支持 lexer-style integer literal 的 constant tuple indexing、array index type checking、tuple out-of-bounds diagnostics
- same-file local import alias value/callable typing（function / const / static）
- comparison operand compatible-numeric checking
  - equality operand compatibility checking
  - bare mutable binding assignment diagnostics（`var` local / `var self`）
  - const / static / function / import assignment-target diagnostics
  - explicit unsupported member / index assignment-target diagnostics
  - ambiguous method member diagnostics
  - invalid projection receiver diagnostics
  - invalid struct-literal root diagnostics
  - invalid pattern-root shape diagnostics
  - invalid path-pattern root diagnostics
  - unsupported const/static path-pattern diagnostics
  - struct member existence checking
  - pattern root / literal compatibility checking
  - calling non-callable values
- 当前 unresolved diagnostics 仍然保持保守：bare single-segment value/type root 已报错，但 multi-segment unresolved global / unresolved type 仍等 import / module / prelude 语义成熟后再收紧
- assignment target 目前仍故意保守：bare mutable binding 会继续做可写性与 RHS 类型约束；`const` / `static` / function / import binding 现在会显式报不可赋值诊断，而 member/index place assignment 也会显式报“尚未支持”，继续等更完整的 place-sensitive 语义后再开放
- ambiguous method 目前只开放到 type diagnostics；query / completion / rename 仍然保持“仅唯一 candidate”这条保守边界，不提前引入 ambiguous member 的共享真值层
- invalid projection receiver diagnostics 目前也只覆盖已知必错的 builtin / tuple-array boundary / non-indexable known type；generic、unresolved 与 deeper import-module case 仍然保守
- invalid struct-literal root diagnostics 目前也覆盖 builtin / generic root、root 已解析成功且明确不支持 struct-style 字段构造的 case，以及 same-file 已解析二段 enum variant path 上的 unknown variant；deeper module-path 仍然保守
- unsupported 或仍 deferred 的 struct literal root 现在也会回退成 `unknown`，避免再泄漏伪造的具体 item type 并触发误导性的后续 return/assignment mismatch
- deferred multi-segment type path 现在也会保持 source-backed `Named` 形态，不再把 same-file local item / import alias 过早 canonicalize 成具体 item type
- invalid pattern-root shape diagnostics 目前也只覆盖 pattern root 已解析成功且明确不支持当前 struct/tuple 构造形状的 case，以及 same-file 已解析二段 enum variant path 上的 unknown variant；path pattern shape 与 deeper module-path 仍然保守
- invalid path-pattern root diagnostics 目前也只覆盖 path root 已解析成功且明确不支持 bare path 形状的 case；unit variant 仍允许，同文件已解析二段 enum variant path 的 unknown variant 现在也会显式报错
- const/static bare path pattern 目前也已改成显式 unsupported diagnostics，但仍只覆盖 same-file root 与 same-file local import alias；cross-file / deeper module-path constant pattern 语义仍然保守
- 默认参数目前仍停留在语言设计文档，尚未进入 AST / HIR / type checking 的已实现范围
- 黑盒 UI diagnostics snapshot harness 已建立，当前通过仓库根 `tests/ui/` fixture 和 `crates/ql-cli/tests/ui.rs` 锁定 parser / resolve / semantic / type 的最终 CLI stderr 输出
- `ql-analysis` 已建立统一分析边界，当前可稳定暴露 AST / HIR / resolve / typeck 结果以及 `expr` / `pattern` / `local` 的第一层类型查询
- `ql-analysis` 现在已经补上 position-based semantic query surface：
  - `symbol_at(offset)`
  - `hover_at(offset)`
  - `definition_at(offset)`
  - `references_at(offset)`
- `ql-analysis` 现在还补上了保守的 same-file rename surface：
  - `prepare_rename_at(offset)`
  - `rename_at(offset, new_name)`
- `ql-analysis` 现在还补上了保守的 same-file completion surface：
  - `completions_at(offset)`
- `ql-analysis` 现在还补上了保守的 same-file semantic token surface：
  - `semantic_tokens()`
- 当前查询覆盖 item / local / regular param / generic / receiver `self` / enum variant token / struct field member / unique method member / named type root / pattern path root / struct literal root
- 显式 struct literal / struct pattern 字段标签现在也会进入同一份 field query surface
- import alias 现在会作为 source-backed binding 进入统一 query surface，因此 hover / definition / 同文件 references / 同文件 rename / semantic tokens 都共用同一份 semantic identity；builtin type 则继续作为非 source-backed stable symbol 参与 hover / references / semantic tokens，但不提供 definition / rename
- `ql-typeck` 现在还会把同文件单段 local import alias 规范化回本地 item，用于 struct literal 字段检查、struct / enum pattern root 检查，以及同文件 function / const / static item alias 的 value typing / callable signature
- `ql-typeck` 现在还会把 enum struct-variant literal 的字段检查接到同一条路径上，same-file local import alias -> local enum item 也会复用这条 canonicalization；这仍是 typing 能力，不代表 query surface 已经支持 variant field symbols
- 同文件 local import alias -> local struct item 现在也会进入 field query / references / rename / semantic-token surface，显式字段标签和从 field symbol 发起的 shorthand rewrite 会继续映射回原 struct field symbol
- `ql-typeck` 现在还会对 struct pattern 的未知字段报错，这条校验同样会复用同文件 local import alias -> local item 的 canonicalization
- shorthand field token 当前仍故意保守，让 `Point { x }` 里的 `x` 继续落在 local/binding 语义上；但当从 source-backed field symbol 发起 rename，或从该 shorthand token 上发起 renameable binding symbol 的 rename 时，query 层现在都会把这些 shorthand site 自动扩写成显式标签
- same-file rename 当前只开放 function / const / static / struct / enum / variant / trait / type alias / import / field / method（仅唯一 candidate）/ local / parameter / generic；ambiguous method / receiver / builtin type / 从 shorthand field token 本身发起的 field-symbol rename 与 cross-file rename 仍然故意保守
- same-file completion 当前会复用 `ql-resolve` 的 scope graph 和 `ql-analysis` 的 symbol identity，已覆盖 lexical scope 的 value/type 位置、稳定 receiver type 的 parsed member token、same-file parsed enum variant path，以及 local import alias -> local enum item 的 variant follow-through；同一条 follow-through 也已经进入 same-file query / rename / semantic token surface；completion 候选现在还会区分语义 label 与源码 insert text，因此 keyword-named escaped identifier 会继续写回合法源码；ambiguous member completion、parse-error tolerant dot-trigger completion、import-graph/module-path deeper completion、foreign import alias variant semantics 与 cross-file completion 仍然故意保守
- same-file semantic tokens 当前会直接复用 `ql-analysis` 的 source-backed occurrence 与 `SymbolKind`，已覆盖当前统一 query surface 中的 item / local / param / generic / import / field / method / variant 等稳定语义 token；ambiguous member、parse error token 与跨文件分类仍然故意保守
- `ql-resolve` 现在保留 item scope / function scope 映射，后续查询和 LSP 不需要依赖脆弱的 resolver 遍历顺序去回推绑定定义
- receiver param 的精确 span 已从 AST 打通到 HIR，避免 hover / diagnostics / rename 这类按位置能力误锚到整个函数 span
- function / method declaration span 现在也会精确保留到 HIR，避免 trait / impl / extend 中多个方法共享同一个 function scope
- named path segment span 现在也会保留下来，因此 enum variant 的 pattern / constructor token 可以直接挂到稳定 query identity
- 新增 `ql-lsp`
- `qlsp` 最小服务端已落地，当前支持：
  - open / change / close 文档同步
  - live diagnostics
  - hover
  - go to definition
  - same-file find references
  - same-file lexical-scope completion
  - same-file parsed member-token completion
  - same-file parsed enum variant-path completion
  - local import alias -> local enum item 的 variant-path query / completion / rename / semantic-token follow-through
  - local import alias -> local struct item 的 struct-field query / references / rename / semantic-token follow-through
  - same-file semantic tokens
  - same-file prepare rename / rename
- LSP bridge 已经把协议层与语义层解耦：
  - UTF-16 `Position` 到源码 byte offset 的换算
  - compiler `Span` 到 LSP `Range`
  - compiler diagnostics 到 LSP diagnostics
  - `ql-analysis` hover / definition / references / completion / semantic tokens / rename 到 LSP 响应

当前验证集新增：

- `cargo fmt`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test`
- `cargo test -p ql-analysis`
- `cargo test -p ql-lsp`
- `cargo test -p ql-cli --test ui`
- `cargo run -p ql-cli -- check fixtures/codegen/pass/minimal_build.ql`
- 手工负例验证：struct pattern shorthand duplicate binding
- 手工负例验证：duplicate enum variant
- 手工负例验证：duplicate named call argument
- 手工负例验证：duplicate trait / impl / extend method
- rename 回归：analysis same-file rename target / validation / unsupported-kind filtering
- rename 回归：LSP prepare rename / workspace edit bridging
- semantic tokens 回归：analysis source-backed occurrence projection
- semantic tokens 回归：LSP legend / delta encoding bridging
- field query 回归：explicit struct literal / pattern field label precision
- field query 回归：shorthand field token 继续落在 local/binding 语义
- field rename 回归：local struct field rename 会自动把 shorthand field site 扩写成显式标签
- binding rename 回归：从 shorthand struct token 发起的 local / parameter / import / function / const / static 等已开放绑定 rename 会保留原 field label，并继续覆盖 local-import-alias struct root 下的 literal / pattern 组合路径
- function shorthand-binding rename parity 回归：analysis / LSP 会继续锁住 `Ops { add_one }` 这类 shorthand token 的 same-file prepare-rename / rename 行为，保证 field label 保留且 rename 只落到 function declaration / use
- import-alias struct field 回归：analysis / LSP 会继续锁住 explicit field label 的 references / rename / semantic-token follow-through
- type-namespace item rename 回归：analysis / LSP 会继续锁住 `type` / `opaque type` / `struct` / `enum` / `trait` 的 same-file rename 行为
- root value-item rename parity 回归：analysis / LSP 会继续锁住 `function` / `const` / `static` 的 same-file prepare-rename / rename 行为，而不是只靠聚合 analysis 测试或 shorthand-binding 回归间接覆盖
- type-namespace item parity 回归：analysis / LSP 会继续锁住 `type` / `opaque type` / `struct` / `enum` / `trait` 的 same-file references / semantic-token 一致性
- type-namespace item hover/definition 回归：analysis / LSP 会继续锁住 `type` / `opaque type` / `struct` / `enum` / `trait` 的 same-file hover / definition 一致性
- type-namespace item aggregate parity 回归：analysis / LSP 会继续锁住 `type` / `opaque type` / `struct` / `enum` / `trait` 这组 same-file item surface 的聚合 hover / definition / references / semantic-token 一致性
- global value item parity 回归：analysis / LSP 会继续锁住 `const` / `static` 的 same-file hover / definition / references / semantic-token 一致性
- extern callable parity 回归：analysis / LSP 会继续锁住 `extern` block function declaration、顶层 `extern "c"` declaration，以及顶层 `extern "c"` function definition / call site 的 same-file hover / definition / references / rename / semantic-token 一致性
- extern callable completion parity 回归：analysis / LSP 会继续锁住 `extern` block function declaration、顶层 `extern "c"` declaration，以及顶层 `extern "c"` function definition 在 value-context completion 中的 `FUNCTION` 候选形态、detail 与 text-edit 投影
- free function query parity 回归：analysis / LSP 会继续锁住 ordinary free function direct call site 的 same-file hover / definition / references 一致性，而不是只靠 completion / rename 或聚合 root-binding 测试间接覆盖
- free function semantic-token parity 回归：analysis / LSP 会继续锁住 ordinary free function declaration / direct call site 的 same-file semantic-token 一致性，而不是只靠聚合 semantic-token 快照间接覆盖
- callable surface aggregate parity 回归：analysis / LSP 会继续锁住 `extern` block callable、顶层 `extern "c"` 声明、顶层 `extern "c"` 定义与 ordinary free function 这组 same-file callable surface 的聚合 hover / definition / references / semantic-token 一致性
- import alias query parity 回归：analysis / LSP 会继续锁住 plain `import` binding 的 same-file hover / definition / references / semantic-token 一致性
- import alias completion parity 回归：analysis / LSP 会继续锁住 plain `import` binding 在 type-context completion 中的候选形态，以及 LSP `MODULE` completion item 投影
- free function completion parity 回归：analysis / LSP 会继续锁住 lexical value completion 中的 free-function 候选形态，以及 LSP `FUNCTION` completion item 投影
- import alias value completion parity 回归：analysis / LSP 会继续锁住 plain `import` binding 在 lexical value completion 中的候选形态，以及 LSP `MODULE` completion item 投影
- builtin / struct type completion parity 回归：analysis / LSP 会继续锁住 type-context completion 中的 builtin type / local struct 候选形态，以及 LSP `CLASS` / `STRUCT` completion item 投影
- type alias completion parity 回归：analysis / LSP 会继续锁住 same-file type-context completion 中的 `type alias` 候选形态，以及 LSP `CLASS` completion item 投影
- opaque type completion parity 回归：analysis / LSP 会继续锁住 same-file type-context completion 中的 `opaque type` 候选形态，以及带 `opaque type ...` detail 的 LSP `CLASS` completion item 投影
- generic completion parity 回归：analysis / LSP 会继续锁住 same-file type-context completion 中的 `generic` 候选形态，以及 LSP `TYPE_PARAMETER` completion item 投影
- enum completion parity 回归：analysis / LSP 会继续锁住 same-file type-context completion 中的 `enum` 候选形态，以及 LSP `ENUM` completion item 投影
- trait completion parity 回归：analysis / LSP 会继续锁住 same-file type-context completion 中的 `trait` 候选形态，以及 LSP `INTERFACE` completion item 投影
- field completion parity 回归：analysis / LSP 会继续锁住 stable receiver member completion 中的 `field` 候选形态，以及 LSP `FIELD` completion item 投影
- unique method completion parity 回归：analysis / LSP 会继续锁住 stable receiver member completion 中的唯一 `method` 候选形态，以及 LSP `FUNCTION` completion item 投影
- const / static completion parity 回归：analysis / LSP 会继续锁住 same-file lexical value completion 中的 `const` / `static` 候选形态，以及 LSP `CONSTANT` completion item 投影
- local value completion parity 回归：analysis / LSP 会继续锁住 same-file lexical value completion 中的 `local` 候选形态，以及 LSP `VARIABLE` completion item 投影
- parameter completion parity 回归：analysis / LSP 会继续锁住 same-file lexical value completion 中的 `parameter` 候选形态，以及 LSP `VARIABLE` completion item 投影
- lexical value candidate-list parity 回归：analysis / LSP 会继续锁住 import / const / static / extern callable / free function / local / parameter 这些 same-file value 候选的完整有序列表、detail 渲染与 replacement text-edit 投影
- enum variant completion parity 回归：analysis / LSP 会继续锁住 parsed enum path completion 中的 `variant` 候选形态，以及 LSP `ENUM_MEMBER` completion item 投影
- import alias variant completion parity 回归：analysis / LSP 会继续锁住 local import alias -> same-file enum item 这条 parsed variant-path completion 的 `variant` 候选形态，以及 LSP `ENUM_MEMBER` completion item 投影
- import alias struct-variant completion parity 回归：analysis / LSP 会继续锁住 local import alias -> same-file enum item 这条 struct-literal variant-path completion 的 `variant` 候选形态，以及 LSP `ENUM_MEMBER` completion item 投影
- remaining variant-path completion parity 回归：analysis / LSP 会继续锁住 direct struct-literal path 与 direct/local-import-alias pattern path 上既有的 `variant` 候选形态，以及 LSP `ENUM_MEMBER` completion item 投影
- variant-path candidate-list parity 回归：analysis / LSP 会继续锁住 enum-root / struct-literal / pattern path 及其 same-file import-alias 镜像上下文的完整有序 `variant` 候选列表、detail 渲染与 replacement text-edit 投影
- completion filtering parity 回归：analysis / LSP 会继续锁住 lexical value visibility/shadowing 与 impl-preferred member filtering 这两条 already-supported same-file completion boundary；其中 lexical value visibility 的聚合回归会显式覆盖 import / function / local 的 detail 与 text-edit 投影，而 impl-preferred member 聚合回归会显式覆盖 surviving candidate count 以及稳定 detail / text-edit 投影
- completion candidate-list parity 回归：analysis / LSP 会继续锁住 type-context 与 stable-member completion 的完整候选列表、排序与命名空间边界；其中 type-context 总表会显式覆盖 builtin / import / struct / `type` / `opaque type` / `enum` / `trait` / generic，而 stable-member 总表会显式覆盖 method / field 的 detail 与 text-edit 投影
- shorthand query boundary parity 回归：analysis / LSP 会继续锁住 shorthand struct field token 落在 local/binding surface 而不是 field surface 的既有 same-file 查询边界
- direct query parity 回归：analysis / LSP 会继续锁住 direct enum variant token 与 direct explicit struct field label 的 same-file definition / references 一致性，而不是只靠 import-alias 路径间接覆盖
- direct semantic-token parity 回归：analysis / LSP 会继续锁住 direct enum variant token 与 direct explicit struct field label 的 same-file highlighting 一致性，而不是只靠 import-alias 路径或聚合 semantic-token 测试间接覆盖
- direct symbol surface aggregate parity 回归：analysis / LSP 会继续锁住 direct enum variant token 与 direct explicit struct field label 这组 same-file direct-symbol surface 的聚合 hover / definition / references / semantic-token 一致性
- direct member query parity 回归：analysis / LSP 会继续锁住 direct field member 与唯一 method member 的 same-file hover / definition / references 一致性，而不是只剩字段 hover 或其他间接覆盖
- direct member semantic-token parity 回归：analysis / LSP 会继续锁住 direct field member 与唯一 method member 的 same-file highlighting 一致性，而不是只靠聚合 semantic-token 测试间接覆盖
- direct member surface aggregate parity 回归：analysis / LSP 会继续锁住 direct field member 与唯一 method member 这组 same-file direct-member surface 的聚合 hover / definition / references / semantic-token 一致性
- impl-preferred member query parity 回归：analysis / LSP 会继续锁住 direct member query 中 `impl` 优先于 `extend` 的既有 hover / definition / references 结果，而不是让桥接层重新漂移同名成员优先级
- lexical semantic symbol parity 回归：analysis / LSP 会继续锁住 `generic` / `parameter` / `local` / `receiver self` / `builtin type` 的 same-file hover / definition / references / semantic-token 一致性；builtin type 仍刻意不开放 definition / rename
- lexical rename parity 回归：analysis / LSP 会继续锁住 `generic` / `parameter` / `local` 的 same-file rename 行为，并继续保持 `receiver self` / `builtin type` 的 rename surface 关闭
- `npm run build` in `docs/`

### P2 下一切片

在当前切片之后，下一步不应直接跳到 LLVM 或 borrow checking，而应继续把语义层补完整：

1. 继续补足表达式 typing 覆盖面：成员调用、通用索引协议、枚举 / 结构体模式约束、更多 binary operator 规则
2. 引入更明确的 `unknown` / deferred-constraint 边界，避免后续 inference 重构 `ql-typeck`
3. 在当前 `qlsp` / 查询层之上继续把 same-file rename 扩到更完整语义面，并把当前 completion / semantic tokens 继续推进到 ambiguous member / module-path / project-indexing 等数据面
4. 在 import / module / prelude 规则稳定后，把当前 bare single-segment unresolved diagnostics 扩成完整 unresolved global / type diagnostics

## Phase 3: 所有权与内存语义

### 目标

- 建立 MIR
- 实现移动检查和基础借用推断
- 引入 drop 语义和 `defer`
- 明确 `unsafe` 边界

### 当前进度（2026-03-26）

P3 已经完成前两个切片，但当前仍然只宣称“ownership foundation 已建立”，而不是“所有权系统已完成”。

已落地：

- `ql-mir` crate
- HIR -> MIR lowering
- function body 的 stable local / block / statement / scope / cleanup ID
- `defer` 的注册与显式 cleanup 执行顺序
- `if` / block tail / `while` / `loop` / `break` / `continue` 的 CFG lowering
- `match` / `for` / `for await` 的结构化 terminator 保留
- `ql-analysis` / `ql mir` 已能消费和展示这层 MIR
- `ql-borrowck` crate
- MIR 上的 forward ownership facts：
  - local `Unavailable` / `Available` / `Moved(certainty, origins)` 状态
  - block entry / exit state merge
  - read / write / consume 事件记录
- 第一类用户可见 ownership diagnostics 已落地：
  - direct local receiver 调用唯一匹配的 `move self` method 后，再次使用该 local 会报错
  - 分支合流后会给出 `may have been moved` 级诊断
- cleanup-aware ownership 已进入下一切片：
  - `RunCleanup` 现在会真实参与 ownership analysis
  - deferred cleanup 会按 LIFO 执行顺序产生 read / consume / root-write effects
  - cleanup 中的 root local reassignment 可以为后续 cleanup 重新建立 `Available`
  - `move self` 调用现在在参数求值之后才真正消费 receiver
- move closure capture ownership 已进入当前切片：
  - `move` closure 会在创建时消费当前 body 中被捕获的 direct local
  - 普通 closure capture 现在会被视为一次真实 local 读取
  - `ql ownership` 能展示 `consume(move closure capture)` 事件
  - closure capture facts 现在已经 materialize 到 MIR，borrowck 不再需要在这条路径上重新遍历 HIR 收集 capture
- closure escape groundwork 已进入当前切片：
  - MIR closure 现在有稳定 identity
  - `ql ownership` 现在会渲染第一版 conservative may-escape facts
  - 当前 escape kind 只覆盖 `return` / `call-arg` / `call-callee` / `captured-by-cl*`
- `ql-analysis` 已聚合 borrowck diagnostics，并提供 ownership dump 渲染
- `ql ownership <file>` 已可直接输出当前 ownership facts

当前仍刻意未完成：

- 通用 call consume contract
- place-sensitive move analysis
- path-sensitive borrow / escape analysis
- cleanup closure capture / nested defer runtime modeling
- 完整 closure environment / escape graph
- drop elaboration
- `match` / `for` 的更低层 elaboration
- 完整 ownership diagnostics 体系

### 建议切片

P3 不应一次性推进，而应拆成：

1. P3.1 MIR foundation
2. P3.2 ownership facts 与显式 `move self` 诊断
3. P3.3 cleanup-aware ownership 与 borrow / escape analysis
4. P3.4 ownership diagnostics 与 drop elaboration
5. P3.5 codegen-ready MIR simplification

### 交付物

- 值语义和移动规则可工作
- 基础资源释放模型成立
- 所有权相关 diagnostics 可解释

### 出口标准

- 常见资源管理场景可编译并运行
- 无需显式生命周期即可覆盖大多数样例

## Phase 4: LLVM 后端与原生产物

### 目标

- 从 MIR lowering 到 LLVM IR
- 生成对象文件、静态库、可执行文件
- 打通链接驱动

### 交付物

- `ql build`
- debug / release profile
- 基础 codegen golden tests

### 当前进度（2026-03-26）

P4 已经启动，但当前完成的是 “backend foundation” 而不是“完整原生产物”：

- 新增 `ql-driver`
- 新增 `ql-codegen-llvm`
- `ql build` 已经接入统一 build 路径
- 当前默认可输出文本 LLVM IR：`target/ql/<profile>/<stem>.ll`
- 当前已经补上 native artifact foundation：
  - `ql build --emit obj`
  - `ql build --emit exe`
  - `ql build --emit staticlib`
  - clang-style toolchain discovery
  - compiler / archiver toolchain boundary 已拆开
  - 用户态 `main` 现在会在 program mode 下 lower 成内部 Qlang entry，并额外生成宿主 `main` wrapper
  - `staticlib` 现在走 library mode，因此单文件库构建不再要求顶层 `main`
  - toolchain failure 时保留中间 `.codegen.ll`，link 或 archive failure 时再额外保留 `.codegen.obj/.o` 便于调试
- 当前支持的 MIR/codegen 子集已经可测试：
  - 顶层 free function
  - `extern "c"` 顶层声明、extern block 声明与顶层函数定义
  - `main` 入口
  - scalar integer / `Bool` / `Void`
  - direct function call
  - arithmetic / compare / branch / return
- extern C direct-call 路径已经进入真实后端：
  - resolve / typeck / MIR / codegen 现在共享 callable identity，而不再把 extern block 成员粗暴折叠成宿主 item
  - direct `extern "c"` 调用现在会在 program mode 和 library mode 下 lower 成 LLVM `declare @symbol` + `call @symbol`
  - extern block direct call 与 top-level extern declaration call 现在都会参与参数个数与参数类型检查
- top-level `extern "c"` function definition export 路径已经进入真实后端：
  - parser / formatter 现在允许顶层 `extern "c"` 函数定义保留 body，而不是只允许 declaration
  - 顶层 `extern "c"` 函数定义现在会 lower 成稳定 C 符号名，而不是内部 mangled name
  - `staticlib` 已可直接承载这类定义，形成 P5 的第一块 C export 地基
  - program-mode 入口 `main` 当前仍必须使用默认 Qlang ABI，避免与宿主 `@main` wrapper 冲突
- 基础 codegen golden harness 已开始落地：
  - `crates/ql-cli/tests/codegen.rs`
  - `tests/codegen/pass/`
  - `tests/codegen/fail/`
  - 黑盒锁定 `llvm-ir` / `obj` / `exe` / `dylib` / `staticlib`、library-mode extern C direct-call lowering、`extern "c"` definition export lowering 与 build-time unsupported diagnostics
- 受约束的 dynamic-library slice 也已经落地：
  - `ql build --emit dylib`
  - `dylib` 与 `staticlib` 一样走 library mode，不要求顶层 `main`
  - 当前要求至少一个 public 顶层 `extern "c"` 函数定义，避免产出没有导出面的共享库
  - Windows 下会把这些稳定导出符号显式转成 `/EXPORT:<symbol>` 透传给 linker
- 当前 unsupported backend features 会返回结构化 diagnostics，而不是静默跳过

当前仍刻意未完成：

- 更完整的系统 LLVM / linker family 组合探测
- runtime startup object / richer ABI glue
- first-class function value / closure / tuple / struct / cleanup lowering
- 一般化 shared-library surface、exported ABI 的 linkage/visibility 控制与 extern ABI 更完整支持
- codegen golden snapshot harness 扩容到更多 lowering / toolchain / fail 场景

当前 P4 的核心目标不是“一次性做完整原生平台层”，而是先把 driver/codegen 边界、program/library 入口模型、失败模型和测试面固定住，避免后续为了补链接和运行时而大规模返工。

### 出口标准

- hello world、简单容器、简单匹配逻辑可运行
- Windows 和 Linux 至少打通一种 CI

## Phase 5: C FFI 与运行时地基

### 目标

- 支持 `extern "c"` 导入导出
- 建立 FFI 安全包装模式
- 标准库 `core`、`alloc`、`io` 打底

### 当前状态（2026-03-26）

P5 当前已经完成多条最小可用切片，但仍只宣称“最小 C 互操作闭环已建立”，而不是“FFI 已完成”：

- 顶层 `extern "c"` 函数定义现在可以保留 body 进入真实编译流水线
- `ql-codegen-llvm` 会把这类定义 lower 成稳定 C 符号名，而不是内部 mangled name
- `ql-driver` / `ql build --emit staticlib` 已可承载这类 exported symbol 的构建路径
- `ql-driver` / `ql build --emit dylib` 现在也已可承载这类 exported symbol 的共享库构建路径
- parser、formatter、LLVM backend、driver 和 CLI black-box codegen snapshot 都已补上回归
- `tests/ffi/pass/` 与 `crates/ql-cli/tests/ffi.rs` 现在已经建立真实 C 宿主集成 harness
- 在 clang-style compiler 与 archiver 可用时，这层 harness 会真实执行“Qlang staticlib + build-side header -> C link -> 可执行文件运行”的端到端回归
- 在 clang-style compiler 可用时，这层 harness 还会真实执行“Qlang dylib + build-side header -> C loader harness -> 运行时装载并调用导出符号”的端到端回归
- imported-host staticlib harness 现在还会真实执行“宿主定义 imported C symbol -> Qlang exported helper 调用 imported symbol -> C 宿主断言结果”的端到端回归
- `ql ffi header <file>` 已经落地，并支持 `--surface exports|imports|both`
- `ql build <file> --emit dylib|staticlib` 现在还支持 `--header`、`--header-surface` 和 `--header-output`
- 当前头文件生成会复用 analysis 结果投影三类 surface，而不是在 CLI 里重扫语法：
  - public 顶层 `extern "c"` 函数定义
  - 顶层 `extern "c"` 声明
  - `extern "c"` block 成员声明
- 默认命名当前固定为：
  - `exports` -> `target/ql/ffi/<stem>.h`
  - `imports` -> `target/ql/ffi/<stem>.imports.h`
  - `both` -> `target/ql/ffi/<stem>.ffi.h`
- build-side sidecar 默认命名当前固定为：
  - `exports` -> `<library-dir>/<source-stem>.h`
  - `imports` -> `<library-dir>/<source-stem>.imports.h`
  - `both` -> `<library-dir>/<source-stem>.ffi.h`
- `tests/ffi/pass/*.header-surface` 现在还允许单夹具声明 build-time header surface，当前 imported-host 夹具会使用 `both`
- 当前已支持的 header surface 包括：
  - `Bool` / `Void`
  - 当前后端已稳定的整数和浮点标量
  - 原始指针和多级原始指针
- build-side sidecar 当前还会提前拒绝非 library emit、拒绝主 artifact/header 路径冲突，并在 sidecar 失败时回收主 library artifact
- `crates/ql-driver/src/ffi.rs`、`crates/ql-driver/src/build.rs`、`crates/ql-cli/tests/ffi_header.rs`、`crates/ql-cli/tests/codegen.rs` 和真实 FFI harness 已经把这条路径锁进回归
- imported-host staticlib 现在已覆盖 extern block 与 top-level extern 两条真实混编路径
- 当前 program-mode 仍明确要求用户入口 `main` 使用默认 Qlang ABI；稳定 C 导出入口应定义为独立 helper

当前仍刻意未完成：

- ABI 布局检查与 richer diagnostics
- exported symbol 的 visibility/linkage 控制
- bridge code generation 与更完整 `ql ffi` 子命令面
- runtime `core` / `alloc` / `io` 地基

### 交付物

- 调用 libc 或自定义 C 库示例
- `tests/ffi` 成体系
- `ql ffi` 最小桥接能力

### 出口标准

- Qlang 可以作为真实本地库参与链接
- FFI 边界报错可定位

## Phase 6: LSP、格式化器与开发体验

### 目标

- 基于统一语义数据库实现 `qlsp`
- 补全、跳转、引用、rename、semantic tokens
- `qfmt` 风格冻结

### 交付物

- VS Code 最小扩展
- LSP 场景回归测试
- code action 雏形

### 出口标准

- 用编辑器写 Qlang 不再“只能靠编译试错”

## Phase 7: 并发、异步与 Rust 互操作

### 目标

- 支持 `async fn`、`await`、`spawn`
- 提供 executor 抽象
- 通过 C ABI 打通 Rust 互操作工作流

### 交付物

- async 标准库雏形
- `examples/ffi-rust`
- 并发能力 trait 检查

当前基线补充：

- `examples/ffi-rust` 的最小 Cargo host 示例已落地，后续 Phase 7 继续在此基础上扩展更完整的 Rust 工作流与运行时边界
- `crates/ql-runtime` 的最小 runtime/executor 抽象已落地，当前提供 `Task` / `JoinHandle` / `Executor` trait 与单线程 `InlineExecutor`
- `ql-analysis` 已开始暴露 runtime requirement truth surface，当前会按源码顺序枚举 `async fn`、`spawn`、`await`、`for await` 对应的 capability 需求
- `ql-driver` 已开始保守消费 runtime requirement truth surface，当前会把 `async-function-bodies`、`task-spawn`、`task-await`、`async-iteration` 映射成稳定的 build diagnostics，并与 backend 同类 unsupported 诊断去重
- `ql-runtime` 还新增了第一版共享 runtime hook ABI skeleton，当前不仅固定 hook 名称和符号名，也固定了统一 `ccc` + opaque `ptr` 的 LLVM-facing contract string；`async-function-bodies` 当前会映射到 `async-frame-alloc` + `async-task-create`，`task-await` 当前会映射到 `task-await` + `task-result-release` 两个 hook，`ql runtime <file>` 也会直接渲染 dedupe 后的 hook 计划和 ABI 文本
- `ql-codegen-llvm` 现已直接复用这份共享 hook ABI contract 渲染 LLVM declarations，`ql-driver` 也会把 dedupe 后的 `RuntimeHookSignature` 传入后端；backend 现在统一把 `async fn` body 降成 `ptr frame` 入口，并且能为带参数的 `async fn` 生成最小 heap-frame wrapper scaffold，当前该 wrapper 已支持递归可加载的 tuple / fixed array / 非泛型 struct 参数，其中也包含 zero-sized fixed arrays 与递归 zero-sized aggregates，同时为 async 返回值预计算 `AsyncTaskResultLayout`，当前已支持 `Void`、scalar builtin，以及递归可加载的 tuple / fixed array / 非泛型 struct 结果，其中也包含 zero-sized fixed arrays 与递归 zero-sized aggregates，冻结“await 拿到 opaque payload ptr 后再按 LLVM value `load` 并释放”的内部前提，并已在 backend 内打通 loadable `await` lowering；此外 backend 还支持把 `spawn` 降成 `qlrt_executor_spawn(ptr null, task)` 并返回可继续 `await` 的 task handle，且当前已覆盖 direct async call、局部绑定 handle 与 sync helper 返回 handle 这些 task-handle operand；backend 现在还支持嵌套只读 struct field / constant tuple index / array index projection lowering，并会把已有具体 expected array type 的约束保守回传到 direct temp locals 与 tuple / array / struct 聚合字面量内部，从而让 `return []`、`take([])`、`([], 1)`、`Wrap { values: [] }` 与 `[[]]` 这类 `[T; N]` 上下文稳定 lowering，但仍不开放 projection assignment target、`await` / `spawn` 的 projected operand 与无期望类型的裸 `[]`；`ql-resolve` / `ql-typeck` 现也已开放保留的 `Task[T]` 类型面，使 direct async call、spawned task 与 sync helper 都能通过统一的 task-handle 值语义流动；`ql-driver` 现允许这条受控 async library 子集通过 `staticlib` 构建，当前仍不开放 `for await` lowering、`dylib` async 或 program async build 承诺
- `ql-typeck` 现已把 direct `async fn` calls 统一建模成 `Task[T]` 句柄值：这些句柄可被局部绑定、helper 参数/返回值传递、通过 `spawn` 再次提交，并在后续 `await`；流入非 `Task[T]` 上下文时会退化成普通类型不匹配；这保证前端语义不会在 task/result ABI 仍未冻结时把 async 调用伪装成同步返回值
- `ql-borrowck` 现已把 direct-local task handle 的 `await` / `spawn`、静态可判定的 helper `Task[T]` 参数传递和 direct-local `return task` 接进当前 ownership facts：这些路径会消费本地 `Task[T]`，后续复用会触发稳定的 moved / maybe-moved 诊断，而重赋值仍能重新初始化该 local；更广义的 helper 边界与 place-sensitive handle lifecycle 仍保持未开放
- `ql-codegen-llvm` / `ql-driver` / `ql-cli` 现已把 helper `Task[T]` 流动路径的端到端合同锁住：`await schedule()`、`let task = schedule(); await task`、`let task = schedule(); let running = spawn task; await running`、local-`return task` helper、`await forward(task)` 与 `spawn forward(task)` 这些“helper 产出/接收 task handle，再作为 task handle 继续流动”的组合路径已被 codegen、driver staticlib build 与 CLI blackbox fixture 明确覆盖

### 出口标准

- 能写基本网络服务样例
- Rust 混编路径清晰可复现

## Phase 8: 文档、包管理与工作区增强

### 目标

- `ql doc`
- lockfile
- feature flags
- 模板项目生成

### 交付物

- API 文档生成站点
- 多包 workspace 示例
- 项目模板

### 出口标准

- 新用户能从脚手架到文档再到编译运行形成顺畅体验

## Phase 9: 深水区能力

### 目标

- 更完整效果系统
- actor 标准库
- 更深的 C++ 互操作
- 增量编译和性能优化完善

### 交付物

- RFC 驱动的高级特性演进
- 性能基线与回归体系
- 更成熟的生态支撑

### 出口标准

- 项目进入“可持续迭代生态期”，而不是只靠主线作者硬推

## 每阶段都必须交付的横向事项

- 文档更新
- RFC 或 ADR 沉淀
- 回归测试补全
- CI 更新
- 示例代码补全

如果少了这些横向事项，项目会形成“代码前进，工程后退”的假繁荣。
