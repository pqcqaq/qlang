# Qlang

Qlang 是一门独立设计的编译型系统语言。当前编译器、CLI、LSP 和 VSCode 插件都在这个 Rust workspace 里实现。

## 当前状态

- Phase 1 到 Phase 6 的编译器和 same-file tooling 已落地。
- Phase 7 正在收口 async/runtime/build 的最小可用子集。
- Phase 8 正在推进 package/workspace、`.qi`、本地依赖、project-aware `build/run/test` 和 dependency-backed LSP。
- 仓库内已开始随源码分发普通 Qlang package 形态的最小 `stdlib`，当前包含 `std.core`、`std.option`、`std.result`、`std.array` 与 `std.test`，覆盖 3/4/5 项基础整数/布尔 helper、固定数组 helper、generic fixed-array accessor / transform / query、concrete/generic Option/Result carrier、generic Result/Option 转换、数组断言与 smoke-test 断言；它仍需通过本地 `[dependencies]` 显式依赖，不是自动 prelude。
- 当前跨包执行路径仍然很窄：稳定支持 direct local dependency 的 bridgeable public `const/static` values、受限 public top-level free function、public `extern "c"` 符号、被这些 value/function 签名直接引用的 public `struct` / `enum` / 非 opaque `type alias`，以及这些 bridgeable public `struct` 上的受限 public receiver method；public generic free function 目前支持 direct-call 多实例 specialization，要求每次调用的所有类型参数可从字面量、基础数值/布尔/比较表达式、tuple / fixed-array 字面量、一层 tuple/fixed-array projection、简单 `if` / `match` 表达式、top-level `const/static`、显式参数、显式 typed value、简单局部别名、generic carrier value、单字段 generic enum variant constructor 表达式或显式结果上下文推断，命名实参会按形参声明顺序参与同一套推断。当前 `enum` 运行闭环已稳定覆盖 unit / tuple / struct variant；其中 tuple variant 已补齐 `Enum.Variant(...)` 构造、按值返回和 tuple pattern `match`。
- root target 的 dependency bridge 现在会按实际导入情况为直依赖注入 public type declaration、value declaration、function wrapper 和受限 method forwarder；当前稳定覆盖 data-only public `struct`、public `enum`（含显式实例化泛型 enum 签名）type bridge、bridgeable public `const/static` values、受限 public free function / `extern "c"` 符号、value initializer 对同模块 bridgeable public function 的直接命名或直接调用，以及 bridgeable public `struct` 上的受限 public `impl` / `extend` / 唯一 trait `impl for` receiver method。未导入 sibling dependency 的同名符号不再提前卡住 `ql build/run/test`，但实际导入的同名直依赖 type/value/function/extern 仍会显式失败。
- LLVM 可执行主路径现在已支持本地与 direct local dependency 的 `impl` / `extend` / 唯一 trait `impl for` receiver method 直接调用、经不可变局部 alias 的 method value 直接调用（如 `let add = value.add; add(1)`），以及 public 非泛型 `enum` 的按值返回与 unit / tuple / struct variant `match`。当前边界仍然很窄：更广义的 escaping / higher-order method value 仍未打通。
- rename 仍以 same-file 为主；LSP 现已开放 source-backed dependency `method / field / enum variant` 的 workspace rename，以及 workspace root `function / const / static / struct / enum / trait / type alias / enum variant / struct field / receiver method` 的受限 workspace rename。root 顶层符号会继续联动 import path/direct-use；root members 当前可从导出包源码定义点或同文件使用点发起，只联动真实 member references，不改同名顶层 import path；其余符号仍未开放更广义的 cross-file rename / workspace edits。
- `ql build` / `ql run` 已支持从 package 根目录和已声明 target 的源码路径进入 project-aware 流程；workspace member 源码路径会继承外层 workspace profile 和输出目录语义。
- `ql build --list` / `ql run --list` 已可直接列出当前 package / workspace 下的 build targets；workspace member 目录或源码路径也会回到外层 workspace 视角；`--json` 复用 `ql.project.targets.v1`，`ql run --list` 只展示 runnable targets。
- `ql project status` 已提供只读 package/workspace 健康摘要；会聚合 members、targets、直接本地依赖和默认 `.qi` artifact 状态，支持 `--package` 与 `--json`，不会写 lock 或生成接口文件。
- `ql project add` 已能向现有 workspace 增量加入 `packages/<name>` member scaffold，并可在创建时直接写入 workspace 内本地依赖到 `[dependencies]`；也支持 `--existing` 把现有 package 或已移出的 member 重新纳入 workspace。
- `ql project remove` 已能按 package 名把现有 member 从 `[workspace].members` 里安全摘除；若仍被其他 workspace member 依赖会先拒绝删除，也可用 `--cascade` 自动清理依赖边后继续移除，并保留磁盘上的包目录，便于渐进式重构。
- `ql project add-dependency` / `remove-dependency` 已能直接维护已有 workspace member 的本地依赖；现在从 workspace 根也可配合 `--package` 直接指定目标 member，`add-dependency --path <file-or-dir>` 可把 workspace 外本地 package 按其真实 `[package].name` 写入相对路径依赖，`remove-dependency` 同时兼容清理旧的 `[references].packages` 入口，并支持 `--all` 按 package 名一次性清理所有 dependents；若从依赖包自身的 package / workspace member 路径进入，`--all` 也可直接自动推断目标包名。
- `ql project dependents` 已能直接查询某个 workspace package 当前被哪些 members 依赖，便于清理依赖边或定位删除阻塞；现在从 package / workspace member 目录或源码路径进入时也可自动推断目标包，不必每次手写 `--name`。
- `ql project dependencies` 已能直接查询某个 workspace package 当前依赖了哪些 workspace members 与 workspace 外本地路径依赖，并支持 `--json` 标出 `workspace` / `local` kind 与声明路径；正反向依赖审计都不必再手读 manifest 或 `project graph`，现在从 package / workspace member 目录或源码路径进入时也可自动推断目标包。
- `ql project targets` 现在也支持 `--package`、`--lib`、`--bin`、`--target` 过滤；项目级 target 查询不再只能全量输出，真实 workspace 下排查目标会更直接。
- `ql project init` 现在支持 `--stdlib <path>`；会把 `std.core` / `std.option` / `std.result` / `std.array` / `std.test` 作为 quoted-key 本地依赖写入新 package 或 workspace member，并生成直接消费 stdlib 的 `src/lib.ql`、`src/main.ql` 和 `tests/smoke.ql`；生成的 lib/main 会直接使用 `std.array` 的 generic accessor / repeat / reverse / contains / count helpers，生成的 smoke 使用 `std.test` 的 concrete/generic carrier、generic Result/Option 转换和 array assertion 断言，不再内联手写 Option/Result 状态函数，也不再手写数组断言表达式。
- `ql project target add --bin <name>` 现在也已落地；新增 bin target 时会自动创建 `src/bin/<name>.ql`，并在第一次显式写入 `[[bin]]` 时保留当前默认发现到的 `src/main.ql` / `src/bin/**/*.ql` targets，workspace 根也可配合 `--package` 直接改指定 member。
- `ql project graph` 现在也支持 `--package` 聚焦到单个 workspace member 的包图；workspace 根图查询不再只能看全量成员展开。
- `ql project emit-interface` 现在也支持在 workspace 入口配合 `--package <name>` 只发射或检查单个 member；plain / `--changed-only` / `--check` 继续可用，定向发射时也可配合 `--output` 导出到自定义路径，`--check` 仍不支持 `--output`。
- `ql check` / `ql build` / `ql run` / `ql test` 与 `ql project targets` / `graph` / `lock` 已提供第一版 `--json` 机器输出；`ql run --json` 当前输出 `ql.run.v1`，`ql project lock --json` 当前输出 `ql.project.lock.result.v1`。
- `ql check` 现在也会在 workspace member 目录或源码路径入口上恢复外层 workspace 语义，不再悄悄退回单 package 检查。
- `ql check` 现在也支持在 workspace 入口配合 `--package <name>` 只检查单个 member；排查大型 workspace 时不必再全量跑所有包。
- `ql test` 直接执行 project `tests/*.ql` 文件时会保留 package/workspace-aware smoke 或 UI test 语义；`ql project graph` / `ql project targets` / `ql project lock` 指向 workspace member 目录或源码文件时都会回到外层 workspace 上下文；`ql project emit-interface` 在不带 `--output` 时，无论 plain / `--changed-only` / `--check`，都会对 workspace member 目录或 `.ql` 源码路径恢复这一视角。
- `qlsp` diagnostics 现在会在当前打开文档 parser / semantic diagnostics 之外补 package preflight：当前 buffer 干净时，会把缺失 dependency `.qi`、损坏 interface 或 manifest/source-root 问题发布到当前文档；仍不做 workspace-wide diagnostics 推送，也不会让 package preflight 覆盖当前未保存 buffer 的 parser / semantic 错误。
- healthy package/workspace 下，LSP 的 source-preferred dependency tooling 已覆盖 workspace members 和 workspace 外本地路径依赖；definition、typeDefinition、references、`documentHighlight`、completion、`workspace/symbol` 与 source-backed dependency `method / field / enum variant` workspace rename 都会按 manifest 身份区分同名本地依赖，并优先读取已打开但未落盘的源码。
- healthy workspace/local dependency 下，source-backed dependency `method / field` 的 `hover`、`definition`、`typeDefinition`、`references`、`documentHighlight`、semantic tokens、`prepareRename`、workspace rename 现在都会在成员只存在于未保存源码、磁盘 `.qi` 仍旧过期时继续优先读取 open docs；一旦能定位到真实 workspace 源码，rename 会跳过生成的 `.qi` 编辑。
- `qlsp` 现已支持 `textDocument/formatting`、`rangeFormatting` 与 `onTypeFormatting`；VSCode 可直接复用 `ql fmt` 背后的格式化实现。当前仅支持可成功解析的源码，局部格式化会保守返回能安全落在请求范围或触发行内的逐行 edit。
- `qlsp` 现已支持 `textDocument/implementation`；same-file trait/type surface、same-file 已唯一解析的 receiver method call、workspace root source-backed `struct / enum / trait` 定义点、workspace root source-backed concrete / trait-typed method call、workspace / 本地路径依赖 source-preferred 导航下的 `struct / enum / trait`、trait method definition，以及能回到打开中本地源码的 source-backed dependency concrete / trait-typed method call 都可直接跳转。type/trait surface 会聚合当前包、可见 workspace members 与本地依赖源码里的 `impl` / `extend` / trait `impl` block；trait method definition 会聚合匹配的 `impl` method；已唯一解析的 method call、workspace root source-backed concrete method call 和 source-backed dependency concrete method call 会直接返回真实方法定义，trait-typed method call 则会聚合匹配的 `impl` method，并在 open-doc source-preferred 时回到真实 impl method。workspace / 本地依赖两条路径继续优先读取 open docs；healthy source 下，root trait method definition 与 dependency value / enum variant / struct field / method return type 这类非 import 位点也会继续回到真实 impl / extend block；当前 consumer 处于 broken-source / parse-error 时，source-backed dependency concrete / trait-typed method call、`current.extra` / `current.pulse()` 这类 dependency member/type-driven 位点、same-named 本地依赖的 concrete / trait method call、type / trait surface、field-driven / method-return-type implementation，以及依赖这些 open consumers 反查出来的 workspace root concrete / trait-typed method call，在依赖 open docs 自身 broken-open 时也会继续保守回到匹配依赖或当前 workspace 的真实源码，而不是退回磁盘旧内容或串到兄弟依赖；现在普通本地依赖的 concrete method call 在 dependency open doc 自身 broken-open 时，也会继续优先回到该 open dependency 里的真实方法定义。当前还没做更宽的全局 implementation index。
- `qlsp` 的 workspace root `Find References` 这一轮也把 trait method definition 往前推了一步：从导出源码里的 trait method 发起时，现会聚合可见 workspace members / 本地依赖源码里的匹配 impl methods，并优先读取 open docs。
- 这一轮补齐了 healthy workspace import 的 open-doc 导航一致性：`hover`、`definition`、`declaration`、`typeDefinition` 现在也会优先读取已打开但未落盘的 workspace 源码，而不是回退到磁盘旧内容。
- healthy workspace import `documentHighlight` 这一轮也补上了 open-doc 路径；未保存的导出 workspace 源码现在会直接参与当前文件 import/use 高亮。
- healthy workspace import semantic tokens 这一轮也补上了 open-doc 路径；healthy 与 parse-error fallback 两条着色路径都会直接读取未保存的导出 workspace 源码。
- workspace import references 现在会聚合当前文件、open unsaved 的导出源码与其他 workspace consumer 源码；workspace root `function / const / static / struct / enum / trait / type alias` 的 references / rename 已覆盖定义点、同文件使用点与 import/use 位置。
- healthy workspace 下，workspace root source-backed `enum variant / struct field / receiver method` 的 references 现在也会补回当前 package 可见的 analyzed workspace consumers；可以直接从导出包源码侧回收其他 members 里的真实成员使用。
- broken-source / parse-error 下，workspace root source-backed `enum variant / struct field / receiver method` 的 references 现在也会补回当前 package 可见的 broken workspace consumers；从导出包源码定义点或同文件使用点发起时，不再只看到 healthy members。
- 这一轮补齐了 open-doc 一致性：healthy workspace import/use `prepareRename`、broken-source workspace root import/use `prepareRename`，以及 broken-source workspace import alias rename，都会优先读取已打开但未落盘的 workspace 源码，而不是回退到磁盘旧内容。
- broken-source / parse-error 下，workspace import `hover/definition/typeDefinition`、direct imported-result member hover / completion / query / `documentHighlight`、dependency enum variant / struct field 的保守 fallback、workspace import references / query，以及 source-backed dependency rename 仍保留可用；workspace root `function / const / static / struct / enum / trait / type alias` 也允许从当前 broken consumer 的 import/use 发起 rename，并保守联动当前文件、当前 package、可见 workspace consumers 与导出包源码。
- `qlsp` 现在会声明 `.`, `:`, `"`, `/`, `@`, `<` completion triggers，并补上 keyword hover、keyword/snippet completion、可补齐缺失文档/detail 的 `completionItem/resolve`、与 full 共用 workspace/open-doc token 逻辑的 `semanticTokens/range`、基础 `signatureHelp`、局部类型 `inlayHint`、`foldingRange`、`selectionRange`、安全局部 `rangeFormatting` / `onTypeFormatting`、`codeAction/resolve`、`source.organizeImports` 和当前文档 `codeLens`；VSCode 中关键字、字面量、运算符和基础结构编辑体验不再只依赖同文件 symbol tokens。

