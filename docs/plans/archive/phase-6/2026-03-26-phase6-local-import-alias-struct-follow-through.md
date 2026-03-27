# 2026-03-26 P6 Local Import Alias Struct Follow-through

在 same-file field query / rename 与 local import alias variant follow-through 落地之后，还残留了另一条很具体的断层：

- `Point { x: value }`、`Point { x }`、`match point { Point { x } => ... }` 已经能复用 local struct field truth surface
- 但如果源码先写 `use Point as P`，再写 `P { x: value }` / `P { x }` / `match point { P { x } => ... }`，field owner 仍然只会停在 import alias root
- 同样地，`ql-typeck` 在 struct literal root 与 pattern root 上，也还没有把这类 same-file alias 规范化回真正的本地 item

这一步的目标仍然很窄：把已经存在的 same-file struct-field / pattern-root 语义，继续接到一个更窄且可证明安全的 alias 表面上。

## 目标

- 支持同文件 `use Point as P` 这类 local import alias 指向本地 root struct item 时的 struct-field query / same-file rename follow-through
- 支持同文件单段 local import alias 指向本地 root item 时的 struct literal 字段检查，以及 struct / enum pattern root type checking
- 继续保持 `ql-analysis::QueryIndex` 和 `ql-typeck` 作为共享语义真值源
- 不引入 LSP 本地猜测逻辑

## 非目标

- foreign import alias 的 struct-field 语义
- multi-segment import path 的 deeper module follow-through
- import graph / package graph
- enum struct literal 的字段语义扩展
- cross-file rename / completion / references

## 设计

resolver 仍然只记录 root binding：

- `P` 继续解析成 source-backed `ImportBinding`
- 不改 `lookup_value_path` / `lookup_type_path`
- 不让 resolver 假装已经拥有 module graph

真正的 follow-through 发生在 `ql-typeck` 与 `ql-analysis`：

1. 只在 `ImportBinding.path` 恰好是单段路径时尝试继续
2. 只在这单段路径命中同文件 root item 时才继续
3. `ql-typeck` 会把这类 alias 规范化回本地 item，用于：
   - struct literal root 的字段检查
   - struct pattern root 的类型检查
   - tuple-/struct-variant pattern root 的类型检查
4. `ql-analysis` 只在 canonicalized item 确实是 struct 时，才继续把显式字段标签与 field-driven shorthand rename 映射回原 struct field
5. root token 仍然保留 import alias 自己的 source-backed symbol identity

这意味着：

- `P` 继续是 import alias
- `x` 这类显式字段标签才会继续是 struct field
- `P { x }` 里的 shorthand token 本身仍然保持 local/binding 语义，只在 field rename 时作为 rewrite site 被扩写
- foreign import、multi-segment import、deeper module graph 仍不会被误报成“已经支持”

## 代码落点

- `crates/ql-typeck/src/types.rs`
  - 复用 `local_item_for_import_binding`
- `crates/ql-typeck/src/typing.rs`
  - 新增 `item_id_for_value_resolution`
  - 新增 `item_id_for_type_resolution`
  - 扩展 `check_struct_literal`
  - 扩展 `pattern_root_ty`
  - 扩展 `tuple_struct_pattern_items`
  - 扩展 `struct_pattern_fields`
- `crates/ql-analysis/src/query.rs`
  - 新增 `struct_item_for_value_resolution`
  - 新增 `struct_item_for_type_resolution`
  - 新增 `local_struct_item_for_import_binding`
  - 扩展 struct literal / struct pattern 字段 owner 的取数

## 覆盖用例

- type checking
  - `accepts_struct_literals_through_same_file_import_aliases`
  - `reports_struct_literal_shape_errors_through_same_file_import_aliases`
  - `reports_pattern_root_type_mismatches_through_same_file_import_aliases`
  - `reports_variant_pattern_type_mismatches_through_same_file_import_aliases`
- analysis query / rename
  - `explicit_struct_field_labels_follow_same_file_import_alias_roots`
  - `field_rename_expands_shorthand_struct_sites_through_import_alias_paths`
- LSP bridge
  - `hover_and_definition_bridge_follow_struct_field_labels_through_import_alias_paths`
  - `rename_bridge_expands_shorthand_field_sites_through_import_alias_paths`

## 完成后的边界

当前 same-file query / rename / type checking 已额外覆盖：

- local import alias -> local struct item 的显式字段标签 query
- local import alias -> local struct item 的 field-driven shorthand rename rewrite
- local import alias -> local struct item 的 struct literal 字段检查
- local import alias -> local struct / enum item 的 pattern root type checking

当前仍明确未覆盖：

- foreign import alias struct-field semantics
- multi-segment import path deeper semantics
- enum struct literal field ownership through import alias
- import-graph / package-graph aware query
- cross-file rename / completion / references

## 验证

- `cargo fmt --all`
- `cargo test -p ql-typeck --test typing`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
- full `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`

## 下一步建议

- 继续沿 shared truth surface 收口 import/module deeper semantics，而不是把 alias 解释逻辑散到 LSP
- 如果后续要支持 enum struct literal field ownership，应先决定 variant field owner 的稳定 symbol model，再扩展当前 field surface
