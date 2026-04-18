# Qlang

Qlang 是一门独立设计的编译型系统语言。当前编译器、CLI、LSP 和 VSCode 插件都在这个 Rust workspace 里实现。

## 当前状态

- Phase 1 到 Phase 6 的编译器和 same-file tooling 已落地。
- Phase 7 正在收口 async/runtime/build 的最小可用子集。
- Phase 8 正在推进 package/workspace、`.qi`、本地依赖、project-aware `build/run/test` 和 dependency-backed LSP。
- 当前跨包执行路径仍然很窄：只稳定支持 direct local dependency 的 public `extern "c"` 符号。
- 当前 rename 仍以 same-file 为边界；cross-file rename / workspace edits 尚未开放。
- `ql build` / `ql run` 现在既可从 package 根目录进入 project-aware 流程，也可直接从 package target 源码路径进入；workspace member 下的 `src/main.ql`、`src/lib.ql`、`src/bin/*.ql` 也会继承外层 workspace profile 和输出目录语义。

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
cargo run -p ql-cli -- project graph demo-workspace
cargo run -p ql-cli -- build demo-workspace
cargo run -p ql-cli -- run demo-workspace
cargo run -p ql-cli -- build path/to/package/src/main.ql --json
cargo run -p ql-cli -- run path/to/package/src/main.ql
cargo run -p ql-cli -- build path/to/workspace/packages/app/src/main.ql --json
cargo run -p ql-cli -- run path/to/workspace/packages/app/src/main.ql
cargo run -p ql-cli -- test demo-workspace
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
