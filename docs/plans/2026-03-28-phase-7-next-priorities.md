# Phase 7 Next Priorities Implementation Plan

> Historical note (2026-03-29): Tasks 1-5 in this plan have now landed. This file is kept as an execution record for the 2026-03-28 planning snapshot, not as the current source of truth. Current implementation status lives in `docs/roadmap/phase-progress.md`, `docs/roadmap/development-plan.md`, and `docs/plans/phase-7-concurrency-and-rust-interop.md`.

**Goal:** 在当前 Phase 7 的保守 async/staticlib 闭环上，优先补齐最能继续放大 async 可用面的几个缺口。

**Architecture:** 先锁住当前 `Task[T]` 与 projected task-handle 的既有行为，再把 borrowck 从“按 base local 粗暴消费”推进到 projection-sensitive。随后再打开 projection write/reinit 与首个 `for await` lowering 竖切片，最后再评估是否扩大 async build surface。整个顺序遵循现有记忆里的约束：TDD、保守扩面、文档/测试/实现同步。

**Tech Stack:** Rust workspace, Serena memories, `ql-typeck`, `ql-borrowck`, `ql-mir`, `ql-codegen-llvm`, `ql-driver`, `ql-runtime`, VitePress docs

---

### Task 1: 冻结当前 projected task-handle 合同

**Files:**
- Modify: `crates/ql-typeck/tests/async_typing.rs`
- Modify: `crates/ql-borrowck/tests/ownership.rs`
- Modify: `crates/ql-driver/src/build.rs`

**Why now:** 这条支持刚进入 codegen/staticlib 路径，先把“当前允许什么、当前保守拒绝什么”锁死，再做 ownership 精化，能避免后续改动把现有闭环打穿。

**Step 1: 补 failing/regression tests**
- 保留 tuple/struct-field projected `await` / `spawn` 成功路径。
- 新增对“当前 projected use 会消费 base local”这一保守事实的显式断言，避免行为漂移。

**Step 2: 运行最小测试集**
- Run: `cargo test -p ql-typeck --test async_typing projected`
- Run: `cargo test -p ql-borrowck --test ownership projected`
- Run: `cargo test -p ql-driver projected_zero_sized_task_handle`

**Step 3: 只修测试与断言表述，不扩语义**
- 这一任务不改实现，只把现状固定下来。

### Task 2: 把 task-handle consume 从 base-local 提升到 projection-sensitive

**Files:**
- Modify: `crates/ql-borrowck/src/analyze.rs`
- Modify: `crates/ql-borrowck/src/render.rs`
- Modify: `crates/ql-borrowck/tests/ownership.rs`
- Modify: `crates/ql-typeck/tests/async_typing.rs`

**Why now:** 这是当前记忆里最明确、最局部、最阻塞后续扩面的技术债。只要 projected handle 仍按 base local 整体 consume，后面的 reinit、partial move、更多 payload/loop 形态都会被保守误伤。

**Step 1: 先写失败用例**
- `await pair[0]; await pair[1]` 对独立 tuple slot 应该允许。
- `let running = spawn pair.left; await pair.right` 对独立 struct field 应该允许。
- 同一 projection 二次消费仍应报 use-after-move。

**Step 2: 跑失败测试**
- Run: `cargo test -p ql-borrowck --test ownership projected_task_handle`

**Step 3: 最小实现**
- 在 `BodyAnalyzer` 的 consume/origin 跟踪里，引入 projection path 维度，而不是只按 root local 记账。
- `render.rs` 的调试输出同步显示更精确的 consume 来源，避免后续排查困难。

**Step 4: 回归**
- Run: `cargo test -p ql-borrowck --test ownership`
- Run: `cargo test -p ql-typeck --test async_typing projected`
- Run: `cargo test -p ql-driver projected_zero_sized_task_handle`

### Task 3: 打开 projection write / reinit 路径

**Files:**
- Modify: `crates/ql-codegen-llvm/src/lib.rs`
- Modify: `crates/ql-borrowck/src/analyze.rs`
- Modify: `crates/ql-driver/src/build.rs`
- Create: `tests/codegen/pass/projection_writes.ll`

