# 安装与版本配套

当前没有预编译 release、包管理器分发或 VSCode Marketplace 发布。

可用方式：从同一份源码 checkout 构建匹配版本的 `ql`、`qlsp` 和 VSIX。

## 安装

```powershell
cargo install --path crates/ql-cli
cargo install --path crates/ql-lsp

cd editors/vscode/qlang
npm install
npm run package:vsix
code --install-extension dist/qlang-<package.json version>.vsix
```

确认版本：

```powershell
ql --version
qlsp --version
```

扩展启动后会读取 LSP `serverInfo.version`。扩展版本和 `qlsp` 不一致时会提示 warning。

## 使用模式

| 模式 | 用途 | 说明 |
| --- | --- | --- |
| 安装模式 | 日常试用 | `ql`/`qlsp` 来自 `cargo install`，VSCode 使用打包 VSIX |
| 仓库开发模式 | 改 LSP 或扩展 | 打开 `editors/vscode/qlang`，运行 `Run qlang` |

如果 `PATH` 中存在多个 `qlsp`，用 `qlang.server.path` 指向正确二进制。
