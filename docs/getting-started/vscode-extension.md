# VSCode 插件 `qlang`

`editors/vscode/qlang` 是仓库内置的 VSCode thin client。

它只做三件事：

- 注册 `.ql` 语言
- 启动现有的 `qlsp`
- 在扩展版本和 `qlsp` 版本不一致时直接提示

真正的语义边界仍以 `crates/ql-lsp` 和 [当前支持基线](/roadmap/current-supported-surface) 为准。

## 当前支持

- diagnostics（当前文档 parser / semantic + package preflight）
- hover
- keyword hover
- definition / declaration / typeDefinition
- implementation
- references
- documentHighlight
- completion
- completionItem/resolve（当前用于补齐缺失的 keyword / symbol 文档和 detail）
- signatureHelp
- inlayHint（当前覆盖稳定的局部变量推断类型提示）
- foldingRange
- selectionRange
- documentFormatting / rangeFormatting / onTypeFormatting
- documentSymbol / workspaceSymbol
- semanticTokens/full / semanticTokens/range（range 与 full 共用 workspace/open-doc token 路径）
- codeAction / codeAction resolve（unresolved symbol auto-import / missing workspace dependency quick fix / source.organizeImports）
- codeLens / codeLens resolve（当前文档 references / implementations 入口）
- conservative rename / narrow workspace rename

## 当前边界

- 这仍然不是完整 workspace-wide index。
- diagnostics 仍只发布当前打开文档；当前 buffer 干净时才补 manifest / interface preflight 错误，不做 workspace-wide diagnostics 推送。
- formatting 复用 `ql fmt`，支持 parseable source 的整文档格式化、能安全落在请求范围内的逐行 range formatting，以及只作用于触发行的 on-type formatting；无法安全拆成局部行 edit 的格式化差异会保守返回空 edit。
- `source.organizeImports` 当前只对连续顶层 `use ...` block 做排序和去重；不会重写分组 import、删除未使用 import 或跨文件推断。
- `codeLens` 当前只覆盖可解析的当前文档；它基于现有 `references` / `implementation` 查询给 document symbols 生成 references / implementations 入口，不是完整 workspace-wide lens service。
- `signatureHelp`、`inlayHint`、`foldingRange`、`selectionRange` 已有可用基础实现，但不是 TypeScript 那种完整 workspace service。
- rename 只开放 same-file 和一部分 source-backed dependency / workspace root 保守路径。
- 插件不自带 `qlsp` 二进制；没有 Marketplace 发布流。

## 仓库开发模式

先在仓库根目录构建 language server：

```powershell
cargo build -p ql-lsp
```

再构建扩展：

```powershell
cd editors/vscode/qlang
npm install
npm run compile
npm run test:grammar
```

然后在 VSCode 打开 `editors/vscode/qlang`，运行 `Run qlang` launch configuration。

扩展按这个顺序寻找 `qlsp`：

1. `qlang.server.path`
2. `<repo>/target/debug/qlsp`
3. `<repo>/target/release/qlsp`
4. `PATH` 中的 `qlsp`

## 安装模式

如果只是日常使用，不调试扩展本身，先看 [安装与版本配套](/getting-started/install)。

VSIX 打包命令：

```powershell
cd editors/vscode/qlang
npm install
npm run package:vsix
```

产物位置：

```text
editors/vscode/qlang/dist/qlang-<package.json version>.vsix
```

安装：

```powershell
code --install-extension editors/vscode/qlang/dist/qlang-<package.json version>.vsix
```

## 版本匹配

扩展启动后会读取 `qlsp` 返回的 `serverInfo.version`。

- 如果扩展版本和 `qlsp` 版本一致，继续正常工作
- 如果版本不一致，扩展会给出 warning，并提示你打开 README 或设置 `qlang.server.path`

最直接的检查方式仍然是：

```powershell
qlsp --version
```

## 配置项

- `qlang.server.path`: 显式指定要启动的 `qlsp`
- `qlang.server.args`: 额外传给 `qlsp` 的命令行参数

修改后扩展会自动重启 language server；也可以手动执行 `Qlang: Restart Language Server`。
