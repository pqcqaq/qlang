# 工具链设计

## 目标

Qlang 不是“只有一个编译器二进制”的项目，而是一整套开发体验工程。工具链应围绕统一 CLI 和共享语义数据库展开。

如果要看当前 `ql build`、`ql check`、`qlsp` 背后分别如何编排 analysis、codegen、toolchain invocation 和协议桥接，继续看：

- [实现算法与分层边界](/architecture/implementation-algorithms)

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

### `ql build`

P4 的第一刀已经落地为“LLVM IR backend foundation”，当前 `ql build` 的职责是：

- 读取单个 `.ql` 文件
- 复用 `ql-analysis` 完成 parse / HIR / resolve / typeck / MIR
- 在无语义错误时调用 `ql-codegen-llvm`
- 输出文本 LLVM IR、对象文件、动态库、静态库和基础可执行文件

当前实现边界：

- `ql-cli` 只负责参数解析与诊断渲染
- `ql-driver` 负责 build orchestration
- `ql-codegen-llvm` 负责 MIR 子集 -> LLVM IR

当前可用命令形态：

- `ql build <file>`
- `ql build <file> --emit llvm-ir`
- `ql build <file> --emit obj`
- `ql build <file> --emit exe`
- `ql build <file> --emit dylib`
- `ql build <file> --emit staticlib`
- `ql build <file> --release`
- `ql build <file> -o <output>`
- `ql build <file> --emit dylib --header`
- `ql build <file> --emit staticlib --header-surface imports`
- `ql build <file> --emit dylib --header-output <output>`

当前默认输出路径：

- `target/ql/debug/<stem>.ll`
- `target/ql/release/<stem>.ll`
- `target/ql/debug/<stem>.obj` / `target/ql/debug/<stem>.o`
- `target/ql/release/<stem>.obj` / `target/ql/release/<stem>.o`
- `target/ql/debug/<stem>.exe` / `target/ql/debug/<stem>`
- `target/ql/release/<stem>.exe` / `target/ql/release/<stem>`
- `target/ql/debug/<stem>.dll` / `target/ql/debug/lib<stem>.so` / `target/ql/debug/lib<stem>.dylib`
- `target/ql/release/<stem>.dll` / `target/ql/release/lib<stem>.so` / `target/ql/release/lib<stem>.dylib`
- `target/ql/debug/<stem>.lib` / `target/ql/debug/lib<stem>.a`
- `target/ql/release/<stem>.lib` / `target/ql/release/lib<stem>.a`

当 `--emit` 是 `dylib` 或 `staticlib` 且启用了 build-side header 时：

- `--header` 生成默认 `exports` surface
- `--header-surface exports|imports|both` 会隐式启用 header sidecar
- `--header-output <output>` 也会隐式启用 header sidecar
- 未显式指定 header 输出路径时，header 会写到主 library artifact 同目录，但使用源码 stem 而不是 library 文件名：
  - `target/ql/debug/libffi_export.so` + `--header` -> `target/ql/debug/ffi_export.h`
  - `target/ql/debug/math.lib` + `--header-surface imports` -> `target/ql/debug/math.imports.h`

当前支持矩阵刻意收窄为：

- 顶层 free function
- `extern "c"` 顶层声明、extern block 声明与顶层函数定义
- `main` 入口
- 标量整数 / `Bool` / `Void`
- direct function call
- arithmetic / compare / branch / return
- `.ll` 文本产物始终可用
- `.obj` / `.o` / 基础 `.exe` 产物依赖 clang-style compiler
- `.dll` / `.so` / `.dylib` 产物依赖 clang-style compiler
- `.lib` / `.a` 产物依赖 clang-style compiler 与 archive tool
- codegen 会在 program mode 下把 Qlang 用户入口 lower 成内部符号，并额外生成宿主 `main` wrapper
- `dylib` 和 `staticlib` 都走 library mode，因此当前单文件库不要求顶层 `main`
- `dylib` 当前要求模块里至少存在一个 public 顶层 `extern "c"` 函数定义，避免生成没有明确导出面的共享库
- direct `extern "c"` 调用现在会在 program mode 和 library mode 下都 lower 成 LLVM `declare @symbol` + `call @symbol`
- 顶层 `extern "c"` 函数定义现在会 lower 成稳定 C 符号名，例如 `define i64 @q_add(...)`
- Windows 上 `dylib` 链接会为这些稳定导出符号显式追加 `/EXPORT:<symbol>`，确保 DLL 导出表和 Qlang 的 exported C surface 保持一致
- `dylib` / `staticlib` 现在还可以在构建成功后直接附带 C header sidecar，而不需要额外再跑一次 `ql ffi header`
- build-side header 会复用同一份 analysis 结果，而不是重新 parse / resolve / typeck
- build-side header 只允许出现在 `dylib` / `staticlib` 上；对 `llvm-ir` / `obj` / `exe` 会直接拒绝
- 如果显式 `--header-output` 与主 artifact 路径相同，driver 会在真正构建前直接报错，避免 header 覆盖库文件
- 如果 library 已成功产出但 sidecar header 生成失败，driver 会回收刚生成的主 artifact，避免留下“库成功、头文件失败”的半成功状态
- program mode 的入口 `main` 仍必须使用默认 Qlang ABI；如果需要导出稳定 C 符号，应定义独立的 `extern "c"` helper
- `extern` callable 现在有共享 callable identity，因此 extern block 调用也能稳定参与参数类型检查与代码生成
- first-class function value 不会再把后端打崩，而是返回结构化 unsupported diagnostics
- Windows 上如果使用 `QLANG_CLANG` 覆盖路径，建议指向 `clang.exe` 或 `.cmd` wrapper，而不是裸 `.ps1`
- 如果使用 `QLANG_AR` 覆盖路径，建议指向 `llvm-ar` / `ar` / `llvm-lib.exe` / `lib.exe` 或对应 `.cmd` wrapper
- 如果 `QLANG_AR` 指向的 wrapper 文件名本身看不出是 `ar` 风格还是 `lib` 风格，可以额外设置 `QLANG_AR_STYLE=ar|lib`

