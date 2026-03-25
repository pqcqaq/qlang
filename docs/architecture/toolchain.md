# 工具链设计

## 目标

Qlang 不是“只有一个编译器二进制”的项目，而是一整套开发体验工程。工具链应围绕统一 CLI 和共享语义数据库展开。

## 统一入口

建议统一入口命令为 `ql`，由它驱动各子工具：

- `ql new`
- `ql init`
- `ql build`
- `ql run`
- `ql test`
- `ql check`
- `ql fmt`
- `ql doc`
- `ql bench`
- `ql clean`
- `ql ffi`

这样可以减少工具数量膨胀，让使用者把它当成一个系统，而不是一堆松散命令。

## 子工具

### `ql check`

当前 `ql check` 已经进入 Phase 2 的第一层语义阶段，不再只是 parser 验证。它现在负责：

- 读取单个 `.ql` 文件并执行 lexer / parser 验证
- 将 AST lowering 到 HIR
- 在 HIR 上执行名称解析与作用域图构建
- 运行第一批 semantic checks
- 对目录执行批量检查
- 统一输出带 span 的 parser 与 semantic diagnostics

当前已落地的 semantic checks 包括：

- top-level duplicate definition
- duplicate generic parameter
- duplicate function parameter
- duplicate enum variant
- duplicate method in trait / impl / extend block
- duplicate binding inside a pattern
- duplicate field in `struct` declaration
- duplicate field in `struct` pattern
- duplicate field in `struct` literal
- duplicate named call argument
- positional argument after named arguments
- invalid use of `self` outside a method receiver scope
- return-value type mismatches
- non-`Bool` conditions in `if` / `while` / match guards
- callable arity / argument type mismatches
- tuple destructuring arity mismatches
- struct literal unknown-field / missing-field / field-type mismatches
- equality operand compatibility mismatches
- unknown struct member access
- pattern root / literal type mismatches in destructuring and `match`
- calling non-callable values

当前 `ql check` 的内部边界也进一步明确了：

- `ql-analysis` 负责统一 parse / HIR / resolve / typeck 分析入口
- `ql-cli` 不再自己拼装语义流水线，而是消费 `ql-analysis`
- 这让 CLI、测试和未来 LSP 可以共享同一份分析快照，而不是各自拷贝一份流程

这一层现在还有两个明确的架构保证：

- AST 会保留 declaration name、generic param、regular param、pattern field、struct literal field、named call arg、closure param 的精确 name span
- HIR 会提前正规化 shorthand sugar，例如 struct pattern / struct literal 的缩写字段，后续 name resolution 和 type checking 不需要再区分“缩写”和“完整写法”

现在又额外补上了一条关键边界：

- `ql-resolve` 专门承接 lexical scope graph 与 best-effort name resolution，避免把作用域查找逻辑散落进 `ql-typeck`
- 当前 resolution 故意只做保守诊断，不抢跑 unresolved global / unresolved type 的全面报错，这样可以先把语义架构打稳，再补 import / module / prelude 规则
- `ql-typeck` 现在已经不只是 duplicate checker，而是开始承接真正的 first-pass typing；但它依然刻意保守，未知成员访问、未知索引结果、未建模模块语义仍然会回退成 `unknown`，避免过早把当前样例集打成错误
- top-level `const` / `static` 的声明类型现在会进入后续表达式 typing，因此函数值常量的调用也能拿到参数类型诊断

当前目录扫描策略也已经收紧，避免把仓库噪音当成真实源码：

- 会跳过 `target`、`node_modules`、`dist`、`build`、`coverage`
- 会跳过隐藏目录
- 会跳过仓库里的 `fixtures/` 和临时测试目录，例如 `ramdon_tests/`
- 如果用户显式传入某个 fixture 文件或 fail fixture 目录，仍然允许直接检查

这个策略不是“保守”，而是为了避免 `ql check .` 在仓库根目录误扫失败夹具、构建产物和杂项测试目录，污染真实前端回归结果。

当前仍需明确的一条状态边界：

- 默认参数仍是设计稿能力，不属于当前已经实现并验证的 `ql check` 语义范围

当前测试基建也已经进入下一层：

