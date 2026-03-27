# P6 Semantic Tokens

## 背景

在 same-file completion 已经落地之后，`qlsp` 仍然缺一个编辑器每天都会直接感知的基础能力：semantic tokens。

如果这一步直接在 LSP 层按词法或字符串规则补一套高亮分类，后面很容易出现三个问题：

- hover / definition / references / rename / completion 与高亮来自不同真相源
- ambiguous member、parse error token、import alias 等边界会出现前后不一致
- 后续继续扩 module-path、cross-file index 或 rename 时，还得回头重做 semantic token 数据面

更稳的做法，是继续沿着已经建立好的 `ql-analysis::QueryIndex` 往前走。

## 目标

- 在 `ql-analysis` 中导出 same-file source-backed semantic token occurrence
- 在 `ql-lsp` 中实现 `textDocument/semanticTokens/full`
- 让 semantic tokens 与现有 hover / definition / references / completion / rename 共用同一份语义索引

## 明确不做

- ambiguous / unresolved token 的语义高亮
- parse-error tolerant semantic token 猜测
- range semantic tokens
- delta semantic tokens
- cross-file / project-indexed semantic classification
- method rename 或更广义 rename 能力扩张

## 设计

`QueryIndex` 已经持有统一的 source-backed occurrence：

- declaration occurrence
- resolved use occurrence
- field / method / variant 等精确 token occurrence

因此 semantic tokens 不需要新建另一份“高亮专用索引”，而是直接复用 occurrence：

1. 遍历 occurrence，提取 `span + SymbolKind`
2. 按 `(start, end, kind)` 排序
3. 对完全重复的 token 去重
4. 在 LSP bridge 中把 `SymbolKind` 映射到固定 legend
5. 再按 LSP 协议编码成 delta line / delta start

这条路径的核心收益是：

- 只要某个 token 已经进入统一 query surface，它就天然能进入 semantic tokens
- 如果某个 token 还没有稳定语义 identity，它也不会被伪造成“看似正确”的高亮结果
- 后续继续扩展 query surface 时，semantic tokens 会自然跟着变完整，而不是单独维护

## 实现边界

当前 semantic tokens 是保守的 same-file 版本：

- 只覆盖当前已有 source-backed symbol
- 只提供 `textDocument/semanticTokens/full`
- 不在 LSP 层补任何 ad-hoc semantic heuristic

这意味着当前已经可稳定高亮：

- import alias
- type / struct / enum / trait / type alias
- generic parameter
- function / method
- parameter / local / receiver
- field / enum variant

但以下仍故意不宣称完成：

- ambiguous member token
- unresolved / parse-error token
- module-path deeper semantics
- cross-file semantic classification

## 回归测试

- `ql-analysis`：same-file semantic token occurrence 投影
- `ql-lsp`：legend 与 delta 编码桥接

## 结果

这一步完成后，Phase 6 的当前状态变为：

- hover / definition / references / rename / completion / semantic tokens 全部复用统一 analysis truth surface
- LSP 仍然保持薄桥接，不复制编译器语义
- 未完成项继续明确收敛在 ambiguous member、parse-error tolerant completion、method rename 和 cross-file/project indexing 上
