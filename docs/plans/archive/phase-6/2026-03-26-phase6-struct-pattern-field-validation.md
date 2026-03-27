# 2026-03-26 P6 Struct Pattern Field Validation

在 first-pass typing 已经支持 struct literal 字段检查之后，struct pattern 还残留了一个明显缺口：

- `Point { missing: value }` 这类 struct literal 已经会报 `unknown field`
- 但 `match point { Point { missing } => ... }` 这类 struct pattern 仍然只是把绑定类型降成 `unknown`
- 这会让编译器漏掉真实错误，也会让 alias 路径上的 pattern typing 看起来比 literal checking 更弱

这一步的目标是把 struct pattern 的未知字段错误补齐，并继续复用已经存在的 local import alias canonicalization。

## 目标

- 对 direct struct pattern 的未知字段给出稳定诊断
- 对 same-file local import alias -> local struct item 的 struct pattern 未知字段给出同样诊断
- 对 same-file local import alias -> local enum item 的 struct-variant pattern 未知字段给出同样诊断
- 不改变 shorthand token 的 query 归属

## 非目标

- pattern 缺失字段强制报错
- module graph / foreign import alias 语义
- enum struct literal field ownership
- hover / rename 的语义归属调整

## 设计

`bind_pattern` 在 `PatternKind::Struct` 分支里已经会拿到 `struct_pattern_fields(pattern_id)`：

1. 如果字段存在，继续沿原逻辑把 pattern 绑定到对应 field type
2. 如果字段不存在，但 root pattern 已经成功拿到 field surface，就补一条 `unknown field ... in struct pattern` 诊断
3. 缺失字段的子 pattern 仍然绑定为 `Ty::Unknown`，避免后续类型遍历崩掉
4. 如果 root pattern 自身都还没有稳定 field surface，则保持保守，不额外伪造未知字段诊断

这样可以让 direct path、same-file alias -> local struct item、same-file alias -> local enum item struct variant 都复用同一条 truth path，而不需要给 alias 再写一套分叉诊断逻辑。

## 覆盖用例

- `accepts_struct_patterns_through_same_file_import_aliases`
- `accepts_variant_struct_patterns_through_same_file_import_aliases`
- `reports_unknown_struct_pattern_fields`
- `reports_unknown_struct_pattern_fields_through_same_file_import_aliases`
- `reports_unknown_variant_struct_pattern_fields_through_same_file_import_aliases`

## 完成后的边界

当前 first-pass typing 已额外覆盖：

- struct pattern unknown-field validation
- same-file local import alias -> local struct item 的 struct pattern unknown-field validation
- same-file local import alias -> local enum item 的 struct-variant pattern unknown-field validation
- same-file local import alias -> local struct item 的 struct pattern success path regression
- same-file local import alias -> local enum item 的 struct-variant pattern success path regression

当前仍明确未覆盖：

- pattern 缺失字段语义收紧
- foreign import alias pattern semantics
- deeper module-path / import-graph aware pattern resolution

## 验证

- `cargo fmt --all`
- `cargo test -p ql-typeck --test typing`
- full `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`
