# 实现算法与分层边界

## 文档范围

这页只记录当前代码实际采用的算法、每层的输入输出，以及新功能应该接入的层次。未落地能力不在这里做承诺。

## 端到端实现总览

| 子系统 | 当前输入 | 当前输出 | 核心算法 | 当前稳定边界 |
| --- | --- | --- | --- | --- |
| `ql-span` | 原始 byte offset | `Span` 与行列换算基础 | 纯定位数据结构 | 不承载词法或语义逻辑 |
| `ql-lexer` | 源码字符串 | token 流 + recoverable lex errors | 单游标字符扫描 | 只做词法切分，不承载语义 |
| `ql-parser` | token 流 | AST + parser diagnostics | 手写递归下降 + precedence climbing + postfix loop | item / expr / pattern / stmt 分治 |
| `ql-diagnostics` | parser / semantic diagnostics | 统一文本渲染 | span + primary/secondary label 渲染 | 诊断模型独立于前端和后端 |
| `ql-fmt` | AST | 稳定格式化文本 | AST 定向 pretty-print | 不依赖 resolve / typeck |
| `ql-hir` | AST | arena-backed HIR | 结构化 lowering + sugar normalization | 只做语义前置正规化，不做求解 |
| `ql-resolve` | HIR | scope graph + resolution map + 保守诊断 | 先 seed，再按 lexical scope 递归解析 | 值/类型命名空间分离 |
| `ql-typeck` | HIR + resolution | first-pass types + semantic diagnostics | 结构化递归检查 + `unknown` 受控退化 | 不假装已经实现完整约束求解 |
| `ql-analysis` | 源码 | 聚合分析快照 | 顺序编排 + query index 双阶段索引 | CLI/LSP 共用分析入口 |
| `ql-project` | manifest path + package/workspace roots + public AST items | manifest graph + `.qi` artifact/status + interface summaries | TOML parsing + source discovery + interface render/load | project/workspace tooling 不复制语言语义 |
| `ql-mir` | HIR + resolution | 结构化 MIR body | 显式 CFG lowering + temp/local/scope 分配 | 为 ownership / codegen 提供稳定中层 |
| `ql-borrowck` | MIR + HIR/type info | ownership facts + diagnostics | worklist 前向数据流 | 当前聚焦 moved-vs-usable 与 cleanup/capture 事实 |
| `ql-codegen-llvm` | HIR + MIR + resolution + typeck | 文本 LLVM IR 或 codegen diagnostics | 受控子集 lowering | LLVM 只存在于 backend crate |
| `ql-driver` | 文件路径 + build/ffi options | `.ll` / `.obj` / `.exe` / `.lib` / `.a` / `.h` | 分析编排 + 工具链调用 + C surface projection + 失败保留中间产物 | CLI 不直接碰底层构建细节 |
| `ql-cli` | 命令行参数 | 文本输出 / 进程退出码 | 薄分发层 | 不重复实现 analysis/build/ffi logic |
| `ql-lsp` | 文档文本 + LSP 请求 | diagnostics / hover / definition / references / rename | 文档缓存 + analysis 重算 + 协议桥接 | 不复制编译器语义 |

## 前端基础层

### Span And Source Mapping

`ql-span` 是所有前端和工具层都共享的最底层定位抽象。

当前职责非常克制：

- `Span { start, end }` 统一使用 byte offset
- lexer、parser、HIR、diagnostics、query index、LSP bridge 都沿用同一套 span 语义
- 行列换算留到真正需要协议桥接或文本渲染时再做

这个设计的意义是：

- 编译器内部始终用 byte offset，避免一层层传不同坐标体系
- LSP 和 diagnostics 只做边界转换，而不是在核心语义层到处混入行列逻辑

### Lexer

当前 lexer 是手写单游标扫描器，不依赖生成器。

核心算法：

1. 通过 `char_indices()` 预先拿到 `(offset, char)` 序列。
2. 维护一个 `idx` 作为当前游标，用 `peek` / `peek_char` / `peek_next_char` 做有限前瞻。
3. 在 `lex_all` 中按优先级分派：
   - 空白字符直接跳过
   - `//` 与 `/* ... */` 注释直接消费
   - `f"` 和 `"` 进入字符串分支
   - 数字进入 `lex_number`
   - `` `ident` `` 进入 escaped identifier 分支
   - 标识符和关键字进入 `lex_ident_or_keyword`
   - 其余符号用单字符或双字符组合分派
4. 词法错误尽量恢复，而不是立即终止；最终总会追加一个 `Eof` token。

当前数据结构：

- `Token { kind, text, span }`
- `LexError { message, span }`
- `TokenKind` 为手写枚举，不在 lexer 阶段引入额外分类表

关键工程点：

- 数字字面量支持 `0x` / `0b` / `0o` 与 `_` 分隔，但非法后缀会给出 recoverable error。
- `_` 既可能是 wildcard token，也可能是 `_value` 这类合法标识符前缀，lexer 先看后一个字符再决定。
- span 从第一层开始就精确到 token，本身就是后续 diagnostics、HIR 和 LSP 的底座。

当前刻意不做：

- lexer 不做缩进语义
- lexer 不做插值字符串拆分
- lexer 不做宏 token tree

### Parser

当前 parser 采用手写递归下降，但表达式部分不是“每个优先级一套函数”的传统写法，而是混合了 precedence climbing 和 postfix loop。

