# Phase 6 Item Shorthand Rename Coverage

## 背景

Phase 6 已经修过 shorthand struct site 的 binding rename correctness：当 token 仍然故意落在 binding / value symbol 上时，rename 不应该把 field label 一起改坏，而应该展开成 `field_label: new_name`。

这条逻辑在实现上并不只覆盖 local / parameter / import / function。`const` 和 `static` 也已经属于 same-file rename 支持集，并且通过现有 value-resolution 会落到同一份 `QueryIndex` truth surface 上。

问题在于，之前没有专门的回归测试去锁住这两个 item-value 场景。后续如果继续调整 rename rewrite 或 occurrence 分组，很容易在不自知的情况下让 `const` / `static` 的 shorthand site 退化回“直接替换整个 token”的错误行为。

## 本次目标

- 不新增任何语义能力
- 不修改 rename 算法边界
- 只补齐 `const` / `static` 在 shorthand struct literal 中的 rename regression coverage

## 实现方式

直接复用现有实现，不加新的 query 数据结构：

1. analysis 层新增 const shorthand rename regression
2. analysis 层新增 static shorthand rename regression
3. LSP bridge 层新增 const shorthand rename regression
4. LSP bridge 层新增 static shorthand rename regression
5. 文档同步把这条覆盖面从“隐含支持”变成“明确声明并有回归保护”

## 非目标

- shorthand token 改绑 field symbol
- struct pattern 上的 const/static 新语义
- cross-file rename
- module/import-graph rename
- 更广义 item-path rename 扩张

## 验证

- `cargo test -p ql-analysis --test queries const_rename_preserves_shorthand_struct_literal_sites -- --exact`
- `cargo test -p ql-analysis --test queries static_rename_preserves_shorthand_struct_literal_sites -- --exact`
- `cargo test -p ql-lsp --test bridge rename_bridge_preserves_const_shorthand_binding_sites -- --exact`
- `cargo test -p ql-lsp --test bridge rename_bridge_preserves_static_shorthand_binding_sites -- --exact`
- 后续再跑全量 `fmt` / `test` / `clippy` / docs build

## 结果

完成后，Phase 6 shorthand binding rename 的 coverage 会更完整：

- local / parameter / import / function / const / static 都有显式回归保护
- rename 仍然完全复用统一 `QueryIndex`
- 文档会更准确反映“哪些 same-file rename correctness 已经被锁住”
