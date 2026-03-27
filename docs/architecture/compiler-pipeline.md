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
- 当前 diagnostics 采取保守策略，只落地绝对可靠的语义错误：method receiver 作用域外非法使用 `self`，以及 bare single-segment value/type root 的 unresolved 诊断
- multi-segment unresolved global / unresolved type 的系统性报错仍刻意延后，直到 import / module / prelude 语义稳定，否则会把现有 fixture 误判成失败

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
- 新增 struct 与 enum struct-variant literal 的 field / missing-field 检查，并复用 same-file local import alias -> local item 的 canonicalization
- 新增源码层 fixed array type expr `[T; N]`，并把 lexer-style length literal lowering 成统一的语义长度
- 新增 homogeneous array literal inference，并在 expected fixed-array context 下复用声明元素类型收紧 literal item checking
- 新增保守 tuple / array indexing：array element projection、支持 lexer-style integer literal 的 constant tuple indexing、array index type checking、tuple out-of-bounds diagnostics
- 新增 equality operand compatibility checking
- 新增 comparison operand compatible-numeric checking
- 新增 bare mutable binding assignment diagnostics：`=` 现在会约束 `var` local / `var self`
- 同一条路径现在还会显式拒绝 `const` / `static` / function / import binding 赋值，以及 member/index assignment target，避免语义层静默放过 backend 尚未承诺的写入模型
- 新增 struct member existence checking
- 新增 same-file local import alias value/callable typing：同文件单段 alias 指向 function / const / static 时，会复用本地 item 的 value type 与 callable signature
- 新增 ambiguous method member diagnostics：当前同名多 candidate 的成员方法访问会给出显式 type diagnostics，而不是静默退化成 `unknown`
- 新增 invalid projection receiver diagnostics：已知不支持 member/index 语义的接收者现在会显式报错，而不是静默退化成 `unknown`
- 新增 invalid struct-literal root diagnostics：已知不支持 struct-style field construction 的 root 现在会显式报错，而不是静默退化成 `unknown`
- 新增 invalid pattern-root shape diagnostics：已解析成功但 struct/tuple pattern 构造形状确定不匹配的 root 现在会显式报错，而不是静默退化成 `unknown`
- 新增 invalid path-pattern root diagnostics：已解析成功但 bare path pattern 构造形状确定不匹配的 root 现在会显式报错，而不是静默退化成 `unknown`
- 新增 unsupported const/static path-pattern diagnostics：同文件 `const` / `static` item 及其 local import alias 作为 bare path pattern 时，现在会显式报 unsupported，而不是静默漏诊
- 新增 pattern root / literal compatibility checking
- 新增 struct pattern unknown-field checking，并复用 same-file local import alias -> local item 的 canonicalization

当前依然保守的边界：

- 未解析成员调用、通用索引协议、import prelude 细节时，表达式类型会主动退化为 `unknown`；当前只对源码层 fixed array、inferred array 和 constant tuple index 开放一层 typing
- `=` 当前只对 bare mutable binding 开放真实可写语义；field/index 写入仍未开放，但已经有显式 unsupported diagnostics，不再静默伪装成“可能可用”
- local import alias 的 value/callable typing 当前仍只限 same-file、single-segment、可规范化到本地 function / const / static item 的场景；foreign import 与更深 module graph 仍然延后
- invalid projection receiver diagnostics 当前也只在“类型已知且明确不支持当前语义”时触发；`unknown` / generic / deeper import-module 相关场景仍保持保守，不提前下结论
- invalid struct-literal root diagnostics 当前也只在“root 已解析成功且明确不支持 struct-style 字段构造”时触发；same-file 已解析二段 enum variant path 的 unknown variant 现在也会显式报错，但 deeper module-path case 仍保持保守，不提前下结论
- unsupported 或仍 deferred 的 struct literal root 当前也会直接回退成 `unknown`，不再把首段解析结果伪装成真实 item type 并继续污染 return/assignment diagnostics
- deferred multi-segment type path 当前也保持 source-backed `Named` 表示，不再把首段解析出来的 same-file local item / import alias 过早伪装成真实 concrete type
- deferred multi-segment `impl` / `extend` target 当前也不会被投影到 concrete local receiver surface：member typing / analysis completion 只继续枚举真实 receiver 的稳定候选，不把 `Counter.Scope.Config` 这类 deferred path 的方法伪装成 same-file `Counter` 成员
- invalid pattern-root shape diagnostics 当前也只在“pattern root 已解析成功且构造形状已知必错”时触发；same-file 已解析二段 enum variant path 的 unknown variant 现在也会显式报错，但 path pattern shape / deeper module-path case 仍保持保守，不提前下结论
- invalid path-pattern root diagnostics 当前也只在“path pattern root 已解析成功且 bare path 形状已知必错”时触发；unit variant 仍允许，same-file 已解析二段 enum variant path 的 unknown variant 与 const/static path 语义现在也会给出显式诊断，但 deeper module-path case 仍保持保守，不提前下结论
- const/static bare path pattern 现在也已经改成显式 unsupported diagnostics；但这仍然只覆盖 same-file root / same-file local import alias，cross-file / deeper module-path case 继续保守
- ambiguous method 的 type diagnostics 已开放，但 query / completion / rename 仍只接受唯一 candidate，不提前伪造模糊成员 truth surface
- bare single-segment unresolved diagnostics 已落地，但 multi-segment unresolved global / unresolved type diagnostics 仍然延后
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
  - 基于源码 offset 的 `symbol_at` / `hover_at` / `definition_at` / `references_at` / `prepare_rename_at` / `rename_at`
  - 基于源码 offset 的 `completions_at`
  - 基于源码文件快照的 `semantic_tokens`