核心算法：

1. `item`、`stmt`、`pattern`、`expr` 分文件分责任，避免单个巨型 parser 文件失控。
2. 表达式入口先走 `parse_binary_expr(min_prec, mode)`。
3. `parse_binary_expr` 先解析 prefix/head expression，再在循环里依据当前 token 决定：
   - 运算符种类
   - 优先级
   - 左结合还是右结合
4. postfix 规则单独由 `parse_postfix_expr` 循环吸收：
   - `call`
   - `member`
   - `bracket`
   - postfix `?`
5. 结构体字面量歧义通过 `StructLiteralMode` + `looks_like_struct_literal` 控制，不让 `if` / `for` / `while` 头部被 `{ ... }` 误判成字面量。

当前数据与约束：

- AST 节点全部带 span
- declaration name、generic param、regular param、pattern field、struct literal field、named call arg、closure param 都保留精确 name span
- 函数签名抽象在 free function、trait item、impl method、extern function 之间复用

为什么这样分层：

- 语法树保留接近源码的表面结构，方便 formatter 和 parser diagnostics
- 名称解析、类型检查和 IDE 查询不反向污染 parser

当前刻意不做：

- parser 不尝试做语义恢复
- parser 不做宏展开
- parser 不做复杂 desugaring，除了维持语法上必要的结构化节点

### Diagnostics Renderer

`ql-diagnostics` 是独立 crate，而不是 parser/typeck 各自打印错误文本。

当前算法：

1. 各层只构造 `Diagnostic`、`Label` 和 message。
2. 文本渲染器按 span 定位源码切片。
3. 按 primary / secondary label 输出头部与补充信息。
4. CLI 和测试统一消费相同渲染结果。

这个边界的意义：

- parser、resolve、typeck、borrowck、codegen 都能共享同一种错误表现形式
- 测试可以直接锁 stderr 快照，不需要为每层发明一套输出格式

### Formatter

`ql-fmt` 当前仍然是 AST 定向格式化，而不是“先做语义分析再格式化”。

当前算法：

1. parser 先生成 AST。
2. formatter 递归遍历 item / type / stmt / pattern / expr。
3. 对 block、参数列表、泛型列表、模式和表达式应用固定排版规则。
4. 始终输出单一风格，不提供样式选项。

为什么坚持 AST 边界：

- formatter 依赖语法，不依赖 resolve/typeck
- 语言语义层扩展时，不会被格式化器反向卡住

## 语义前端层

### HIR Lowering

`ql-hir` 的工作不是“再造一个 AST”，而是把前端表面语法整理成稳定、可引用、适合后续语义层消费的结构。

当前算法：

1. `Lowerer` 顺序遍历 AST module。
2. `item`、`type`、`block`、`stmt`、`pattern`、`expr`、`local` 分别进入独立 arena。
3. lowering 时为每个节点分配稳定 ID。
4. 语法糖在这里做第一层正规化：
   - 结构体 pattern shorthand 补成真实 binding pattern
   - 结构体字面量 shorthand 补成真实 `Name("field")` 表达式
5. 所有节点继续保留 span，尤其保留更精确的 name span。

当前数据结构特点：

- HIR arena + stable ID 是后续 resolve、typeck、query、MIR 的共同引用基础
- `LocalId` 在 lowering 时就建立，pattern binding 不需要等到 typeck 才临时发明局部变量身份

为什么不把 resolve/typeck 塞进这一层：

- HIR 只负责“语义前置正规化”
- 名称解析和约束求解是另一层职责，混在一起会让查询系统和增量分析以后很难拆

### Name Resolution

`ql-resolve` 是当前语义前端最关键的边界之一，它负责构建 lexical scope graph 和 resolution map，而不是把查找逻辑散落进 type checker。

当前算法是典型的“两阶段种子 + 递归解析”：

1. 创建 module scope。
2. `seed_builtins` 把内建类型放入类型命名空间。
3. `seed_imports` 先把 import alias 构造成 source-backed `ImportBinding`，再放进值/类型 lookup 表。
4. `seed_top_level_items` 先登记顶层 item。
5. 再按 module item 顺序递归进入 `resolve_item`。
6. 对函数、block、closure、match arm、for-loop 等创建子 scope。
7. 解析表达式/模式/类型时，沿 scope parent 链向上查找。
8. 值命名空间和类型命名空间分离，避免后续 trait/type item 扩展时互相污染。

当前已覆盖的解析对象：

- local binding
- regular parameter
- receiver `self`
- generic parameter
- top-level item
- import alias（携带本地名称、导入路径与定义 span 的 `ImportBinding`）
- builtin type
- struct literal root
- pattern path root
- extern callable identity

当前保守策略：

- 只对绝对可靠的语义错误给诊断，例如 method receiver 作用域外非法使用 `self`，以及 bare single-segment value/type root 的 unresolved
- multi-segment unresolved global / unresolved type 暂不全面报错，避免 import / module / prelude 规则未定前制造假阳性

为什么这个边界必须单独存在：

- `ql-typeck`、`ql-analysis`、`ql-lsp` 都需要稳定 resolution 结果
- item scope 和 function scope 一旦被 resolver 记录，IDE 查询不需要再“重演一遍语义遍历”

### First-pass Type Checking

`ql-typeck` 现在是第一版真实类型检查层，但仍然明确保持“first-pass typing”定位。

核心算法：

