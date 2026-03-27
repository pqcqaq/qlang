# Phase 6 Same-file Rename Scaffolding

Date: 2026-03-26

## Goal

在已经落地 same-file references、member query precision、variant query precision 之后，把 `rename` 作为一条真实可用但边界保守的能力接入 `ql-analysis` 和 `qlsp`。

这一步的重点不是“尽快宣称 rename 已完成”，而是：

- 继续复用 `QueryIndex` 作为唯一语义真相源
- 明确哪些 symbol kind 现在已经安全到可以改源码
- 在 analysis 层先把 rename 的数据结构、校验和编辑集边界做稳
- 让 LSP 只做协议桥接，不额外复制一遍语义遍历

## Scope

本次只承诺同文件 rename。

当前开放的 symbol kind：

- `Function`
- `Const`
- `Static`
- `Struct`
- `Enum`
- `Variant`
- `Trait`
- `TypeAlias`
- `Local`
- `Parameter`
- `Generic`

当前明确不开放：

- `Field`
- `Method`
- `SelfParameter`
- `Import`
- `BuiltinType`
- cross-file symbol

原因很直接：这些未开放对象要么当前引用面还不完整，要么还缺项目级索引。现在强行开放，只会把错误编辑集固化到用户体验里。

## Analysis Design

`ql-analysis` 新增三组类型：

- `RenameTarget`
- `RenameEdit`
- `RenameResult`

并新增：

- `prepare_rename_at(offset)`
- `rename_at(offset, new_name)`

算法保持和 `references_at` 同源：

1. 先用 `occurrence_at(offset)` 找当前最窄命中 symbol。
2. 按 `SymbolKind` 过滤掉暂不安全的 rename 对象。
3. 用 lexer helper 校验新名字。
4. 裸关键字直接拒绝；如果用户确实想用关键字，必须显式写成转义标识符，例如 `` `match` ``。
5. 通过同一个 `SymbolKey` 收集当前文件内的 occurrence。
6. 按源码顺序排序并按 span 去重后产出 text edits。

这样做的结果是：

- hover / definition / references / rename 共用同一份语义索引
- rename 不会退化成“按同名字符串全局替换”
- 后续扩到 cross-file rename 时，也仍然可以从同一条 identity 链路往上加 project index

## LSP Design

`qlsp` 新增：

- `textDocument/prepareRename`
- `textDocument/rename`

桥接层只负责：

- `Position -> byte offset`
- `Span -> Range`
- `RenameResult -> WorkspaceEdit`

`prepareRename` 的 placeholder 直接取当前 occurrence 的源码切片，而不是重新拼接 symbol name。这样可以保留真实 token 形态，并避免后续 declaration/use token 形态不一致时桥接层自作主张。

## Verification

本次新增回归覆盖：

- analysis rename target 发现
- analysis rename edit 集合
- analysis 非法标识符 / 关键字校验
- analysis unsupported symbol kind 过滤
- LSP prepare rename placeholder/range
- LSP rename -> same-file `WorkspaceEdit`
- LSP 非法 rename 名字错误上抛

已验证命令：

- `cargo fmt --all`
- `cargo test -p ql-analysis -p ql-lsp`
- `cargo clippy -p ql-analysis -p ql-lsp --all-targets -- -D warnings`

## Deferred Work

本次刻意不做：

- field label rename
- method rename
- receiver rename
- import alias rename
- builtin type rename
- module-path deeper rename
- cross-file rename
- project/package 级 rename graph

这些能力都需要更完整的 member / module-path / package indexing 事实面。现在不应该为了追求“功能看起来更多”而把 query 边界提前做坏。
