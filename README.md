# Qlang

Qlang 是一门独立设计的编译型系统语言。当前编译器、CLI、LSP 和 VSCode 插件都在这个 Rust workspace 里实现。

## 当前状态

- Phase 1 到 Phase 6 的编译器和 same-file tooling 已落地。
- Phase 7 正在收口 async/runtime/build 的最小可用子集。
- Phase 8 正在推进 package/workspace、`.qi`、本地依赖、project-aware `build/run/test` 和 dependency-backed LSP。
- 当前跨包执行路径仍然很窄：只稳定支持 direct local dependency 的 bridgeable public `const/static` values、受限 public top-level free function（非 `async` / 非 `unsafe`、无 generics / `where`、仅普通参数）、public `extern "c"` 符号、被这些 value/function 签名直接引用的 public 非泛型 `struct`，以及这些 bridgeable public `struct` 上的受限 public receiver method。
- root target 的 dependency bridge 现在会按实际导入情况为直依赖注入 public type declaration、value declaration、function wrapper 和受限 method forwarder；当前稳定覆盖 data-only public `struct`、bridgeable public `const/static` values、受限 public free function / `extern "c"` 符号、value initializer 对同模块 bridgeable public function 的直接命名或直接调用，以及 bridgeable public `struct` 上的受限 public `impl` / `extend` receiver method。未导入 sibling dependency 的同名符号不再提前卡住 `ql build/run/test`，但实际导入的同名直依赖 type/value/function/extern 仍会显式失败。
- LLVM 可执行主路径现在已支持本地与 direct local dependency 的 `impl` / `extend` receiver method 直接调用（如 `value.read()`）；当前只覆盖 direct call position，不包含 method value 或 trait receiver method。
- 当前 rename 仍以 same-file 为边界；cross-file rename / workspace edits 尚未开放。
- `ql build` / `ql run` 已支持从 package 根目录和已声明 target 的源码路径进入 project-aware 流程；workspace member 源码路径会继承外层 workspace profile 和输出目录语义。
- `ql build --list` / `ql run --list` 已可直接列出当前 package / workspace 下的 build targets；workspace member 目录或源码路径也会回到外层 workspace 视角；`--json` 复用 `ql.project.targets.v1`，`ql run --list` 只展示 runnable targets。
- `ql project add` 已能向现有 workspace 增量加入 `packages/<name>` member scaffold，并可在创建时直接写入 workspace 内本地依赖到 `[dependencies]`。
- `ql project remove` 已能按 package 名把现有 member 从 `[workspace].members` 里安全摘除，并保留磁盘上的包目录，便于渐进式重构。
- `ql check` / `ql build` / `ql run` / `ql test` 与 `ql project targets` / `graph` / `lock` 已提供第一版 `--json` 机器输出；`ql run --json` 当前输出 `ql.run.v1`，`ql project lock --json` 当前输出 `ql.project.lock.result.v1`。
- `ql check` 现在也会在 workspace member 目录或源码路径入口上恢复外层 workspace 语义，不再悄悄退回单 package 检查。
- `ql test` 直接执行 project `tests/*.ql` 文件时会保留 package/workspace-aware smoke 或 UI test 语义；`ql project graph` / `ql project targets` / `ql project lock` 指向 workspace member 目录或源码文件时都会回到外层 workspace 上下文；`ql project emit-interface` 在不带 `--output` 时，无论 plain / `--changed-only` / `--check`，都会对 workspace member 目录或 `.ql` 源码路径恢复这一视角。
- healthy package/workspace 下，LSP 的 source-preferred navigation 已覆盖 workspace members 和 workspace 外本地路径依赖；definition、typeDefinition、references、`workspace/symbol` 会按 manifest 身份区分同名本地依赖，且 `workspace/symbol` 在源码可用时优先返回源码符号。
- broken-source / parse-error 下，import references fallback、direct imported-result member hover / completion / 查询 / `documentHighlight`、dependency struct field label completion、dependency enum variant 的 `completion/definition/typeDefinition/references/documentHighlight`、dependency value/member semantic tokens fallback，以及 current-document dependency method / struct field / enum variant rename 都会继续走保守可用路径；同名本地依赖仍按 manifest 身份区分。
- `qlsp` 现在会声明 `.` completion trigger，VSCode 中输入成员访问或点分路径时可直接自动弹出补全。

## 先看哪些文档

- [当前支持基线](./docs/roadmap/current-supported-surface.md)
- [开发计划](./docs/roadmap/development-plan.md)
- [阶段总览](./docs/roadmap/phase-progress.md)
- [编译器入门](./docs/getting-started/compiler-primer.md)
- [VSCode 插件](./docs/getting-started/vscode-extension.md)

如果文档与实现或测试冲突，以 `crates/*` 和回归测试为准，再回头修正文档。

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
cargo run -p ql-cli -- check fixtures/parser/pass/basic.ql
cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_build.ql --emit llvm-ir
cargo run -p ql-cli -- project init demo-workspace --workspace --name app
cargo run -p ql-cli -- project add demo-workspace --name tools --dependency app
cargo run -p ql-cli -- project remove demo-workspace --name tools
cargo run -p ql-cli -- check path/to/workspace/packages/app
cargo run -p ql-cli -- check path/to/workspace/packages/app/src/lib.ql
cargo run -p ql-cli -- project graph demo-workspace
cargo run -p ql-cli -- project lock demo-workspace --json
cargo run -p ql-cli -- project emit-interface path/to/workspace/packages/app
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

## 文档开发

在线文档：

- https://qlang.zust.online/

本地启动：

```powershell
cd docs
npm install
npm run dev
```
