# 当前支持基线

> 最后同步：2026-04-23

这页只记录今天真实可依赖的能力边界。

## 真相源

- 实现：`crates/*`
- CLI / build / project 回归：`crates/ql-cli/tests/*`
- analysis / LSP 回归：`crates/ql-analysis/tests/*`、`crates/ql-lsp/src/backend/tests.rs`（helper / unit matrix）、`crates/ql-lsp/tests/*`（protocol / integration）
- executable smoke：`ramdon_tests/executable_examples/`、`ramdon_tests/async_program_surface_examples/`

如果文档与实现冲突，以代码和回归测试为准。

## 已支持

### 编译器主路径

- lexer、parser、formatter、diagnostics、HIR、resolve、typeck、MIR、borrowck 已在主路径工作。
- LLVM 产物当前稳定开放：`llvm-ir`、`asm`、`obj`、`exe`、`dylib`、`staticlib`。
- LLVM 可执行主路径现在已支持本地 `impl` / `extend` receiver method 的直接调用，以及经不可变局部 alias 的 method value 直接调用（如 `let add = value.add; add(1)`）；更广义的 escaping / higher-order method value 仍未打通。
- 当前稳定互操作边界仍是 C ABI。

### CLI 与项目工作流

- 已实现：`ql check`、`ql fmt`、`ql mir`、`ql ownership`、`ql runtime`、`ql build`、`ql run`、`ql test`、`ql project`、`ql ffi`。
- `ql` 与 `qlsp` 现在都已提供 `--version` / `-V` / `version`，可直接校验当前安装的产物版本。
- `ql project init` 已能生成最小 package / workspace 脚手架，并附带 `src/lib.ql`、`src/main.ql`、`tests/smoke.ql`。
- `ql project add` 已能向现有 workspace 增量加入 `packages/<name>` member scaffold，并可在创建时直接写入 workspace 内本地依赖到 `[dependencies]`；`--existing` 也可把现有 package 或已移出的 member 重新纳入 workspace。
- `ql project remove` 已能按 package 名把现有 member 从 `[workspace].members` 里摘除；若仍被其他 workspace member 依赖会先拒绝删除，也支持 `--cascade` 自动清理这些 members 指向目标包的本地依赖边，并保留原包目录，先支持安全退出 dependency graph，再由用户决定是否清理文件。
- `ql project add-dependency` / `ql project remove-dependency` 已能直接修改已有 workspace member 的本地依赖；现在从 workspace 根也可配合 `--package` 直接指定目标 member，`remove-dependency` 还兼容清理旧的 `[references].packages` 入口，并支持 `--all` 按 package 名批量清理所有 dependents；若从依赖包自身的 package / workspace member 路径进入，`--all` 也可直接自动推断目标包名。
- `ql project dependents` 已能直接反查某个 workspace package 的反向本地依赖成员，并支持 `--json`；删除前定位阻塞和脚本化清理路径已不必手扫 manifest。从 package / workspace member 目录或源码路径进入时也可直接自动推断目标包。
- `ql project dependencies` 已能直接正查某个 workspace package 的直接本地依赖成员，并支持 `--json`；正反向依赖排查都不必再手扫 manifest 或解析 `ql project graph`。从 package / workspace member 目录或源码路径进入时也可直接自动推断目标包。
- `ql project targets` 现在也支持 `--package`、`--lib`、`--bin`、`--target` 过滤；项目级 target 查询不再只能全量输出。
- `ql project target add --bin <name>` 现在也已落地；新增 bin target 会自动创建 `src/bin/<name>.ql`，第一次显式写入 `[[bin]]` 时也会保留当前默认发现到的 `src/main.ql` / `src/bin/**/*.ql` targets，workspace 根可配合 `--package` 直接指定目标 member。
- `ql project graph` 现在也支持 `--package` 聚焦到单个 workspace member 的包图；workspace 根图查询不再只能全量展开所有 members。
- `ql project targets`、`ql project graph`、`ql project lock`、`ql project emit-interface` 已落地。
- `qlang.toml` 当前只稳定支持：
  - `[package].name`
  - `[workspace].members`
  - `[references].packages`
  - `[dependencies]` 本地路径依赖
  - `[lib].path`、`[[bin]].path`
  - `[profile].default = "debug" | "release"`