## 先看哪些文档

- [当前支持基线](./docs/roadmap/current-supported-surface.md)
- [安装与版本配套](./docs/getting-started/install.md)
- [开发计划](./docs/roadmap/development-plan.md)
- [阶段总览](./docs/roadmap/phase-progress.md)
- [编译器入门](./docs/getting-started/compiler-primer.md)
- [VSCode 插件](./docs/getting-started/vscode-extension.md)

如果文档与实现或测试冲突，以 `crates/*` 和回归测试为准，再回头修正文档。

## 安装与版本配套

当前还没有预编译 release 或 VSCode Marketplace 分发。

如果要“安装使用”，仍然需要从同一份源码 checkout 构建一套匹配版本的 `ql`、`qlsp` 和 VSIX：

```powershell
cargo install --path crates/ql-cli
cargo install --path crates/ql-lsp
cd editors/vscode/qlang
npm install
npm run package:vsix
```

安装后先确认版本：

```powershell
ql --version
qlsp --version
```

VSCode 扩展会在连接到版本不匹配的 `qlsp` 时直接给出 warning；如果机器上同时存在多套 `qlsp`，建议显式设置 `qlang.server.path`。

## 仓库结构

- `crates/`: 编译器、CLI、project/workspace、runtime、LSP
- `docs/`: 文档站点与开发文档
- `fixtures/`: parser / codegen / diagnostics fixtures
- `tests/`: 集成与 FFI 测试输入
- `ramdon_tests/`: executable smoke corpus
- `examples/`: C / Rust FFI 示例
- `editors/vscode/qlang/`: VSCode thin client

