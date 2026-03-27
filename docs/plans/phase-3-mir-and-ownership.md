# Phase 3 MIR 与所有权分析地基

## 目标

Phase 3 的核心任务是建立一个可维护、可调试、可测试的中间表示层，并在其上接出第一版 ownership facts，而不是直接跳到“完整 borrow checker”。

核心交付：

- MIR
- cleanup / `defer` 显式建模
- ownership facts
- cleanup-aware analysis
- move closure capture
- closure escape groundwork

## 合并后的切片结论

### 1. MIR foundation

这一组切片解决的是“中层表示能否长期承载后续分析与后端”：

- 新增 `ql-mir`
- stable local / block / statement / scope / cleanup / closure ID
- block tail、`if`、`while`、`loop`、`break`、`continue` 的 CFG lowering
- `match` / `for` / `for await` 继续保留结构化 terminator
- `ql mir` 可直接渲染中层表示

关键设计：

- MIR 不依赖 LLVM
- MIR 不提前把 borrow 规则硬编码进节点形状
- cleanup 必须在 MIR 中显式存在，不能后面再靠猜

### 2. Ownership facts

这一组切片建立了第一版前向 ownership analysis：

- local `Unavailable` / `Available` / `Moved(...)` 状态
- block entry / exit state
- read / write / consume 事件
- direct-local `move self` diagnostics
- branch join 上的 `may have been moved`

关键设计：

- 当前不是完整 borrow checker
- 先把可解释的 facts 体系做出来，再扩具体规则

### 3. Cleanup-aware ownership

这一组切片把 `defer` 真正接进所有权分析：

- `RunCleanup` 进入 MIR / borrowck 分析路径
- cleanup 以 LIFO 顺序执行
- cleanup 中的 consume / read / root-write effects 会影响后续状态
- root local reassignment 可以重新建立 `Available`

### 4. Closure capture foundation

这一组切片把闭包相关事实从“推断式旁路”变成显式数据：

- `move` closure 在创建时消费 direct local capture
- non-move closure capture 视为真实读取
- capture facts materialize 到 MIR
- closure 具有稳定 identity
- ownership dump 渲染 conservative may-escape facts

## 当前架构收益

P3 现在已经把三条后续主线固定下来：

- borrow / move / escape 可以建立在 MIR 上扩展
- drop elaboration 不需要回头重造中层
- codegen 可以继续消费结构化 MIR，而不是直接理解高层 HIR

## 当前仍刻意保留的边界

- 通用 call consume contract
- place-sensitive move / borrow
- 完整 closure environment lowering
- nested defer runtime modeling
- drop elaboration
- `match` / `for` 的更低层 elaboration
- 完整 ownership diagnostics 体系

## 归档

本阶段原始切片稿已归档到 [`/plans/archive/phase-3`](/plans/archive/index)。
