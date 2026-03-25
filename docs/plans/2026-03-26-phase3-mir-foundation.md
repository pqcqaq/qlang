# 2026-03-26 P3.1: MIR 基础层设计

## 背景

P2 已经完成了 parser -> HIR -> resolve -> typeck -> query/LSP 的最小语义闭环，但 P3 不能直接跳到 borrow checker 或 LLVM。

如果没有一个稳定、可调试、可测试的中间表示层，后续几个方向都会互相打架：

- 所有权 / move / drop / defer 语义无处承载
- borrow / escape 分析只能硬啃 HIR，后期必然重写
- codegen 被迫直接理解高层表达式和块尾值语义
- diagnostics 很难解释“为什么这里会释放 / 为什么这里不能再用”

所以 P3 的第一步应当先把 MIR 建起来。

## 本轮目标

本轮只做 P3.1，不假装“一次完成所有权系统”。

交付范围：

1. 新增 `ql-mir` crate
2. 定义可维护的 MIR 数据模型
3. 实现当前 HIR 子集到 MIR 的 lowering
4. 把 `defer` 和 cleanup 调度显式编码进 MIR
5. 提供 MIR 文本渲染与 CLI 观察面
6. 为后续 ownership diagnostics、borrowck、codegen 留出稳定扩展点

## 本轮明确不做

- 完整 borrow checker
- 生命周期显式语法
- drop elaboration 到具体析构调用
- LLVM lowering
- 复杂优化 pass
- `match` 的完整 CFG 展开
- `for` / `for await` 的协议级 lowering

这些能力都依赖 MIR，但不应和 MIR 基础层耦在同一轮里。

## 分层原则

P3.1 的核心分层如下：

```text
AST
  -> HIR
  -> MIR (structural control-flow + cleanup intent)
  -> ownership / borrow / escape analysis
  -> drop elaboration
  -> LLVM IR
```

关键约束：

- MIR 不直接依赖 LLVM
- MIR 不把 borrow 规则硬编码进节点形态
- MIR 保留和 HIR 的 source 映射，方便 diagnostics
- ownership 相关分析应该建立在 MIR 上，而不是倒逼 HIR 变成半个 CFG

## 数据模型

### 顶层容器

- `MirModule`
  - 保存所有已 lowering 的函数体
  - 使用 `ItemId -> BodyId` 建立映射
- `MirBody`
  - 一个函数体对应一个 body
  - 独立拥有 `locals`、`blocks`、`scopes`、`cleanup actions`

### 稳定 ID

MIR 需要自己的 arena ID，避免直接复用 HIR arena：

- `BodyId`
- `BasicBlockId`
- `StatementId`
- `LocalId`
- `ScopeId`

这样后续无论是 pass、diagnostics、dump 还是增量缓存，引用关系都更稳定。

### Local 模型

MIR local 必须明确区分“是什么槽位”，否则后面 move/drop 分析会失真。

建议的 `LocalKind`：

- `Return`
- `Param`
- `Binding`
- `Temp`

每个 local 同时保留：

- `name`
- `span`
- `mutable`
- `kind`
- `origin`

其中 `origin` 先记录 HIR local / param / synthetic temp 来源，后续 ownership diagnostics 可以回溯到用户代码。

### 基本块

每个 `BasicBlock` 包含：

- `statements: Vec<StatementId>`
- `terminator: Terminator`

语句和 terminator 分离，是为了后续：

- CFG 分析
- cleanup edge 插入
- codegen 降低复杂度

### Statement

P3.1 的 statement 先保持最小但语义明确：

- `Assign { place, value }`
- `BindPattern { pattern, source }`
- `Eval { value }`
- `StorageLive { local }`
- `StorageDead { local }`
- `RegisterCleanup { cleanup }`
- `RunCleanup { cleanup }`

其中：

