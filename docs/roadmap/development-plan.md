# 开发计划

如果你想先看当前已经完成到什么程度，而不是看后续应该怎么推进，请先阅读：[P1-P7 阶段总览](/roadmap/phase-progress)。

这份文档的职责不是重复阶段归档，而是统一回答四个问题：

1. Qlang 现在到底处于什么阶段
2. 当前主线应该继续往哪里推进
3. 后续阶段应该按什么顺序展开
4. 每个阶段必须交付哪些横向工程结果

文档分工约定：

- [`/roadmap/phase-progress`](/roadmap/phase-progress) 负责“已经做到了什么”
- [`/plans/`](/plans/) 负责“各阶段是怎么设计和收口的”
- 本页负责“接下来按什么顺序推进，哪些边界必须继续保守”

## 当前判断

截至 2026-03-28，Qlang 已经不是“只有语言设计文档的预研空壳”，而是一个真实的 Rust 编译器与工具链工作区：

- Phase 1 到 Phase 6 的基础设施已经在仓库中落地
- 当前主线工作是保守的 Phase 7：async/runtime/staticlib/Rust interop
- 现阶段最重要的目标不是盲目扩语法，而是沿着现有边界持续扩展，不推翻已有真相源
- 文档、测试、实现必须继续保持同一事实面，否则项目会重新退化成“代码和路线图各说各话”

## 总体原则

### 1. 尽早形成真实闭环

对 Qlang 来说，闭环从来不只是“parser 能跑”，而是至少包括：

- 有用户可执行的 CLI 路径
- 有稳定 diagnostics
- 有回归测试
- 有文档与示例
- 有明确的失败合同

### 2. 一层只维护一份真相源

- AST/HIR/resolve/typeck/MIR/codegen/runtime 各层只维护自己的事实
- CLI、LSP、FFI、文档不要复制实现层语义
- 能从共享 analysis/query/runtime contract 派生的内容，不要在边缘工具再写一套

### 3. 先把失败模型做对，再扩公开能力

- 保守拒绝比错误支持更可维护
- 诊断、回归和边界说明必须先于更宽的表面承诺
- async、ownership、FFI、editor semantics 都继续沿用这个原则

### 4. 编译器开发必须测试驱动

- 回归测试属于功能本身，不是收尾工作
- 新能力至少覆盖正例、负例、边界和回归路径
- 发现 bug 时，先补会失败的测试，再修实现

### 5. C ABI 是当前稳定互操作边界

- C ABI 继续作为稳定外部边界
- Rust 互操作继续走 “Rust host <-> C ABI <-> Qlang” 路线
- 更深的 Rust effect/runtime 绑定、C++ 深度互操作都应后置

### 6. 文档要跟着实现一起推进

- 路线图、阶段总览、README、示例和测试结果必须同步
- 不允许 README 说一套、roadmap 说一套、代码里又是另一套

## 已完成阶段（P0-P6）

这些阶段已经形成稳定地基。后续工作应该在其上扩展，而不是回头推翻。

| 阶段 | 状态 | 已形成的稳定边界 |
| ---- | ---- | ---- |
| Phase 0 | 已完成 | 语言定位、设计原则、仓库结构、阶段划分 |
| Phase 1 | 已完成 | lexer / parser / AST / formatter / CLI 前端最小闭环 |
| Phase 2 | 已完成 | HIR / resolve / typeck / diagnostics / 最小 query / 最小 LSP |
| Phase 3 | 已完成 | 结构化 MIR / ownership facts / cleanup-aware 分析 / closure groundwork |
| Phase 4 | 已完成 | `ql build`、LLVM IR、`obj` / `exe` / `staticlib` / `dylib` 路径、driver/codegen 边界 |
| Phase 5 | 已完成 | 最小 C ABI 闭环、header projection、真实 C host 集成、sidecar header |
| Phase 6 | 已完成 | same-file query / rename / completion / semantic tokens / LSP parity |

对这些阶段的正确理解是：

- 它们已经“能持续迭代”，不是“所有细节都已做完”
- 后续切片应该在这些边界之上扩容
- 任何需要推翻这些边界的提案，都必须先说明为什么现有设计已经失效

## 当前主线：Phase 7 并发、异步与 Rust 互操作

### 目标

- 把 `async fn`、`await`、`spawn` 从“语法存在”推进到“可分析、可诊断、可逐步 lowering”
- 把 runtime / executor / task-handle / hook ABI 的边界固定下来
- 在不放宽过头的前提下，建立可复现的 Rust 互操作路径

### 当前已落地基线

当前已经形成的 Phase 7 事实面：

- `Task[T]` 已作为显式 task-handle 类型面进入 `ql-resolve` / `ql-typeck`
- `ql-analysis` 已暴露 runtime requirement truth surface
- `ql-runtime` 已提供最小 `Task` / `JoinHandle` / `Executor` / `InlineExecutor`
- runtime hook ABI skeleton 已存在，并被 `ql-driver` / `ql-codegen-llvm` 共享消费
- backend 已支持最小 async body wrapper、frame scaffold、loadable `await`、task-handle-aware `spawn`
- projected task-handle operand 已支持 tuple index / struct field 只读投影路径
- `staticlib` 已开放第一条受控 async library build 子集
- `examples/ffi-rust` 与对应回归测试已经建立

