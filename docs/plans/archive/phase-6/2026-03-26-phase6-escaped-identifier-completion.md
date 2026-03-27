# Phase 6 Escaped-Identifier Completion

## 背景

Phase 6 已经把 same-file completion 接到统一的 `ql-analysis::QueryIndex` 上，但当前 completion 还有一个 correctness 缺口：

- Qlang 支持转义标识符，例如 `` `type` ``
- parser / HIR / query 层都会把它们归一化成语义名字 `type`
- completion 目前直接把这个语义名字当成插入文本返回给 LSP

结果就是编辑器会在需要转义的位置给出非法文本：

- label 还是对的
- 但 text edit 会写入 `type`
- 实际合法源码应当写入 `` `type` ``

这不是“新功能未做”，而是当前 same-file completion 在已有语义面上的 correctness 问题。

## 目标

- 让 `ql-analysis` completion 候选区分“语义显示名”和“源码插入文本”
- 让 `ql-lsp` completion 在 keyword / escaped-identifier 场景下生成合法 text edit
- 保持 hover / definition / references / rename / semantic tokens 的 symbol identity 不变

## 明确不做

- parse-error tolerant escaped identifier completion
- cross-file completion
- module-path / import-graph completion
- rename / references / semantic tokens 的额外语义扩张

## 设计

`CompletionItem` 在现有字段之外增加 `insert_text`：

- `label` 继续表示语义名字，保持 query 层与测试读起来直接
- `insert_text` 表示实际应写回源码的文本

规则保持保守：

1. 普通标识符：`insert_text == label`
2. 关键字名字：`insert_text = escaped(label)`

LSP bridge 的调整也保持最小：

1. 前缀过滤同时匹配 `label` 和 `insert_text`
2. `textEdit.newText` 使用 `insert_text`
3. 其余展示数据不变

## 回归

- `ql-analysis`：lexical completion 对转义参数返回合法 `insert_text`
- `ql-lsp`：在 `` `type` `` 这类已解析 token 上，completion 仍能命中并生成合法 text edit

## 边界

这一步不会让 completion 变得“更聪明”，只会让它在已支持的 same-file surface 上更正确。