## 常用命令

```bash
cargo test
cargo run -p ql-cli -- --version
cargo run -p ql-lsp -- --version
cargo run -p ql-cli -- check fixtures/parser/pass/basic.ql
cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_build.ql --emit llvm-ir
cargo run -p ql-cli -- project init demo-workspace --workspace --name app
cargo run -p ql-cli -- project add demo-workspace --name tools --dependency app
cargo run -p ql-cli -- project status demo-workspace --json
cargo run -p ql-cli -- project add demo-workspace --existing demo-workspace/vendor/core
cargo run -p ql-cli -- project remove demo-workspace --name tools
cargo run -p ql-cli -- project remove demo-workspace --name core --cascade
cargo run -p ql-cli -- project add-dependency demo-workspace/packages/app --name core
cargo run -p ql-cli -- project add-dependency demo-workspace --package app --name core
cargo run -p ql-cli -- project add-dependency demo-workspace --package app --path demo-workspace/vendor/core
cargo run -p ql-cli -- project remove-dependency demo-workspace/packages/app --name core
cargo run -p ql-cli -- project remove-dependency demo-workspace --package app --name core
cargo run -p ql-cli -- project remove-dependency demo-workspace --name core --all
cargo run -p ql-cli -- project remove-dependency demo-workspace/packages/core/src/main.ql --all
cargo run -p ql-cli -- project dependencies demo-workspace --name app
cargo run -p ql-cli -- project dependencies demo-workspace/packages/app
cargo run -p ql-cli -- project dependents demo-workspace --name core
cargo run -p ql-cli -- project dependents demo-workspace/packages/core/src/main.ql
cargo run -p ql-cli -- project targets demo-workspace --package app --bin main
cargo run -p ql-cli -- project target add demo-workspace --package app --bin worker
cargo run -p ql-cli -- project graph demo-workspace --package app
cargo run -p ql-cli -- check path/to/workspace/packages/app
cargo run -p ql-cli -- check path/to/workspace/packages/app/src/lib.ql
cargo run -p ql-cli -- check demo-workspace --package app
cargo run -p ql-cli -- project graph demo-workspace
cargo run -p ql-cli -- project lock demo-workspace --json
cargo run -p ql-cli -- project emit-interface path/to/workspace/packages/app
cargo run -p ql-cli -- project emit-interface demo-workspace --package app
cargo run -p ql-cli -- project emit-interface demo-workspace --package app --output artifacts/app.qi
cargo run -p ql-cli -- project emit-interface path/to/workspace/packages/app --changed-only
cargo run -p ql-cli -- project emit-interface path/to/workspace/packages/app --check
cargo run -p ql-cli -- build path/to/workspace/packages/app --list --json
cargo run -p ql-cli -- run path/to/workspace/packages/app --list --json
cargo run -p ql-cli -- build demo-workspace --list
cargo run -p ql-cli -- build demo-workspace
cargo run -p ql-cli -- run demo-workspace --list
cargo run -p ql-cli -- run demo-workspace
cargo run -p ql-cli -- build path/to/package/src/main.ql --json
cargo run -p ql-cli -- run path/to/package/src/main.ql
cargo run -p ql-cli -- run path/to/package/src/main.ql --json
cargo run -p ql-cli -- build path/to/workspace/packages/app/src/main.ql --json
cargo run -p ql-cli -- run path/to/workspace/packages/app/src/main.ql
cargo run -p ql-cli -- test demo-workspace
cargo run -p ql-cli -- test path/to/workspace/packages/app/tests/smoke.ql
cargo run -p ql-cli -- test demo-workspace --target packages/app/tests/smoke.ql
```