- `qlang.lock` 第一版已落地，当前锁定本地 package graph、默认 profile 和 target 输入面。
- package/workspace 根目录已经可以直接进入 project-aware `ql build` / `ql run` / `ql test`。
- `ql check` 直接指向 workspace member 目录或源码文件时，也会恢复外层 workspace 上下文，而不是退化成单 package 检查。
- `ql check` 现在也支持在 workspace 入口配合 `--package <name>` 只检查单个 member；workspace 根或 member 路径入口都可直接收敛到目标包。
- `ql build` / `ql run` 对 project 内已声明 target 的单个源码文件也已支持 project-aware 入口；无论直接执行 package 自身的 `src/main.ql`、`src/lib.ql`、`src/bin/*.ql`，还是执行 workspace member 下对应源码路径，都会保留 package / workspace profile、依赖构建和 project 输出目录语义。
- `ql test` 已支持对已发现测试使用 `--target` 做精确 rerun；直接执行 package `tests/` 下的单个 `.ql` 文件，或执行 workspace member 下对应测试文件时，也会保留 package/workspace-aware smoke / UI test 语义。
- `ql project graph` / `ql project targets` / `ql project lock` 直接指向 workspace member 目录或源码文件时，会解析外层 workspace，而不是退化成单 package 视图。
- `ql project emit-interface` 在不带 `--output` 时，直接指向 workspace member 目录或 `.ql` 文件也会解析外层 workspace；plain、`--changed-only`、`--check` 都按 workspace member 集合执行发射/检查，workspace 根也可配合 `--package <name>` 只收敛到单个 member。定向发射单个 member 时，也可继续配合 `--output` 导出到自定义路径。
- `ql build --list` / `ql run --list` 已可直接列出当前 package / workspace 下的 discovered build targets；直接指向 workspace member 目录或源码路径时，也会回到外层 workspace；`--json` 复用 `ql.project.targets.v1`，`ql run --list` 只展示 runnable targets。
- `ql check` / `ql build` / `ql run` / `ql test` 都已有第一版 `--json` 输出；其中 `ql run --json` 当前稳定导出 `ql.run.v1`，包含 built target、程序参数、捕获到的 stdout/stderr 和子进程退出码。更早的 selector / project preflight 失败仍保留既有 stderr failure surface。
- `ql project lock --json` 当前稳定导出 `ql.project.lock.result.v1`，覆盖写锁文件成功、`--check` 命中 up-to-date，以及 stale / missing / read / write 失败；最早的 package-context / manifest preflight 失败仍保留既有 stderr surface。
- 当前真正打通的跨包执行路径仍然很窄：只稳定覆盖 direct local dependency 的 bridgeable public `const/static` values、受限 public top-level free function（非 `async` / 非 `unsafe`、无 generics / `where`、仅普通参数）、public `extern "c"` 符号、被这些 value/function 签名直接引用的 public 非泛型 `struct` / `enum`，以及这些 bridgeable public `struct` 上的受限 public receiver method。当前 `enum` 运行闭环稳定覆盖按值返回与 unit / tuple / struct variant `match`；tuple variant 也已补齐 `Enum.Variant(...)` 构造和 tuple pattern 解构。
- root target 的 dependency bridge 现在会按实际导入情况为直依赖注入 public type declaration、value declaration、function wrapper 和受限 method forwarder。当前稳定覆盖 data-only public `struct`、public 非泛型 `enum` type bridge、data-only initializer、递归引用同模块其他 public `const/static`、value initializer 对同模块 bridgeable public free function 的直接命名或直接调用，以及 bridgeable public `struct` 上的 public `impl` / `extend` / 唯一 trait `impl for` receiver method direct call，与经不可变局部 alias 的 method value direct call；当导入的 value/function/method 签名依赖同模块 bridgeable public `struct` / `enum` 时，root target 也会隐式补齐所需 type bridge。未导入 sibling dependency 的同名符号不会再提前打断 `ql build/run/test`，但实际导入的同名直依赖 type/value/function/extern 仍会分别触发 `dependency-type-conflict` / `dependency-value-conflict` / `dependency-function-conflict` / `dependency-extern-conflict`；root source 的同名顶层定义也会分别触发 `dependency-type-local-conflict` / `dependency-value-local-conflict` / `dependency-function-local-conflict`。