### 当前仍刻意未开放

这些边界仍然应该明确写成“未完成”，而不是模糊描述成“后面再看”：

- projection-sensitive ownership / partial-place move tracking
- projection assignment lowering
- `for await` lowering
- cancellation / polling / drop 语义
- generic async ABI 与 layout substitution
- async `dylib` 构建承诺
- async executable / program entry 构建承诺
- 更广义的 task result transport 协议
- 更广义的 place-sensitive task-handle lifecycle

### 推荐推进顺序

#### P7.1 继续收紧 task-handle 语义与 ownership 边界

优先级最高的不是继续扩新语法，而是把当前已经开放的 task-handle 路径继续收口：

- direct-local、helper 参数/返回值、projected operand 的 consume 事实继续锁定
- branch-join、cleanup、helper forward/reinit 这类边界继续补定向回归
- 继续保持“保守报错优先于过度承诺”

#### P7.2 扩 runtime/result/frame 合同，但继续保持最小可验证切片

- 在现有 hook ABI skeleton 上，补更完整的 task result transport 边界
- 在现有 frame scaffold 上，逐步明确 result/frame/layout contract
- 保持“先冻结内部合同，再扩公开能力”的顺序

#### P7.3 扩 Rust interop 的真实工作流矩阵

- 持续扩 `examples/ffi-rust`
- 补齐 Cargo host、build matrix、错误输出和可复现说明
- 至少维持一条 CI 可复现的 Rust host 路径

#### P7.4 谨慎评估是否扩大 async build surface

只有在前几步稳定之后，才考虑是否继续扩大公开能力：

- 是否放宽更多 `await` / `spawn` payload 路径
- 是否推进 `for await`
- 是否开放 async `dylib` 或 program build
- 是否开放更广义的 async callable / effect surface

如果这些前提还不稳定，就继续保持拒绝合同，不提前开放。

### Phase 7 出口标准

- `async fn` / `await` / `spawn` 在当前受控子集上有稳定语义、诊断和回归
- 至少一条 Rust 混编路径可在 CI 中复现
- runtime hook ABI、driver build rejection 与 backend lowering 三者不再互相漂移
- 文档、测试、实现三者对当前 async 边界给出同一描述

## 后续阶段

### Phase 8：项目级工具链、文档与工作区能力

### 目标

- 建立 project/workspace 级开发体验，而不仅是单文件或 same-file 语义
- 建立文档产物、包管理与模板初始化能力
- 让新用户从模板、文档、构建、编辑器到混编形成更顺畅路径

### 重点工作

- `ql doc`
- lockfile / package / workspace 元数据
- 项目模板与初始化能力
- 跨文件 / 项目级 semantic index
- 在 P6 same-file 基础上扩展 cross-file query / references / rename / completion

### 出口标准

- 新项目可以从模板初始化到构建运行形成稳定闭环
- 文档产物、workspace 元数据与编辑器语义形成统一工程面

### Phase 9：深水区语义、运行时与性能

### 目标

- 在已有地基上推进更广义的语言与运行时能力
- 建立更成熟的编译性能、增量分析和生态支撑

### 重点工作

- 更广义的 ownership / borrow / drop 规则
- 更完整的 async/runtime/effect 设计
- 更成熟的增量编译与性能回归体系
- 更深的 C++ 互操作
- 更成熟的标准库、生态与工程工具

### 出口标准

- 项目进入“可持续扩展”的生态阶段，而不是继续靠单点人工推进

## 每个阶段都必须交付的横向事项

无论推进到哪个阶段，以下事项都不是可选项：

- 文档更新
- 示例代码更新
- 回归测试补全
- CI 或验证命令更新
- 重要设计变更的 RFC / ADR 沉淀

对这个项目来说，真正的“完成”不是代码合入，而是下面几件事同时成立：

- 实现已经存在
- 测试能锁住行为
- 文档准确描述当前边界
- 用户能通过 CLI / 示例 / docs 复现正确路径

## 当前最推荐的阅读顺序

如果你要继续接手这个项目，建议按这个顺序恢复上下文：

1. [P1-P7 阶段总览](/roadmap/phase-progress)
2. [Phase 7 并发、异步与 Rust 互操作](/plans/phase-7-concurrency-and-rust-interop)
3. [阶段设计稿总览](/plans/)
4. [编译器流水线](/architecture/compiler-pipeline)
5. [实现算法与分层边界](/architecture/implementation-algorithms)

这份开发计划的核心结论只有一条：Qlang 当前最重要的工作不是重写基础设施，而是沿着已经成立的前端、语义、中端、后端、FFI、LSP 与 runtime 边界，继续做保守、可验证、可回归的扩展。
