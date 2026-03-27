# 2026-03-26 P6 Method Rename

在 member query precision、same-file rename 脚手架、member completion 都落地之后，Phase 6 还剩一个很明显的不一致点：

- 唯一 method candidate 已经有 source-backed hover / definition / references
- 唯一 method candidate 已经能驱动 member completion
- 但 same-file rename 仍然把 method 一刀切排除在外

这一步的目标不是宣称“所有 method rename 都已完成”，而是把已经进入统一 query surface 的唯一 method symbol，保守地接进同文件 rename。

## 目标

- 开放唯一 method candidate 的 same-file rename
- 让 method declaration token 与唯一解析到的 member use token 共享同一个 rename identity
- 继续复用 `ql-analysis::QueryIndex`，不在 LSP 层补语义

## 非目标

- ambiguous method/member surface rename
- receiver `self` rename
- builtin type rename
- cross-file rename
- parse-error tolerant member rename

## 设计

当前 method symbol 已经具备稳定 identity：

- `define_method_site` 会用 `SymbolKey::Method(MethodTarget)` 建 declaration occurrence
- 唯一解析成功的 member use 会通过 `typeck.member_target(expr_id)` 命中同一个 `MethodTarget`
- `references_at` 已经能按这条 key 聚合同文件 occurrence

因此 rename 只需要：

1. 把 `SymbolKind::Method` 加入 `supports_same_file_rename`
2. 继续复用已有 `prepare_rename_at` / `rename_at`
3. 让 ambiguous member surface 保持关闭，因为它本来就没有稳定 `MethodTarget`

## 覆盖用例

- analysis
  - `rename_queries_follow_unique_method_symbols`
  - `rename_queries_keep_ambiguous_method_surfaces_closed`
- LSP bridge
  - `rename_bridge_supports_unique_method_symbols`
  - `rename_bridge_keeps_ambiguous_method_surfaces_closed`

## 完成后的边界

当前 same-file rename 已覆盖：

- function / const / static
- struct / enum / variant / trait / type alias
- import
- field
- method（仅唯一 candidate）
- local / parameter / generic

当前仍明确未覆盖：

- ambiguous method/member rename
- receiver `self`
- builtin type
- 从 shorthand field token 本身发起的 rename
- cross-file rename

## 验证

- `cargo fmt --all`
- `cargo test -p ql-analysis -p ql-lsp`
- full `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`

## 下一步建议

- 如果继续扩 rename，不应直接跳到 cross-file
- 更合理的顺序是先把 ambiguous member / module-path 的共享 truth surface 设计清楚，再考虑更大范围 rename
