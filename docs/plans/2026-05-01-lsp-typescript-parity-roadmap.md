# LSP TypeScript Parity Roadmap

目标不是复制 TypeScript Server，而是把 Qlang 的日常编辑体验做到稳定、可测、可维护。

## 已落地

- TextMate grammar 覆盖当前 lexer keyword set。
- keyword hover、keyword/snippet completion、completionItem/resolve 已接入。
- semantic tokens 支持 full/range。
- formatting 支持 document/range/on-type，并复用 `ql fmt`。
- signature help、inlay hints、folding range、selection range 有基础实现。
- code action/resolve、organize imports、code lens 有基础实现。
- implementation、workspace symbol、保守 workspace rename 已覆盖当前 workspace/dependency 切片。
- call hierarchy 和 type hierarchy 已覆盖 same-file 基础路径。

## 下一步顺序

1. 先补 `ql-analysis` query，不在 LSP 层复制语义。
2. 扩 workspace diagnostics 和 package preflight。
3. 扩 dependency-backed definition/references/implementation/hierarchy。
4. 扩 source actions：remove unused imports、fix all、match branch、trait stubs。
5. 扩真实 workspace 的 code lens、file-operation hooks 和 request-level 回归。

## 边界

- TypeScript 只是能力形状参考，不是实现模板。
- 关键字颜色主要来自 grammar；semantic tokens 负责语义层。
- open document source 优先于磁盘 `.qi`。
- 声明 capability 前必须有 request-level 回归。

## 验证

```powershell
cargo test -p ql-lsp --test request_smoke
cargo test -p ql-lsp --test bridge
cd editors/vscode/qlang
npm run compile
npm run test:grammar
```
