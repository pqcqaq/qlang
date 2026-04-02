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

P4/P5 地基已经落地，且当前正在保守扩展 Phase 7 async library/program build 子集；当前 `ql build` 的职责是：

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
- fixed-array iterable 的 sync `for`
- 普通表达式与 `if` / `while` 条件里可直接 materialize same-file foldable `const` / `static` item value 及其 same-file `use ... as ...` alias；当前已支持的 tuple / fixed-array / plain-struct literal 子集复用同一条 const-evaluation lowering
- 普通表达式、`if` / `while` 条件里的 `Bool` 短路 `&&` / `||`
- 最小 literal `match` lowering：`Bool` scrutinee 仅支持 unguarded `true` / `false` / same-file foldable `const` / `static`-backed bare path pattern（含 same-file local import alias）+ `_` 或单名 binding catch-all arm，并额外接受 literal `if true` / `if false` guard、same-file foldable `const` / `static`-backed `Bool` guard 与其 same-file `use ... as ...` 别名；这条 `Bool` const folding 现也覆盖由当前支持的 `!`、`&&`、`||`、`==`、`!=` 与整数比较子集构成的 computed same-file foldable `const` / `static` `Bool` expression；包裹当前 bool guard 子集的一层 unary `!`、由当前 bool guard 子集继续组合出的 runtime `&&` / `||`、当前 arm 单名 binding catch-all 变量作为 direct scalar operand 参与 guard，也可作为 fixed-array 只读投影里的 dynamic index operand 参与 guard；fixed-array 只读 guard projection 的 index 现也接受当前 runtime `Int` scalar 子集，例如 `values[index + 1]` 与 `values[current + 1]`，但 tuple index 仍要求 foldable constant；当前简单 scalar comparison guard 子集（`Bool` `==` / `!=`，以及由 integer literal、unary-negated supported `Int` operand、same-file foldable `const` / `static`-backed `Int` 与其 same-file `use ... as ...` 别名、由当前支持的 `Int` operand 继续递归组成的最小整数算术表达式（`+` / `-` / `*` / `/` / `%`）、以 local / parameter / `self` 为根的 read-only scalar projection operand、以及能经由 struct field / tuple literal-index / fixed-array index 折叠成 scalar 的 same-file foldable `const` / `static`-backed aggregate projection 及其 same-file `use ... as ...` 别名组成的 `Int` `==` / `!=` / `>` / `>=` / `<` / `<=`；其中 tuple / fixed-array index operand 现也接受同一条可折叠的 same-file const/static-backed `Int` arithmetic 子集），以及 direct bool-valued guard 子集（same-scope `Bool` local / parameter、以 local / parameter / `self` 为根的 read-only bool scalar projection、以及能折叠成 `Bool` 的 same-file foldable `const` / `static`-backed aggregate projection 及其 same-file `use ... as ...` 别名；要求后续 arm 仍提供 guaranteed fallback coverage）；`Int` scrutinee 仅支持 unguarded integer literal / unary-negated integer literal / same-file foldable `const` / `static`-backed bare path pattern（含 same-file local import alias）+ `_` 或单名 binding catch-all arm，并额外接受同一组 literal / const/static-backed / scalar-comparison guard 子集，以及 integer-literal arm 或 guarded catch-all arm 上的同一组 direct bool-valued guard 子集（要求后续存在 unguarded catch-all fallback）；不能折叠成 `Bool`/`Int` 的 bare path pattern 仍显式拒绝
- 保守的 async `staticlib` 子集：async library body、scalar/tuple/array/struct/void `await`、task-handle-aware `spawn`、以及 fixed-array iterable 的 `for await`
- 最小 async `dylib` 子集：在仍通过同步 `extern "c"` 顶层导出暴露公开 ABI 时，内部 async helper / `await` / 已支持的 task-handle lowering 也可进入 library build，并对 fixed-array iterable 打开同一条 `for await` lowering
- 最小 async executable 子集：`BuildEmit::Executable` 下的 `async fn main`、已接入的 task-handle / aggregate payload lowering，以及 fixed-array iterable 的 `for await`
- projected task-handle operand：tuple index / fixed-array literal index / struct-field 只读投影，也包括它们的递归嵌套组合（例如 `await pair[0]`、`await tasks[0]`、`spawn pair.task`、`await pending[0].task`）
- dynamic fixed-array `Task[...]` 子集：generic dynamic path 继续保持 sibling-safe consume/spawn 与 maybe-overlap write/reinit，而 same immutable stable source path 会获得 precise consume/reinit；这条稳定化路径现在也已覆盖 same-file `const` / `static` item 及其 same-file `use ... as ...` alias，因此 `tasks[index]`、`tasks[slot.value]`、`tasks[INDEX]`、`tasks[STATIC_INDEX]`、`tasks[INDEX_ALIAS.value]` 都会尽量回收到同一条 stable 或 literal/projection path，而不是无差别退化成 generic dynamic overlap
- arithmetic / compare / `Bool` unary `!` / short-circuit `&&` / `||` / branch / return
- `.ll` 文本产物始终可用
- `.obj` / `.o` / 基础 `.exe` 产物依赖 clang-style compiler
- `.dll` / `.so` / `.dylib` 产物依赖 clang-style compiler
- `.lib` / `.a` 产物依赖 clang-style compiler 与 archive tool
- codegen 会在 program mode 下把 Qlang 用户入口 lower 成内部符号，并额外生成宿主 `main` wrapper
- `dylib` 和 `staticlib` 都走 library mode，因此当前单文件库不要求顶层 `main`
- async public build 当前已开放两类受控子集：`staticlib` 与 `dylib` 都支持已接入 backend 的 async library body，并为 fixed-array iterable 打开 `for await` lowering；`BuildEmit::LlvmIr` / `BuildEmit::Object` / `BuildEmit::Executable` 也已开放 `async fn main` 的最小程序入口生命周期与 fixed-array `for await` 子集。其中 `dylib` 仍只开放不暴露 async ABI 的最小 library-style 子集；更广义的 async program/bootstrap surface，以及非 fixed-array iterable 的 `for await` 仍保持保守拒绝
- 当前最小 async executable program subset 还额外由 106 个 committed 样例锁定，见 `ramdon_tests/async_program_surface_examples/` 与 `crates/ql-cli/tests/executable_examples.rs`
- sync executable examples 现也额外收录 `ramdon_tests/executable_examples/04_sync_static_item_values.ql`，把 same-file foldable `const` / `static` item value 及其 same-file `use ... as ...` alias 的普通表达式 / bool 条件 lowering 锁进真实 `--emit exe` 合同
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
- Windows 下如果 PATH 中没有 clang / archiver，`ql-driver` 现在还会 best-effort 探测常见 LLVM 安装目录，包括 Scoop 的 `llvm/current/bin`、`%LOCALAPPDATA%\\Programs\\LLVM\\bin`、`%ProgramFiles%\\LLVM\\bin` 与 `%ProgramFiles(x86)%\\LLVM\\bin`
- 当这些位置也没有找到工具时，`ToolchainError::NotFound` 会把候选路径直接放进 hint，减少“只知道要配环境变量，但不知道应该指向哪里”的恢复成本

