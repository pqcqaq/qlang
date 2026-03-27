# Phase 6 Explicit Field Label Query Precision

Date: 2026-03-26

## Goal

在 same-file rename 脚手架已经落地之后，继续补齐字段语义面的真实缺口：让显式 struct literal / struct pattern 字段标签也进入统一的 field query surface。

这一步仍然不是直接开放 field rename，而是先把“哪些字段 token 现在真的有稳定语义 identity”做实。

## Problem

此前 field query 只覆盖：

- struct field declaration
- member access token，例如 `point.x`

但还缺两类真实字段使用位点：

- struct literal label，例如 `Point { x: value }`
- struct pattern label，例如 `Point { x: alias }`

如果这两类 token 不进统一 query surface，后续 field references / rename 的结果就天然不完整。

## Important Boundary

`Point { x }` 这类 shorthand token 不能和显式标签一视同仁。

原因是这个单 token 同时扮演两种角色：

- 字段标签
- 局部变量/绑定

现在如果强行把 shorthand token 也当成 field occurrence：

- hover/definition 很容易和 local/binding 语义打架
- rename 更容易生成错误编辑集

所以本次明确采用保守边界：

- 显式字段标签进入 field query surface
- shorthand token 继续落在 local/binding 语义上

## Implementation

为了让 query 层知道某个字段 token 是否来自 shorthand，`ql-hir` 现在给两类节点增加了 `is_shorthand`：

- `PatternField`
- `StructLiteralField`

HIR lowering 在 sugar normalization 时同步保留这个事实：

- `Point { x }` -> 会补出真实 binding/name expr，但 `is_shorthand = true`
- `Point { x: alias }` / `Point { x: value }` -> `is_shorthand = false`

`ql-analysis::QueryIndex` 随后只为 `is_shorthand = false` 的字段标签压入 field occurrence，并复用现有 `FieldTarget { item_id, field_index }` 与 `field_defs`。

这保持了两个关键性质：

- field declaration / member use / explicit label 共用同一份 `SymbolKey`
- shorthand token 不会因为补 query precision 而破坏 local/binding 查询行为

## Verification

本次新增回归：

- HIR 保留 struct pattern / struct literal 的 `is_shorthand`
- analysis：
  - explicit struct literal label -> field hover/definition/references
  - explicit struct pattern label -> field definition/references
  - shorthand struct literal token 继续命中 local symbol
- LSP bridge：
  - explicit struct literal label markdown hover

已验证命令：

- `cargo fmt --all`
- `cargo test -p ql-hir -p ql-analysis -p ql-lsp`
- `cargo clippy -p ql-hir -p ql-analysis -p ql-lsp --all-targets -- -D warnings`

## Deferred Work

本次仍刻意不做：

- field rename
- shorthand field rename
- method rename
- imported-struct field label precision
- cross-file field references / rename

下一步更合理的方向，是继续沿着同一条数据面补强：

1. method / ambiguous member 查询边界
2. imported struct / module-path deeper field precision
3. 在字段引用面真正完整之后，再讨论 whether field rename can be opened safely
