# 2026-03-26 P6 Enum Struct Variant Literal Typing

在 first-pass typing 已经支持普通 struct literal 字段检查之后，`enum` 的 struct-variant literal 仍然残留一条真实缺口：

- `User { name: "ql" }` 已经会检查字段类型、未知字段、缺失必填字段
- 但 `Command.Config { retries: 3 }` 这类 enum struct-variant literal 还没有复用同一条字段检查逻辑
- same-file local import alias -> local enum item 的 `Cmd.Config { ... }` 也会一起掉进这个缺口

这一步的目标是把 enum struct-variant literal 的字段检查接到已有的 typeck truth surface 上，同时保持 query/LSP 边界不扩张。

## 目标

- 支持 direct enum struct-variant literal 的字段类型检查
- 支持 direct enum struct-variant literal 的未知字段与缺失必填字段诊断
- 支持 same-file local import alias -> local enum item 的 enum struct-variant literal 复用同样规则
- 继续保持 query/LSP 不声明 variant field symbol 已建模

## 非目标

- enum variant field 的 hover / definition / rename / references
- foreign import alias 的 variant literal 字段语义
- multi-segment import path / module graph
- variant field completion

## 设计

当前 `check_struct_literal` 已经能拿到：

1. 原始 `path`
2. struct literal root 的 `TypeResolution`
3. same-file local import alias -> local item 的 canonicalization

这次只把字段信息提炼成统一 helper：

1. 先从 `TypeResolution` 规范化回本地 item
2. 再用 `item + path` 取字段信息
3. 如果 item 是普通 struct，继续走原字段列表
4. 如果 item 是 enum，且 path 尾段命中 struct variant，就取该 variant 的 named fields
5. 后续的字段类型检查、未知字段、缺失必填字段逻辑完全复用现有 struct literal 分支

这样 direct `Command.Config { ... }` 和 alias `Cmd.Config { ... }` 都能进入同一条 truth path，而不需要额外写一套 variant-special-case 检查器。

## 覆盖用例

- `accepts_enum_struct_variant_literals`
- `accepts_enum_struct_variant_literals_through_same_file_import_aliases`
- `reports_enum_struct_variant_literal_shape_and_field_type_errors`
- `reports_enum_struct_variant_literal_shape_errors_through_same_file_import_aliases`

## 完成后的边界

当前 first-pass typing 已额外覆盖：

- enum struct-variant literal field typing
- enum struct-variant literal unknown-field diagnostics
- enum struct-variant literal missing-required-field diagnostics
- same-file local import alias -> local enum item 的 enum struct-variant literal typing
- same-file local import alias -> local enum item 的 enum struct-variant literal success path regression

当前仍明确未覆盖：

- query-side enum variant field symbol 建模
- variant field hover / rename / references / semantic tokens
- foreign import alias / multi-segment import path

## 验证

- `cargo fmt --all`
- `cargo test -p ql-typeck --test typing`
- full `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`
