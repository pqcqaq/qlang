# 2026-03-26 P3.3b: Move Closure Capture Ownership

## 背景

P3.2 已经支持 direct-local `move self` 方法消费，P3.3a 也已经把 deferred cleanup 接进 ownership facts。

但当前语言里还有另一类明确的“值被转移”信号：

```ql
let closure = move () => value
```

如果 `move` closure 的 capture 不进入 ownership analysis，就会留下一个明显缺口：

- `move` closure 创建后，外层 local 理应被消费
- 普通 closure 捕获 moved local 也理应至少算一次真实读取

所以这轮要把 closure capture 接进当前 ownership facts。

## 本轮目标

只做第一版 direct-local closure capture ownership：

1. `move` closure 创建时会消费当前 body 中被捕获的 direct local
2. 普通 closure 创建时会把捕获视为一次读取
3. 这层行为同时进入：
   - MIR-based borrowck facts
   - `ql-analysis` 聚合 diagnostics
   - `ql ownership` render output

## 不做的事情

- 完整 closure environment lowering
- capture mode inference
- projection-sensitive capture
- closure escape / lifetime graph
- nested closure environment object layout

## 设计原则

### 1. 只依赖当前 body 可回映的 local

closure body 里出现的名字，只有当它能回映到当前 body 的：

- binding local
- regular param
- receiver `self`

时，才纳入当前 ownership analysis。

这样 closure 自己的参数和局部绑定不会被误判成外层 capture。

### 2. 先做 capture fact，而不是环境实现

当前 MIR 的 `Rvalue::Closure` 只有：

- `is_move`
- params
- body

它还没有显式 capture list。

所以这轮不在 MIR 数据模型里强行补环境对象，而是在 borrowck 中通过 HIR + resolution 收集 capture facts。

后续如果真的要把 capture list materialize 到 MIR，也不会破坏当前用户可见行为。

### 3. 区分 move capture 和 borrow-style capture

- `move` closure:
  - capture 会消费 local
- 普通 closure:
  - 当前先把 capture 视为一次 read

这不是完整 borrow 规则，但已经能稳定表达：

- moved value 不能再被 closure capture 使用
- `move` closure 会夺走 captured local 的所有权

## 实现方式

## capture 收集

在 `BodyAnalyzer` 中新增 closure capture collector：

- 遍历 closure body 的 HIR expression / block / stmt
- 对每个 `ExprKind::Name`
  - 通过 `ResolutionMap` 获取 `ValueResolution`
  - 再通过当前 body 的 local 映射表回到 `MirLocalId`

最终得到一个去重后的 captured local 列表。

## capture effect

在 `Rvalue::Closure` 分支里：

- `is_move = true`
  - 对每个 capture 执行 `apply_consume(..., MoveReason::MoveClosureCapture)`
- `is_move = false`
  - 对每个 capture 执行 `read_local(...)`

## diagnostics

新增：

- `MoveReason::MoveClosureCapture`

并补上：

- secondary label: `captured here by \`move\` closure`
- render event: `consume(move closure capture)`

普通 closure capture 的 moved-use 诊断则通过专门的 use-site label 表达：

- `captured here by closure`

## 测试范围

本轮至少锁定：

1. `move` closure capture 后再次使用 local 会报 use-after-move
2. closure capture moved local 会报错
3. ownership render 会展示 `consume(move closure capture)`
4. `ql-analysis` 会聚合 move closure capture diagnostics

## 当前限制

- projection capture 仍未做 place-sensitive 建模
- nested closure 仍然只是语义遍历，不是完整 environment 模型
- closure capture 只进入 ownership facts，不进入完整 borrow/escape graph

## 结论

这轮的价值在于把 Qlang 当前已有的第二类显式移动语义纳入统一 ownership facts：

- `move self` method
- `move` closure capture

这样 P3 后续再做 borrow / escape analysis 时，就不再只围绕方法调用打转，而是已经覆盖了 closure 这条真实语言路径。
