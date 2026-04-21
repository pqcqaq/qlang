# VSCode 插件 `qlang`

仓库内置了一个最小 VSCode 插件工程：`editors/vscode/qlang`。

它只是 `qlsp` 的 thin client，职责只有两件事：

- 注册 `.ql` 语言
- 在 VSCode 里启动 `qlsp`

真正的语义边界仍以 `crates/ql-lsp` 和 [当前支持基线](/roadmap/current-supported-surface) 为准。

## 当前支持

- diagnostics
- hover
- definition / declaration / typeDefinition
- implementation
- references
- documentHighlight
- completion
- documentFormatting
- documentSymbol / workspaceSymbol
- semanticTokens
- codeAction（unresolved symbol auto-import / missing workspace dependency quick fix）
- conservative rename / narrow workspace rename

## 当前边界

- 当前最可靠的仍是 same-file 语义，以及 healthy package/workspace 下已经接通的 dependency-backed 导航与高亮。
- workspace/source-preferred navigation 已经落地，但还不是完整的 workspace-wide index。
- codeAction 当前只覆盖两类 quick fix：`unresolved value/type` 时补 `use ...`，以及显式 `use demo.xxx...` 指向未声明的 sibling workspace member 时补当前 package `qlang.toml` 的本地依赖；若 unresolved symbol 的候选本身就来自未声明的 sibling workspace member，则会同时补 import 和依赖。不含 match 分支补齐等更宽 refactor。
- 格式化当前只支持 parseable source 的整文档 `Format Document`；底层直接调用 `qfmt`，暂不支持 range formatting / on-type formatting。
- `Go to Implementation` 当前只覆盖 same-file trait/type surface：trait/type 返回当前文件里的 `impl` / `extend` block，trait method 返回匹配 impl method；暂不支持 workspace/dependency 级 implementation graph。
- rename 已开放 same-file，以及一批 source-backed dependency / workspace root 的保守 workspace rename；其余符号仍未开放更广 cross-file rename。
- parse-error 下只保留保守子集；当前已锁住的 rename slice 包括 `config.child()?.leaf().value` 这类 question-unwrapped method-result member field。
- 插件内置了最小 TextMate grammar fallback，但更细粒度高亮仍主要依赖 `qlsp` 的 semantic tokens。
- 还不提供 Marketplace 发布流，只提供本地 VSIX 打包。

## 构建与打包

先在仓库根目录构建 language server：

```powershell
cargo build -p ql-lsp
```

再构建插件：

```powershell
cd editors/vscode/qlang
npm install
npm run compile
```

打包 VSIX：

```powershell
npm run package:vsix
```

产物位置：

```text
editors/vscode/qlang/dist/qlang.vsix
```

安装：

```powershell
code --install-extension editors/vscode/qlang/dist/qlang.vsix
```

## 运行方式

1. 用 VSCode 打开 `editors/vscode/qlang`
2. 运行 `Run qlang` launch configuration
3. 在新的 Extension Development Host 里打开 Qlang 仓库或任意 `.ql` 工作区

插件按这个顺序寻找 `qlsp`：

1. `qlang.server.path`
2. `<repo>/target/debug/qlsp`
3. `<repo>/target/release/qlsp`
4. `PATH` 中的 `qlsp`

## 配置项

- `qlang.server.path`
- `qlang.server.args`

修改后插件会自动重启 language server；也可以手动执行 `Qlang: Restart Language Server`。