当前明确未完成：

- 独立 linker family discovery
- runtime startup object
- first-class function value lowering
- closure lowering
- 更广义的 projection assignment（当前已开放 tuple-index / struct-field / fixed-array literal-index write、非 `Task[...]` 元素的 dynamic array assignment，以及 `Task[...]` 动态数组的 generic maybe-overlap write/reinit + same immutable stable index path precise consume/reinit 子集）、更广义 `for` / `for await` lowering（当前已开放 sync fixed-array `for`，以及 library-mode 与 `BuildEmit::Executable` `async fn main` 子集下的 fixed-array iterable `for await`）、更广义 `match` lowering（当前只开放 `Bool` scrutinee + unguarded `true` / `false` / same-file const/static/alias bare path + `_` 或单名 binding catch-all arm，以及 `Int` scrutinee + unguarded integer literal / same-file const/static/alias bare path + `_` 或单名 binding catch-all arm 子集；两者额外接受 literal `if true` / `if false` guard、same-file foldable `const` / `static`-backed `Bool` guard 与其 same-file `use ... as ...` 别名、包裹当前 bool guard 子集的一层 unary `!`、由当前 bool guard 子集继续组合出的 runtime `&&` / `||`、当前 arm 单名 binding catch-all 变量作为 direct scalar operand 参与 guard，以及当前 `Bool ==/!=` 与 `Int ==/!=/>/>=/</<=` 的简单 scalar comparison guard 子集；其中 `Int` operand 当前开放 integer literal、same-file foldable `const` / `static`-backed `Int` 与其 same-file `use ... as ...` 别名、由当前支持的 `Int` operand 继续递归组成的最小整数算术表达式（`+` / `-` / `*` / `/` / `%`）、以 local / parameter / `self` 为根的 read-only struct field / tuple literal-index / fixed-array index projection，以及能经由这些投影折叠成 scalar 的 same-file foldable `const` / `static`-backed aggregate root 及其 same-file `use ... as ...` alias，而 direct bool guard operand 当前开放 same-scope `Bool` local / parameter、以 local / parameter / `self` 为根的 read-only bool scalar projection，以及能折叠成 `Bool` 的 same-file foldable `const` / `static`-backed aggregate root 及其 same-file `use ... as ...` alias；`Bool` scrutinee 子集额外接受这组 direct bool guard + guaranteed fallback coverage 子集，而 `Int` scrutinee 子集额外接受 integer-literal arm / guarded catch-all arm 上的同一组 direct bool guard + later unguarded catch-all fallback 子集；不能折叠成 `Bool` / `Int` 的 bare path pattern 仍显式拒绝）与更广义 aggregate / cleanup lowering
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
- source-level fixed array type expr `[T; N]` lowering and compatibility checks
- expected fixed-array context guided array literal item diagnostics
- equality operand compatibility mismatches
- comparison operand compatible-numeric mismatches
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
  - `references_at(offset)`
  - `completions_at(offset)`
  - `semantic_tokens()`
  - `prepare_rename_at(offset)`
