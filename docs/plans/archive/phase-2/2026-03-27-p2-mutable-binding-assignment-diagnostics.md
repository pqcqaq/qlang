# 2026-03-27 P2 Mutable Binding Assignment Diagnostics

## 背景

`ql-typeck` 之前对 `=` 只做左右两侧类型兼容检查，没有约束左侧绑定是否真的可写。

这会带来两个明显问题：

1. `let value = 1; value = 2` 会被当作纯类型问题处理，缺少“不可变绑定被赋值”的诊断。
2. `self` 接收者已经有 `self` / `var self` / `move self` 的语法区分，但 typeck 还没有把这条差异落实到赋值语义。

## 本次收口范围

只补一个保守但真实有价值的语义切片：

- `var` 引入的 local binding 允许作为 bare assignment target
- `var self` 允许作为 bare assignment target
- `let` local、regular parameter、非 `var self` receiver 在 bare assignment 上报错

本次**不**扩展到：

- field assignment
- index assignment
- place projection / aliasing / write-through 语义
- flow-sensitive reassignability

## 设计原则

1. 不引入新的 resolver truth surface。
2. 不为 future place system 预埋一套容易推翻的 ad-hoc member/index 写入规则。
3. 复用 HIR 已有的唯一 `LocalId`，把 `var` 语义映射成 typeck 内部的 mutable binding 集合。
4. 让 diagnostic 锚定到 assignment target token，而不是整条表达式。

## 实现策略

### 1. mutable local 记录

`Checker` 在遍历 `StmtKind::Let { mutable, pattern, .. }` 时：

- 先照常检查 initializer
- 绑定 pattern 类型
- 若 `mutable == true`，递归收集 pattern 中所有 `Binding(LocalId)`，记录到 `mutable_locals`

这样 `var (left, right) = ...` 这种 pattern binding 也能统一继承可写性。

### 2. receiver mutability

`check_function` 进入函数时，从第一参数里识别 receiver：

- `self` / `move self` -> 不可写
- `var self` -> 可写

该状态只影响当前函数体内对 bare `self = ...` 的检查。

### 3. assignment target 检查

在 `BinaryOp::Assign` 路径上新增 `check_assignment_target(left)`：

- `Name` -> 查询 resolver 的 `ValueResolution`
- `Local(local_id)` -> 必须在 `mutable_locals`
- `Param(_)` -> 报不可变参数错误
- `SelfValue` -> 只有 `var self` 通过
- 其它 resolution 或非 `Name` lhs 本轮保持保守，不新增 place-level 结论

## 新增回归

- 正向：
  - `var` tuple destructuring binding 可以被再次赋值
  - `var self` 可以被整体替换
- 负向：
  - `let` local 赋值报错
  - regular parameter 赋值报错
  - 非 `var self` receiver 赋值报错
- 渲染：
  - rendered diagnostics 锚定到 lhs target span

## 后续安全下一步

如果要继续扩展 assignment 语义，安全顺序应当是：

1. 先设计统一的 place model
2. 再决定 field/index 写入的 target 分类
3. 再把 borrowing / aliasing / cleanup state 与写入语义对齐

在这之前，不应把 member/index assignment 伪装成“已经有完整可写性检查”。
