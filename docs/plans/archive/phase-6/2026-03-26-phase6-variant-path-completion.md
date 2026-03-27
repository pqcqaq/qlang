# P6 Variant Path Completion

## 背景

lexical-scope completion、稳定 receiver type 的 parsed member completion、same-file semantic tokens 都已经落地之后，Phase 6 在 completion 这条线上仍然有一个明显缺口：

- `Command.Retry(...)`、`Command.Config { ... }`、`Command.Retry(value)` 这类 parsed enum variant 路径在 query / hover / definition 上已经有稳定 variant 语义
- 但 completion 仍然只会把它当成“普通 member token”，而对象表达式 `Command` 并没有 receiver `Ty`
- 结果就是同一条 variant truth surface 已经存在，completion 却还没复用上

这一步的重点不是宣称“module-path completion 已完成”，而是把已经存在的 same-file enum variant 语义真正接进 completion 数据面。

## 目标

- 在 `ql-analysis` 中为 same-file parsed enum variant 路径提供 variant completion
- 保持 `ql-lsp` 只做桥接和前缀过滤
- 不引入 import-graph / cross-file / heuristic completion

## 明确不做

- ambiguous member completion
- parse-error tolerant dot-trigger completion
- imported alias 到 foreign enum 的 variant completion
- deeper module-path / import-graph completion
- method rename 或 cross-file rename

## 设计

现有 `ExprKind::Member` completion site 已经存在。

之前的逻辑只做一件事：

1. 读取对象表达式的 `Ty`
2. 只有 `Ty::Item` 时，才继续收集 struct field / impl method / extend method completion

这对实例成员是对的，但对 `Command.Retry` 这种 enum root + variant tail 路径不成立，因为：

- 对象 `Command` 在 value namespace 中解析成 `ValueResolution::Item`
- 它不是一个“实例值”，因此不会有 receiver `Ty::Item`
- 但我们已经知道它对应一个本地 enum item

因此这一步把 member completion site 的取数逻辑改成两段：

1. 先尝试原有的 receiver-type member completion
2. 如果没有结果，再检查对象表达式是否直接解析成 `ValueResolution::Item`
3. 只有该 item 是 enum，才暴露其 variant completion items

这样可以保证：

- 现有实例成员 completion 逻辑不变
- parsed enum variant path completion 复用同一份 variant definition truth surface
- imported alias / foreign item / deeper module graph 不会被误报成“已支持”

## 回归测试

- `ql-analysis`
  - `completion_queries_follow_variant_candidates_on_enum_item_roots`
  - `completion_queries_follow_variant_candidates_in_struct_literal_paths`
  - `completion_queries_follow_variant_candidates_in_pattern_paths`
- `ql-lsp`
  - `completion_bridge_filters_variant_candidates_by_prefix`
  - `completion_bridge_filters_struct_variant_candidates_by_prefix`

## 结果边界

这一步之后，same-file completion 的当前覆盖面变为：

- lexical scope value/type completion
- 稳定 receiver type 的 parsed member completion
- same-file parsed enum variant path completion

仍然明确未完成：

- ambiguous member completion
- parse-error tolerant completion
- import-graph / deeper module-path completion
- cross-file / project-indexed completion
