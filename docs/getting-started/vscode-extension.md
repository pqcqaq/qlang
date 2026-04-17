# VSCode 插件 `qlang`

这份仓库现在已经内置了一个最小 VSCode 插件项目：`editors/vscode/qlang`。

它不是新的语义实现，只是现有 `qlsp` 的 thin client，负责两件事：

- 把 `.ql` 文件注册成 `qlang` 语言
- 在 VSCode 里启动 `qlsp` 并接通语言服务

## 当前范围

这个插件当前只做最小可用集，不额外复制语义逻辑：

- diagnostics
- hover
- definition / declaration / type definition
- references
- completion
- document symbol / workspace symbol
- semantic tokens
- conservative rename

真正的语义边界仍以 `crates/ql-lsp` 和 `docs/roadmap/current-supported-surface.md` 为准。

## 先决条件

先在仓库根目录构建 language server：

```powershell
cargo build -p ql-lsp
```

然后安装并编译 VSCode 插件：

```powershell
cd editors/vscode/qlang
npm install
npm run compile
```

## 在 VSCode 里跑起来

最直接的开发方式：

1. 用 VSCode 打开 `editors/vscode/qlang`
2. 运行 `Run qlang` launch configuration
3. 在新的 Extension Development Host 里打开 Qlang 仓库或任意 `.ql` 工作区

插件会按这个顺序寻找 `qlsp`：

1. `qlang.server.path`
2. `<repo>/target/debug/qlsp`
3. `<repo>/target/release/qlsp`
4. `PATH` 里的 `qlsp`

## 可配置项

- `qlang.server.path`
  用绝对路径或相对当前 workspace 的路径显式指定 `qlsp`
- `qlang.server.args`
  给 `qlsp` 追加额外命令行参数

改动这些设置后，插件会自动重启 language server；也可以手动执行命令 `Qlang: Restart Language Server`。

## 当前不做

- 不内置 `qlsp` 二进制
- 不提供 Marketplace 发布流
- 不提供 TextMate grammar；当前高亮主要来自 `qlsp` 的 semantic tokens
- 不扩大 `qlsp` 现有支持边界
