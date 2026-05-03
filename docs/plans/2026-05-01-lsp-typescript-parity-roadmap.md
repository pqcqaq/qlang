# LSP TypeScript Parity Roadmap

## Goal

Qlang 的编辑器体验要接近 TypeScript 的日常可用基线：关键字有语法高亮和 hover，补全/跳转/格式化/code action 能在真实 workspace 中工作，并且所有 LSP 能力都由 `ql-analysis` 提供语义真相。

## Current status

已落地的用户可见能力：

- VSCode grammar 已覆盖当前 lexer keyword set
- keyword hover 和 keyword/snippet completion 已接入
- `completionItem/resolve` 已用于补齐文档和 detail
- semantic tokens 已支持 full / range
- signature help、inlay hints、folding range、selection range 已有基础实现
- document/range/on-type formatting 已复用 `ql fmt`
- `codeAction/resolve`、`source.organizeImports`、当前文档 code lens 已接入
- `textDocument/implementation` 已覆盖 same-file、workspace root 和 source-backed dependency 的当前保守切片
- `callHierarchy` 已覆盖同文件 function / method 直接调用的 prepare / incoming / outgoing calls，prepare 支持定义点和已解析调用点
- `typeHierarchy` 已覆盖同文件 trait / struct / enum / type alias 的 prepare / supertypes / subtypes，prepare 支持定义点和已解析类型引用

TypeScript reference 仍然只是能力形状参考。TypeScript 的关键字颜色主要来自编辑器 grammar，不是 LSP semantic tokens；Qlang 也保持 grammar + semantic tokens 双层策略。

## Remaining work

- 更完整的 source actions，例如 remove unused imports、fix all、match branch generation、trait stubs
- workspace / dependency call hierarchy 和 type hierarchy
- workspace-wide code lens 和 references index
- workspace file-operation hooks
- 更深的 AST/type-driven signature help、inlay hints、folding 和 selection
- 更完整的 workspace diagnostics pipeline

## Boundaries

- 不在 `ql-lsp` 里复制语义规则；缺 query 就先补 `ql-analysis`
- 不因为声明 capability 就声称 parity；每个能力必须有 request-level 回归
- open document source 优先于磁盘 `.qi`
- 同名本地依赖必须按 manifest identity 隔离

## Verification

```powershell
cargo test -p ql-lsp --test request_smoke
cargo test -p ql-lsp --test bridge
cd editors/vscode/qlang
npm run compile
```
