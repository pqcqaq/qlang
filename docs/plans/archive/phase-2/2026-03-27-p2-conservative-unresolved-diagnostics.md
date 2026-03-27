# 2026-03-27 P2 Conservative Unresolved Diagnostics

## 背景

- 当前 same-file query / completion / semantic-token surface 已经做了大量聚合回归锁定。
- 下一步如果继续堆 editor parity，而不补真实语义边界，会让 P2 后续目标继续漂在文档里。
- 现有 resolver 只对 method receiver 作用域外非法使用 `self` 报错；普通 unresolved value/type 仍大面积静默退化。

## 目标

- 为绝对可靠的 bare single-segment unresolved root 补上 resolver diagnostics。
- 不引入 module-path、import graph、prelude、跨文件索引等尚未稳定的语义假设。
- 保持 `ql-analysis` / `qlsp` / CLI 可以直接消费这批 diagnostics，而不改动查询真值源。

## 收口范围

本次只覆盖：

- bare value name：`missing`
- bare named type：`Missing`
- single-segment pattern root：`Missing => ...`
- single-segment struct literal root：`Missing { ... }`

本次刻意不覆盖：

- multi-segment value/type path，例如 `pkg.value`、`pkg.Missing`
- module/import/prelude deeper unresolved strictness
- ambiguous member / static member / module-path 语义

## 设计

### 诊断策略

- 继续让 `ql-resolve` 作为 unresolved 诊断入口，而不是把这类错误塞进 `ql-typeck`。
- 只在 resolver 当前已经有稳定 lexical/type namespace 查找语义的节点上报错。
- 诊断文案分两类：
  - `unresolved value \`name\``
  - `unresolved type \`Name\``

### 关键实现点

1. `resolve_type`
   - `TypeKind::Named` lookup 失败后，只在 `path.segments.len() == 1` 时补 unresolved type diagnostic。
2. `resolve_pattern`
   - `PatternKind::Path` / `TupleStruct` / `Struct` lookup 失败后，只在单段 path 时补 unresolved value diagnostic。
3. `resolve_expr`
   - bare `ExprKind::Name` unresolved 时补 unresolved value diagnostic。
   - `ExprKind::StructLiteral` root type lookup 失败后，只在单段 path 时补 unresolved type diagnostic。
4. path-like member expression
   - `pkg.value` 这种表达式不能因为递归先走到 `pkg` 就过早报错。
   - 因此新增 path-like member helper，只为这条链保留 resolution 投影，不把根名字当作真正的 bare unresolved expr。

## 测试

- `crates/ql-resolve/tests/value_resolution.rs`
  - bare unresolved value name
  - single-segment unresolved pattern root
  - multi-segment value path defer boundary
- `crates/ql-resolve/tests/type_resolution.rs`
  - bare unresolved named type
  - single-segment unresolved struct literal root
  - multi-segment type path defer boundary
- `crates/ql-resolve/tests/rendering.rs`
  - unresolved value span rendering
  - unresolved type span rendering

## 验证

- `cargo test -p ql-resolve --test value_resolution`
- `cargo test -p ql-resolve --test type_resolution`
- `cargo test -p ql-resolve --test rendering`
- `cargo test -p ql-resolve`
- `cargo test`
- `npm run build` in `docs/`

## 后续安全下一步

- 若要继续扩 unresolved diagnostics，应先明确 import / module / prelude 语义。
- 更合适的下一步仍然是：
  - 表达式 typing 覆盖面继续补齐
  - `unknown` / deferred boundary 继续收紧
  - ambiguous member / module-path 查询与 completion 数据面设计