### async / runtime

- 已有最小 async 子集：`async fn`、`await`、`spawn`、`for await`、program-mode `async main`。
- 已有最小 `ql-runtime` 和 task-handle lowering。
- async library/program build 仍是保守子集，不应按“完整 runtime”理解。

### LSP 与 VSCode

- same-file 语义已经接通：hover、definition、declaration、typeDefinition、references、documentHighlight、completion、semanticTokens、documentSymbol、rename。
- `workspace/symbol` 已落地。
- `textDocument/codeAction` 第一版已落地：当前会对 unresolved value/type 提供 quick fix，并从 workspace member / 本地依赖源码或 `.qi` 的 `package ...` 声明推导完整 `use ...` 路径；若候选来自未声明的 sibling workspace member，还会同时给当前 package `qlang.toml` 补本地 `[dependencies]` 项。显式 `use demo.xxx...` 指向未声明的 sibling workspace member 时，也会直接提供只改 manifest 的 missing-dependency quick fix。
- healthy package/workspace 下，dependency-backed navigation 已能提供一批可依赖能力：
  - import root / dependency value / enum variant / struct field / method member 的 hover / definition / declaration / references / typeDefinition / current-document `documentHighlight`
  - dependency enum variant / struct field / member field / method completion 的源码优先返回
  - source-preferred navigation：对 workspace members 和 workspace 外本地路径依赖，能唯一回溯到源码时优先跳源码而不是 `.qi`
  - package-aware semantic tokens
