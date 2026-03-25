# 2026-03-26 P3.3d: Stable Closure Identity And First Escape Facts

## 背景

P3.3c 已经把 closure capture fact 正式下沉到了 MIR，但当时还有一个缺口：

- MIR 里虽然有 capture list
- 但 closure 本身还没有稳定 identity
- 后续如果要做 escape graph / environment object / drop elaboration，仍然缺一个可靠挂点

同时，`ql ownership` 虽然已经能看 move closure capture，但还看不到 closure 值本身是否会离开当前 local-only 语境。

所以这轮继续推进的目标不是“完整 escape analysis”，而是：

- 先给 closure 一个稳定 MIR identity
- 再建立第一版 conservative may-escape facts

## 本轮目标

本轮只做下面这些事情：

1. MIR closure 拥有稳定 `ClosureId`
2. `Rvalue::Closure` 改为引用 closure decl，而不是内联所有字段
3. `ql ownership` 输出 closure facts：
   - captures
   - may-escape surfaces
4. may-escape 只覆盖少量当前可稳定表达的 surface

## 不做的事情

- 完整 escape graph
- path-sensitive closure escape reasoning
- closure environment object lowering
- borrow kind / capture kind inference
- cleanup runtime 下的 closure escape 建模
- drop elaboration

## 设计原则

### 1. closure identity 必须属于 MIR

如果 closure 只有 capture list 而没有稳定 ID，后续所有和 closure 相关的分析都只能围绕：

- statement span
- temp local
- ad hoc pattern matching

打转。

这会让后续 escape / environment / drop 继续长成难维护的隐式逻辑。

所以这轮直接引入：

- `ClosureId`
- `ClosureDecl`

让 closure 从“某个 rvalue 的一坨字段”升级成 MIR 里的正式实体。

### 2. 先做 may-escape facts，不伪装成精确 escape graph

当前语言和 MIR 还没有完整 closure environment，也没有 place-sensitive move/drop。

所以这轮故意不宣称“escape analysis 已完成”，只渲染：

- `return`
- `call-arg`
- `call-callee`
- `captured-by-cl*`

这些“已经明确看见的可能逃逸面”。

### 3. 让 ownership 调试输出真正能服务后续 P3

这轮之后，`ql ownership` 不只是看 local moved/unavailable 状态，还能看：

- closure 捕获了谁
- closure 可能沿哪条 surface 逃逸

这会直接帮助后续：

- closure environment design
- escape graph validation
- drop elaboration 调试

## 数据模型

新增：

```rust
pub struct ClosureDecl {
    pub span: Span,
    pub is_move: bool,
    pub params: Vec<String>,
    pub captures: Vec<ClosureCapture>,
    pub body: ExprId,
}
```

以及：

```rust
ClosureId
```

`MirBody` 现在显式持有 closure arena。

`Rvalue::Closure` 只保留：

```rust
Rvalue::Closure { closure: ClosureId }
```

## 第一版 may-escape facts

`ql-borrowck` 现在除了 local ownership facts 以外，还会做一层 closure fact 收集。

当前策略是：

- 基于 MIR local 做 conservative dataflow
- 跟踪“哪个 local 当前持有哪些 closure value”
- 在遇到明确的 escape surface 时记录 may-escape fact

当前 surface：

1. `return`
2. `call-arg`
3. `call-callee`
4. `captured-by-cl*`

其中：

- `captured-by-cl*` 表示一个 closure value 被另一个 closure capture
- 这是 future closure environment / nested escape graph 的第一层可见事实

## 调试输出

`ql mir` 现在可以稳定看到：

- closure section
- `cl0` / `cl1` 这类 stable IDs
- closure rvalue 对应的 identity

`ql ownership` 现在可以看到：

- local ownership facts
- closure capture list
- closure may-escape facts

例如：

```text
cl0 captures=[l1:base@211..215] escapes=[captured-by-cl1@240..245, call-arg@257..269]
```

## 当前限制

- 这是 conservative may-escape，不是精确 escape
- 当前没有 certainty 等级
- 当前没有 cleanup runtime 下的 closure escape
- 当前没有 path-sensitive closure state
- 当前没有完整 environment graph

## 结论

这轮完成后，P3 的 closure 相关基础层已经从：

- capture facts

推进到：

- stable closure identity
- first escape surfaces

这为后续继续做：

- closure environment
- path-sensitive escape analysis
- drop elaboration

提供了真正可扩展的落点。 