- 当前位置语义查询已覆盖：
  - top-level item name
  - local binding
  - regular parameter
  - generic parameter
  - receiver `self`
  - enum variant token（definition、pattern use、constructor use）
  - struct field member token
  - unique impl / extend method member token
  - named type root、pattern path root、struct literal root
  - import alias 的 source-backed hover / definition / 同文件 reference / 同文件 rename / semantic-token 信息，以及 builtin type 的 hover / references / semantic-token 信息（但 builtin 仍无 source-backed definition / rename）
- `ql-resolve` 现在额外保留 item scope 和 function scope，`ql-analysis` 不再需要靠“重放 resolver 遍历顺序”去回推参数和泛型的声明位置
- receiver parameter 的精确 span 也已从 AST 打通到 HIR，查询和 diagnostics 会锚定 `self` / `var self` / `move self` 本身，而不是整个函数 span
- function / method declaration span 现在也保持精确，trait / impl / extend 里的多个方法会各自拥有独立 function scope，而不是共享整个块的 span
- named path 现在也会保留 segment span，因此 `Command.Config` / `Command.Retry(...)` 这类 variant use 可以锚定到尾段 token，而不是退回整个 path 或 enum root
- 显式 struct literal / struct pattern 字段标签现在也会进入 field query surface；但 shorthand `Point { x }` 这类双义 token 仍故意保守，让该 token 继续落在 local/binding 语义上
- import alias 现在也会先在 resolver 中固化成 source-backed `ImportBinding`，再被 query index 作为真实符号索引定义点与引用点
- 当同文件 local import alias 的原始路径恰好是单段、且命中本地 root struct item 时，显式 struct literal / struct pattern 字段标签与 field-driven shorthand rename 现在也会继续映射回原 struct field；同一条 canonicalization 也会服务 struct literal 字段检查与 struct/variant pattern root type checking
- same-file rename 现在也复用这套 query index，并且会先复用 lexer 的 identifier 规则校验新名字；裸关键字会被拒绝，确实要用关键字时必须显式写成转义标识符；当前 import alias、local struct field 与唯一 method symbol 都已经进入这组可安全 rename 的符号集，其中 field rename 会把 shorthand field site 自动扩写成显式标签，而从 shorthand token 上发起的 renameable binding rename 现在也会保持这条展开逻辑，避免把字段标签一起改坏；这条保真回归现在也已经锁住了 local / parameter / import / function / const / static 这类 item-value / binding 场景；ambiguous method surface 仍保持关闭
- 其中 free-function shorthand binding rename 现在也已经有显式 LSP parity 回归：`Ops { add_one }` 这类 shorthand token 会继续保留 field label，并把 rename 只落到绑定的 function declaration / use 上，而不是让桥接层退化成普通字段或局部变量改名
- plain import alias symbol 现在也有显式 same-file parity 回归：analysis / LSP 两层都会继续锁住 `import` binding 的 hover / definition / references / semantic-token 一致性，而不是让 source-backed import binding 与编辑器高亮、导航行为再度漂移
- 同一套 same-file rename truth surface 现在也已经有显式回归覆盖去锁住 type-namespace item：`type`、`opaque type`、`struct`、`enum`、`trait` 在 analysis / LSP 两层都会继续共享同一份 item identity，而不是在协议层各自做特判
- 同一套 same-file rename truth surface 现在也已经有显式回归覆盖去锁住 root value item：`function`、`const`、`static` 在 analysis / LSP 两层都会继续共享同一份 rename identity，而不是只在聚合 analysis 测试或 shorthand-binding 回归里被间接覆盖
- 这组 type-namespace item 现在也有 references / semantic-token parity 回归：`type`、`opaque type`、`struct`、`enum`、`trait` 的同文件 definition / references / semantic tokens 会继续复用同一份 `QueryIndex` occurrence，而不是在 LSP bridge 上额外猜测
- 同一组 type-namespace item 现在也有 hover / definition parity 回归：`type`、`opaque type`、`struct`、`enum`、`trait` 在 analysis 与 LSP 两层都会继续映射回同一份 item definition span
- same-file type-namespace item surface 现在也有显式聚合回归：`type`、`opaque type`、`struct`、`enum`、`trait` 会继续共享同一组 item truth surface，因此 hover / definition / references / semantic tokens 的 editor-facing 契约不会只靠分散的单类用例间接维持
- global value item 现在也有显式 query parity 回归：`const`、`static` 的定义点与 value-use 会继续复用同一份 `QueryIndex` symbol identity，因此 hover / definition / references / semantic tokens 在 analysis 与 LSP 两层都会一起收敛，而不是在 bridge 层做二次推断
- `extern` callable surface 现在也有显式 same-file parity 回归：无论是 `extern` block 成员、顶层 `extern "c"` 声明，还是带 body 的顶层 `extern "c"` 函数定义，定义点与 call site 都会继续复用同一份 `Function` identity，因此 hover / definition / references / rename / semantic tokens 在 analysis 与 LSP 两层都会一起收敛，而不是把 extern callable 当成特殊字符串表面处理
- extern callable value completion 现在也有显式 parity 回归：这三类 extern callable 会继续作为 value-context 下的 `Function` completion candidate 暴露给 analysis，并由 LSP bridge 稳定投影成 `FUNCTION` completion item，而不是只靠 ordinary free-function completion 用例间接覆盖
- ordinary free function 现在也有显式 same-file query parity 回归：direct call site 会继续复用同一份 `Function` identity，因此 hover / definition / references 在 analysis 与 LSP 两层都会一起收敛，而不是只靠 completion / rename 或 root-binding 聚合测试间接覆盖
- ordinary free function 现在也有显式 same-file semantic-token parity 回归：declaration 与 direct call site 会继续复用同一份 `Function` token surface，因此 analysis occurrence 与 LSP semantic tokens 不会再只靠聚合快照间接覆盖
- same-file callable surface 现在也有显式聚合回归：`extern` block callable、顶层 `extern "c"` 声明、顶层 `extern "c"` 定义与 ordinary free function 会继续共享同一组 callable truth surface，因此 hover / definition / references / semantic tokens 的 editor-facing 契约不会只靠分散的单类用例间接维持
- lexical semantic symbol 现在也有显式 same-file parity 回归：`generic`、`parameter`、`local`、`receiver self` 与 `builtin type` 在 analysis 与 LSP 两层都会继续复用同一套 lexical truth surface；其中 builtin type 仍然不是 source-backed declaration，因此保留 hover / references / semantic tokens，但不开放 definition / rename
- lexical rename surface 现在也有显式回归覆盖：`generic`、`parameter`、`local` 会继续共享同一套 same-file rename truth surface，而 `receiver self` 与 `builtin type` 会继续保持关闭，避免把没有 source-backed declaration 的 lexical surface 误开放成可编辑符号
- same-file completion 现在也复用同一份 query index：`ql-analysis` 会先把 value/type 位置映射到 resolver scope，再沿 parent scope 链聚合可见 symbol data；对于已经成功解析、且 receiver type 稳定的 member token，会提供保守 member completion；对于 same-file enum item root 的 parsed variant path，以及指向同文件根 enum item 的 local import alias path，也会直接补出 variant completion；同一条 follow-through 也会继续服务 hover / definition / references / same-file rename / semantic tokens；completion candidate 现在还会区分语义 label 与源码 insert text，因此 escaped identifier 不会在 LSP text edit 中退化成非法 keyword 文本；LSP 只负责把候选映射成协议 completion item 并按当前源码前缀过滤
- plain import alias 的 type-context completion 现在也有显式 parity 回归：analysis 会继续把 `import` binding 作为 type 候选产出，LSP bridge 会继续把它映射成 `MODULE` completion item，并保持稳定 text edit，而不是把 source-backed import candidate 在协议层退化成普通字符串候选
- free function 的 lexical value completion 现在也有显式 parity 回归：analysis 会继续把 callable declaration 作为 value 候选产出，LSP bridge 会继续把它映射成 `FUNCTION` completion item，并保持稳定 text edit，而不是让 lexical completion 与协议投影的 callable surface 再次漂移
- plain import alias 的 lexical value completion 现在也有显式 parity 回归：analysis 会继续把 source-backed `import` binding 作为 value 候选产出，LSP bridge 会继续把它映射成 `MODULE` completion item，并保持稳定 text edit，而不是让 value completion 与既有 import symbol identity 再次漂移
- builtin type 与 local struct item 的 type-context completion 现在也有显式 parity 回归：analysis 会继续把它们作为 type 候选产出，LSP bridge 会继续分别映射成 `CLASS` / `STRUCT` completion item，并保持稳定 text edit，而不是让 type completion 的 editor-facing 投影再次与 `QueryIndex` 脱节
- same-file type alias 的 type-context completion 现在也有显式 parity 回归：analysis 会继续把 `type alias` 作为 type 候选产出，LSP bridge 会继续把它映射成 `CLASS` completion item，并保持稳定 text edit，而不是让 source-backed alias candidate 在 editor-facing 投影中再次漂移
- same-file `opaque type` 的 type-context completion 现在也有显式 parity 回归：analysis 会继续把 `TypeAlias`-backed opaque alias 作为 type 候选产出，LSP bridge 会继续把它映射成 `CLASS` completion item，并保持 `opaque type ...` detail 与稳定 text edit，而不是让 opaque alias candidate 在 editor-facing 投影中再次漂移
- same-file generic 的 type-context completion 现在也有显式 parity 回归：analysis 会继续把 `generic` 作为 type 候选产出，LSP bridge 会继续把它映射成 `TYPE_PARAMETER` completion item，并保持稳定 detail 与 text edit，而不是让 lexical generic candidate 在 editor-facing 投影中再次漂移
- same-file enum 的 type-context completion 现在也有显式 parity 回归：analysis 会继续把 `enum` 作为 type 候选产出，LSP bridge 会继续把它映射成 `ENUM` completion item，并保持稳定 detail 与 text edit，而不是让 enum candidate 在 editor-facing 投影中再次漂移
- same-file trait 的 type-context completion 现在也有显式 parity 回归：analysis 会继续把 `trait` 作为 type 候选产出，LSP bridge 会继续把它映射成 `INTERFACE` completion item，并保持稳定 detail 与 text edit，而不是让 trait candidate 在 editor-facing 投影中再次漂移
- stable receiver field completion 现在也有显式 parity 回归：analysis 会继续把 `field` 作为 member 候选产出，LSP bridge 会继续把它映射成 `FIELD` completion item，并保持稳定 detail 与 text edit，而不是让 field candidate 在 editor-facing 投影中再次漂移
- stable receiver unique-method completion 现在也有显式 parity 回归：analysis 会继续把唯一 `method` candidate 作为 member 候选产出，LSP bridge 会继续把它映射成 `FUNCTION` completion item，并保持稳定 detail 与 text edit，而不是让稳定 receiver member surface 的 method 投影再次漂移
- same-file const / static 的 value completion 现在也有显式 parity 回归：analysis 会继续把 `const` / `static` 作为 value 候选产出，LSP bridge 会继续把它们映射成 `CONSTANT` completion item，并保持稳定 detail 与 text edit，而不是让这些 item-value candidate 在 editor-facing 投影中再次漂移
- same-file local 的 value completion 现在也有显式 parity 回归：analysis 会继续把 `local` 作为 value 候选产出，LSP bridge 会继续把它映射成 `VARIABLE` completion item，并保持稳定 detail 与 text edit，而不是让 lexical local candidate 在 editor-facing 投影中再次漂移
- same-file parameter 的 value completion 现在也有显式 parity 回归：analysis 会继续把 `parameter` 作为 value 候选产出，LSP bridge 会继续把它映射成 `VARIABLE` completion item，并保持稳定 detail 与 text edit，而不是让 parameter candidate 在 editor-facing 投影中再次漂移
- same-file lexical value candidate-list parity 现在也有显式回归：analysis / LSP 会继续让 import / const / static / extern callable / free function / local / parameter 这些 already-supported value surface 维持同一份有序候选列表、detail 渲染与 replacement text-edit 投影，而不是只靠分散的单类 completion 用例间接覆盖
- same-file enum variant completion 现在也有显式 parity 回归：analysis 会继续把 parsed enum path 上的 `variant` candidate 作为 completion 候选产出，LSP bridge 会继续把它映射成 `ENUM_MEMBER` completion item，并保持稳定 detail 与 text edit，而不是让 variant completion 的 editor-facing 投影再次漂移
- same-file import alias variant completion 现在也有显式 parity 回归：analysis 会继续让指向同文件根 enum item 的 local import alias path 产出 `variant` candidate，LSP bridge 会继续把它映射成 `ENUM_MEMBER` completion item，并保持稳定 detail 与 text edit，而不是让这条 alias follow-through 的 editor-facing 投影再次漂移
- same-file import alias struct-variant completion 现在也有显式 parity 回归：analysis 会继续让指向同文件根 enum item 的 local import alias struct-literal path 产出 struct-style `variant` candidate，LSP bridge 会继续把它映射成 `ENUM_MEMBER` completion item，并保持稳定 detail 与 text edit，而不是让这条 struct-variant alias follow-through 的 editor-facing 投影再次漂移
- remaining same-file variant-path completion contexts 现在也有显式 parity 回归：analysis / LSP 会继续锁住 direct struct-literal path 以及 direct/local-import-alias pattern path 上既有的 `variant` candidate 形态、detail 与 text edit，而不是让 constructor / pattern 上下文里的 editor-facing 投影再次漂移
- same-file variant-path candidate-list parity 现在也有显式回归：analysis / LSP 会继续让 enum-root / struct-literal / pattern path 及其 same-file import-alias 镜像上下文维持同一份有序 `variant` 候选列表、detail 渲染与 replacement text-edit 投影，而不是让这组 editor-facing 契约只靠单个候选测试间接覆盖
- same-file completion filtering parity 现在也有显式回归：analysis 已经锁住的 lexical scope visibility/shadowing 与 impl-preferred member filtering 行为，会继续原样投影到 LSP，而不是只通过单个 prefix 命中结果被间接覆盖；其中 lexical value visibility 的聚合回归现在也已经显式覆盖 import / function / local 的 detail 与 text edit 投影，而 impl-preferred member 聚合回归现在也已经显式覆盖 surviving candidate count 以及稳定 detail / text edit 投影
- same-file completion candidate-list parity 现在也有显式回归：analysis 已经锁住的 type-context 与 stable-member 完整候选列表，会继续原样投影到 LSP，而不是只覆盖单个 item 映射却放过排序、命名空间边界和完整成员集合；其中 type-context 总表现在已经显式覆盖 builtin / import / struct / `type` / `opaque type` / `enum` / `trait` / generic，而 stable-member 总表也已经显式覆盖 method / field 的 detail 与 text edit 投影
- shorthand struct field token query parity 现在也有显式回归：analysis 已经锁住的“shorthand token 继续落在 local/binding surface，而不是 field surface”这条边界，会继续原样投影到 LSP hover / definition，而不是在编辑器侧悄悄把 shorthand token 提升成 field symbol
- direct same-file variant / explicit field-label query parity 现在也有显式回归：analysis 已经锁住的 direct enum variant token 与 direct explicit struct field label 的 definition / references 行为，会继续原样投影到 LSP，而不是只有 import-alias 路径才被端到端覆盖
- direct same-file variant / explicit field-label semantic-token parity 现在也有显式回归：analysis 已经锁住的 direct enum variant token 与 direct explicit struct field label 的 occurrence-based highlighting，会继续原样投影到 LSP semantic tokens，而不是只有 import-alias 路径或总表测试在间接覆盖
- same-file direct symbol surface 现在也有显式聚合回归：direct enum variant token 与 direct explicit struct field label 会继续共享同一组 direct-symbol truth surface，因此 hover / definition / references / semantic tokens 的 editor-facing 契约不会只靠分散的单类用例间接维持
- direct stable-member query parity 现在也有显式回归：analysis 已经锁住的 direct field member 与唯一 method member 的 hover / definition / references，会继续原样投影到 LSP，而不是只剩 hover markdown 或其他间接覆盖
- direct stable-member semantic-token parity 现在也有显式回归：analysis 已经锁住的 direct field member 与唯一 method member 的 occurrence-based highlighting，会继续原样投影到 LSP semantic tokens，而不是只靠总表 semantic-token 测试间接覆盖
- same-file direct member surface 现在也有显式聚合回归：direct field member 与唯一 method member 会继续共享同一组 direct-member truth surface，因此 hover / definition / references / semantic tokens 的 editor-facing 契约不会只靠分散的单类用例间接维持
- impl-preferred member query parity 现在也有显式回归：analysis 已经锁住的“同名成员里优先选择 `impl` 方法而不是 `extend` 方法”这条 direct member query 边界，会继续原样投影到 LSP hover / definition / references，而不是在桥接层重新漂移
  - same-file semantic tokens 现在也复用同一份 query index：`ql-analysis` 直接从 source-backed occurrence 提取 token span 与 symbol kind，LSP 只负责按协议 legend 编码；没有稳定语义 identity 的 token 不会被伪装成高亮结果