- `rename_at(offset, new_name)`
- 这组 API 当前已经能回答 item / local / param / generic / receiver `self` / named type root / pattern root / struct literal root 的基础语义查询，并导出同文件 completion 与 semantic tokens 所需的稳定语义数据
- import alias 现在会作为 source-backed binding 进入同一份 query index，因此可以返回真实 definition span，并参与同文件 hover / references / rename / semantic tokens；builtin type 则继续作为非 source-backed stable symbol 参与 hover / references / semantic tokens，但不提供 definition / rename
- 当 import alias 的原始路径恰好是单段、且命中同文件 root struct item 时，field label query / rename 与 struct literal 字段检查也会继续沿用原 struct item / field symbol；struct 或 enum pattern root 也会沿同一条 canonicalization 回到本地 item

这一层现在还有两个明确的架构保证：

- AST 会保留 declaration name、generic param、regular param、pattern field、struct literal field、named call arg、closure param 的精确 name span
- receiver param 现在也会保留精确 span，而不是退化成整个函数 span
- HIR 会提前正规化 shorthand sugar，例如 struct pattern / struct literal 的缩写字段，后续 name resolution 和 type checking 不需要再区分“缩写”和“完整写法”

现在又额外补上了一条关键边界：

- `ql-resolve` 专门承接 lexical scope graph 与 best-effort name resolution，避免把作用域查找逻辑散落进 `ql-typeck`
- 当前 resolution 故意只做保守诊断：先落地 `self` misuse 与 bare single-segment value/type root 的 unresolved，不抢跑 multi-segment unresolved global / unresolved type 的全面报错，这样可以先把语义架构打稳，再补 import / module / prelude 规则
- `ql-typeck` 现在已经不只是 duplicate checker，而是开始承接真正的 first-pass typing；但它依然刻意保守，未知成员访问、通用索引协议结果、未建模模块语义仍然会回退成 `unknown`，避免过早把当前样例集打成错误；当前只对 source-level fixed array、inferred array 与支持 lexer-style integer literal 的 constant tuple index 开了一层稳定 typing
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

截至 2026-03-28，当前工具链与文档基线常用的验证命令包括：