## VSCode

仓库内已包含最小 VSCode 插件工程：`editors/vscode/qlang`。

仓库开发模式：

```powershell
cargo build -p ql-lsp
cd editors/vscode/qlang
npm install
npm run compile
```

打包 VSIX：

```powershell
npm run package:vsix
```

安装插件后可直接在 VSCode 使用 `Format Document`。这条能力由 `qlsp` 复用 `ql fmt` 背后的格式化实现提供，当前只做整文档格式化；若源码存在 parse error，会跳过格式化并给出 warning。

Diagnostics 也由 `qlsp` 提供：当前打开文档会先发布 parser / semantic diagnostics；当当前 buffer 干净时，会额外发布 package preflight 的 manifest / interface 错误，例如缺失 dependency `.qi`。当前仍只发布打开文档本身的 diagnostics，不做 workspace-wide 推送。

VSIX 当前会输出到：

```text
editors/vscode/qlang/dist/qlang-<package.json version>.vsix
```

同一套 `qlsp` 现在也支持 `Go to Implementation`。same-file trait/type surface 会跳到当前文件里的 `impl` / `extend` block；same-file 已唯一解析的 receiver method call、workspace root source-backed concrete method call，以及能回到打开中本地源码的 source-backed dependency concrete method call 会直接回到真实方法定义；workspace root source-backed trait-typed method call 与 source-backed dependency trait-typed method call 会聚合匹配的 impl methods，并在 open-doc source-preferred 时回到真实 impl method；workspace root source-backed `struct / enum / trait` 定义点，以及 workspace / 本地路径依赖 source-preferred 导航下的 `struct / enum / trait`，都会聚合当前包、可见 workspace members 与本地依赖源码里的实现块；trait method definition 也会回到匹配的方法定义。workspace / 本地依赖两条路径继续优先读取 open docs；healthy source 下，root trait method definition 与 dependency value / enum variant / struct field / method return type 这类非 import 位点也会继续回到真实 impl / extend block；当前 consumer 处于 broken-source / parse-error 时，source-backed dependency concrete / trait-typed method call、`current.extra` / `current.pulse()` 这类 dependency member/type-driven 位点、same-named 本地依赖的 concrete / trait method call、type / trait surface、field-driven / method-return-type implementation，以及依赖这些 open consumers 反查出来的 workspace root concrete / trait-typed method call，在依赖 open docs 自身 broken-open 时也会继续保守回到匹配依赖或当前 workspace 的真实源码，而不是退回磁盘旧内容或串到兄弟依赖；现在普通本地依赖的 concrete method call 在 dependency open doc 自身 broken-open 时，也会继续优先回到该 open dependency 里的真实方法定义。当前还没做更宽的全局 implementation index。

`qlsp` 也已开放 `textDocument/codeLens` 与 `codeLens/resolve`。当前会在可解析的当前文档里为 document symbols 生成 references / implementations 入口，并直接复用 VSCode `editor.action.showReferences` 展示位置；parse-error 文档会保守不返回 code lens，暂不提供 workspace-wide lens index。

扩展启动后会读取 `qlsp` 的 `serverInfo.version`；如果扩展版本和 `qlsp` 版本不一致，会直接给出 warning，避免 repo 开发产物和安装产物混用时静默漂移。

## 文档开发

在线文档：

- https://qlang.zust.online/

本地启动：

```powershell
cd docs
npm install
npm run dev
```
