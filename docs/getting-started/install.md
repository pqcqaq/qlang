# 安装与版本配套

当前还没有预编译 release、包管理器分发或 VSCode Marketplace 发布流。

可用的“安装模式”仍然是：从同一份源码 checkout 构建一套版本匹配的 `ql`、`qlsp` 和 VSIX。

## 推荐安装流程

先安装 CLI 和 language server：

```powershell
cargo install --path crates/ql-cli
cargo install --path crates/ql-lsp
```

再构建 VSCode 插件：

```powershell
cd editors/vscode/qlang
npm install
npm run package:vsix
```

VSIX 会输出到：

```text
editors/vscode/qlang/dist/qlang-<package.json version>.vsix
```

安装 VSIX：

```powershell
code --install-extension editors/vscode/qlang/dist/qlang-<package.json version>.vsix
```

## 版本检查

安装后先确认 CLI 和 LSP 来自同一版本：

```powershell
ql --version
qlsp --version
```

当前仓库要求扩展和 `qlsp` 尽量保持同版本。

- `qlang` 扩展启动后会读取 LSP `serverInfo.version`
- 如果扩展版本和 `qlsp` 版本不一致，会直接弹出 warning
- 这时应重新构建一套匹配产物，或用 `qlang.server.path` 显式指向正确的 `qlsp`

## 两种使用模式

### 安装模式

- 适合日常使用
- `ql` / `qlsp` 来自 `cargo install`
- VSCode 使用打包好的 VSIX
- 若 `PATH` 里存在多个 `qlsp`，建议显式设置 `qlang.server.path`

### 仓库开发模式

- 适合改 `crates/ql-lsp` 或扩展前端
- 扩展默认优先查找 `<repo>/target/debug/qlsp`、`<repo>/target/release/qlsp`
- 直接打开 `editors/vscode/qlang`，运行 `Run qlang` 即可

开发模式细节见 [VSCode 插件](/getting-started/vscode-extension)。