1. `check_module` 逐 item 进入。
2. `check_function` 建立当前返回类型、参数类型和 `self_type` 上下文。
3. `check_block` 线性检查语句，收集 block tail 类型。
4. `check_expr` 递归分派表达式种类。
5. 对需要上下文的表达式传入 `expected` 类型，例如 closure 和分支统一。
6. 检查结果写入：
   - `expr_types`
   - `pattern_types`
   - `local_types`
7. 当前无法可靠建模的地方退化为 `Ty::Unknown`，但已落地的诊断照常给出。

当前真实已做的检查包括：

- return-value 类型检查
- `if` / `while` / match guard 的 `Bool` 条件检查
- callable arity / argument type 检查
- tuple destructuring arity 检查
- direct closure 对 expected callable type 的 first-pass 检查
- struct literal 字段存在性、缺失字段、字段类型检查
- 源码层 fixed array type expr `[T; N]`：保留长度源码文本用于 formatter，同时 lowering 成统一语义长度供 HIR / typeck / query 使用
- homogeneous array literal inference，与 expected fixed-array context 下的元素类型约束
- 保守 tuple / array indexing：array element projection、支持同文件 foldable integer constant expression 与 immutable direct local alias 复用的 constant tuple indexing、array index type 检查、tuple out-of-bounds 检查
- equality operand compatibility
- bare mutable binding assignment diagnostics：对 `var` local / `var self` 做 assignment target 可写性检查
- 非 binding assignment target diagnostics：`const` / `static` / function / import binding 赋值会显式报错；而 tuple-index / struct-field / fixed-array literal-index 写入、非 `Task[...]` 元素的 dynamic array assignment，以及 `Task[...]` 动态数组的保守 write/reinit 子集已经进入稳定实现
- struct member existence
- same-file local import alias value/callable canonicalization
- ambiguous method member diagnostics
- invalid projection receiver diagnostics
- invalid struct-literal root diagnostics
- invalid pattern-root shape diagnostics
- invalid path-pattern root diagnostics
- same-file const bare path-pattern literal folding + unsupported static/non-scalar path-pattern diagnostics
- pattern root / literal compatibility
- calling non-callable values

关键设计取舍：

- `unknown` 不是偷懒，而是明确的退化阀门
- assignment target 已不再只接管 bare binding：稳定 projection path（tuple/struct-field/fixed-array literal index）、普通 dynamic array assignment，以及 `Task[...]` dynamic array 的保守子集都已接入；更广义的 arbitrary dynamic overlap / 完整 place system 仍保持保守
- import alias 的 value/callable typing 先只复用 same-file single-segment canonicalization，不把这条路径误写成完整 module graph / foreign import 语义
- ambiguous method 先只升级成显式 type diagnostics，不提前宣称 completion / rename / query 已经具备模糊候选真值模型
- projection receiver diagnostics 也先只覆盖“已知必错”的类型，不拿 generic / unresolved / module-path deferred case 冒进报错
- deeper path-like call 在 receiver 已知必错时，也不能回头复用 root callable signature；如果 `callee` 不是 bare name，就只能走真实 member-target 或 value type，而不能从 `ping.scope` 的 root `ping` 偷出函数签名继续做 call checking
- struct-literal root diagnostics 也先只覆盖“root 已解析成功且构造形状已知必错”的 case；same-file 已解析二段 enum variant path 的 unknown variant 现在也会升级成显式错误，但 deeper module-path 仍不提前下结论
- unsupported 或仍 deferred 的 struct literal root 也直接回退成 `unknown`，避免把首段解析结果伪装成稳定 item type 后继续触发级联 type mismatch
- deferred multi-segment type path 也保持 source-backed `Named`，不把 same-file local item / import alias 的首段解析结果误升级成稳定 concrete type
- deferred multi-segment `impl` / `extend` target 也不参与 concrete local receiver 的成员投影；只有真实 receiver type 自己的稳定 field / method candidate 会继续出现在 typing 与 completion surface 上
- pattern-root shape diagnostics 也先只覆盖“pattern root 已解析成功且构造形状已知必错”的 case；same-file 已解析二段 enum variant path 的 unknown variant 现在也会升级成显式错误，但 path-pattern semantics / deeper module-path 仍不提前下结论
- bare path-pattern diagnostics 也先只覆盖“path root 已解析成功且 bare path 形状已知必错”的 case；unit variant 保持允许，同文件已解析二段 enum variant path 的 unknown variant 与 const/static pattern 语义现在会显式报错，但 cross-file / deeper module-path 仍继续保守
- const/static bare path-pattern diagnostics 也先只覆盖 same-file root 与 same-file local import alias，不把 cross-file constant semantics 误写成已完成
- 还没建立完整 import/module/member/索引协议前，不把每个未知都提前升级成硬错误

### Unified Analysis And Query Index

`ql-analysis` 是当前 CLI 和 LSP 共用的分析总入口。

当前编排算法：

1. lexer + parser 产出 AST 和 parser diagnostics。
2. 若 parse 成功，lower 到 HIR。
3. 运行 resolve。
4. 运行 typeck。
5. lower 到 MIR。
6. 在 MIR 上跑 borrowck。
7. 聚合 diagnostics。
8. 构建 query index。
9. 返回 `Analysis { ast, hir, mir, resolution, typeck, borrowck, index, diagnostics }`

`QueryIndex` 当前采用双阶段索引算法：