- `Assign` 负责临时值、块尾值汇合和赋值表达式
- `BindPattern` 保留 `let` / `match arm` / `for` 模式绑定的结构语义，避免过早把模式展开成脆弱的伪低层赋值
- `Eval` 承接仅保留副作用的表达式
- `StorageLive` / `StorageDead` 为未来资源释放和 liveness 分析打底
- `RegisterCleanup` / `RunCleanup` 明确区分“注册 defer”与“作用域退出时执行 defer”

### Terminator

P3.1 需要的 terminator：

- `Goto`
- `Branch { condition, then_bb, else_bb }`
- `Match { scrutinee, arms, else_bb }`
- `ForLoop { iterable, item_local, body_bb, exit_bb }`
- `Return`
- `Terminate` 作为内部占位，防止未完成块悬空

`break` / `continue` 会在 lowering 时解析成 `Goto` 到对应 loop 目标，而不是作为 MIR 语法本身保留。

这里的 `Match` / `ForLoop` 是刻意保留的结构化 terminator。原因不是“偷懒”，而是当前阶段还不该在没有 ownership / iteration 协议完整语义的前提下，把它们压扁成未来一定会重做的低层状态机。

### Place / Operand / Rvalue

为避免后续“大改 MIR 形状”，P3.1 直接分成三层：

- `Place`
  - `Local`
  - `Field`
  - `Index`
- `Operand`
  - `Place`
  - `Constant`
- `Rvalue`
  - `Use(operand)`
  - `Tuple`
  - `Array`
  - `Call`
  - `Binary`
  - `Unary`
  - `AggregateStruct`

当前没有在 MIR 里强行写死 `Copy` / `Move`，而是先保留中性的 `Operand::Place`。这样 move classification 可以在下一切片作为单独分析层落下，而不是现在就把错误的所有权假设焊死进 MIR 节点。

### Scope 与 cleanup

P3.1 不直接做完整析构，但必须把 cleanup 作用域建好。

每个 body 维护 lexical scope 树：

- `ScopeId`
- parent
- owned locals
- registered `defer` actions

作用：

- 表达 `defer` 的 LIFO 顺序
- 为 future drop elaboration 提供“此处退出哪些作用域”的依据
- 为 diagnostics 提供“资源在何处注册、何处释放”的解释面

### CleanupAction

`defer expr` 不应该在 MIR 里只是“一个普通语句”，否则作用域退出时的执行顺序就无法分析。

因此本轮约定：

- `defer` lowering 时生成一个 `CleanupAction`
- action 记录：
  - 所属 scope
  - 原始 `ExprId`
  - 执行时的 MIR operand / rvalue
  - source span

当前先把 action 作为结构化数据挂到 body 上，并通过 block 退出规则推导“哪些出口会执行哪些 cleanup”。

## Lowering 规则

### 函数入口

每个函数 body 至少包含：

1. `entry` block
2. `return_local`
3. 参数 locals
4. body root scope

如果函数体为空，直接 `Return`。

### `let`

`let pattern = value` 的 lowering 分两步：

1. 先把 `value` 计算到一个 operand / temp
2. 再发出 `BindPattern`，并为模式里出现的 binding local 生成 `StorageLive`

这样一来，普通 `let`、`match arm` 和 `for item in ...` 都能共享同一种模式绑定表示。模式的进一步 elaboration 留给后续 pass。

### 表达式语句

- 有副作用的表达式保留 `Eval`
- 纯值表达式统一落到 temp，再在未被消费时允许保持为 no-op friendly 形式

### 块尾值

块表达式最后的 tail 统一写入指定目标 local。

这条规则很重要，因为它让：

- `if` 分支汇合
- block expression
- future `match` lowering

都能共享同一套“目标槽位”模型，而不是每种表达式各搞一套返回方式。

### `return`

1. 先把返回值写入 `return_local`
2. 触发当前作用域到函数根的 cleanup
3. 跳转到统一 `Return` 终结块

这样后面引入显式 drop elaboration 时，不需要推翻 `return` 路径。

### `defer`

