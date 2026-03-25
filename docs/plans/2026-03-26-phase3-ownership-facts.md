# 2026-03-26 P3.2: Ownership Facts 与显式消费诊断

## 背景

P3.1 已经把 MIR、CFG、lexical scope 和 `defer` cleanup 的结构层建起来了，但“有 MIR”并不等于“已经有所有权系统”。

下一步不能直接做一个假装完整的 borrow checker。原因很简单：

- Qlang 还没有把“哪些类型默认 copy、哪些默认 move、哪些上下文自动借用”的规则定死
- `match` / `for` / `for await` 还保留在结构化 MIR terminator 上
- method call 的语义虽然已经够识别 receiver 形态，但通用调用还没有 ownership contract

所以 P3.2 只做一件现在可以稳定落下的事情：

- 建立 MIR 上的 ownership facts
- 在此基础上实现“显式消费接收者 `move self`”的 moved-after-consume diagnostics

## 目标

这轮的目标不是“所有权检查完成”，而是：

1. 把 MIR local state analysis 做成独立层
2. 建立 block entry / exit 状态、use / write / consume 事件和 merge 规则
3. 只对显式消费语义做 diagnostics：
   - `move self` 方法调用会消费 receiver
   - 之后再次使用该 local 会报错
4. 让这套分析能自然扩展到后续：
   - 通用 move classification
   - borrow / escape analysis
   - drop elaboration

## 为什么先只做 `move self`

`move self` 是当前语法里最明确的消费信号之一：

- `self` 表示读
- `var self` 表示可变借入 / 独占修改语义
- `move self` 表示消费 receiver

这和未来的自动借用策略无冲突，而且用户也容易理解。

如果现在就把“普通调用参数默认 move 还是 borrow”硬编码成规则，后面大概率会返工。

## 新增分层

新增 `ql-borrowck` crate，职责严格收窄：

- 输入：
  - HIR
  - resolution
  - typeck result
  - MIR
- 输出：
  - ownership diagnostics
  - body 级 ownership facts

它不负责：

- type inference
- 通用 method type checking
- LLVM lowering

## 数据模型

### `BorrowckResult`

- `diagnostics`
- `bodies`

### `BodyFacts`

- `owner`
- `entry_states`
- `exit_states`
- `events`

这样后续如果要做 `ql ownership` 或 IDE 解释面，不需要重新遍历分析过程。

### `LocalState`

第一版状态足够简单：

- `Unavailable`
- `Available`
- `Moved(MoveInfo)`

含义：

- `Unavailable`
  - 还没 live，或者已经 dead
- `Available`
  - 当前路径上可继续使用
- `Moved`
  - 当前路径上已经被消费
  - `MoveInfo` 继续区分：
    - `certainty = Definite`
    - `certainty = Maybe`
    - `origins = Vec<MoveOrigin>`

### `MoveOrigin`

要做可解释 diagnostics，必须保留 move 来源：

- span
- local
- reason

当前 `reason` 只需要支持：

- `MoveSelfMethod { method_name }`

### 输出面

为了避免 ownership analysis 只能在 crate 内部自说自话，这一轮需要同时暴露两个观察面：

- `ql-analysis`
  - 聚合 borrowck diagnostics
  - 提供可调试的 ownership text render
- `ql ownership <file>`
  - 直接输出 body / block / local state / event 结果

## 分析规则

### 作用对象

P3.2 只跟踪 MIR local，不直接跟踪任意 projection。

原因：

- 对 `foo.bar.into_json()` 这种 projection receiver，真正的“部分 move”语义需要更完整的 place-sensitive analysis
- 现在先把 direct local receiver 做稳更重要

所以当前只对“直接 local 上调用 `move self` 方法”触发消费。

### 状态流

前向数据流：

- `StorageLive(local)` -> `Available`
- `StorageDead(local)` -> `Unavailable`
- `Assign` 到 local 根 place -> `Available`
- `BindPattern` 中新 binding 在前面的 `StorageLive` 后已经视为 `Available`
- `move self` 消费 -> `Moved(origin)`
- block merge:
  - 全部 `Available` -> `Available`
  - 全部 `Moved` -> `Moved`
  - `Available` + `Moved` -> `MaybeMoved`
  - 其他混合暂时保守归入 `MaybeMoved` 或维持非致错状态

### use 检查

读取 local 时：

- `Available` -> 允许
- `Moved` -> definite diagnostic
- `MaybeMoved` -> conditional diagnostic
- `Unavailable` -> 当前切片不抢着做 uninitialized / dead-use 诊断

### consume 识别

当前只识别这一类 call：

- callee 是 member call
- 能唯一匹配到一个方法 candidate
- 该方法 receiver 是 `move self`
- receiver object 是“直接 local place”

一旦满足，就把该 local 记为 consume。

### method candidate 匹配

基于：

- local 当前类型
- impl / extend method 列表
- method 名称

如果候选数不是 1，则本切片不产生 consume 事实，避免不稳定误报。

## Diagnostics 形式

### definite

```text
local `user` was used after move
```

label：

- primary: 当前 use 位置
- secondary: 之前的 `move self` 调用位置

### conditional

```text
local `user` may have been moved on another control-flow path
```

这类报错能提前验证 merge 逻辑是否可解释。

## 测试范围

至少覆盖：

1. direct `move self` 后再次使用 local 会报错
2. `self` / `var self` 方法不会触发 move
3. 条件分支里一侧消费、出口继续使用会报 `may have been moved`
4. local 重新赋值后重新变为 available
5. 无法唯一匹配 method candidate 时不误报

## 与后续切片的关系

P3.2 完成后，下一步就能更自然地接：

1. 更一般的 call consume contract
2. place-sensitive move facts
3. deferred cleanup 读取的 captured value 分析
4. drop elaboration
5. borrow / escape analysis

## 结论

P3.2 的价值不在于“多报几个错误”，而在于先把 ownership analysis 变成一个独立、可验证、可扩展的编译器层。

先吃下 `move self` 这类显式消费语义，是对架构最稳、对用户最可解释、对后续扩展最不浪费的一步。