- `crates/ql-typeck/tests/` 继续承载 crate-local duplicates / typing / rendering 回归
- `crates/ql-analysis` 现在承载统一分析边界和查询 API 的单元测试
- 仓库根 `tests/ui/` 现在开始承载黑盒 UI diagnostics fixture
- `crates/ql-cli/tests/ui.rs` 负责驱动真实 `ql` 二进制，对 parser / resolve / semantic / type diagnostics 的最终 stderr 做 snapshot 比对

### `qlsp`

LSP 服务端，复用编译器 HIR 与查询系统，支持：

- go to definition
- find references
- hover
- completion
- semantic tokens
- rename
- code action
- diagnostics

### `qfmt`

格式化器必须尽早做，并尽量做到：

- 输出稳定
- 风格单一
- 对 AST 变化敏感度低

现代语言生态一旦放任格式风格分裂，后面会一直付成本。

当前阶段 `qfmt` 已覆盖的语法切片包括：

- 基础声明：`const`、`static`、`type`、`opaque type`
- 可调用声明：`fn`、`trait` method、`impl`、`extend`、`extern`
- 类型表达式：named type、tuple、callable type、声明泛型、`where`
- 表达式：调用、成员访问、结构体字面量、闭包、`unsafe`、`if`、`match`
- 控制流：`while`、`loop`、`for`、`for await`
- 模式：tuple、path、tuple-struct、struct、字面量、`_`

Phase 1 结束后，`qfmt` 的下一步重点不是增加风格选项，而是跟随后续 HIR / diagnostics 演进，保持语法扩展时的稳定输出与可维护实现。

### 当前已验证命令

截至 2026-03-25，当前前端基线已经反复验证过以下命令：

- `cargo fmt`
- `cargo test`
- `cargo run -p ql-cli -- check fixtures/parser/pass/basic.ql`
- `cargo run -p ql-cli -- check fixtures/parser/pass/control_flow.ql`
- `cargo run -p ql-cli -- check fixtures/parser/pass/phase1_declarations.ql`
- `cargo run -p ql-cli -- check <含重复定义/重复绑定的源码>`
- `cargo run -p ql-cli -- check target/review/if_empty_block.ql`
- `cargo run -p ql-cli -- check target/review/while_empty_block.ql`
- `cargo run -p ql-cli -- check target/review/match_empty_arms.ql`
- `cargo run -p ql-cli -- check target/review/one_tuple_type.ql`
- `cargo run -p ql-cli -- fmt fixtures/parser/pass/phase1_declarations.ql`
- `cargo run -p ql-cli -- fmt target/review/one_tuple_type.ql`

### `qdoc`

文档生成器负责：

- 从公共 API 提取签名
- 展示效果、错误、trait 约束和 FFI 标记
- 输出静态站点内容

### 测试工具

`ql test` 不只是运行单元测试，还应逐步支持：

- UI tests
- doc tests
- integration tests
- benchmark harness

## 包与工作区

Qlang 应提供统一 manifest，例如 `qlang.toml`，支持：

- package metadata
- dependencies
- features
- build profiles
- ffi libraries
- workspace members

工作区模型必须在早期就纳入，因为编译器、标准库、示例、FFI 包和工具链本身都会依赖它。

借鉴 TypeScript 的 project references，Qlang 还应支持显式项目引用图：

- 工作区成员能声明上游接口依赖
- 增量构建优先基于接口产物判断失效范围
- LSP 可直接消费依赖包的公共 API 元数据

## 接口产物

Qlang 建议为每个包输出公共接口产物，例如 `.qi` 文件：

- 包含公共类型、函数签名、trait、effect、布局约束等元数据
- 供下游类型检查和 LSP 使用
- 避免每次都重新解析全部依赖源码

这相当于把 TypeScript 的 declaration emit 和 project references 经验，转化为适合编译型系统语言的工程能力。

## 编辑器体验

LSP 的目标不是“有就行”，而是从第一阶段就支撑日常开发：

- 补全要基于真实类型，不是纯文本猜测
- 报错位置要稳定
- rename 要有跨文件可信度
- code action 要能生成 `match` 分支、导入、trait stub

Qlang 的目标不是“有一个能用的 LSP”，而是像 TypeScript 一样，把语言服务当成语言本体的一部分来设计。

## 发布与生态

P1 之后可以逐步加入：

- package registry
- lockfile
- binary caching
- doc hosting
- template generator

但这些必须建立在前面的语义和构建基础上，而不是为了“看起来像成熟生态”提前堆功能。
