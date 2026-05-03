# Qlang

Qlang 是一门编译型系统语言。这个仓库包含编译器、CLI、LSP、VSCode 插件、文档站和仓库内 `stdlib` workspace。

## 当前状态

- 可从源码构建并本地使用 `ql`、`qlsp` 和 VSIX。
- `ql check/build/run/test` 已支持单文件和项目入口。
- `ql project` 已支持本地 workspace、依赖、lock、interface 产物和 stdlib 模板。
- LSP 已覆盖基础编辑体验，但仍不是完整 TypeScript 级 workspace service。
- 还没有预编译 release、VSCode Marketplace 发布或 registry。

当前事实以代码和回归测试为准，文档只记录入口、边界和开发顺序。

## 快速开始

```powershell
cargo install --path crates/ql-cli
cargo install --path crates/ql-lsp

cd editors/vscode/qlang
npm install
npm run package:vsix
```

创建并验证项目：

```powershell
cargo run -q -p ql-cli -- project init D:\Projects\my-qlang-app --stdlib D:\Projects\language_q\stdlib
cargo run -q -p ql-cli -- check D:\Projects\my-qlang-app
cargo run -q -p ql-cli -- run D:\Projects\my-qlang-app
cargo run -q -p ql-cli -- test D:\Projects\my-qlang-app
```

验证仓库：

```powershell
cargo test
cd docs
npm install
npm run build
```

## 主要文档

- [当前支持基线](./docs/roadmap/current-supported-surface.md)
- [开发计划](./docs/roadmap/development-plan.md)
- [阶段总览](./docs/roadmap/phase-progress.md)
- [安装与版本配套](./docs/getting-started/install.md)
- [VSCode 插件](./docs/getting-started/vscode-extension.md)
- [Stdlib](./stdlib/README.md)

## 仓库结构

- `crates/`: 编译器、CLI、project/workspace、runtime、LSP
- `stdlib/`: 仓库内标准库 workspace
- `editors/vscode/qlang/`: VSCode thin client
- `fixtures/`、`tests/`、`ramdon_tests/`: 回归和 smoke 输入
- `docs/`: 文档站、路线图、设计稿
