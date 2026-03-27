# Phase 6 Shorthand-Binding Rename Preservation

## 背景

Phase 6 当前已经有两条和 shorthand struct site 相关的语义：

- token 本身仍然落在 local / parameter / import 等原始 binding symbol 上
- 如果从 source-backed field symbol 发起 rename，query 层会把 shorthand site 扩写成显式标签

但这里仍有一个 correctness 缺口：

- same-file rename 本来就已经支持 local / parameter / import / function / const / static 等符号
- 当这些符号恰好出现在 shorthand struct literal / struct pattern token 上时，rename 目前会直接把整段 token 替换成新名字
- 这会把原本隐含的 field label 一起改坏，导致语义漂移

例如：

- `Point { x }` 上把 local `x` rename 成 `coord_x`
- 当前错误结果会变成 `Point { coord_x }`
- 正确结果应当是 `Point { x: coord_x }`

## 目标

- 保持 shorthand token 仍然属于原始 binding symbol，而不是偷偷改成 field symbol
- 当从 shorthand token 上发起绑定 rename 时，自动扩写成显式字段标签
- 继续保持 field rename 与 binding rename 使用统一 `QueryIndex`

## 明确不做

- 不把 shorthand token 变成 field query symbol
- 不引入“field rename 还是 binding rename”的交互式选择
- 不扩到 cross-file rename

## 设计

`QueryIndex` 在现有 `field_shorthand_occurrences` 之外，再记录一组 binding-driven shorthand occurrence：

- key：当前 token 真正绑定到的 symbol key
- value：`{ span, label_text }`

记录规则：

1. struct literal shorthand:
   - 读取 shorthand value expr 的真实 resolution
   - 如果该 resolution 已经有稳定 query symbol，就记录一条 binding-driven shorthand occurrence
2. struct pattern shorthand:
   - 读取 shorthand binding pattern 对应的 local symbol
   - 记录一条 binding-driven shorthand occurrence

rename 时的策略：

1. 先照常收集同 symbol 的 same-file edits
2. 如果某个 edit span 同时是 binding-driven shorthand occurrence：
   - 不再直接替换整段 token
   - 改成 `label_text: <new_name>`

这样可以保证：

- token 的 hover / definition / references / rename target 仍然是 binding symbol
- rename 结果不会破坏 struct field 语义

## 回归

- `ql-analysis`：从 shorthand literal / shorthand pattern token 发起 local rename 时，会展开成显式字段标签
- `ql-lsp`：workspace edit 同样保持这条展开逻辑

## 边界

这一步修的是“当前已开放 rename surface 的 correctness”，不是在开放新的 field rename 语义。
