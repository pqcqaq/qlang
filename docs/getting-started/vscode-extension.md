# VSCode 插件 `qlang`

`editors/vscode/qlang` 是 VSCode thin client。

它负责：

- 注册 `.ql` 语言。
- 启动 `qlsp`。
- 检查扩展版本和 `qlsp` 版本是否匹配。

语义能力来自 `crates/ql-lsp` 和 [当前支持基线](/roadmap/current-supported-surface)。

## 当前能力

- diagnostics、hover、keyword hover
- definition、declaration、typeDefinition、implementation
- references、documentHighlight、documentSymbol、workspaceSymbol
- completion、completionItem/resolve、signatureHelp、inlayHint
- foldingRange、selectionRange
- semanticTokens full/range
- document/range/on-type formatting
- codeAction/resolve、organize imports、codeLens
- same-file callHierarchy/typeHierarchy
- same-file rename 和保守 workspace rename

## 边界

- 没有 bundled `qlsp`。
- 没有 Marketplace 发布流。
- diagnostics 不是 workspace-wide push。
- organize imports 只处理连续顶层 `use` block。
- hierarchy、rename、references、code action 仍是保守 workspace 切片。

## 开发模式

```powershell
cargo build -p ql-lsp

cd editors/vscode/qlang
npm install
npm run compile
npm run test:grammar
```

然后在 VSCode 打开 `editors/vscode/qlang`，运行 `Run qlang`。

`qlsp` 查找顺序：

1. `qlang.server.path`
2. `<repo>/target/debug/qlsp`
3. `<repo>/target/release/qlsp`
4. `PATH` 中的 `qlsp`

## 配置

- `qlang.server.path`: 显式指定 `qlsp`
- `qlang.server.args`: 传给 `qlsp` 的额外参数

修改配置后扩展会重启 language server，也可以手动执行 `Qlang: Restart Language Server`。
