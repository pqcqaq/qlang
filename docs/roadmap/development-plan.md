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

截至 2026-03-29，Qlang 已经不是“只有语言设计文档的预研空壳”，而是一个真实的 Rust 编译器与工具链工作区：

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
- backend 已支持最小 async body wrapper、frame scaffold、loadable `await`（当前已覆盖 scalar / `Task[T]` / 递归可加载 aggregate payload，以及由 tuple / fixed-array / non-generic struct 递归构成、并在内部继续携带 `Task[T]` 的 fixed-shape payload 子集）、task-handle-aware `spawn`
- projected task-handle operand 已支持 tuple index / fixed-array literal index / struct field 只读投影路径，以及 mutable root 下的 projection write/reinit（tuple index / struct-field / fixed-array literal index）
- 非 `Task[...]` 元素的 dynamic array assignment 已开放并在 driver/CLI 两层锁定；`Task[...]` 动态数组索引写入已在 driver 内部单测与 CLI 黑盒层补齐显式 fail contract
- `staticlib` 已开放第一条受控 async library build 子集
- `dylib` 已开放最小受控 async library build 子集：当前允许带内部 async helper 的 library body 通过，但公开导出面仍收敛在同步 `extern "c"` C ABI surface
- `for await` 已开放首个受控 lowering 竖切片：当前支持 library-mode async body 内对 fixed array iterable 的 lowering（`staticlib` 与最小 async `dylib` 子集）
- `examples/ffi-rust` 与对应回归测试已经建立

### 当前仍刻意未开放

这些边界仍然应该明确写成“未完成”，而不是模糊描述成“后面再看”：

- 更广义的 projection-sensitive ownership / partial-place move tracking（当前已开放 tuple/struct-field task-handle path 的只读 consume 与同路径 write/reinit，以及 fixed-array literal index task-handle path 的只读 consume/write-reinit；非 `Task[...]` 元素 dynamic array assignment 已开放；`Task[...]` 的 dynamic index 写入已有显式 fail contract 锁定，但该能力本身仍刻意未开放；更广义 projection 仍未开放）
- 更广义的 projection assignment lowering（当前仅开放 tuple index / struct-field / fixed-array literal index projection write/reinit）
- 更广义的 `for await` lowering（当前仅开放 library-mode async body 内的 fixed array iterable）
- cancellation / polling / drop 语义
- generic async ABI 与 layout substitution
- 更广义的 async `dylib` 构建承诺（当前仅开放带同步 `extern "c"` 导出面的最小 library-style async body 子集）
- 更广义的 async executable surface（当前仅开放 `BuildEmit::Executable` 下的 `async fn main` 最小程序入口生命周期；更复杂的 program/runtime bootstrap 仍未开放）
- 更广义的 task result transport 协议
- 更广义的 place-sensitive task-handle lifecycle

### 推进阶段记录

#### P7.1 收紧 task-handle 语义与 ownership 边界 ✓ 已完成（2026-03-29）

所有目标均已落地：

- projected task-handle consume/write-reinit（tuple/struct-field/fixed-array literal index）已在 typeck/borrowck/codegen/driver/CLI 各层锁定
- branch-join、cleanup、helper forward/reinit 定向回归均已完成
- 非 `Task[...]` 元素 dynamic array assignment 已在 `ql-driver` 内部单测与 `ql-cli` 黑盒层锁定
- `Task[...]` 动态数组索引赋值的 fail contract 已在 driver/CLI 两层补齐

详细执行记录见 [2026-03-28 近期优先级计划](/plans/2026-03-28-phase-7-next-priorities)。

---

#### P7.2 runtime hook ABI 合同细化 ✓ 已完成（2026-03-29）

**目标：把当前 backend 中隐含的 hook 合同假设补成显式文档和测试。**

不扩新 hook，不引入多态变体。只把”opaque ptr 指向可直接 load 的 payload”这类现有假设在 `ql-runtime` 里写成注释规约，并在单测中体现。

已落地：
- `ql-runtime/src/lib.rs`：每条 hook symbol 补充完整 caller/callee 生命周期约定注释，enum-level overview 展示两组生命周期，`TaskAwait` 明确”backend load assumption”
- `ql-runtime/tests/executor.rs`：新增三项生命周期单测，14 项全通过
- `ql-codegen-llvm/src/lib.rs`：await lowering load 位置补充 INVARIANT 注释，显式引用 `RuntimeHook::TaskAwait` 合同

详细任务分解见 [2026-03-29 P7.2 计划](/plans/2026-03-29-phase-7-p7.2-runtime-and-interop)。

---

#### P7.3 扩 Rust interop 双向工作流矩阵 ✓ 已完成（2026-03-29）

**目标：把 `examples/ffi-rust` 从单向 export 示例扩展到更接近真实双向互操作的场景。**

不新增 ABI surface，不引入跨 crate 模块系统。只把”Qlang 导出 + 导入 C/Rust callback”这条双向路径在示例和 CLI 集成测试中锁定。

已落地：
- `examples/ffi-rust/ql/callback_add.ql`：新增 `q_host_multiply` import 与 `q_scale` export，建立第二条独立双向路径
- `examples/ffi-rust/host/src/main.rs`：提供两个 Rust 回调，调用两个 Qlang 导出，两条路径均验证返回 42
- `crates/ql-cli/tests/ffi.rs`：`ffi_rust_example_cargo_host_runs` 扩展断言 `q_scale(6, 7) = 42`

详细任务分解见 [2026-03-29 P7.2 计划](/plans/2026-03-29-phase-7-p7.2-runtime-and-interop)。

---

#### P7.4 扩大 async build surface（条件评估）