当前明确未完成：

- 独立 linker family discovery
- runtime startup object
- first-class function value lowering
- closure / struct / tuple / cleanup lowering
- 任意共享库 surface、exported ABI 的 linkage/visibility 控制与 richer ABI surface
- extern ABI 与 runtime glue 的其余部分

这不是功能缺失，而是为了先把 Phase 4 的后端边界做稳，而不是把系统 LLVM、链接器和运行时问题一口气缠死。

### `ql ffi`

P5 当前已经落地最小可用的 C header emit slice：

- `ql ffi header <file>`
- `ql ffi header <file> --surface exports`
- `ql ffi header <file> --surface imports`
- `ql ffi header <file> --surface both`
- `ql ffi header <file> -o <output>`
- `ql build <file> --emit dylib|staticlib --header`
- `ql build <file> --emit dylib|staticlib --header-surface exports|imports|both`
- `ql build <file> --emit dylib|staticlib --header-output <output>`

当前默认输出路径：

- `target/ql/ffi/<stem>.h`
- `target/ql/ffi/<stem>.imports.h`
- `target/ql/ffi/<stem>.ffi.h`

build-side sidecar 默认输出路径：

- `<library-dir>/<source-stem>.h`
- `<library-dir>/<source-stem>.imports.h`
- `<library-dir>/<source-stem>.ffi.h`

当前 `ql ffi header` 的职责是：

- 读取单个 `.ql` 文件
- 复用 `ql-analysis` 完成 parse / HIR / resolve / typeck
- 默认投影 public 顶层 `extern "c"` 函数定义作为 exported surface
- 在 `--surface imports` 下投影顶层 `extern "c"` 声明和 `extern "c"` block 成员声明
- 在 `--surface both` 下按源码顺序合并 import/export surface
- 将当前已支持的标量 / 指针类型投影到确定性的 C declaration
- 输出 include guard、`<stdbool.h>` / `<stdint.h>`、C++ `extern "C"` wrapper
- include guard 会按最终输出头文件名生成，避免 export/import/both 三份 header 互相冲突

而 `ql build` 上的 header sidecar 只是复用同一套投影逻辑，但把默认输出目录改成 library artifact 同目录，并挂到 build orchestration 上统一交付。

当前支持矩阵刻意收窄为：

- 顶层 `pub extern "c" fn ... { ... }`
- 顶层 `extern "c" fn ...`
- `extern "c" { fn ... }`
- `Bool` / `Void`
- `Int` / `UInt` / `I8` / `I16` / `I32` / `I64` / `ISize`
- `U8` / `U16` / `U32` / `U64` / `USize`
- `F32` / `F64`
- 原始指针和多级原始指针
- `exports` / `imports` / `both` 三种 surface 选择
- 按源码顺序稳定输出 declaration

当前明确未完成：

- struct / tuple / callable / function-pointer ABI
- layout 校验与 richer diagnostics
- exported symbol 的 visibility/linkage 控制
- bridge code generation

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
- `ql-analysis` 现在还额外暴露了 position-based query surface：
  - `symbol_at(offset)`
  - `hover_at(offset)`
  - `definition_at(offset)`
- 这组 API 当前已经能回答 item / local / param / generic / receiver `self` / named type root / pattern root / struct literal root 的基础语义查询
- import alias 与 builtin type 现在也能返回 hover 级信息，但因为它们没有本地源码定义点，所以不会伪造 definition span

这一层现在还有两个明确的架构保证：