1. `index_definitions`
   - 先把 item、function、param、generic、self、local 的定义位置全部登记
2. `index_uses`
   - 再遍历 type / pattern / expr，把 use-site 映射到前面登记的 symbol data
3. 为每个 occurrence 绑定稳定 `SymbolKey`
   - item / extern function / variant / field / method / local / param / generic / receiver `self` 使用真实语义 ID
   - 暂时没有可解析语义 ID 的 declaration site 退化为 `DefinitionSpan`
   - import alias 使用 source-backed `ImportBinding`
   - builtin type 使用可稳定分组的轻量 key
4. 把 occurrence 按 `(span.len(), span.start, span.end)` 排序
5. 查询 `symbol_at(offset)` 时优先命中更窄的 span，避免整个大表达式覆盖住精确名字

这个 query index 目前支撑：

- `symbol_at`
- `hover_at`
- `definition_at`
- `references_at`
- `completions_at`
- `semantic_tokens`
- `prepare_rename_at`
- `rename_at`

当前覆盖面：

- top-level item name
- local binding
- parameter
- generic parameter
- receiver `self`
- enum variant token
- struct field member token
- explicit struct literal field label
- explicit struct pattern field label
- unique method member token
- named type root
- pattern path root
- struct literal root
- import alias 的 source-backed hover / definition / references / rename / semantic-token 信息，以及 builtin type 的 hover / references / semantic-token 信息（builtin 仍无 source-backed definition / rename）

当前 rename 算法也直接建立在这套 occurrence 分组之上：

1. 先用 `occurrence_at(offset)` 找到当前最窄命中的 symbol
2. 只允许已经证明“引用面足够稳定”的 symbol kind 进入 rename
3. 新名字先走 lexer 级 identifier 校验
4. 裸关键字直接拒绝；确实要用关键字时，要求用户显式传入转义标识符
5. 再复用同一个 `SymbolKey` 收集同文件 occurrence，按源码顺序输出 text edits
6. 如果目标是 local struct field，还会额外把 shorthand struct literal / struct pattern site 重写成显式标签，例如 `x` -> `coord_x: x`
7. 如果 rename 是从 shorthand token 上发起，且该 token 绑定到 renameable symbol（例如 local / parameter / import / function / const / static），同样会把该 site 改写成 `field_label: new_name`，避免把字段标签一起改坏

当前已经有显式 shorthand-binding rename parity coverage 的 root value-item：

- `function`

当前刻意保守不开放 rename 的对象：

- ambiguous method/member surface
- receiver `self`
- builtin type
- 从 shorthand struct field token 本身发起的 field-symbol rename
- cross-file symbol

当前已经有显式 regression coverage 的 renameable item-kind：

- `type alias`
- `struct`
- `enum`
- `trait`
- `function`
- `const`
- `static`
- `variant`
- `field`
- `method`（仅唯一 candidate）
- `local` / `parameter` / `generic` / `import`

当前已经有显式 root value-item rename parity coverage：

- `function`
- `const`
- `static`

当前已经有显式 references / semantic-token parity coverage 的 type-namespace item：

- `type`
- `opaque type`
- `struct`
- `enum`
- `trait`

当前已经有显式 hover / definition parity coverage 的 type-namespace item：

- `type`
- `opaque type`
- `struct`
- `enum`
- `trait`

当前已经有显式完整 query parity coverage 的 global value item：

- `const`
- `static`

当前已经有显式完整 parity coverage 的 extern callable surface：

- `extern` block function declaration / call site
- top-level `extern "c"` declaration / call site
- top-level `extern "c"` function definition / call site

当前已经有显式完整 completion parity coverage 的 extern callable value surface：

- `extern` block function declaration
- top-level `extern "c"` declaration
- top-level `extern "c"` function definition

当前已经有显式完整 query parity coverage 的 ordinary free function surface：

- free function declaration / direct call site

当前已经有显式 semantic-token parity coverage 的 ordinary free function surface：

- free function declaration / direct call site

当前已经有显式完整 parity coverage 的 lexical semantic symbol surface：

- `generic`
- `parameter`
- `local`
- `receiver self`
- `builtin type`（仅 hover / references / semantic tokens；无 definition / rename）

当前已经有显式 rename parity coverage 的 lexical surface：

- `generic`
- `parameter`
- `local`

当前已经有显式 type-context completion parity coverage 的 type surface：

- `builtin type`
- `struct`
- `type`
- `opaque type`
- `trait`
- `generic`
- `enum`
- `receiver self`（刻意关闭）

当前已经有显式 direct semantic-token parity coverage 的 same-file surface：

- enum variant token（definition + direct use）
- explicit struct field label（definition + literal/pattern/member uses）

当前已经有显式 direct query parity coverage 的 stable-member surface：

- struct field member token
- unique method member token
- impl-preferred method member token

其中 `impl-preferred method member token` 当前也显式锁住了 references：

- 同名成员冲突时继续选择 `impl` 方法 declaration
- 同文件 references 继续只聚合该 `impl` 方法 declaration + use
- 不会把同名 `extend` 方法 declaration 误并入同一组 query result

当前已经有显式 direct semantic-token parity coverage 的 stable-member surface：

- struct field member token
- unique method member token

当前 lexical-scope completion 也直接建立在同一套 query / scope 数据之上：