- `cargo test`
- `cargo test -p ql-cli --test codegen`
- 在 clang-style compiler 与 archiver 可用时：`cargo test -p ql-cli --test ffi`
- `cargo test -p ql-cli --test ffi_header`
- `cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_build.ql --emit llvm-ir`
- `cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_library.ql --emit staticlib`
- `cargo run -p ql-cli -- build fixtures/codegen/pass/extern_c_library.ql --emit staticlib --header-surface imports`
- `cargo run -p ql-cli -- ffi header tests/ffi/pass/extern_c_export.ql`
- `cargo run -p ql-cli -- ffi header tests/ffi/header/extern_c_surface.ql --surface imports`
- 在 clang 可用或 mock toolchain 注入时：`cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_build.ql --emit obj`
- 在 clang 可用或 mock toolchain 注入时：`cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_build.ql --emit exe`
- 在 clang 与 archiver 可用或 mock toolchain 注入时：`cargo run -p ql-cli -- build tests/ffi/pass/extern_c_export.ql --emit staticlib --header`
- `npm run build` in `docs/`

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
- `crates/ql-cli/tests/ffi.rs` 现在直接复用 `ql-driver` 的 toolchain discover 结果，因此这些回归对 clang / archiver 的可用性判断与真实 `ql build` 路径保持一致
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
- struct field 与唯一 method candidate 的 member token 现在也能直接复用同一套查询面
- explicit struct literal / struct pattern field label 现在也能直接复用同一套 field 查询面
- enum variant declaration / pattern use / constructor use 现在也能直接复用同一套查询面
- `ql-analysis` 现在也能基于同一份 occurrence 索引导出 same-file semantic tokens
- `qlsp` 的第一版已经落地在 `crates/ql-lsp`
- 当前通过 stdio 运行，复用 `ql-analysis`
- 当前已实现：
  - `textDocument/didOpen`
  - `textDocument/didChange`（full sync）
  - `textDocument/didClose`
  - `textDocument/hover`
  - `textDocument/definition`
  - `textDocument/references`（当前为 same-file）
  - `textDocument/completion`（当前为 same-file lexical scope + parsed member token + parsed enum variant path，且支持 local import alias -> local enum item 的 variant follow-through，并保留 escaped identifier 的合法 insert text）
  - `textDocument/semanticTokens/full`（当前为 same-file source-backed symbol）
  - `textDocument/prepareRename`
  - `textDocument/rename`（当前为 same-file）
  - `textDocument/publishDiagnostics`
- LSP 协议桥接已单独分层：
  - 位置 `Position <-> byte offset` 换算
  - `Span -> Range`
  - compiler diagnostics -> LSP diagnostics
  - analysis hover / definition / references / completion / semantic tokens / rename -> LSP response