- AST 会保留 declaration name、generic param、regular param、pattern field、struct literal field、named call arg、closure param 的精确 name span
- receiver param 现在也会保留精确 span，而不是退化成整个函数 span
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

截至 2026-03-26，P4 基线新增验证命令：

- `cargo test -p ql-codegen-llvm`
- `cargo test -p ql-driver`
- `cargo test -p ql-cli`
- `cargo test -p ql-cli --test codegen`
- 在 clang-style compiler 与 archiver 可用时：`cargo test -p ql-cli --test ffi`
- `cargo test -p ql-cli --test ffi_header`
- `cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_build.ql --emit llvm-ir`
- `cargo run -p ql-cli -- build fixtures/codegen/pass/extern_c_build.ql --emit llvm-ir`
- `cargo run -p ql-cli -- build tests/ffi/pass/extern_c_export.ql --emit staticlib --header`
- `cargo run -p ql-cli -- build tests/ffi/pass/extern_c_export.ql --emit dylib --header`
- `cargo run -p ql-cli -- build fixtures/codegen/pass/extern_c_library.ql --emit staticlib --header-surface imports`
- `cargo run -p ql-cli -- ffi header tests/ffi/pass/extern_c_export.ql`
- `cargo run -p ql-cli -- ffi header tests/ffi/header/extern_c_surface.ql --surface imports`
- 在 clang 可用或 mock toolchain 注入时：`cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_build.ql --emit obj`
- 在 clang 可用或 mock toolchain 注入时：`cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_build.ql --emit exe`
- 在 clang 与 archiver 可用或 mock toolchain 注入时：`cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_library.ql --emit staticlib`

当前新增的黑盒 codegen harness 位于：

- `crates/ql-cli/tests/codegen.rs`
- `tests/codegen/pass/`
- `tests/codegen/fail/`

它会直接驱动真实 `ql build`，锁定：

- LLVM IR 快照
- extern C direct-call LLVM IR 快照
- mock object / executable / static library 产物
- build-side export/import header sidecar 快照
- build 路径上的 unsupported diagnostics

当前还新增了第一版真实 FFI smoke harness：

- `crates/ql-cli/tests/ffi.rs`
- `tests/ffi/pass/`
- `tests/ffi/pass/*.header-surface`

静态库回归会在 clang-style compiler 和 archiver 可用时：

- 构建导出 `extern "c"` 符号的 Qlang `staticlib`
- 在同一次 `ql build --header-output` 里生成对应的 C 头文件
- 用包含该头文件的真实 C harness 链接该库
- 运行宿主可执行文件确认导出符号可被调用
- imported-host 夹具还会通过 `both` surface header 同时拿到 imported/exported 声明，并验证 Qlang 导出函数体内的 imported C 调用能真实命中宿主实现
- 当前 imported-host staticlib 已覆盖：
  - extern block declaration
  - top-level extern declaration

共享库回归会在 clang-style compiler 可用时：

- 构建导出 `extern "c"` 符号的 Qlang `dylib`
- 在同一次 `ql build --header-output` 里生成对应的 C 头文件
- 用真实 C loader harness 编译宿主可执行文件
- 运行宿主可执行文件，并在进程内通过 `LoadLibraryA` / `dlopen` 解析并调用导出符号

当前 `.header-surface` fixture 元数据规则：

- 不存在时默认 `exports`
- 存在时内容必须是 `exports` / `imports` / `both`
- 这让 FFI harness 可以在不硬编码 case 名称的前提下，为 imported-host 夹具切换到 combined surface

### `qlsp`

LSP 服务端，复用编译器 HIR 与查询系统。长期目标支持：

- go to definition
- find references
- hover
- completion
- semantic tokens
- rename
- code action
- diagnostics

当前已经有的地基：

- `ql-analysis` 已提供最小可用的 hover / definition / references 查询面
- `ql-analysis` 现在还提供基于稳定 symbol identity 的 same-file references 查询面
- `qlsp` 的第一版已经落地在 `crates/ql-lsp`
- 当前通过 stdio 运行，复用 `ql-analysis`
- 当前已实现：
  - `textDocument/didOpen`
  - `textDocument/didChange`（full sync）
  - `textDocument/didClose`
  - `textDocument/hover`
  - `textDocument/definition`
  - `textDocument/references`（当前为 same-file）
  - `textDocument/publishDiagnostics`
- LSP 协议桥接已单独分层：
  - 位置 `Position <-> byte offset` 换算
  - `Span -> Range`
  - compiler diagnostics -> LSP diagnostics
  - analysis hover / definition / references -> LSP response
- 这意味着 `qlsp` 的第一版不需要重新发明一套“源码位置 -> 语义实体”的逻辑
- 但这还不是完整 LSP 语义层，member / method / variant / module-path 查询以及 completion / rename 仍需要后续继续补齐

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