1. 遍历 item / function / block / pattern / expr / type，记录 `CompletionSite { span, scope, namespace }`
2. 记录 site 时，把对应 `ScopeId` 的 value/type bindings 预先映射成可复用的 `SymbolData`
3. 查询 `completions_at(offset)` 时，先选出“覆盖当前位置的最窄 site”
4. 按 site 的 namespace 决定走 value 还是 type completion
5. 沿 parent scope 向外收集可见 binding，并用名字去重，保证 shadowing 语义正确
6. completion candidate 同时保留语义 label 和源码 insert text；如果 symbol 名字本身是 keyword，就把 insert text 转成 escaped identifier，避免协议层写回非法源码
7. LSP bridge 再把这些候选转成协议层 `CompletionItem`，并只在桥接层做源码前缀过滤与 text edit range 生成

当前 completion 也刻意保守：

- 只覆盖同文件 lexical scope + parsed member token + parsed enum variant path
- lexical-scope 部分只覆盖已经进入 HIR / resolver 的 value/type 位置
- 不宣称 ambiguous member completion 已完成
- 不尝试为 parse error / incomplete member token 伪造语义结果
- 不做 cross-file / package-indexed completion

当前 parsed member-token completion 进一步复用同一套 `SymbolData`：

1. 只在已经成功解析的 `ExprKind::Member` 上记录 member completion site
2. 先读取 receiver 的 `Ty`
3. 只有 receiver 是稳定的 `Ty::Item` 时才继续
4. impl method 先收集；同名多个 candidate 直接视为 ambiguous，不产生 completion
5. extend method 再收集；若名称已被 impl method 占用，则不覆盖
6. struct field 最后补入；若与 method 同名，则按现有 member 选择优先级继续让 method 胜出
7. 最终候选仍走统一的 LSP 前缀过滤和 text edit 生成

当前 enum item-root variant completion 进一步复用同一条 variant truth surface：

1. 仍然只在已经成功解析、且对象仍是 root `Name` 的第一段 `ExprKind::Member` 上工作
2. 如果对象表达式没有稳定 receiver `Ty`，再回退看对象表达式本身是否解析成 `ValueResolution::Item`
3. 只有该 item 确实是 enum，才暴露 variant completion items
4. deeper variant-like member chain 不会继续复用 root enum truth；`Command.Retry.more` 这类 case 只保留上层 lexical completion fallback，不再伪造 variant completion / query identity
5. imported alias / deeper module graph / foreign enum 不会被伪装成“已经支持”

当前 struct-literal / pattern variant path 也沿用同一个保守边界：

1. 只有严格两段 `Root.Variant` path 才继续复用 enum variant truth surface
2. `Command.Config { ... }`、`Cmd.Retry(...)`、`Command.Stop` 这类两段 path 继续保留既有 variant query / completion / rename / semantic-token 能力
3. `Command.Scope.Config { ... }`、`Cmd.Scope.Retry(...)` 这类更深 path 不会继续从 root enum/import alias 偷出 variant identity
4. 这类更深 path 只保留上层 lexical fallback，不伪造 variant occurrence 或 `ENUM_MEMBER` completion
5. deeper module graph / foreign enum 仍明确不支持

当前 struct field truth surface 也沿用同样的 root-path 边界：

1. 只有严格 root struct path 才允许把显式字段标签接进 field query / rename / semantic-token surface
2. `Point { x: value }`、`P { x: value }` 这类 root struct path 继续保留既有字段标签语义
3. `Point.Scope.Config { x: value }`、`P.Scope.Config { x: value }` 这类更深 path 不会继续从 root struct 偷出 field identity
4. 这类更深 path 只保留上层 lexical fallback，不伪造字段 occurrence
5. 如果更深 path 上使用 shorthand token，例如 `Point.Scope.Config { x }` / `Point.Scope.Config { source }`，token 仍复用它本来已有的 lexical symbol（local / pattern binding / import alias）；因此 hover / definition / references / semantic tokens 继续走 lexical surface，同文件 rename 也只做 binding/import 自身的 raw replacement，不执行依赖 field owner 的 shorthand 扩写
6. 这仍然不是 module-path struct semantics；cross-file / deeper module graph 继续关闭

当前同文件 local import alias variant follow-through 继续复用这条 truth surface：

1. 不改 resolver 的 root-binding 语义，也不引入 module graph
2. 只在 import alias 的原始路径恰好是单段、且该单段命中同文件根 enum item 时继续跟进
3. root token 仍然保持 import alias 自身的 source-backed symbol identity
4. 只有 alias root 的第一段 variant tail token / completion / rename occurrence / semantic token 才继续复用 enum variant truth surface
5. `Cmd.Retry.more` 这类更深 member chain 不会继续从 alias root 偷出 enum variant identity
6. foreign import alias、multi-segment import path、deeper module graph 仍明确不支持

也就是说，当前这块能力只是“在稳定 receiver type、same-file enum item root，以及同文件 local import alias -> local enum item 上复用已有 member/variant semantics”，并继续服务 query / completion / same-file rename / semantic tokens；它仍然不是“完整编辑器智能补全”。

当前同文件 local import alias struct-item follow-through 则继续复用现有 field/type truth surface：