- 这意味着 `qlsp` 的第一版不需要重新发明一套“源码位置 -> 语义实体”的逻辑
- 当前 same-file rename 也明确只开放保守符号集：function / const / static / struct / enum / variant / trait / type alias / import / field / method（仅唯一 candidate）/ local / parameter / generic
- 这组 type-namespace item rename 现在也已经由 analysis / LSP 回归明确锁住：`type`、`opaque type`、`struct`、`enum`、`trait` 不依赖协议层特判，而是继续走统一 `QueryIndex`
- 这组 root value-item rename 现在也已经由 analysis / LSP 回归明确锁住：`function`、`const`、`static` 不依赖协议层特判，而是继续走统一 `QueryIndex`
- 同一组 type-namespace item 现在也已经有 references / semantic-token parity 回归，确保 `type`、`opaque type`、`struct`、`enum`、`trait` 的 query、LSP references 与语义高亮继续站在同一份 item occurrence 上
- 这组 item 现在还额外有 hover / definition parity 回归，确保 `type`、`opaque type`、`struct`、`enum`、`trait` 的导航与悬浮信息继续落回同一份 definition span，而不是在 bridge 层退化成字符串级猜测
- same-file type-namespace item surface 现在也已经有显式聚合回归：`type`、`opaque type`、`struct`、`enum`、`trait` 会继续共享同一组 item truth surface，因此 hover / definition / references / semantic tokens 不需要分别靠零散的单类回归来兜底
- global value item 现在也已经有 query parity 回归：`const`、`static` 的 item definition 与 value-use 会继续共享同一份 `QueryIndex` truth surface，因此 hover / definition / references / semantic tokens 不需要在 LSP 层额外补特判
- `extern` callable surface 现在也已经有 same-file parity 回归：无论是 `extern` block 成员、顶层 `extern "c"` 声明，还是带 body 的顶层 `extern "c"` 函数定义，定义点与 call site 都会继续共享同一份 `Function` truth surface，因此 hover / definition / references / rename / semantic tokens 不需要在 LSP 层额外做 extern 特判
- extern callable 的 value completion 现在也已经有显式 parity 回归：analysis 会继续把 `extern` block 成员、顶层 `extern "c"` 声明和顶层 `extern "c"` 函数定义作为 `function` 候选产出，LSP bridge 会继续把它们投影成 `FUNCTION` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- ordinary free function 现在也已经有 same-file query parity 回归：direct call site 会继续共享同一份 `Function` truth surface，因此 hover / definition / references 不需要在 LSP 层额外补自由函数特判
- ordinary free function 现在也已经有 same-file semantic-token parity 回归：declaration 与 direct call site 会继续共享同一份 `Function` truth surface，因此 semantic tokens 不需要在 LSP 层额外补自由函数特判
- same-file callable surface 现在也已经有显式聚合回归：`extern` block callable、顶层 `extern "c"` 声明、顶层 `extern "c"` 定义与 ordinary free function 会继续共享同一组 callable truth surface，因此 hover / definition / references / semantic tokens 不需要分别靠零散的单类回归来兜底
- plain import alias symbol 现在也已经有 same-file parity 回归：`import` binding 会继续作为 source-backed symbol 共享同一份 truth surface，因此 hover / definition / references / semantic tokens 不需要在 analysis 与 LSP 两层分别做例外处理
- plain import alias 的 type-context completion 现在也已经有显式 parity 回归：analysis 会继续产出 `import` 候选，LSP bridge 会继续把它投影为 `MODULE` completion item，并沿用同一份 insert-text / text-edit 语义
- free function 的 lexical value completion 现在也已经有显式 parity 回归：analysis 会继续产出 `function` 候选，LSP bridge 会继续把它投影为 `FUNCTION` completion item，并沿用同一份 insert-text / text-edit 语义
- plain import alias 的 lexical value completion 现在也已经有显式 parity 回归：analysis 会继续产出 source-backed `import` 候选，LSP bridge 会继续把它投影为 `MODULE` completion item，并沿用同一份 insert-text / text-edit 语义
- builtin type 与 local struct item 的 type-context completion 现在也已经有显式 parity 回归：analysis 会继续产出这两类 type 候选，LSP bridge 会继续把它们投影为 `CLASS` / `STRUCT` completion item，并沿用同一份 insert-text / text-edit 语义
- same-file type alias 的 type-context completion 现在也已经有显式 parity 回归：analysis 会继续产出 `type alias` 候选，LSP bridge 会继续把它投影为 `CLASS` completion item，并沿用同一份 insert-text / text-edit 语义
- same-file `opaque type` 的 type-context completion 现在也已经有显式 parity 回归：analysis 会继续产出 `TypeAlias`-backed opaque alias 候选，LSP bridge 会继续把它投影为 `CLASS` completion item，并沿用 `opaque type ...` detail 与同一份 insert-text / text-edit 语义
- same-file generic 的 type-context completion 现在也已经有显式 parity 回归：analysis 会继续产出 `generic` 候选，LSP bridge 会继续把它投影为 `TYPE_PARAMETER` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- same-file enum 的 type-context completion 现在也已经有显式 parity 回归：analysis 会继续产出 `enum` 候选，LSP bridge 会继续把它投影为 `ENUM` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- same-file trait 的 type-context completion 现在也已经有显式 parity 回归：analysis 会继续产出 `trait` 候选，LSP bridge 会继续把它投影为 `INTERFACE` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- stable receiver field completion 现在也已经有显式 parity 回归：analysis 会继续产出 `field` 候选，LSP bridge 会继续把它投影为 `FIELD` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- stable receiver unique method completion 现在也已经有显式 parity 回归：analysis 会继续产出唯一 `method` 候选，LSP bridge 会继续把它投影为 `FUNCTION` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- same-file const / static 的 value completion 现在也已经有显式 parity 回归：analysis 会继续产出 `const` / `static` 候选，LSP bridge 会继续把它们投影为 `CONSTANT` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- same-file local 的 value completion 现在也已经有显式 parity 回归：analysis 会继续产出 `local` 候选，LSP bridge 会继续把它投影为 `VARIABLE` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- same-file parameter 的 value completion 现在也已经有显式 parity 回归：analysis 会继续产出 `parameter` 候选，LSP bridge 会继续把它投影为 `VARIABLE` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- same-file lexical value candidate-list parity 现在也已经有显式回归：analysis / LSP 会继续让 import / const / static / extern callable / free function / local / parameter 这些 already-supported value surface 共享同一份有序候选列表、detail 渲染与 replacement text-edit 投影，而不是让这组 editor-facing 契约分散在单类候选测试里
- same-file enum variant completion 现在也已经有显式 parity 回归：analysis 会继续产出 parsed enum path 上的 `variant` 候选，LSP bridge 会继续把它投影为 `ENUM_MEMBER` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- same-file import alias variant completion 现在也已经有显式 parity 回归：analysis 会继续让指向同文件根 enum item 的 local import alias path 产出 `variant` 候选，LSP bridge 会继续把它投影为 `ENUM_MEMBER` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- same-file import alias struct-variant completion 现在也已经有显式 parity 回归：analysis 会继续让指向同文件根 enum item 的 local import alias struct-literal path 产出 struct-style `variant` 候选，LSP bridge 会继续把它投影为 `ENUM_MEMBER` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- remaining same-file variant-path completion contexts 现在也已经有显式 parity 回归：analysis 会继续产出 direct struct-literal path 以及 direct/local-import-alias pattern path 上既有的 `variant` 候选，LSP bridge 会继续把它们投影为 `ENUM_MEMBER` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- same-file variant-path candidate-list parity 现在也已经有显式回归：analysis / LSP 会继续让 enum-root / struct-literal / pattern path 及其 same-file import-alias 镜像上下文共享同一份有序 `variant` 候选列表、detail 渲染与 replacement text-edit 投影，而不是让这组 editor-facing 契约停留在单个候选映射测试
- deeper variant-like member chain 现在也明确保持关闭：只有 root enum item 或 same-file import alias 的第一段 variant tail 还能复用 enum variant truth surface，`Command.Retry.more` / `Cmd.Retry.more` 这类更深 member chain 不会再伪造 hover / definition / references 或 `ENUM_MEMBER` completion
- deeper struct-literal / pattern variant-like path 现在也明确保持关闭：只有严格两段 `Root.Variant` path 才继续复用 enum variant truth surface，`Command.Scope.Config { ... }` / `Cmd.Scope.Retry(...)` 这类更深 path 不会再伪造 hover / definition / references / rename / semantic tokens 或 `ENUM_MEMBER` completion
- deeper struct-like path 的 field truth 现在也明确保持关闭：只有严格 root struct path 才继续复用 field query / rename / semantic-token surface，`Point.Scope.Config { x: ... }` / `P.Scope.Config { x: ... }` 这类更深 path 不会再伪造字段标签的 hover / definition / references / rename / semantic tokens
- deeper struct-like shorthand token 现在也有显式 parity 回归：当 `Point.Scope.Config { x }` / `Point.Scope.Config { source }` 这类路径仍处于 field semantics 关闭状态时，analysis 会继续把 shorthand token 保留在 local / binding / import lexical surface，LSP bridge 也会继续按该 lexical symbol 提供 hover / definition / references / semantic tokens / rename，并保持 raw binding edit 而不是伪造 `label: new_name` 扩写
- same-file completion filtering parity 现在也已经有显式回归：analysis 会继续按 lexical scope visibility/shadowing 与 impl-preferred member filtering 产出候选，LSP bridge 会继续原样投影这些结果，而不会在协议层额外扩张或放宽歧义 surface；其中 lexical value visibility 的聚合回归现在也已经显式覆盖 import / function / local 的 detail 与 text-edit 投影，而 impl-preferred member 聚合回归现在也已经显式覆盖 surviving candidate count 以及稳定 detail / text-edit 投影
- same-file completion candidate-list parity 现在也已经有显式回归：analysis 会继续按 type-context 与 stable-member 的完整候选列表产出结果，LSP bridge 会继续原样投影这些列表，而不会在协议层悄悄改变排序、命名空间边界或完整成员集合；其中 type-context 总表现在已经显式覆盖 builtin / import / struct / `type` / `opaque type` / `enum` / `trait` / generic，而 stable-member 总表也已经显式覆盖 method / field 的 detail 与 text-edit 投影
- shorthand struct field token query parity 现在也已经有显式回归：analysis 会继续把 `Point { x }` 这种 shorthand token 视为 local/binding surface，LSP bridge 会继续按这个结果提供 hover / definition，而不会在协议层把 shorthand token 误投影成 struct field
- direct same-file variant / explicit field-label query parity 现在也已经有显式回归：analysis 会继续把 direct enum variant token 与 direct explicit struct field label 的 definition / references 原样投影给 LSP，而不是只在 import-alias follow-through 路径上有端到端覆盖
- direct same-file variant / explicit field-label semantic-token parity 现在也已经有显式回归：analysis 会继续把 direct enum variant token 与 direct explicit struct field label 的 highlighting occurrence 原样投影给 LSP semantic tokens，而不是只在 import-alias follow-through 路径或聚合总表里被间接覆盖
- same-file direct symbol surface 现在也已经有显式聚合回归：direct enum variant token 与 direct explicit struct field label 会继续共享同一组 direct-symbol truth surface，因此 hover / definition / references / semantic tokens 不需要分别靠零散的单类回归来兜底
- direct stable-member query parity 现在也已经有显式回归：analysis 会继续把 direct field member 与唯一 method member 的 hover / definition / references 原样投影给 LSP，而不是只剩字段 hover 或其他间接覆盖
- direct stable-member semantic-token parity 现在也已经有显式回归：analysis 会继续把 direct field member 与唯一 method member 的 highlighting occurrence 原样投影给 LSP semantic tokens，而不是只靠聚合总表测试间接覆盖
- same-file direct member surface 现在也已经有显式聚合回归：direct field member 与唯一 method member 会继续共享同一组 direct-member truth surface，因此 hover / definition / references / semantic tokens 不需要分别靠零散的单类回归来兜底
- impl-preferred member query parity 现在也已经有显式回归：analysis 会继续把 impl-over-extend 的既有 direct member 选择结果原样投影给 LSP hover / definition / references，而不是在桥接层重新解释同名成员优先级
- lexical semantic symbol 现在也已经有 same-file parity 回归：`generic`、`parameter`、`local`、`receiver self` 与 `builtin type` 会继续共享同一份 lexical truth surface；其中 builtin type 仍没有 source-backed declaration，所以 definition / rename 保持关闭，但 hover / references / semantic tokens 已经显式回归锁住
- lexical rename surface 现在也已经有显式回归：`generic`、`parameter`、`local` 会继续沿用 analysis 的 same-file rename 结果直通到 LSP，而 `receiver self` / `builtin type` 继续返回 closed surface，不在协议层做特判补开
- 显式字段标签虽然已经能 hover / definition / references 到 struct field，shorthand `Point { x }` token 仍故意保守地继续解析为 local/binding；但从 source-backed field symbol 发起 rename 时，这些 shorthand site 会被自动扩写成显式标签，而从 shorthand token 上发起的 renameable binding rename 现在也会保持这条展开逻辑；这条回归已经明确覆盖 local / parameter / import / function / const / static；同文件 local import alias -> local struct item 的路径现在也会继续复用这条 field rename surface
- 其中 free-function shorthand binding rename 现在也已经有显式 LSP parity 回归：analysis 会继续把 shorthand token 解析为 `function` binding，bridge 会继续保留 field label 并只改 declaration / use，而不是把这类 site 退化成普通字段编辑
- 但这还不是完整 LSP 语义层：当前 completion 只做到 same-file lexical scope + parsed member token + parsed enum variant path，以及 local import alias -> local enum item 的 variant follow-through；这条 follow-through 也已经进入 hover / definition / references / same-file rename / semantic tokens；同文件 local import alias -> local struct item 现在也已经进入显式字段标签与 field-driven shorthand rename 的 query surface；struct field、显式字段标签、唯一 method candidate、enum variant token 这些精确 query 已经可复用，而唯一 method candidate 现在也已进入 same-file rename；completion 现在也会把 keyword-named symbol 写回 escaped identifier；import-graph/module-path deeper completion、foreign import alias variant semantics、ambiguous method、parse-error tolerant member completion、从 shorthand field token 本身发起的 field-symbol rename 和 cross-file rename 仍需要后续继续补齐

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

截至 2026-03-27，当前工具链基线已经反复验证过以下命令：

- `cargo fmt`
- `cargo test`
- `cargo run -p ql-cli -- check fixtures/codegen/pass/minimal_build.ql`
- `cargo run -p ql-cli -- check <含重复定义/重复绑定的源码>`
- 手工负例验证：`cargo run -p ql-cli -- check tests/ui/type_unknown_member.ql`
- `cargo run -p ql-cli -- fmt fixtures/parser/pass/phase1_declarations.ql`
- `npm run build` in `docs/`

说明：

- `fixtures/parser/pass/` 仍然是 parser / formatter regression surface，不再等价于“对当前完整语义流水线一定无错的输入”
- 其中部分 fixture 故意保留了 `tick`、`IoError`、`parse_int` 这类占位符符号，用来覆盖语法面，而不是充当当前 `ql check` 的 semantic-clean sample

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
