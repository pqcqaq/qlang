# Qlang

Qlang 是一门独立设计的编译型系统语言。这个仓库包含编译器、CLI、LSP、VSCode 插件和普通 `stdlib` workspace。

## 当前状态

- 编译器主路径、project-aware CLI、基础 same-file tooling 和最小 `stdlib` 已可用。
- 真实项目可以通过本地依赖、`ql project init/add/remove/...`、`ql build/run/test` 和 `qlsp` 形成最小闭环。
- 目前仍以本地源码开发为主，没有预编译 release、Marketplace 发布或 registry 分发。

## 先看这些文档

- [当前支持基线](./docs/roadmap/current-supported-surface.md)
- [开发计划](./docs/roadmap/development-plan.md)
- [阶段总览](./docs/roadmap/phase-progress.md)
- [Stdlib README](./stdlib/README.md)
- [工具链设计](./docs/architecture/toolchain.md)
- [类型系统](./docs/design/type-system.md)

如果文档与实现冲突，以 `crates/*` 和回归测试为准。

## 本地使用

从同一份源码构建一套匹配版本的 CLI、LSP 和 VSCode 插件：

```powershell
cargo install --path crates/ql-cli
cargo install --path crates/ql-lsp
cd editors/vscode/qlang
npm install
npm run package:vsix
```

确认版本：

```powershell
ql --version
qlsp --version
```

常用命令：

```powershell
cargo test
cargo run -p ql-cli -- project init demo --workspace --name app --stdlib stdlib
cargo run -p ql-cli -- check demo
cargo run -p ql-cli -- build demo
cargo run -p ql-cli -- run demo
cargo run -p ql-cli -- test demo
cargo run -q -p ql-cli -- check --sync-interfaces stdlib
```

## 仓库结构

- `crates/`: 编译器、CLI、project/workspace、runtime、LSP
- `docs/`: 文档站点与开发文档
- `fixtures/`: parser / codegen / diagnostics fixtures
- `tests/`: 集成与 FFI 测试输入
- `ramdon_tests/`: executable smoke corpus
- `examples/`: C / Rust FFI 示例
- `editors/vscode/qlang/`: VSCode client

## 文档开发

```powershell
cd docs
npm install
npm run dev
```