`defer expr` 的 lowering 规则：

1. 当前点创建一个 cleanup action
2. 发出 `RegisterCleanup`
3. action 注册到当前 lexical scope
4. 在 `return`、`break`、`continue`、块尾退出时显式插入 `RunCleanup`

这条规则决定了 `defer` 在 MIR 里是“退出时动作”，而不是“延迟语法糖”。

### `if`

`if` / `else` lowering 为：

- condition block
- then block
- else block
- join block

如果 `if` 是表达式，则提前分配 result temp，分支分别写入同一目标，再跳到 join。

### `while` / `loop`

循环 lowering 统一维护 loop frame：

- `continue_target`
- `break_target`
- 退出循环时需要弹出的 scope 集合

这样后面引入 move/drop 检查时，可以准确知道从 loop 内跳出时需要执行哪些 cleanup。

### `match`

`match` 当前 lowering 为结构化 terminator：

- 先求值 scrutinee，并物化成可重用 local
- 终结点保存 arm 的 pattern / guard / target block
- 每个 arm block 负责：
  - `BindPattern`
  - arm body lowering
  - 退出 arm scope 并回到 join block

guard 暂时保留在 terminator 上，而不是现在就强行摊平成脆弱的条件链。

### `for` / `for await`

`for` 当前也保留为结构化 terminator：

- iterable 先求值一次
- `ForLoop` terminator 表示“驱动下一次迭代”
- body block 拿到一个 `item_local`
- pattern 通过 `BindPattern` 绑定到 body scope

迭代协议的完整展开和异步 next 语义留到后续切片。

### 赋值表达式

当前语法已支持 `=` 作为二元表达式，因此 MIR 需要把它特殊对待：

- 左值 lowering 为 `Place`
- 右值 lowering 为 operand / temp
- 生成 `Assign`

P3.1 先支持：

- 变量赋值
- 字段赋值
- 索引赋值的结构表示

是否允许这些赋值目标、是否需要借用或可变权限，留给后续 ownership / mutability pass。

## 诊断与扩展点

P3.1 不做完整所有权报错，但必须把未来报错的落点留好。

需要预留的数据：

- `origin`：MIR local / temp 对应的 HIR 来源
- `span`：statement / terminator / cleanup action 的 source span
- `scope`：每次 cleanup 注册和退出的词法作用域

后续 ownership diagnostics 可以直接建立在这些信息上，例如：

- 值在这里 move 出去
- 这里继续使用已经失效的 local
- 这里离开作用域，因此会触发 defer / drop

## CLI 与可观测性

P3.1 必须新增一个调试入口，例如：

```bash
cargo run -p ql-cli -- mir path/to/file.ql
```

输出必须稳定、可读、适合测试 snapshot，至少包含：

- 函数名
- locals
- basic blocks
- statements
- terminators
- cleanup action 与所属 scope

没有这个入口，后续 borrow/drop pass 的调试成本会非常高。

## 测试策略

本轮至少覆盖：

1. 线性函数 lowering
2. `if` 表达式分支汇合
3. `while` / `loop` 的 break / continue CFG 形态
4. 赋值表达式 lowering 到 `Assign`
5. `defer` 的注册顺序和 cleanup 顺序
6. CLI `ql mir` 文本输出稳定性

## 下一步切片

在 P3.1 之后，再继续：

1. `match` / `for` / `for await` 的完整 lowering
2. move classification
3. cleanup elaboration 到显式 drop chain
4. borrow / escape analysis
5. ownership diagnostics
6. codegen-ready MIR simplification

## 结论

P3 要先赢在抽象层，而不是先堆规则。

MIR-first 的价值不在“多一层中间表示”，而在于把下面这些高复杂度能力拆开：

- 结构化控制流
- cleanup / defer 执行边界
- move / borrow / drop 分析
- 代码生成
- 可解释 diagnostics

这层如果现在做干净，后面的 ownership 和 LLVM 才不会变成一次大规模返工。