**Why now:** 当前 projected read 已通，但 projected assignment lowering 仍关闭，导致“读得到、写不回、重初始化不精确”这个非对称状态持续存在。把 write/reinit 打开后，才能自然承接 task-handle 的 partial-place 生命周期。

**Step 1: 先写失败测试**
- 新增 tuple element / struct field 的重赋值与后续 `await` / `spawn` 组合路径。
- 新增 branch-join 后通过 projection write 恢复可用性的回归。

**Step 2: 跑失败测试**
- Run: `cargo test -p ql-driver projection`
- Run: `cargo test -p ql-borrowck --test ownership reinit`

**Step 3: 最小实现**
- 先只支持当前已经有只读 lowering 能力的 tuple/struct-field projection 写入。
- 不要在这一刀里顺带打开数组元素写语义或更宽泛的 place lowering。

### Task 4: 落首个 `for await` staticlib 竖切片

**Files:**
- Modify: `crates/ql-codegen-llvm/src/lib.rs`
- Modify: `crates/ql-driver/src/build.rs`
- Modify: `crates/ql-runtime/src/lib.rs`
- Modify: `crates/ql-runtime/tests/executor.rs`
- Modify: `tests/codegen/fail/unsupported_async_for_await_library_build.ql`
- Modify: `tests/codegen/fail/unsupported_async_for_await_library_build.stderr`

**Why now:** parser/HIR/MIR/typeck/analysis/driver 的边界已经铺好，当前真正缺的是 backend/runtime 的受控 lowering。相比直接放开更广的 build surface，这是一条更符合“保守 Phase 7”定位的垂直打通任务。

**Step 1: 缩小目标**
- 只支持 `async fn` library body 内最简单的一类 `for await`。
- 先选一种最容易冻结合同的 iterable 形态，避免把 async iteration 设计一次性做大。

**Step 2: 先写失败转通过测试**
- 保留仍不支持形态的 fail fixture。
- 为最小支持子集新增 driver/codegen/runtime 回归。

**Step 3: 最小实现**
- 只在 `staticlib` 子集开放。
- 复用现有 runtime capability / hook 命名，不额外发明第二套 async iteration surface。

**Step 4: 回归**
- Run: `cargo test -p ql-runtime`
- Run: `cargo test -p ql-codegen-llvm for_await`
- Run: `cargo test -p ql-driver async_for_await`

### Task 5: 评估并打开第一条 async `dylib` 子集

**Files:**
- Modify: `crates/ql-driver/src/build.rs`
- Modify: `crates/ql-codegen-llvm/src/lib.rs`
- Modify: `tests/codegen/fail/unsupported_async_fn_dylib_build.ql`
- Modify: `tests/codegen/fail/unsupported_async_fn_dylib_build.stderr`

**Why after Task 4:** `dylib` 扩面比 `staticlib` 风险更高，但如果 `for await` 与 projection lifecycle 已经稳定，就可以先尝试“非 generic、非 main、已受控导出”的最小 async `dylib` 子集，而不是直接开放 async executable。

**Step 1: 先定义最小支持面**
- 仅 library-style async body。
- 不碰 `async fn main`。
- 不碰 generic async symbol。

**Step 2: 测试先行**
- Run: `cargo test -p ql-driver dylib async`

**Step 3: 实现后复核 sidecar/header 行为**
- 确保公开导出的 C header surface 不泄露 async implementation details。

### Deferred: 暂不放在第一优先级的任务

**Items:**
- `cancellation / polling / drop semantics for task handles`
- `generic async ABI/layout substitution`
- `async executable / program entry build surface`

**Reason:** 这三项都比前五项更偏协议设计或更易放大 blast radius。按当前记忆里的“保守 Phase 7”路线，它们不该排在 projected ownership、projection write、`for await` 竖切片之前。

### Docs And Sync

**Files:**
- Modify: `docs/roadmap/development-plan.md`
- Modify: `docs/roadmap/phase-progress.md`

**Rule:**
- 每完成一项实现任务，同步更新 roadmap/progress。
- 任何 async 边界变化都要让文档、测试、实现三者描述一致。
