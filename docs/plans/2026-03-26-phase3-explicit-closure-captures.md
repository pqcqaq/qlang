# 2026-03-26 P3.3c: Explicit Closure Capture Facts In MIR

## 背景

P3.3b 已经让 `move` closure capture 进入 ownership analysis，但那一版还有一个明显架构问题：

- borrowck 需要重新遍历 HIR closure body
- capture list 是分析时临时推导出来的
- MIR 本身并不知道 closure 到底捕获了哪些 direct local

这会带来两个后果：

1. ownership 层和 HIR 绑定得过紧
2. 后续要做 closure environment / escape graph / drop elaboration 时，还得先把 capture facts 再搬回 MIR

所以这轮的目标不是新增表面语义，而是把已经存在的 closure capture fact 下沉到正确的中间表示层。

## 本轮目标

这轮只做一件事：

- 在 `Rvalue::Closure` 中显式 materialize direct-local capture facts

具体包括：

1. MIR lowering 为 closure 生成 capture list
2. capture fact 保留：
   - `local`
   - precise capture span
3. borrowck 改为直接消费 MIR capture list
4. `ql mir` / `ql-analysis.render_mir()` 能展示这层信息

## 不做的事情

- 完整 closure environment object lowering
- capture mode inference
- projection-sensitive capture
- closure escape graph
- closure drop elaboration
- nested defer inside closure environment 的 runtime 建模

## 设计原则

### 1. capture fact 属于 MIR，而不是 borrowck 的临时推导物

一旦 closure capture 已经影响 ownership diagnostics，它就不该只存在于 borrowck 的内部 helper 里。

MIR 是后续：

- ownership
- borrow / escape
- drop elaboration
- codegen-ready closure lowering

共同依赖的中间层，所以 capture fact 必须进入这里。

### 2. 当前仍然只保留 direct-local facts

这轮并不试图宣布“capture system 已完成”，只把当前已经稳定的语义事实明确下来：

- 当前 body 可回映的 binding local
- regular param
- receiver `self`

除此之外，不做新的激进承诺。

### 3. precise span 不能在这一层丢掉

如果 MIR 只记录 captured local 而不记录 capture span，后续 diagnostics 仍然会被迫回头查 HIR。

所以这轮直接把 capture span 一起 materialize 进 MIR。

## 数据模型

新增：

```rust
pub struct ClosureCapture {
    pub local: LocalId,
    pub span: Span,
}
```

并让：

```rust
Rvalue::Closure
```

携带：

- `is_move`
- `params`
- `captures`
- `body`

## lowering 方式

在 `ql-mir` 的 `BodyBuilder` 中新增 closure capture collector：

- 遍历 closure body 的 HIR expression / block / stmt
- 对 `ExprKind::Name` 查询 `ResolutionMap`
- 只在 resolution 能回映到当前 MIR body local 时记录 capture
- 去重后生成稳定 capture list

当前 collector 继续保持与 P3.3b 同样的语义边界，避免行为漂移。

## borrowck 调整

`ql-borrowck` 不再自己重跑 closure capture 收集。

新的路径是：

- `Rvalue::Closure` 直接携带 capture list
- ownership pass 直接消费 `captures`

这样 closure ownership facts 的来源就只有一处：

- MIR lowering

而不是：

- MIR lowering 一套
- borrowck 临时再推导一套

## 调试与验证

这轮至少锁定：

1. `ql mir` 输出包含显式 closure capture facts
2. `ql-analysis.render_mir()` 也能看见 capture facts
3. 原有 move-closure ownership 测试全部继续通过
4. diagnostics 仍然保持 precise capture span

## 当前限制

- capture list 仍然不是完整 closure environment
- 还没有 capture kind / borrow kind
- 还没有 escape edge
- 还没有 drop elaboration

## 结论

这轮完成后，closure capture 已经不再是 borrowck 的“额外聪明”，而是 P3 MIR 的正式事实。

这对后续继续推进：

- closure environment
- escape analysis
- drop elaboration

是必要的架构整理，而不是可有可无的重构。 