1. 仍然不改 resolver 的 root-binding 语义，也不引入 module graph
2. `ql-typeck` 只在 `TypeResolution::Import` / `ValueResolution::Import` 的原始路径恰好是单段、且该单段命中同文件 root item 时，才把 alias 规范化回本地 item
3. struct literal 的字段检查（现在也包括 enum struct-variant literal）与 struct/variant pattern root 检查都复用这条 canonicalization，不额外引入第二套 alias 解释器
4. `ql-analysis` 里的显式 struct literal / struct pattern 字段标签与 field-driven shorthand rename，也只在 canonicalized item 确实是 struct 时才继续映射回原 struct field
5. foreign import alias、multi-segment import path、deeper module graph，以及 query-side enum variant field symbol 建模，仍明确不支持

也就是说，当前这块能力只是“在同文件单段 local import alias -> local struct item / local enum item 时，补齐原本已经存在的 struct-field / enum-variant-literal / pattern-root truth surface”，而不是把 import alias 升级成了完整 module-path 语义。

当前 semantic tokens 也建立在同一套 occurrence / symbol kind 数据之上：

1. 直接复用 `QueryIndex` 已有的 source-backed occurrence
2. 把 occurrence 的 `span + SymbolKind` 投影成 `SemanticTokenOccurrence`
3. 按源码起点排序，并按 `(span, kind)` 去重，避免 definition/use 或多条索引路径产生重复 token
4. LSP bridge 再把 `SymbolKind` 映射到固定 legend，例如 type / class / enum / enumMember / parameter / variable / property / function / method
5. 最终按 LSP 语义高亮协议编码为 delta line / delta start 序列

当前 semantic tokens 也刻意保守：

- 只覆盖已经进入统一 query surface 的 source-backed symbol
- 不为 unresolved / ambiguous / parse-error token 伪造语义高亮
- 不在 LSP 层重跑一遍 ad-hoc 语义判断
- 不宣称跨文件或 project-indexed semantic classification 已完成

## 中层表示与所有权分析

### MIR Lowering

`ql-mir` 不是 LLVM IR 的前一道“薄壳”，而是 Qlang 自己的结构化中层。

当前 `BodyBuilder` 的 lowering 算法：

1. 为函数分配 root scope。
2. 创建 entry block 和 return block。
3. 分配 `$return` return slot local。
4. 先 lower 参数，把 param / `self` 绑定到 MIR local。
5. 把函数体 block lower 到显式 CFG。
6. 语句和表达式按不同入口 lowering：
   - `lower_stmt`
   - `lower_expr_to_operand`
   - `lower_expr_into_target`
   - `lower_expr_to_place`
7. 需要中间值时分配 temp local。
8. `defer` 以 cleanup registration 的形式进入 MIR。
9. scope 退出时插入显式 cleanup run。

当前已 lower 成显式 CFG 的结构：

- block tail value
- `if`
- `while`
- `loop`
- `break`
- `continue`
- `return`

当前仍保留为结构化高层 terminator 的结构：

- `match`
- `for`
- `for await`

关键工程点：

- MIR local、block、scope、cleanup、closure identity 都是稳定 ID
- closure capture facts 已 materialize 到 MIR，不再要求 ownership 层临时回推

### Borrow / Ownership Facts

`ql-borrowck` 目前不是完整 borrow checker，而是“可扩展的前向 ownership facts 分析器”。

当前主算法是 worklist 前向数据流：

1. 以 body local 数量初始化每个 block 的 entry/exit state。
2. 从 entry block 开始推进 worklist。
3. 对每个 block 执行 transfer：
   - `apply_statement`
   - `apply_terminator`
   - `apply_rvalue`
4. block exit 状态变化时，把 successor 重新入队。
5. CFG 稳定后，再按 block 顺序回放事件并产出用户可见 diagnostics。

当前状态模型关注点：

- local 是 usable、moved 还是 maybe-moved
- move origin
- read / write / consume event
- cleanup 对状态的反向影响
- closure may-escape facts

当前已经真实生效的事实包括：

- direct local receiver + 唯一 `move self` method candidate 会触发 consume
- `RunCleanup` 会驱动 deferred expr 的 read / consume / root-write
- cleanup 的 LIFO 顺序会影响 ownership diagnostics
- `move` closure 创建时会消费 direct-local captures
- 非 `move` closure capture 也会记为真实 read
- closure return / call-arg / call-callee / captured-by-closure 会形成 conservative may-escape facts

为什么先做 facts，再做完整 borrow semantics：

- 这样后续可以在已有 MIR 和 state merge 上继续增加 call contract、borrow kind、drop elaboration、escape graph
- 不需要为每个新规则重写一套分析框架

## 后端与工具链层

### LLVM Backend

`ql-codegen-llvm` 只消费上游分析结果，不向前泄漏 LLVM 细节。

当前 lowering 算法：

1. 接收 `CodegenInput { hir, mir, resolution, typeck, module_name, mode }`
2. 先收集本次需要发射的函数集合
3. program mode:
   - 识别用户态 `main`
   - 把用户入口 lower 成内部符号
   - 额外生成宿主 `main` wrapper
4. library mode:
   - 导出可达 free function 集合
   - 保留 reachable `extern "c"` 声明，避免 direct-call library 丢声明
5. 对带 body 的顶层 `extern "c"` 定义：
   - 使用稳定 C 符号名，而不是内部 mangled name
   - 保持和普通 Qlang 函数共用 MIR lowering 路径
5. 按函数签名先生成 `declare` / `define`
6. 再 lower MIR 子集中的：
   - scalar arithmetic
   - compare
   - branch
   - direct call
   - return

当前支持矩阵刻意受控：

