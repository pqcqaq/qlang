# 2026-03-26 P6 Local Import Alias Variant Follow-through

在 same-file parsed enum variant path completion 落地之后，还残留了一个很具体的体验缺口：

- `Command.Re(...)`、`Command.Con { ... }`、`Command.Re(value)` 已经能复用 enum variant truth surface
- 但如果源码先写 `use Command as Cmd`，再写 `Cmd.Re(...)` / `Cmd.Con { ... }` / `Cmd.Re(value)`，root token 只会被当成 import alias
- 结果是尾段 variant token 的 hover / definition / references / completion 还没有继续跟进

这一步的目标不是宣称“import graph 已完成”，而是把已经存在的 same-file enum variant 语义，继续接到一个更窄的已知安全表面上。

## 目标

- 支持同文件 `use Command as Cmd` 这类 local import alias 指向本地根 enum item 时的 variant query / completion / same-file rename / semantic-token follow-through
- 让 `Cmd.Retry(...)`、`Cmd.Config { ... }`、`Cmd.Retry(value)` 这些尾段 token 继续命中 variant symbol
- 继续保持 `ql-analysis::QueryIndex` 作为唯一共享语义真值源
- 不引入 LSP 本地猜测逻辑

## 非目标

- foreign import alias 的 variant 语义
- multi-segment import path 的 deeper module follow-through
- import graph / package graph
- cross-file completion / references / rename
- parse-error tolerant member completion

## 设计

当前 resolver 仍然只记录 root binding：

- `Cmd` 仍然解析成 source-backed `ImportBinding`
- 不改 `lookup_value_path` / `lookup_type_path`
- 不让 resolver 假装已经拥有 module graph

真正的 follow-through 发生在 `ql-analysis`：

1. 只在 `ImportBinding.path` 恰好是单段路径时尝试继续
2. 只在这单段路径命中同文件根 enum item 时才继续
3. root token 仍然保留 import alias 自己的 symbol identity
4. variant tail token / completion item / rename occurrence / semantic token 才继续复用既有 enum variant truth surface

这意味着：

- `Cmd` 继续是 import alias
- `Retry` / `Config` 才会继续是 variant
- foreign import、multi-segment import、deeper module graph 仍不会被误报成“已经支持”

## 代码落点

- `crates/ql-analysis/src/query.rs`
  - 新增 `enum_item_for_value_resolution`
  - 新增 `enum_item_for_type_resolution`
  - 新增 `local_enum_item_for_import_binding`
  - 扩展 variant path completion site 的取数
  - 扩展 variant token occurrence 的取数

## 覆盖用例

- analysis query
  - `variant_queries_follow_same_file_import_alias_roots`
- analysis rename / semantic tokens
  - `rename_queries_follow_variant_symbols_through_import_alias_paths`
  - `semantic_tokens_follow_import_alias_variant_surface`
- analysis completion
  - `completion_queries_follow_variant_candidates_on_same_file_import_alias_roots`
  - `completion_queries_follow_variant_candidates_in_import_alias_struct_literal_paths`
  - `completion_queries_follow_variant_candidates_in_import_alias_pattern_paths`
- LSP bridge
  - `hover_bridge_renders_markdown_for_import_alias_variant_symbols`
  - `definition_bridge_returns_variant_locations_through_import_alias_paths`
  - `references_bridge_follow_variant_symbols_through_import_alias_paths`
  - `completion_bridge_filters_import_alias_variant_candidates_by_prefix`
  - `completion_bridge_filters_import_alias_struct_variant_candidates_by_prefix`
  - `rename_bridge_supports_variants_through_import_alias_paths`
  - `semantic_tokens_bridge_maps_import_alias_variant_surface`

## 完成后的边界

当前 same-file completion / query / rename / semantic tokens 已覆盖：

- lexical scope value/type completion
- 稳定 receiver type 的 parsed member completion
- same-file parsed enum variant path completion
- local import alias -> local enum item 的 variant follow-through

当前仍明确未覆盖：

- foreign import alias variant semantics
- import-graph / deeper module-path completion
- ambiguous member completion
- parse-error tolerant completion
- cross-file / project-indexed completion

## 验证

- `cargo fmt --all`
- `cargo test -p ql-analysis -p ql-lsp`
- full `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`

## 下一步建议

- 优先继续把 query boundary 写清楚，再考虑 import/module deeper semantics
- 如果继续做 completion，应该先决定 multi-segment import path / module-path 的真值来源，而不是把 LSP 变成临时解释器