首个 program-build 切片已落地：`BuildEmit::Executable` 现已开放 `async fn main` 的最小程序入口生命周期，并已锁定 `async fn main` + fixed-array `for await` 的 executable 闭环。其余方向仍按下述前提继续保守推进，其中 Task 4 已完成 docs-first 评估并继续 deferred。

以下四个方向各有明确的推进前提，满足条件前继续保持保守拒绝：

| 方向 | 推进前提 |
| ---- | ---- |
| 放宽更多 `await`/`spawn` payload 路径 | runtime hook 合同（P7.2）已在单测层稳定，且 result layout contract 在注释中显式 |
| 扩大 `for await` iterable surface（slice/span 或通用 iterator） | 已完成 docs-first 评估：在 `qlrt_async_iter_next` 仍是 placeholder 的前提下，继续只开放 fixed-array；只有当 `Slice[T]` / span-like view 能作为 compiler-driven fixed-shape lowering 落地，且无需冻结新的 item release 协议时，才进入下一刀实现 |
| 扩大 async `dylib` 或开放更多 async program build surface | 至少一条 Rust host 双向互操作路径（P7.3）已被 CI 锁定，且 hook ABI 文档已成立；当前已开放 `BuildEmit::Executable` 下的 `async fn main` 最小程序入口生命周期 |
| 开放更广义的 async callable / effect surface | Phase 8 或更晚；需要独立 RFC，不在 Phase 7 范围内 |

##### P7.4 下一步执行顺序（2026-03-30 起，Task 1/2 已完成）

> 目标：继续沿着“保守可验证切片”推进，但优先级从纯 toolchain 体验回到**语言可用子集本身**：先扩用户可写、可编、可测试的语言能力，再补外围 UX。

1. **Task 3：放宽更多 `await` / `spawn` payload 路径**
   - 状态：进行中（首刀已完成：`BuildEmit::Executable` 下的 nested task-handle payload，`let next = await outer(); await next`，已在 codegen / driver / CLI 三层锁定；2026-03-30 又补齐了 tuple / fixed-array / nested aggregate task-handle payload，以及 sync-helper task-handle flow 的 executable 回归矩阵，确认 program-mode async body 已复用既有 fixed-shape / helper task-handle lowering，只是此前缺少显式 regression lock）
   - Why：当前前端语法与类型面已经明显快于 backend executable subset，最大的语言可用性缺口不在 lexer/parser，而在“用户已经能写出的 async 程序里，哪些 payload/aggregate/path 还不能稳定编译”。
   - Deliverables：
     - 扩大 `await` / `spawn` 在 executable / library 两种 build mode 下共享支持的 payload 子集。
     - 优先考虑已在 HIR/typeck/borrowck 层进入事实面的 fixed-shape aggregate / projection-sensitive 路径，而不是新开 ABI surface。
     - 同步补 `ql-codegen-llvm` / `ql-driver` / `ql-cli` 三层回归。

2. **Task 4：`for await` iterable surface 扩展评估（slice/span / dynamic array）**
   - 状态：已完成评估（2026-03-30）
   - Why：当前 `for await` 语法、MIR 与 fixed-array lowering 都已成立，但 backend 现状是直接对 concrete fixed-array layout 做 `getelementptr + load`，这不是 generic iterator protocol 的薄包装。先把 ABI 与 lowering 边界写清楚，比盲目再开一条 runtime-driven 路径更低风险。
   - 结论：
     - 保持 fixed-array 作为当前唯一 shipped iterable surface。
     - `qlrt_async_iter_next` 继续视为 capability/ABI placeholder，不在本轮冻结通用 iterator/item release 协议。
     - dynamic array `for await` 继续 deferred；它依赖更广的动态数组布局与生命周期事实面。
     - 如果后续要扩面，优先单独设计 `Slice[T]` / span-like fixed-shape view，并要求继续由 compiler 侧 index/load lowering 驱动，不新增 runtime hook。
   - Deliverables：
     - `/plans/2026-03-29-phase-7-p7.2-runtime-and-interop` 的“延后评估区”已补充对比矩阵与结论。
     - `/plans/phase-7-concurrency-and-rust-interop` 与本节已同步当前建议：Task 4 评估完成后保持 deferred，当前唯一立即实现项仍是 Task 3。

3. **Task 5：toolchain UX：Windows 下 clang 自动发现/提示收口**
   - 状态：已完成（2026-03-30）
   - Why：这是用户体验问题，重要但不应压过当前语言功能主线；放在语言子集继续扩展之后处理更合适。
   - Deliverables：
     - `ql-driver`：已在 Windows 上补充常见 LLVM 安装路径探测（Scoop、`%LOCALAPPDATA%\\Programs\\LLVM\\bin`、`%ProgramFiles%\\LLVM\\bin`、`%ProgramFiles(x86)%\\LLVM\\bin`），并把缺失时的 diagnostics hint 改成带候选路径的具体提示。
     - `crates/ql-cli/tests/ffi.rs`：已改为复用 `ql-driver` 的 toolchain discover 结果，避免集成测试与真实 build pipeline 使用两套不同的 clang / archiver 判定规则。

### Phase 7 出口标准

- `async fn` / `await` / `spawn` 在当前受控子集上有稳定语义、诊断和回归 ✓（P7.1 已完成）
- 至少一条 Rust 混编路径可在 CI 中复现 ✓（`examples/ffi-rust` + CLI 集成测试已建立，P7.3 已扩展为双向双函数）
- runtime hook ABI、driver build rejection 与 backend lowering 三者不再互相漂移 ✓（P7.2 已完成：hook 合同注释 + 单测 + INVARIANT 注释对齐）
- 文档、测试、实现三者对当前 async 边界给出同一描述（持续维护中；当前已包含 `async fn main` 的 executable 程序入口子集）

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
