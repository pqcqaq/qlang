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
- document highlight
- completion
- document symbol / workspace symbol
- semantic tokens
- conservative rename

真正的语义边界仍以 `crates/ql-lsp` 和 `docs/roadmap/current-supported-surface.md` 为准。

但这里要明确一件事：

- 插件声明了这些 LSP 能力，不等于它们已经在真实项目里做到了稳定可依赖。
- 当前最可靠的仍然是 diagnostics、same-file 语义，以及 healthy package/workspace 下已经接通的那部分 dependency-backed 导航/高亮。
- 这轮开始已经补上两条更接近真实项目的路径：workspace roots 驱动的保守 `workspace symbol` 搜索，以及 package/workspace import、当前文件里命中的 dependency value / enum variant / struct field / method member 的 `definition` / `declaration` / `references` 会优先跳到 workspace 内唯一可定位的源码定义，找不到唯一源码目标时再回退 `.qi`。
- 当前文件内的 symbol occurrence highlighting 也已经接上 `textDocument/documentHighlight`，会复用 same-file / package-aware references 面高亮当前文件里的定义和使用位。
- package-aware `semantic tokens` 现在也已经开始覆盖 imported dependency enum variant、显式 struct field label 与唯一 method member，因此真实项目里不再只剩 TextMate fallback 的基础着色。
- 但 project-scale 跳转、跨包导航、以及“像成熟语言插件那样稳定”的更完整高级高亮，仍然没有完全做实。
- 当前已经内置最小 TextMate grammar fallback；当 `qlsp` 没有返回足够的 semantic tokens 时，编辑器至少还有基础语法着色，但高亮质量仍然偏保守。

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

如果要直接打成可分发的 VSIX：

```powershell
cd editors/vscode/qlang
npm install
npm run package:vsix
```

产物会输出到：

```text
editors/vscode/qlang/dist/qlang.vsix
```

安装方式：

1. VSCode 命令面板执行 `Extensions: Install from VSIX...`
2. 或命令行执行：

```powershell
code --install-extension editors/vscode/qlang/dist/qlang.vsix
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
- 已提供本地 VSIX 打包流，但还不提供 Marketplace 发布流
- 已提供最小 TextMate grammar；更细粒度的高亮仍主要依赖 `qlsp` 的 semantic tokens
- 不扩大 `qlsp` 现有支持边界
- 不承诺当前已经具备成熟语言插件级别的跳转、高亮和重构体验