- 顶层 free function
- `extern "c"` 顶层声明、extern block 声明和顶层函数定义
- 标量整数 / `Bool` / `Void`
- direct function call
- 基础分支与返回

当前失败策略：

- 后端遇到未支持能力时返回结构化 diagnostics，而不是 panic
- first-class function value 当前明确返回 unsupported diagnostic
- deferred multi-segment source-backed type 当前也会在 backend / header unsupported diagnostics 中保留原始路径文本，不把 `Cmd.Scope.Config` 误折叠成 same-file concrete type
- program-mode 入口 `main` 当前明确拒绝显式 ABI，避免和宿主 `@main` wrapper 冲突

这层边界的核心纪律：

- MIR/HIR 里表达语言语义
- LLVM 只负责把当前已支持子集翻译成 IR
- 不把“为了 LLVM 好写”的技巧反向污染前端结构

### Build Driver And Native Artifact Orchestration

`ql-driver` 负责把“分析成功”变成“产物成功”，但它本身不做 CLI 参数解析，也不做语义分析实现。

当前 `build_file` 算法：

1. 校验输入路径必须是单个文件。
2. 读取源码文本。
3. 调用 `analyze_source`。
4. 若 analysis 带错误，直接返回带源码与 diagnostics 的 `BuildError::Diagnostics`。
5. 生成 `module_name`。
6. 调用 `emit_module` 拿到文本 LLVM IR。
7. 计算默认输出路径：
   - `target/ql/<profile>/<stem>.<ext>`
8. 若启用了 build-side C header：
   - 只允许 `emit` 是 `dylib` / `staticlib`
   - 先把 header 输出路径解析成显式路径：
     - `--header-output` 直接使用显式路径
     - 否则使用主 library artifact 所在目录 + 源码 stem
   - 若 header 输出路径与主 artifact 输出路径相同，直接返回 invalid input
9. 根据 `emit` 分派：
   - `llvm-ir` 直接写 `.ll`
   - `obj` 先出中间 `.codegen.ll` 再调用 compiler
   - `exe` 先出中间 `.codegen.ll` 和 `.codegen.obj/.o` 再调用 linker/compiler
   - `dylib` 先筛出 public 顶层 `extern "c"` 导出符号；若为空则直接返回 invalid input，再出中间 `.codegen.ll` 和 `.codegen.obj/.o` 调用 shared-library link
   - `staticlib` 先出中间 `.codegen.ll` 和 `.codegen.obj/.o` 再调用 archiver
10. 若主 artifact 成功且存在 build-side C header 请求：
   - 复用同一份 `source + Analysis` 调用 header 投影
   - 成功则把 `CHeaderArtifact` 挂到 `BuildArtifact`
   - 失败则删除刚生成的主 library artifact，再把 header 错误映射回 build error
11. 失败时按阶段尽量保留中间产物，方便排查 toolchain 问题。

当前工具链探测/配置规则：

- compiler 优先走 `QLANG_CLANG`
- archiver 优先走 `QLANG_AR`
- `QLANG_AR_STYLE` 可显式指定 `ar|lib`
- Windows 下 `dylib` 链接会把导出符号展开成 `/EXPORT:<symbol>` 透传给 linker
- Windows 下建议指向 `.exe` 或 `.cmd` wrapper，而不是裸 `.ps1`

这个分层的意义：

- `ql-cli` 保持薄
- `ql-codegen-llvm` 不直接负责文件系统和外部进程
- 工具链失败能以结构化 build error 对外暴露

当前 `emit_c_header` 算法：

1. 校验输入路径必须是单个文件。
2. 读取源码文本。
3. 复用 `analyze_source`，确保 parser / resolve / typeck diagnostics 和 build 路径一致。
4. `emit_c_header_from_analysis` 作为内部 helper，允许 build orchestration 直接复用已有的 `source + Analysis`，避免 build sidecar 重新 parse / resolve / typeck。
5. 根据 `CHeaderSurface` 分类可投影的函数：
   - `exports` 只收集 public 顶层 `extern "c"` 定义
   - `imports` 收集顶层 `extern "c"` 声明与 `extern "c"` block 成员声明
   - `both` 按源码顺序合并 import/export surface
6. 对选中的 function：
   - 拒绝 generics、`where`、`async`、`unsafe fn`
   - 使用 `ql-typeck::lower_type` 把 HIR type 投影到当前 C 支持矩阵
7. 输出固定结构：
   - include guard
   - `#include <stdbool.h>`
   - `#include <stdint.h>`
   - `extern "C"` wrapper
   - declaration list
8. 默认输出路径按 surface 选择：
   - `exports` -> `target/ql/ffi/<stem>.h`
   - `imports` -> `target/ql/ffi/<stem>.imports.h`
   - `both` -> `target/ql/ffi/<stem>.ffi.h`
9. include guard 按最终输出头文件名生成，确保 export/import/both 三份头文件能并存。
10. build-side sidecar 与 `ql ffi header` 共享同一套 render/write 逻辑，只是默认输出目录不同：
    - `ql ffi header` 默认写 `target/ql/ffi/`
    - `ql build --header*` 默认写主 library artifact 同目录

这层的关键纪律是：头文件生成依然建立在 analysis 之后，而不是让 CLI 重新扫描语法树或手写一套类型映射。

### CLI Dispatch Layer

`ql-cli` 当前是有意保持克制的薄入口。