- 这层边界先服务 CLI，并为后续 LSP 的 hover / definition / references / rename 打稳定地基
- 这层边界现在也开始服务 completion / semantic tokens；completion 当前覆盖同文件 lexical scope + parsed member token + parsed enum variant path（含 local import alias -> local enum item 的 follow-through），而 semantic tokens 也仍只覆盖已经进入统一 query surface 的 source-backed symbol；同文件 rename / references / definition 也会沿用同一份 variant / field symbol identity，并在 local import alias -> local struct item 时继续跟进显式字段标签与 field-driven shorthand rewrite
- 现在 `ql-lsp` 已经成为这层边界的第一个真实消费者，而不只是“未来会用到”
- 当前 rename 也仍刻意保守：目前只开放 function / const / static / struct / enum / variant / trait / type alias / field / method（仅唯一 candidate）/ local / parameter / generic / import 的同文件重命名；ambiguous method / receiver / builtin type、从 shorthand field token 本身发起的 field-symbol rename 以及 cross-file rename 仍需更完整查询面后再开放
- 当前仍刻意不宣称完整 member / module-path 查询：目前只覆盖 struct field、唯一 method candidate、enum variant token，以及稳定 receiver type 的 parsed member completion / enum item-root variant completion / local import alias 到 local enum item 的 variant follow-through / local import alias 到 local struct item 的 field/typeck follow-through / same-file semantic tokens / same-file rename；ambiguous method、import-graph/module-path deeper semantics、foreign import alias variant semantics、parse-error tolerant member completion，以及更广义的 payload/member-like query 仍需后续补齐

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
- backend / header unsupported diagnostics 现在也继续保留 deferred multi-segment source-backed type 文本，不会把 `Cmd.Scope.Config` 这类路径误折叠成 same-file `Command` 再报错
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
- 黑盒 backend / FFI unsupported 回归现在也锁住 deferred multi-segment source-backed type 文本：
  - `tests/codegen/fail/unsupported_deferred_multi_segment_type_build.ql`
  - `crates/ql-cli/tests/ffi_header.rs`
- 黑盒 `ql build` snapshot 现在也锁住 build-side `both` header 在 imported-host library 上的输出：
  - `tests/codegen/pass/extern_c_import_top_level.ffi.h`
- 真实 FFI smoke harness 现在不再手写 prototype，并且已经改成在同一次 `ql build --header-output` 中同时拿到 library artifact 与 C header，再让宿主消费生成的 header
- 真实 FFI smoke harness 现在还覆盖“Qlang 导出函数体内调用宿主提供的 imported C symbol”，并且同时验证 extern block 与 top-level extern 两种导入语法

这也是目录结构设计要前置考虑的原因。
