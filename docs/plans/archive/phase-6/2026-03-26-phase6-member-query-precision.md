# Phase 6 Member Query Precision

## 目标

在已经落地 same-file `references` 的基础上，把 `obj.field` / `obj.method()` 这类成员位点纳入同一套语义查询面，而不是继续停留在“只认 root binding”的状态。

这一步的重点不是直接做 rename 或 completion，而是先把成员 token 的定位、选择和 identity 做实。否则后续任何 IDE 能力都会因为成员语义不稳定而被迫返工。

## 这一步修了什么根因

在现有实现里，trait / impl / extend method 的 HIR `Function.span` 错误地继承了整个块的 span。结果是：

- resolver 的 `function_scope(method.span)` 在同一个 impl 里可能串锚
- receiver `self` 查询会跳到另一个方法的 receiver
- method member token 无法稳定映射回声明

所以这一步不是单纯“补一个 hover case”，而是先修正函数/方法 declaration span 的真实性。

## AST / HIR 变更

新增两条精确位置信息：

- `FunctionDecl.span`
- `ExprKind::Member.field_span`

并且保证它们从 parser 一路保留到 HIR：

- free function、trait method、impl method、extend method、extern function 都带自己的 declaration span
- `user.name` 这种 member expression 会保留 `name` 这个 token 的独立 span，而不是只能拿整个 `user.name`

这样 query / diagnostics / LSP 才能精确锚定成员名本身。

## 类型层的成员选择

`ql-typeck` 现在新增 `MemberTarget`，并把结果保存在 `TypeckResult`：

- `Field(FieldTarget { item_id, field_index })`
- `Method(MethodTarget { item_id, method_index })`

当前策略故意保守：

- struct field 可以稳定选中
- impl method 会优先于 extend method 参与选择
- impl / extend method 只有在各自优先级层内 candidate 唯一时才会稳定选中
- ambiguous method candidate 继续保持未解析，不伪造精确语义

这份成员选择结果既服务类型检查，也服务 query index。

## 查询层与 LSP

`ql-analysis::QueryIndex` 现在把以下成员纳入 occurrence：

- struct field declaration
- struct field member use
- trait / impl / extend method declaration
- unique method member use

对应新增 symbol kinds：

- `Field`
- `Method`

这意味着以下能力现在都能命中成员 token：

- hover
- go to definition
- same-file references

`qlsp` 本身不新增语义遍历逻辑，只是继续消费 `ql-analysis` 的结果。

## 顺带带来的类型收益

既然 typeck 已经能稳定识别唯一 method candidate，这一步也把 member-call 参数检查补上了。

也就是说：

- `counter.add(true)` 现在会对唯一 method candidate `add(self, delta: Int)` 产出参数类型错误
- 但 `let f = counter.add` 这种 first-class method value 仍不在当前支持范围内

## 当前仍不做

这一步仍然故意不扩到：

- ambiguous method candidate 的精确查询
- variant payload / module-path deeper query
- cross-file references
- rename
- completion

这些都需要更完整的模块图、导出面和项目级索引，不能在这个切片里糊成一团。
