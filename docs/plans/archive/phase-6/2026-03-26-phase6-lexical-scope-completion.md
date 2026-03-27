# Phase 6 Lexical-Scope Completion

## 背景

在 same-file references / rename 已经落地之后，`qlsp` 仍缺一个最基础但真实可用的编辑体验能力：completion。

但这里不能直接跳去做“看起来很聪明”的全量补全。原因很简单：

- 当前 query surface 虽然已经覆盖 root binding、field、variant、唯一 method member
- 但 member completion、ambiguous member、module-path deeper semantics 仍未完整
- 如果在 LSP 层临时手搓一套 completion heuristic，后面 method rename / cross-file rename / semantic tokens 很容易重复返工

所以这一刀只做一个保守但架构正确的切片：

- 基于 `ql-resolve` 的 lexical scope
- 基于 `ql-analysis` 已有的 `SymbolData`
- 提供 same-file value/type completion
- 不伪造成员补全或跨文件补全

## 目标

本次切片明确要完成：

- `ql-analysis::completions_at(offset)`
- `qlsp` 的 `textDocument/completion`
- value/type 两类 lexical-scope completion
- shadowing 正确
- import / builtin / generic / item / local / param / self 等符号继续复用同一份语义 identity

本次切片明确不做：

- member completion
- ambiguous member completion
- parse-error tolerant completion
- cross-file / package-indexed completion
- method rename

## 设计

### 1. Completion 仍然属于 query 层，不属于 LSP

LSP 只应该做协议桥接，不应该拥有“当前位置能看见什么符号”的语义判断。

因此 `ql-analysis` 新增 completion query，而 `ql-lsp` 只负责：

- `Position -> offset`
- completion candidate -> LSP `CompletionItem`
- 源码前缀过滤
- text edit range 生成

### 2. 复用 resolver scope graph

`ql-resolve` 已经保留了：

- item scope
- function scope
- block scope
- expr scope
- pattern scope
- type scope
- parent scope graph

因此 completion 不需要重新遍历 resolver 算法，只需要在 query build 阶段把这些 scope 变成可查询索引。

### 3. Completion site 与 visible binding 分离

`QueryIndex` 新增两类内部数据：

- `CompletionSite { span, scope, namespace }`
- `CompletionScope { parent, value_bindings, type_bindings }`

其中：

- site 负责回答“当前位置属于哪个 lexical context”
- scope 负责回答“这个 scope 里有哪些符号”

查询时先选最窄 site，再沿 parent scope 向外收集 binding，并按名字去重，从而保证 shadowing 语义正确。

## 已落地结果

- `ql-analysis` 新增 `CompletionItem`
- `Analysis` 新增 `completions_at(offset)`
- `QueryIndex` 现在会额外构建 same-file lexical-scope completion 索引
- `qlsp` 新增 `textDocument/completion`
- LSP bridge 会把 completion 候选转为 `CompletionItem`
- 桥接层会按当前位置源码前缀过滤，并生成统一 replacement range

## 当前边界

这不是“completion 已完成”，只是第一条正确的数据面。

当前仍故意保守：

- 只有 same-file lexical scope
- 只有 value/type namespace
- 不宣称 `obj.` member completion 已完成
- 不处理 parse-error / incomplete token 的高级恢复
- 不做 project/package 级索引

## 回归测试

新增回归覆盖：

- analysis：value scope completion 的 shadowing 与可见性
- analysis：type context completion 只返回 type namespace 候选
- LSP：value completion 的前缀过滤与 text edit
- LSP：type completion 的 candidate kind 与 text edit

## 后续最合理的下一步

在这个切片之后，更稳的方向是：

1. 继续把 completion 从 lexical scope 扩到 member / module-path
2. 继续把 same-file rename 扩到 method，但前提是先解决 ambiguous method completeness
3. 在 query index 之上补 project/package indexing，再讨论 cross-file rename / completion