- healthy workspace 下，source-preferred dependency definition / typeDefinition / references / current-document `documentHighlight` / method completion 现在会直接读取已打开但未落盘的本地依赖源码，而不是只看磁盘文件。
- healthy workspace / 本地路径依赖下，source-backed dependency `method / field` 的 `hover / definition / typeDefinition / references / current-document documentHighlight / semantic tokens / prepareRename / workspace rename` 现在也会在成员只存在于未保存源码、磁盘 `.qi` 仍旧过期时继续优先读取 open docs；当已成功回到 workspace 源码定义时，rename 不再额外改写生成 `.qi`。
- healthy workspace 下，workspace import `hover/definition/declaration/typeDefinition` 现在也会读取已打开但未落盘的导出 workspace 源码，不再要求先保存文件才能看到正确导航结果。
- healthy workspace 下，workspace import `documentHighlight` 现在也会读取已打开但未落盘的导出 workspace 源码；当前文件 import/use 高亮不再落回磁盘旧版本。
- healthy workspace 下，workspace import semantic tokens 现在也会读取已打开但未落盘的导出 workspace 源码；healthy 与 parse-error fallback 两条着色路径都不再落回磁盘旧版本。
- healthy workspace 下，workspace import references 现在同时覆盖 value import 和 analyzed-source type import 的 alias/use；当前文件、已打开但未落盘的导出源码，以及同 workspace 其他 consumer 文件的 import/use 都会一起回收。
- workspace root `function / const / static / struct / enum / trait / type alias` 现在也补上了 references 聚合：无论从源码定义点还是同文件使用点发起，都会保留当前文件内引用，并联动返回 workspace 中对应 import alias/use 位置；这条 import/use 聚合会读取 open docs；对当前 package 可见的 broken consumers，也会保守补回 import/use。
- healthy workspace 下，workspace root source-backed `enum variant / struct field / receiver method` 的 references 现在也会补回当前 package 可见的 analyzed workspace consumers；当前已锁住从导出包源码定义点或同文件使用点发起的 variant/member references 聚合。
- broken-source / parse-error 下，workspace root source-backed `enum variant / struct field / receiver method` 的 references 现在也会补回当前 package 可见的 broken workspace consumers；当前已锁住从导出包源码定义点或同文件使用点发起的 member references 聚合。
- healthy source 下，workspace root source-backed `enum variant / struct field / receiver method` 也已开放 workspace rename：当前可从导出包源码定义点或同文件使用点发起，并会联动改写当前文件与可见 workspace consumers 里的真实 member uses；这条路径不会误改同名顶层 root import path。
- healthy source 下，workspace root import/use 的 `prepareRename` 现在也会读取已打开但未落盘的导出 workspace 源码，不再要求先保存文件。
- source-preferred dependency tooling 现在按 manifest 身份区分同名本地依赖；definition / typeDefinition / references / current-document `documentHighlight` / dependency completion / `workspace/symbol` 不会再串到另一个依赖实例。
- `workspace/symbol` 对 workspace 外本地路径依赖在源码可用时会优先返回源码里的 value / method / trait / extend symbols；源码不可用时仍回退到 `.qi`。这条行为现在也覆盖 `workspace_roots` / 无打开文档入口；同名本地依赖也不会再因为 source-preferred 排除而误丢另一个依赖的 `.qi` 符号。
- 同名本地依赖的 type / enum / enum member、method / trait method / extend method 组合场景现在也有显式回归保护；`[dependencies]` 本地路径依赖在 open document 和 `workspace_roots` 入口上也都锁住了“源码优先 + 兄弟依赖 `.qi` 保留”这条 `workspace/symbol` 合同。
- `qlsp` 现在会声明 `.` completion trigger，VSCode 中输入成员访问和点分 dependency 路径时可直接自动触发补全。
- VSCode 扩展现在会读取 LSP `serverInfo.version`；若扩展版本与 `qlsp` 版本不一致，会直接给出 warning，避免 repo 开发产物和安装产物混用时静默漂移。
- `textDocument/formatting` 已落地：当前复用 `ql fmt` 背后的格式化实现提供整文档格式化；仅在源码可成功解析时返回编辑，parse-error 文档会保守跳过并记录 warning。
- `textDocument/implementation` 已覆盖 same-file trait/type surface、same-file 已唯一解析的 receiver method call、workspace root source-backed `struct / enum / trait` 定义点、workspace root source-backed concrete / trait-typed method call、workspace / 本地路径依赖 source-preferred 导航下的 `struct / enum / trait`、trait method definition，以及能回到打开中本地源码的 source-backed dependency concrete / trait-typed method call。same-file trait/type 返回本文件里的 `impl` / `extend` block；具体 method call 中，concrete receiver 继续返回唯一解析到的真实方法定义，source-backed method declaration site 不会再把自身声明回给 `implementation`；healthy analyzed source 下的 trait-typed receiver call（如 `runner.run()`）会聚合当前包、可见 workspace members 与本地依赖源码里的匹配 `impl` methods；workspace root/source-preferred trait/type 会聚合当前包、可见 workspace members 与本地依赖源码里的 `impl` / `extend` / trait `impl` block，trait method definition 也会聚合匹配的 `impl` method；workspace / 本地依赖路径继续优先读取 parseable open docs，healthy source 下从 dependency value / enum variant / struct field / method return type 这类 type-driven 位点发起 `implementation` 也会回到真实 source-backed `impl` / `extend` block。当前 consumer 处于 broken-source / parse-error 时，当前 active root/source 自身的 `struct / enum / trait` 定义点与 trait method definition、source-backed dependency concrete / trait-typed method call、依赖这些 open consumers 反查出来的 workspace root concrete / trait-typed method call，以及 broken open 源码里的 workspace root/source-backed 与 workspace / 本地路径依赖 source-preferred `struct / enum / trait` implementation 聚合、source-backed `impl` / `extend` block 与 trait impl method 聚合，也会继续保守回到真实源码，而不是退回磁盘旧内容；当前 buffer 的 broken concrete / trait-typed method call `implementation` 现在都会先尝试修复成可分析调用位点并走 source-backed 路径，失败后再保守回落到旧的同文件唯一方法定义逻辑；broken current consumer 的 source-preferred dependency trait `implementation` 现在也覆盖 `impl Trait for ...` header 里的 trait 名引用，而 broken current consumer 里 `current.extra` / `current.pulse()` 这类依赖 member/type-driven 位点也会继续优先读取 open dependency 源码并回到真实 `impl` / `extend` block；broken current workspace root concrete method call 在无法复用 source-backed consumer 路径时，也会保守回落到当前打开源码里唯一可定位的真实方法定义；same-named 本地路径依赖在这些 broken-source implementation fallback 中也继续按 manifest/source identity 隔离，不会串到兄弟依赖的 impl blocks 或 impl methods。当前这条能力也已有真实 LSP request 回归，直接锁住了 same-file trait surface、same-file type surface、same-file trait method definition 的数组结果、same-file concrete method call、same-file method declaration site 的空结果、healthy source-backed dependency value / enum variant / struct field / method return type `implementation` 定位结果、healthy source-backed dependency trait-typed method call 的 impl method 聚合数组结果、open-doc source-backed dependency trait-typed method call 的 source-preferred scalar 结果、broken-open source-backed dependency trait-typed method call 的 source-preferred scalar 结果、broken-current source-backed dependency trait-typed method call 的 source-preferred scalar 结果、workspace source-backed type surface、workspace trait import surface、broken open workspace type import surface、broken current workspace type / trait import surface、workspace root source-backed concrete method call 的 scalar 结果、workspace root source-backed trait-typed method call 的 impl method 聚合数组结果、open-doc workspace root source-backed trait-typed method call 的 source-preferred scalar 结果、workspace root source-backed method declaration site 的空结果、workspace root source-backed trait method definition 的 open-doc source-preferred scalar 结果、broken open workspace root source-backed concrete method call 的 scalar 结果、broken open workspace root source-backed trait-typed method call 的 source-preferred scalar 结果、broken current workspace root source-backed concrete method call 的 local-fallback scalar 结果、broken current workspace root source-backed trait-typed method call 的 impl method 聚合数组结果、workspace root source-backed trait surface 的 workspace impl block 聚合、broken open workspace root source-backed trait surface 的 workspace impl block 聚合数组结果、broken current workspace root source-backed trait surface 的 workspace impl block 聚合数组结果、workspace root source-backed trait method definition 的 workspace impl method 聚合、broken open workspace root source-backed trait method definition 的 workspace impl method 聚合数组结果、broken current workspace root source-backed trait method definition 的 workspace impl method 聚合数组结果、open local dependency field-driven / method-return-type `implementation` 数组结果、broken-source local dependency field-driven / method-return-type `implementation` 数组结果、open same-named local dependency field-driven / method-return-type `implementation` 数组结果、open same-named local dependency type `implementation`、open same-named local dependency trait surface `implementation`、open same-named local dependency trait method call `implementation`、broken open same-named local dependency type `implementation`、broken open same-named local dependency trait surface `implementation`、broken open same-named local dependency trait method call `implementation`，以及 broken current source 下 same-named dependency type / trait surface / trait method call / field-driven / method-return-type `implementation`。更宽的全局 implementation index 仍未做。
- healthy 与 broken-source source-backed dependency concrete method call 现在也有真实 request 回归；当本地路径依赖的方法只存在于未保存源码里时，从 `build().pulse()` / `build().pulse(` 这类位点发起 `implementation` 会继续优先回到打开中的真实方法定义，而不是退回磁盘旧接口或空结果。
- broken open workspace root source-backed method declaration site 现在也有真实 request 回归；当前 workspace consumer 处于 parse-error 时，`implementation` 继续返回空结果，不会把方法声明自身回给调用方。
- broken current workspace root source-backed method declaration site 现在也有真实 request 回归；当前导出源码处于 parse-error 时，`implementation` 继续返回空结果，不会把方法声明自身回给调用方。
- broken current workspace root source-backed concrete method call 在存在多个同名本地候选方法时，request 路径现在也继续返回空结果；repaired source 只再用于 dependency method 路径，不会在本地歧义时猜实现。
- workspace root concrete trait-impl method call 现在也有真实 request 回归；concrete typed `worker.run()` 继续保持 scalar，到真实 impl method 定义点为止。
- workspace root `Find References` 现在也补到了 trait method definition：从导出源码 trait method 发起时，会聚合可见 workspace members / 本地依赖源码里的匹配 impl methods，并优先读取 parseable open docs。
- broken-source / parse-error 下，当前只保留保守子集，不等于完整恢复；workspace 外本地路径依赖的 import references fallback、workspace import `hover/definition/typeDefinition`、direct imported-result member hover / completion / query / `documentHighlight`（如 `build().ping()` / `build().value`）、dependency struct field label completion、dependency enum variant 的 `completion/definition/typeDefinition/references/documentHighlight`、dependency value/member semantic tokens fallback 都会继续走源码优先路径；其中 broken-source workspace import references、workspace import query、dependency references / current-document `documentHighlight` / method completion 现在也会直接读取已打开但未落盘的 workspace member / 本地依赖源码；`impl Trait for ...` header 里的 trait import/query 现在也纳入这条 broken-source 上下文；在聚合 healthy workspace import/use 时也会读取这些 consumer 的 open docs；同名本地依赖按 manifest 身份区分，不会串到兄弟依赖实例。
- broken-source 下的同名本地依赖 `workspace/symbol` 现在也补上了 `[dependencies]` 路径依赖入口；open document 和 `workspace_roots` 都已锁住顶层 type / interface / enum symbol、enum member，以及 method / trait method / extend method 的“源码优先 + 兄弟依赖 `.qi` 保留”合同。
- source-backed dependency `method / field / enum variant` 现在已开放 workspace rename：可从依赖使用点或导出包源码侧发起，并会同时改写依赖源码定义点、源码内部引用、当前文件和同 workspace 其他使用文件；healthy / broken-source 两条路径都已有最小回归保护，同名本地依赖继续按 manifest 身份区分，不会串改兄弟依赖实例。
- healthy source 下，workspace root `function / const / static / struct / enum / trait / type alias` 也已开放 workspace rename：当前可从 root 源码定义点、同文件使用点，以及 import/use 位置发起（包含 alias import/use），并会同时改写当前文件引用、同 package 其他源码引用、workspace import path/direct-use，以及当前 package / workspace 中可见 broken consumers 的 import/use；其中 alias import 只更新导入路径，不改本地 alias/use。
- broken-source 下，workspace root `function / const / static / struct / enum / trait / type alias` 的 import/use references 现在也会做保守聚合：除当前文件和导出包源码外，还会补回当前 package 可见的 workspace members / 本地路径依赖里的其他 broken consumer import/use。
- broken-source 下，workspace root `function / const / static / struct / enum / trait / type alias` 现在也允许从当前 consumer 的 import/use 位置发起 workspace rename（包含 alias import/use）；当前保守联动范围是当前 broken 文件、当前 package 其他源码文件、当前 package 可见的 workspace members / 本地路径依赖里的其他 consumer 源码，以及导出包源码；alias import 仍只更新导入路径，不改本地 alias/use。
- broken-source 下，workspace root import/use 的 `prepareRename` 与 workspace import alias rename 现在也会读取已打开但未落盘的 workspace 源码；consumer 文件暂时可不保存就能继续做这两条重命名路径。
- rename 仍然以 same-file 为主；import / local 等其余符号仍未开放更广义的 cross-file rename / workspace edits。

## 当前明确未支持

- 普通跨包 Qlang free function / member / const 的完整 dependency-aware backend
- escaping / higher-order dependency method values、超出当前不可变局部 alias direct-call slice 的 dependency receiver method codegen
- registry / version solving / publish workflow
- 预编译 release / VSCode Marketplace 分发
- 更广义的 cross-file rename / workspace edits（超出 source-backed dependency `method / field / enum variant`）
- 更宽的 project-scale references / refactor、补齐 `match` 分支等更完整 code actions / inlay hints
- 超出当前保守 slice 的广义 parse-error member 语义
- 完整 trait solver、完整 monomorphization、更完整 effect system

## 建议阅读

1. [开发计划](/roadmap/development-plan)
2. [阶段总览](/roadmap/phase-progress)
3. [工具链设计](/architecture/toolchain)
4. [VSCode 插件](/getting-started/vscode-extension)