当前算法非常简单：

1. 解析子命令和参数。
2. 按命令把请求路由到对应 crate：
   - `check` -> `ql-analysis`
   - `fmt` -> `ql-fmt`
   - `mir` -> `ql-analysis::render_mir`
   - `ownership` -> `ql-analysis::render_borrowck`
   - `build` -> `ql-driver`
   - `ffi header` -> `ql-driver`
3. 统一做 diagnostics/rendering/exit-code 映射。

为什么要刻意保持薄：

- CLI 是最容易变成“大杂烩”的一层
- 一旦把分析、构建、文本格式化都写回 CLI，后续 LSP 和测试就无法复用

## IDE 与协议桥接层

### LSP Server

`ql-lsp` 是真实的语言服务实现，不重复实现编译器语义。

初始化时声明的主要能力包括：

- full-sync text document lifecycle 与 diagnostics
- hover、declaration、definition、typeDefinition、implementation
- references、documentHighlight、documentSymbol、workspaceSymbol
- completion（含 `.` trigger）、codeAction、formatting
- full document semantic tokens
- prepareRename 与 rename

当前运行算法：

1. `didOpen` / `didChange`
   - 把文档文本写入 `DocumentStore`
   - 重新分析并推送 diagnostics
2. `didClose`
   - 从 store 删除文档并清空 diagnostics
3. request handler
   - 从 `DocumentStore` 读取当前文本
   - 用 `Position -> byte offset` 进入 compiler/query surface
   - 若文件属于 package/workspace，则同时读取 package analysis、workspace roots 与打开中的源码文档
4. healthy source
   - 优先走 source-backed workspace / local dependency 路径
   - 再回落到当前文件 `Analysis` 的 same-file query surface
5. broken-source / parse-error
   - 只开放明确实现的保守 fallback，例如 dependency / workspace import 的 hover、definition、typeDefinition、references、documentHighlight、completion、semantic tokens 和受限 rename
   - 需要完整语义的 formatting、documentSymbol、普通 same-file query 等继续返回空结果
6. response bridge
   - 把 compiler span、diagnostic、query result、completion item、semantic token 和 workspace edit 投影成 LSP response

当前桥接层职责明确在 `bridge.rs`：

- `Position <-> byte offset`
- `Span -> Range`
- 编译器 diagnostics -> LSP diagnostics
- compiler hover / navigation / references / completion / semantic tokens / rename -> LSP response

为什么这样设计：

- 编译器内部保持 byte-offset 和 span 语义
- LSP 协议细节都留在桥接层，不污染 analysis/query 代码

## 分层扩展规则

后续继续开发时，必须遵守下面这些规则，否则前面已经建立的层次很容易被绕坏。

### 规则 1：新语法先接 AST，再接 HIR 正规化

- 需要保留源码形态的，留在 AST
- 需要让后续语义层不再区分语法糖差异的，在 HIR lowering 里正规化
- 不要在 resolver/typeck 临时判断“这个 AST 其实是某种缩写”

### 规则 2：所有名字查找统一经过 `ql-resolve`

- 不要在 `ql-typeck`、`ql-lsp`、`ql-codegen-llvm` 里偷偷自己查名字
- 新的命名空间、新的 item kind、新的 import 规则，都应先扩展 resolution map

### 规则 3：类型检查优先新增受控信息，而不是扩大假阳性

- 不能稳定判断时，优先显式回退到 `unknown`
- 但一旦某个语义规则被认定为“绝对可靠”，就应给明确诊断并加回归

### 规则 4：ownership 规则必须基于 MIR facts 扩展

- 新的 move / borrow / drop / escape 规则，要加在 `ql-borrowck` 的状态机和事件模型上
- 不要回到 HIR 做一次“影子所有权分析”

### 规则 5：LLVM 支持面扩展不能反向污染前端

- 先让 MIR 能表达新语义
- 再决定 LLVM lowering 方案
- 不为了少写几行后端代码，把语言中层直接改成 LLVM 风格

### 规则 6：CLI 与 LSP 必须复用 `ql-analysis`

- 继续做 completion、cross-file rename，或继续把 hover / references / same-file rename 从 root-binding 扩到更深 member 语义时，应优先扩展 query surface
- 不要在 `ql-cli` 或 `ql-lsp` 里各自复制一份语义遍历

### 规则 7：测试要沿分层布局

- 语法/格式化问题优先锁在 parser/fmt crate tests
- 名称解析、类型检查、MIR、borrowck、codegen 各自先有 crate-local 回归
- 仓库根 `tests/` 主要保留黑盒 CLI、UI、codegen、LSP 和 FFI 回归

## 当前最适合作为下一步的扩展点

从算法和架构角度看，后续扩展最稳的路径是：

1. 在 `ql-resolve` 继续补 module/import/prelude 的严格规则
2. 在 `ql-typeck` 把 callable/member/index/泛型实参推断继续做实
3. 在 `ql-mir` 继续把 `match`、`for`、aggregate lowering 和 closure environment 做细
4. 在 `ql-borrowck` 基于现有数据流框架扩展 call contract、borrow kind、drop/escape 规则
5. 在 `ql-codegen-llvm` 基于 MIR 能力继续扩展 aggregate、cleanup、closure 和 ABI surface

也就是说，后续不应该再走“直接在 CLI 或后端补一个特例”的路线，而应顺着已经建立好的分层向下推进。
