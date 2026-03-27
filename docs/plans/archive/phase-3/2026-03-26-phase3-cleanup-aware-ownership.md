# 2026-03-26 P3.3: Cleanup-Aware Ownership Facts

## 背景

P3.1 已经把 `defer` lower 成了显式 cleanup registration 与 `RunCleanup` 执行点，P3.2 也已经建立了 MIR local 上的 moved-vs-usable facts。

但当时有一个刻意留下的缺口：

- borrowck 只分析普通 MIR statement / terminator
- `RunCleanup` 还没有真正参与 ownership analysis

这会带来一个明显问题：

```ql
defer user.name
defer user.into_json()
```

实际执行顺序是后注册的 cleanup 先运行，也就是：

1. `user.into_json()` 先消费 `user`
2. `user.name` 再读取 `user`

如果 borrowck 不理解 cleanup 执行，这类 use-after-move 会直接漏报。

所以 P3.3 的第一步不该直接冲向完整 borrow / escape analysis，而应先把 ownership facts 接到 cleanup runtime order 上。

## 本轮目标

本轮只解决 cleanup-aware ownership，不宣称“borrow checker 已完成”。

交付范围：

1. `RunCleanup` 会真实参与 ownership analysis
2. deferred expression 的 local read / consume / root-write 会影响后续 cleanup
3. `move self` 调用的消费时机改为“参数求值之后”
4. 对 deferred cleanup 里的 moved use 给出更可解释的诊断说明
5. 为后续 escape / closure capture / drop elaboration 预留干净扩展点

## 不做的事情

- 通用 borrow graph
- closure capture ownership
- nested `defer` inside deferred cleanup 的完整 runtime 模型
- 完整 loop-sensitive cleanup execution reasoning
- projection-sensitive partial move
- drop elaboration

这些能力都需要更多语义基础，但不应阻塞 cleanup 进入 ownership facts。

## 设计原则

### 1. 不把 cleanup 再次 lower 成另一套隐式 IR

当前 MIR 已经有：

- `CleanupAction`
- `RegisterCleanup`
- `RunCleanup`

所以这轮不新增第二套 cleanup IR，而是在 borrowck 里对 deferred expr 做受控的 HIR-level effect walk。

这样做的好处：

- 不污染 `ql-mir` 当前形态
- 逻辑集中在 `ql-borrowck`
- 后续如果要把 cleanup expr 真正 elaboration 成 MIR body，也不会影响当前对外接口

### 2. effect walker 只产出 ownership 相关效果

cleanup walker 不做 full interpreter，只关心三类效果：

- read local
- consume local
- root-write local

原因很简单：这轮关注的是 local 可用性，而不是值计算结果。

### 3. direct-local 约束继续保持

P3.2 里已经明确：

- 只对 direct local receiver 的 `move self` 做消费建模

P3.3 不打破这个边界。cleanup 中的 `foo.bar.into_json()` 仍然不做部分 move 推理，避免把 place-sensitive 规则偷偷混进当前实现。

## 代码设计

## `BodyAnalyzer` 扩展

为了让 cleanup walker 能把 HIR name resolution 映射回 MIR local，需要在 body 级预先建立反查表：

- `binding_locals: HIR LocalId -> MIR LocalId`
- `param_locals: param index -> MIR LocalId`
- `receiver_local: Option<MIR LocalId>`

这样 cleanup walker 只要拿到 `ValueResolution`，就能落回当前 body 的 tracked local。

## `UseSite`

普通 use 和 deferred cleanup use 的诊断文案不应完全一样。

因此本轮引入 `UseSite`：

- `span`
- `label`
- `note`

普通路径：

- label: `use here`

deferred cleanup：

- label: `used here when deferred cleanup runs`
- note: `deferred cleanup executes on scope exit in LIFO order`

这样用户在看 diagnostic 时能直接理解“为什么一个看上去较早写下的表达式会在 move 之后才执行”。

## `apply_consume` 与消费时机修正

P3.2 的实现里，`move self` call 会先把 receiver 标成 moved，再分析参数。

这会制造一个假阳性：

```ql
user.rename(user.name)
```

receiver 的消费应该发生在 call boundary，而不是参数求值之前。

所以本轮把调用分析拆成两步：

1. classify pending consume
2. 先分析参数
3. 最后再 `apply_consume`

同时：

- 如果 local 之前已经是 `Moved`
- `apply_consume` 不再粗暴覆盖旧状态
- 会保留并合并 origins

这样后续多次消费和路径汇合也更稳定。

## cleanup effect walker

本轮新增一组私有 helper：

- `eval_cleanup_expr`
- `eval_cleanup_block`
- `eval_cleanup_stmt`
- `eval_cleanup_assign_target`

核心返回值是：

- `states`
- `continues`

也就是：

- 当前 deferred expr 执行到这里后的 local states
- 这条路径是否还会继续执行后续 statement

这样可以最低成本支持：

- block
- `if`
- `match`
- straight-line stmt 序列

并对 `return` / `break` / `continue` 做基本截断。

## branch merge

cleanup 里的分支不会像 MIR body 那样天然已有 CFG，所以这轮用 borrowck 内部 merge helper 做合流：

- 两边都继续执行 -> merge states
- 只有一边继续 -> 后续只沿继续执行的一边传播
- 两边都终止 -> cleanup eval 标记为 stop

这让下面这种 deferred conditional consume 能稳定得到 `maybe moved`：

```ql
defer user.name
defer if flag { user.into_json() } else { "" }
```

## root write 与重建可用性

cleanup 里对 direct local root 的赋值必须能恢复 local 可用性，否则会误报：

```ql
defer user.name
defer { user = fresh_user(); "" }
defer user.into_json()
```

实际执行：

1. consume `user`
2. root-write `user`
3. 读取新的 `user`

所以 `eval_cleanup_assign_target` 会区分：

- direct local root assignment
- projection assignment

只有 root assignment 才会把 local state 写回 `Available`。

## 当前刻意保留的限制

本轮仍然不试图过度承诺：

- nested deferred cleanup registration 目前只跳过，不做 runtime modeling
- closure capture 在 cleanup expr 内仍不建模
- `while` / `for` / `loop` 只做保守单次/merge 近似
- projection write 不会被当作 root reinitialization

这些都已经被限制在私有 cleanup walker 中，后续替换成本可控。

## 测试策略

本轮新增并锁定的回归点：

1. move receiver 在参数求值后才消费
2. deferred cleanup 的 LIFO 顺序会导致 use-after-move
3. deferred conditional consume 会产生 `maybe moved`
4. deferred root-write 会让后续 cleanup read 重新可用
5. ownership render 能看到 cleanup 带来的 read / consume 事件
6. `ql-analysis` 会聚合 deferred cleanup diagnostics

## 对后续 P3.3 / P3.4 的价值

这轮完成后，P3 不再只有“普通语句上的 ownership facts”，而是真正开始覆盖：

- scope exit
- deferred execution
- runtime order

这直接为后面的几件事打底：

1. cleanup capture / escape analysis
2. drop elaboration
3. 更一般的 call contract
4. closure / async suspend-point 的 owned value tracking

## 结论

P3.3 的第一步不应是“写一个看起来很厉害但规则全是假的 borrow checker”。

更正确的做法，是先把已经存在的 cleanup runtime order 接进 ownership facts。

只有这样，后面的 borrow / escape / drop 才会建立在真实执行边界之上，而不是继续在语义空洞上堆规则。
