# Phase 7 并发、异步与 Rust 互操作

## 目标

- 在不破坏 P1-P6 边界的前提下，把 `async fn`、`await`、`spawn` 从“语法存在”推进到“可分析、可诊断、可逐步降级”
- 给 runtime、executor、FFI 互操作建立清晰抽象，避免后续返工
- 保持测试驱动，先锁语义与失败模型，再扩执行能力

## 当前基础

- 前端已经有 `async` / `await` / `spawn` 语法节点
- MIR 已有 `for await` 与相关结构化表示
- LLVM backend 已有保守 async library lowering 子集，但更广义 async/runtime 语义仍显式保守
- C ABI 与 header 投影已经稳定，可作为 Rust 混编入口

## 当前进度（2026-03-30）

> 当前整体判断：**前端语法/词法与 same-file LSP 基线已经成型，当前主线瓶颈不再是“还能不能解析更多语法”，而是“已有语法里哪些能力已经进入可分析、可编译、可产出 artifact 的稳定子集”。** 因此后续优先级应继续落在 runtime / lowering / build-surface 的语言能力扩展，而不是先转向外围工具链 UX。

- 已在 `ql-typeck` 落地 `await` / `spawn` 的 async 上下文约束：在非 `async fn` 内使用会给出显式诊断
- 已补充 `crates/ql-typeck/tests/async_typing.rs`，锁住 `await` / `spawn` 边界与 async 函数内允许路径
- 已在 `ql-resolve` 增加 async 上下文查询契约（`expr_is_in_async_function` / `scope_is_in_async_function`）
- 已在 `ql-analysis` 暴露 `async_context_at`，可查询 `await` / `spawn` / `for await` 在当前位置是否位于 `async fn` 内
- 已补充 `crates/ql-analysis/tests/queries.rs` 的 async 查询回归
- 已在 `ql-lsp` bridge 层增加只读 `async_context_for_analysis` 桥接（不扩展协议面）
- 已补充 `crates/ql-lsp/tests/bridge.rs` 的 async 桥接回归
- 已把 `for await` 纳入 `ql-analysis` / `ql-lsp` async 查询桥接回归，统一 async 运算符语义查询面（`for await` 当前锚定 `await` 关键字 span）
- 已把 `ql-resolve` / `ql-typeck` / `ql-analysis` / `ql-lsp` 的 async 回归扩展到 `trait` / `impl` / `extend` 方法，锁住方法表面的 `await` / `spawn` / `for await` 语义上下文
- 已在 `ql-typeck` 增补 `for await` 上下文约束：非 `async fn` 内使用会给出显式诊断
- 已补充 `crates/ql-typeck/tests/async_typing.rs` 的 `for await` 边界回归
- 已在 `ql-typeck` 收紧 `await` / `spawn` 操作数约束：当前仅允许直接作用于 call expression，非调用操作数会给出显式诊断
- 已补充 `crates/ql-typeck/tests/async_typing.rs` 的非调用操作数回归（`await value` / `spawn value`）
- 已在 `ql-typeck` 继续收紧 `await` / `spawn` 的调用目标约束：当前不仅要求 operand 是 call expression，还要求被调用目标来自 `async fn`；sync function、sync method 与普通 closure/callable 值调用都会给出显式诊断
- 已补充 `crates/ql-typeck/tests/async_typing.rs` 的 async-call-target 回归，覆盖 sync function / async function / method / closure callable 这几类路径
- 已把 closure 视为独立 async 边界：closure body 当前不会继承外层 `async fn` 上下文，`await` / `spawn` / `for await` 会继续走非 async 诊断路径
- 已在 `ql-typeck` 修正 closure block 的显式 `return` 推断：当 closure 存在期望 callable 返回类型时，显式 `return` 会对齐 callable 签名；内层 nested closure 的 `return` 不会抬升外层 closure 返回类型
- 已在 `ql-typeck` 增补保守的 all-path return 分析：函数与 closure body 会拒绝“部分路径 `return`、部分路径 fallthrough”的情形；当前已覆盖 `if` 与最小穷尽性 `match`（`_`、`Bool true/false`、enum 全 variant）；带 guard 的 arm 默认仍保守，只有显式字面量 `true` guard 会计入覆盖
- 已在 `ql-typeck` 把显式常量条件的 `if` 纳入 must-return 收口：`if true { return ... }`、`if false { ... } else { return ... }` 与 closure 中同构写法现在会被接受；`if false { return ... }` 仍不会被误判成保证返回
- 已在 `ql-typeck` 把显式字面量 `Bool` scrutinee 的 `match` 纳入 must-return 收口：`match true/false` 会按 arm 顺序和字面量 guard 做保守裁剪；无可达 arm 或被字面量 `false` guard 挡住的唯一 arm 仍不会被误判成保证返回
- 已在 `ql-typeck` 把非字面量 `Bool` / enum `match` 的字面量 guard 纳入有序 arm 流分析：`true if true`、`false if true`、`_ if true` 与 enum variant `if true` 现在会参与穷尽性与 must-return 推断；未知 guard 仍保持保守，不会提前裁掉后续 arm
- 已在 `ql-typeck` 增补 loop-control 上下文约束：`break` / `continue` 在非 loop body 中会给出显式诊断；closure body 不会继承外层 loop-control 语义
- 已在 `ql-typeck` 把 must-return 收口重构为有序控制流摘要：`loop { return ... }` 与 closure 中同构写法现在会被接受；`break; return ...` 和“无 break 的 loop 之后追加 return”不会再被误判成保证返回；更深层表达式子节点也会按求值顺序参与保守 return 分析
- 已在 `ql-typeck` 把显式常量条件的 `while` 纳入 must-return 收口：`while true { return ... }` 与 closure 中同构写法现在会被接受；`while true` 中的 `break; return ...` 和 `while false { return ... }` 仍不会被误判成保证返回
- 已在 `ql-analysis` / `ql-lsp` 增补 loop-control 只读查询桥接：`break` / `continue` 现在可查询当前位置是否位于 loop body；`impl` / `extend` / `trait` 方法和 closure loop-boundary 也有回归覆盖
- 已在 `ql-driver` 补充 async backend 边界回归：当语义层允许 `async fn` 时，构建流程会在 codegen 阶段稳定返回 `async fn` unsupported 诊断
- 已在 `ql-cli` codegen 黑盒快照中补充 `unsupported_async_fn_build` 用例，锁住用户侧 `ql build` 的 async backend 拒绝输出
- 已开放 `BuildEmit::Executable` 下的 `async fn main` 最小程序入口生命周期：backend host `@main` wrapper 驱动 `task_create -> executor_spawn -> task_await -> result_load -> task_result_release -> trunc/ret`，并在 driver + CLI 黑盒层锁定
- 已锁定 `BuildEmit::Executable` 下的 nested task-handle payload 组合闭环：`async fn main` 中的 `let next = await outer(); await next` 现在在 codegen / driver / CLI 三层都有专项回归，说明 executable program-entry 已可稳定承载至少一条非直接 `await` 的 task-handle 续接路径
- 已补齐 `BuildEmit::Executable` 下的 aggregate task-handle payload regression matrix：tuple、fixed-array 与 nested aggregate payload（例如 `await pair[0]`、`await tasks[1]`、`await pending[0].task`）现在也已在 codegen / driver / CLI 三层有专项回归，证明 program-mode async body 继续复用既有 fixed-shape aggregate lowering，而不是只支持单一路径的 nested task handle
- 已锁定 `BuildEmit::Executable` 下的 helper task-handle parity 路径：`await schedule()`、`let task = schedule(); await task`、`let task = spawn schedule(); await task`、`let forwarded = forward(task); await forwarded` 与 `let running = spawn forward(task); await running` 现在也已有 program-mode codegen / driver / CLI 定向回归，说明 executable async body 继续复用既有 sync-helper task-handle lowering，而不是只在 library-mode 子集里成立
- 已锁定 `BuildEmit::Executable` 下的 local-return helper task-handle parity 路径：当 helper 先把 async call 绑定到本地再 `return task`（例如 `fn schedule() -> Task[Int] { let task = worker(); return task }` 与对应的 zero-sized `Task[Wrap]` 版本）时，`await schedule()` 现在也已有 program-mode codegen / driver / CLI 定向回归，说明 executable async body 同样复用了既有 local-return helper task-handle lowering，而不是只覆盖 direct-return helper 路径
- 已锁定 `BuildEmit::Executable` 下的 zero-sized helper task-handle parity 路径：当 helper 流动的句柄是 `Task[Wrap]` 且 `Wrap` 仅含 zero-sized fixed-array 字段时，`await schedule()`、bound helper handle、`spawn schedule()`、forwarded helper handle 与 `spawn forward(task)` 现在也已有 program-mode codegen / driver / CLI 定向回归，说明 executable async body 同样复用了既有 zero-sized loadable result / helper task-handle lowering
- 已锁定 `BuildEmit::Executable` 下的非零尺寸 aggregate-result parity 路径：当 async body 直接返回 tuple / fixed-array / struct（例如 `(Bool, Int)`、`[Int; 3]`、`Pair { left: Int, right: Int }`）并在 `async fn main` 中通过 direct `await` 读取结果时，program-mode 现在也已有 codegen / driver / CLI 定向回归，说明 executable async body 同样复用了既有 loadable aggregate-result lowering，而不是只覆盖 scalar、zero-sized aggregate 或 task-handle/result-carried-task 路径
- 已锁定 `BuildEmit::Executable` 下的 spawned 非零尺寸 aggregate-result parity 路径：当 tuple / fixed-array / struct 这类 loadable aggregate result 经由 `let task = spawn ...; await task` 读取时，program-mode 现在也已有 codegen / driver / CLI 定向回归，说明 executable async body 同样复用了既有 task-handle-aware spawn 与 loadable aggregate-result lowering，而不是只覆盖 scalar 或 zero-sized aggregate 的 spawned 路径
- 已锁定 `BuildEmit::Executable` 下的递归 aggregate-result parity 路径：当 async body 直接返回 nested loadable aggregate（例如 `(Pair, [Int; 2])`）并在 `async fn main` 中通过 direct `await` 读取结果时，program-mode 现在也已有 codegen / driver / CLI 定向回归，说明 executable async body 同样复用了既有 recursive loadable aggregate-result lowering，而不是只覆盖一层 tuple / fixed-array / struct 结果布局
- 已锁定 `BuildEmit::Executable` 下的 spawned 递归 aggregate-result parity 路径：当 nested loadable aggregate（例如 `(Pair, [Int; 2])`）经由 `let task = spawn worker(); let value = await task` 读取结果时，program-mode 现在也已有 codegen / driver / CLI 定向回归，说明 executable async body 同样复用了既有 task-handle-aware spawn 与 recursive loadable aggregate-result lowering，而不是只覆盖 direct `await` 的递归结果路径
- 已锁定 `BuildEmit::Executable` 下的 zero-sized aggregate-result parity 路径：当 async body 直接返回 `[Int; 0]` 或仅含 zero-sized fixed-array 字段的 `Wrap`，并在 `async fn main` 中通过 direct `await` 或 `let task = spawn worker(); await task` 读取结果时，program-mode 现在也已有 codegen / driver / CLI 定向回归，说明 executable async body 同样复用了既有 zero-sized loadable aggregate-result lowering，而不是只覆盖 task-handle/result-carried-task 路径
- 已锁定 `BuildEmit::Executable` 下的 aggregate-parameter parity 路径：当 async body 接收 recursive aggregate 参数（例如 `Pair` 与 `[Int; 2]`）或 zero-sized aggregate 参数（例如 `[Int; 0]`、`Wrap { values: [Int; 0] }` 与 `[[Int; 0]; 1]`）并在 `async fn main` 中通过 direct `await` 调用时，program-mode 现在也已有 codegen / driver / CLI 定向回归，说明 executable async body 同样复用了既有 async frame 参数物化与 aggregate-parameter lowering，而不是只在 library-mode 子集里成立
- 已锁定 `BuildEmit::Executable` 下的 spawned 递归 aggregate-parameter parity 路径：当 async body 接收 nested fixed-shape aggregate 参数（例如 `spawn worker(Pair { left: 1, right: 2 }, [3, 4])`）并在 `async fn main` 中通过 `let task = spawn ...; await task` 触发调用时，program-mode 现在也已有 codegen / driver / CLI 定向回归，说明 executable async body 同样复用了既有带参 async frame 物化、task-handle-aware spawn 与 aggregate-parameter lowering，而不是只覆盖 direct `await` 的 aggregate-parameter 路径
- 已锁定 `BuildEmit::Executable` 下的 spawned zero-sized aggregate-parameter parity 路径：当 async body 接收 zero-sized fixed-shape aggregate 参数（例如 `spawn worker([], Wrap { values: [] }, [[]])`）并在 `async fn main` 中通过 `let task = spawn ...; await task` 触发调用时，program-mode 现在也已有 codegen / driver / CLI 定向回归，说明 executable async body 同样复用了既有 zero-sized async frame 物化、task-handle-aware spawn 与 aggregate-parameter lowering，而不是只覆盖 direct `await` 或 non-zero-sized aggregate 参数路径
- 已锁定 `BuildEmit::Executable` 下的 zero-sized nested / struct aggregate task-handle parity 路径：当 `async fn outer() -> Task[Wrap]` 或 `async fn outer() -> Pending { first: Task[Wrap], second: Task[Wrap] }` 且 `Wrap` 仅含 zero-sized fixed-array 字段时，`let next = await outer(); await next` 与 `let pending = await outer(); await pending.first; await pending.second` 现在也已有 program-mode codegen / driver / CLI 定向回归，说明 executable async body 同样复用了既有 zero-sized nested task-result 与 aggregate-carried task-handle lowering
- 已锁定 `BuildEmit::Executable` 下的 projected task-handle await parity 路径：当局部 tuple / fixed-array / struct-field projection 的结果本身就是 `Task[Int]` 这类 non-zero-sized task handle 时，`await tuple[0]` / `await tuple[1]`、`await array[0]` / `await array[1]` 与 `await pair.left` / `await pair.right` 现在也已有 program-mode codegen / driver / CLI 定向回归，说明 executable async body 同样复用了既有 projection-sensitive await lowering，而不是只覆盖 aggregate-carried payload 或 zero-sized `Task[Wrap]` 路径
- 已锁定 `BuildEmit::Executable` 下的 projected task-handle spawn parity 路径：当局部 tuple / fixed-array / struct-field projection 的结果本身就是 `Task[Int]` 这类 non-zero-sized task handle 时，`let running = spawn tuple[0]; await running`、`let running = spawn array[0]; await running` 与 `let running = spawn pair.left; await running` 现在也已有 program-mode codegen / driver / CLI 定向回归，说明 executable async body 同样复用了既有 projection-sensitive spawn lowering，而不是只覆盖 aggregate-carried payload 或 zero-sized `Task[Wrap]` 路径
- 已锁定 `BuildEmit::Executable` 下的 projected task-handle direct reinit parity 路径：当局部 tuple / fixed-array / struct-field projection 的结果本身就是 `Task[Int]` 这类 non-zero-sized task handle 时，`let first = await tuple[0]; tuple[0] = worker(7); await tuple[0]`、`let first = await array[0]; array[0] = worker(8); await array[0]` 与 `let first = await pair.left; pair.left = worker(9); await pair.left` 现在也已有 program-mode codegen / driver / CLI 定向回归，说明 executable async body 同样复用了既有 projection write/reinit lowering，而不是只覆盖 zero-sized `Task[Wrap]` 路径
- 已锁定 `BuildEmit::Executable` 下的 zero-sized projected task-handle await parity 路径：当局部 tuple / fixed-array / struct-field projection 的结果本身就是 `Task[Wrap]` 且 `Wrap` 仅含 zero-sized fixed-array 字段时，`await tuple[0]` / `await tuple[1]`、`await array[0]` / `await array[1]` 与 `await pair.left` / `await pair.right` 现在也已有 program-mode codegen / driver / CLI 定向回归，说明 executable async body 同样复用了既有 projection-sensitive await lowering
- 已锁定 `BuildEmit::Executable` 下的 zero-sized projected task-handle spawn parity 路径：当局部 tuple / fixed-array / struct-field projection 的结果本身就是 `Task[Wrap]` 且 `Wrap` 仅含 zero-sized fixed-array 字段时，`let running = spawn tuple[0]; await running`、`let running = spawn array[0]; await running` 与 `let running = spawn pair.left; await running` 现在也已有 program-mode codegen / driver / CLI 定向回归，说明 executable async body 同样复用了既有 projection-sensitive spawn lowering
- 已锁定 `BuildEmit::Executable` 下的 zero-sized projected task-handle direct reinit parity 路径：当局部 tuple / fixed-array / struct-field projection 的结果本身就是 `Task[Wrap]` 且 `Wrap` 仅含 zero-sized fixed-array 字段时，`let first = await tuple[0]; tuple[0] = worker(); await tuple[0]`、`let first = await array[0]; array[0] = worker(); await array[0]` 与 `let first = await pair.left; pair.left = worker(); await pair.left` 现在也已有 program-mode codegen / driver / CLI 定向回归，说明 executable async body 同样复用了既有 projection write/reinit lowering
- 已锁定 `BuildEmit::Executable` 下的 zero-sized projected task-handle conditional reinit parity 路径：当 fixed-array literal index projection 的结果本身就是 `Task[Wrap]` 且 `Wrap` 仅含 zero-sized fixed-array 字段时，`if flag { let first = await tasks[0]; tasks[0] = worker() } await tasks[0]` 现在也已有 program-mode codegen / driver / CLI 定向回归，说明 executable async body 同样复用了既有 fixed-array literal-index projection 的 branch-join reinit lowering
- 已锁定 `BuildEmit::Executable` 下的 zero-sized direct-local branch spawned reinit parity 路径：当 direct-local `Task[Wrap]` 的结果是 zero-sized fixed-shape aggregate，且控制流采用 `if flag { let running = spawn task; task = fresh_worker(); return await running } else { task = fresh_worker() } return await task` 这类模式时，program-mode 现在也已有 codegen / driver / CLI 定向回归，说明 executable async body 同样复用了既有 direct-local task-handle consume 后重初始化 lowering
- 已锁定 `BuildEmit::Executable` 下的 zero-sized direct-local reverse-branch spawned reinit parity 路径：当 direct-local `Task[Wrap]` 的结果是 zero-sized fixed-shape aggregate，且控制流采用 `if flag { task = fresh_worker() } else { let running = spawn task; task = fresh_worker(); return await running } return await task` 这类模式时，program-mode 现在也已有 codegen / driver / CLI 定向回归，说明 executable async body 同样复用了既有 reverse-branch direct-local task-handle consume 后重初始化 lowering
- 已锁定 `BuildEmit::Executable` 下的 zero-sized conditional async-call spawn parity 路径：当 `async fn choose(flag: Bool) -> Wrap` 在一条分支里执行 `let running = spawn worker(); return await running`、另一条分支里直接 `return await worker()`，且 `Wrap` 仅含 zero-sized fixed-array 字段时，`let first = await choose(true); let second = await choose_reverse(false)` 这类正反分支组合现在也已有 program-mode codegen / driver / CLI 定向回归，说明 executable async body 同样复用了既有 direct async-call submit 与 zero-sized loadable result lowering，而不是只覆盖 helper/direct-local task-handle 路径
- 已锁定 `BuildEmit::Executable` 下的 zero-sized conditional helper-task-handle spawn parity 路径：当 `async fn choose(flag: Bool, task: Task[Wrap]) -> Wrap` / `choose_reverse(flag, task)` 在条件分支里交替执行 `spawn task` 与 `await task`，且 `Wrap` 仅含 zero-sized fixed-array 字段时，`await helper(true)` 与 `await helper_reverse(false)` 这类 helper-argument 正反分支组合现在也已有 program-mode codegen / driver / CLI 定向回归，说明 executable async body 同样复用了既有 helper 形参 task-handle submit/await lowering，而不是只覆盖 direct async-call 或 direct-local task-handle 路径
- 已完成 `for await` iterable surface 的 docs-first 评估：当前 fixed-array lowering 直接依赖 concrete `[N x T]` layout 与 index-slot metadata，`qlrt_async_iter_next` 继续保留 placeholder ABI；dynamic array 与通用 iterator 路径继续 deferred，若后续扩面，优先考虑不新增 runtime hook 的 `Slice[T]` / span-like fixed-shape view 设计
- 已补充 `dylib` 路径上的 async backend 回归：当前在存在合法同步 `extern "c"` 导出时，最小 library-style async body 已可稳定通过；fixed-array iterable 的 `for await` 也已进入该受控子集，而非数组 iterable 与更广义 async surface 仍会给出稳定诊断
- 已补充 `async + generic` 并存场景回归，锁住 backend 同阶段多条 unsupported 诊断聚合行为
- 已补充 `async + unsafe fn body` 并存场景回归，锁住 backend 对函数签名级多条 unsupported 诊断的聚合与输出顺序
- 已在 `ql-mir` 增补 async operator lowering 回归：`await` / `spawn` 当前会作为显式 unary rvalue 保留，并消费前面物化的 call 结果；same-file import alias 的 async call 也会继续保留 `Import` callee，而不是退化成 opaque/unresolved operand
- 已在 `crates/ql-cli/tests/ffi.rs` 增补 Rust host 静态链接集成回归：Rust harness 现在既可以直接调用 Qlang `staticlib` 导出，也可以为 Qlang 的 `extern "c"` import 提供最小 callback，实现最保守的双向互操作基线
- 已在 `crates/ql-cli/tests/ffi.rs` 增补 Cargo-based Rust host smoke test：测试会临时生成最小 Cargo 工程，通过 `build.rs` 链接 Qlang `staticlib`，让 Rust 互操作从单文件 `rustc` 基线推进到更接近真实工作流的可复现路径
- 已提交 `examples/ffi-rust`：仓库内现在有真实的 Cargo host 示例，`build.rs` 会编译 sibling Qlang 源码并链接生成的 `staticlib`
- 已在 `crates/ql-cli/tests/ffi.rs` 增补 committed example 回归：会复制 `examples/ffi-rust` 后执行 `cargo run --quiet`，锁住示例本身的可运行性
- 已新增 `crates/ql-runtime`：当前仓库已有最小 runtime/executor 抽象地基，提供 `Task` / `JoinHandle` / `Executor` trait 和单线程 `InlineExecutor`
- 已补充 `crates/ql-runtime/tests/executor.rs`：锁住 run-to-completion、`spawn` + `join`、`block_on` 以及单线程执行顺序
- 已在 `crates/ql-runtime` 固定第一批稳定 capability 名称：`async-function-bodies`、`task-spawn`、`task-await`、`async-iteration`
- 已在 `crates/ql-runtime` 起草第一版共享 runtime hook ABI skeleton：当前固定 `async-frame-alloc`、`async-task-create`、`executor-spawn`、`task-await`、`task-result-release`、`async-iter-next` 的稳定符号名，并给出第一版 LLVM-facing contract string（当前统一走 `ccc` + opaque `ptr` 骨架）
- 已在 `ql-analysis` 暴露 `runtime_requirements()`：当前会按源码顺序枚举 `async fn`、`spawn`、`await`、`for await` 对应的 runtime 需求，为后续 driver/codegen 接线提供共享 truth surface
- 已补充 `crates/ql-analysis/tests/queries.rs` 的 runtime requirement 回归：覆盖 capability 顺序、精确 operator span，以及“仅声明无 body 的 async method 不计入 lowering 需求”的边界
- 已在 `ql-cli` 新增并扩展 `ql runtime <file>`：当前可直接输出该文件的 runtime requirements 与 dedupe 后的 runtime hook 计划，便于开发阶段检查 capability/hook contract 是否符合预期
- `ql-driver` 已开始保守消费这份 runtime requirement surface：当前会把 `async-function-bodies`、`task-spawn`、`task-await`、`async-iteration` 映射成稳定的 build-time unsupported 诊断，并与 backend 同类 unsupported diagnostics 做去重合并
- 已在 `ql-codegen-llvm` 接入共享 runtime hook ABI contract：`CodegenInput` 当前可携带 dedupe 后的 `RuntimeHookSignature` 列表，后端会直接复用 `ql-runtime` 的声明文本渲染 runtime hook declarations，而不是在 backend 内重复维护符号名或 ABI 字符串
- 已在 `ql-codegen-llvm` 增补最小 async frame scaffold：当前 body-bearing `async fn` 会拆成一个统一接收 `ptr frame` 的真实 body symbol（`__async_body`）加一个公开 wrapper；parameterless wrapper 继续调用 `qlrt_async_task_create(entry, null)`，带参数 wrapper 会先通过 `qlrt_async_frame_alloc(size, align)` 构造最小 heap frame、写入参数，再调用 `qlrt_async_task_create(entry, frame)`，用于冻结最小 IR 结构
- 已在 `ql-codegen-llvm` / `ql-driver` / `ql-cli` 补上 library-mode async 边界回归：当前 `spawn` 既支持 statement-position fire-and-forget，也支持 value-position task-handle 绑定后继续 `await`；fixed-array iterable 的 `for await` 已在 library-mode async body 内通过，而 non-array iterable 与其余未开放 async surface 仍保持稳定 unsupported 诊断；`await` 也继续保留缺失 hook / 不支持结果布局等边界回归
- 已在 `crates/ql-runtime` / `ql-cli` / `ql-codegen-llvm` 增补 task-result transport 的第一条共享 ABI 合同：`task-await` 当前会同时暴露 `qlrt_task_await(join_handle: ptr) -> ptr` 与 `qlrt_task_result_release(result: ptr) -> void`，先冻结 result payload 的“返回”和“释放”边界，再延后 typed extraction / await lowering 的细节
- 已在 `ql-codegen-llvm` 增补 `AsyncTaskResultLayout` 内部抽象：当前 async 返回值已支持 `Void`、scalar builtin，以及递归可加载的 tuple / fixed array / 非泛型 struct 结果，并冻结“`qlrt_task_await` 返回的 opaque ptr 指向一块可直接 `load` 的 payload，取值后再调用 `qlrt_task_result_release`”这条 backend 内部假设；closure / 泛型 struct 等更广义聚合仍保持未开放
- 已在 `ql-codegen-llvm` 打开递归可加载 aggregate async frame 参数：带参数的 `async fn` wrapper 现在不再只接受 scalar/task-handle 参数，而是可把递归可加载的 tuple / fixed array / 非泛型 struct 参数写入 heap frame，并在 `__async_body` 中按同一布局回读
- 已在 `ql-codegen-llvm` 打开首个真实 `await` lowering：当前在 backend 内支持 `Void` / scalar builtin / `Task[T]` payload / recursively loadable aggregate async 结果，把 `await` 降成“读取 task handle -> `qlrt_task_await` -> `load` payload -> `qlrt_task_result_release`”；当前既支持直接 `await <async-call>`，也支持 `let task = spawn ...; await task`、`let next = await outer(); await next` 这类局部 task-handle 路径
- 已在 `ql-codegen-llvm` 打开最小 place projection lowering：当前嵌套 struct field read、constant tuple index read、array index read 已走统一投影链并进入 LLVM lowering；struct-field / constant tuple-index write、fixed-array literal index write，以及非 `Task[...]` 元素的 dynamic array assignment 也已接入同一条 lowering，而 `Task[...]` 动态数组元素赋值仍保持关闭
- 已在 `ql-codegen-llvm` / `ql-driver` / `ql-cli` 打开 projected task-handle operand lowering：当 field/index projection 的结果类型本身就是 `Task[T]` 时，`await pair[0]`、`spawn pair[0]`、`await tasks[0]`、`spawn tasks[0]`、`await pair.task`、`spawn pair.task` 这类路径现在都可稳定通过 codegen 与 `staticlib` 构建；当前数组路径仍只承诺 fixed-array literal index 的只读 consume，而非 task projection 与动态 index 仍不会在这里被额外放宽
- 已在 `ql-codegen-llvm` 打开首个真实 `spawn` lowering 子集：当前在 backend 内支持把 task-handle operand 降成“读取 task handle -> `qlrt_executor_spawn(ptr null, task)` -> 返回 task handle”，覆盖 direct async call、局部绑定 handle 与 sync helper 返回 handle 这几条路径；statement-position fire-and-forget 只是显式丢弃返回句柄的特例
- 已在 `ql-codegen-llvm` 收紧 empty-array lowering 的 expected-context 合同：当前会把“返回槽 / 已知 direct temp use / 直调参数 / 已知 tuple-array-struct 聚合字面量”的具体 `[T; N]` 期望类型保守回传到 direct temp locals，因此 `return []`、`take([])`、`([], 1)`、`Wrap { values: [] }` 与 `[[]]` 这类已有 `[T; N]` 上下文的路径都可以稳定 lowering，而裸 `[]` 仍保持显式拒绝
- 已在 `ql-codegen-llvm` / `ql-driver` / `ql-cli` 锁住 zero-sized async parameter 合同：`async fn worker(values: [Int; 0], wrap: Wrap { values: [Int; 0] }, nested: [[Int; 0]; 1])` 这类 zero-sized aggregate 参数现在也稳定走当前递归可加载 frame 模型，对应 wrapper/frame/`await`/`staticlib` 路径都已有回归覆盖
- 已在 `ql-codegen-llvm` / `ql-driver` / `ql-cli` 锁住 zero-sized async result 合同：`[Int; 0]` 与只包含 zero-sized fixed-array 字段的递归 aggregate 现在仍按 loadable async result 处理，`async fn -> [Int; 0]`、`async fn -> Wrap { values: [Int; 0] }`、`let task = spawn worker(); await task`、`let task = worker(); let running = spawn task; await running` 这类 direct/bound task-handle await，以及 `fn schedule() -> Task[Wrap]`、`fn schedule() { let task = worker(); return task }`、`fn forward(task: Task[Wrap]) -> Task[Wrap]`、`let task = spawn schedule(); await task`、`let task = schedule(); let running = spawn task; await running`、`let task = worker(); let running = spawn forward(task); await running` 这些 helper task-handle 路径都已有回归覆盖并通过 `staticlib` 构建
- 已在 `ql-typeck` / `ql-borrowck` / `ql-codegen-llvm` / `ql-driver` / `ql-cli` 锁住 nested task-handle payload 合同：`async fn outer() -> Task[Int] { return worker() }` 与 zero-sized `async fn outer() -> Task[Wrap] { return worker() }` 现在都允许 `let next = await outer(); await next` 这条 chained-await 路径，第一次 `await` 产出的 fresh task handle 会继续走同一套 consume / lowering / staticlib build 合同
- 已在 `ql-typeck` / `ql-borrowck` / `ql-codegen-llvm` / `ql-driver` / `ql-cli` 锁住 aggregate-carried task-handle payload 合同：`async fn outer() -> (Task[Int], Task[Int])`、`async fn outer() -> [Task[Int]; 2]`、带 `Task[Wrap]` 字段的 struct 结果，以及 `[Pending; 2]` 且 `Pending { task: Task[Int], value: Int }` 这类递归 nested fixed-shape aggregate 结果现在都允许先 `await outer()`，再通过 `await pair[0]` / `await pair[1]`、`await tasks[0]` / `await tasks[1]`、`await pending.first` / `await pending.second` 与 `await pending[0].task` / `await pending[1].task` 继续消费内部 task handle；这条路径复用现有 loadable aggregate result、projection-sensitive consume 与 projection await lowering，而不额外引入第二套 await-join 协议
- 已补上 helper branch-join consume/reinit 的零尺寸 `Task[Wrap]` 前端回归：`if flag { forward(task) } else { task = fresh_worker() }` 与 reverse-branch 版本现在都已在 `ql-typeck` / `ql-borrowck` 里锁住，并同步了 borrowck debug render 事实；`ql-driver` 现也把这条 helper 边界收紧成稳定的分析期失败合同，保证它不会误入 `ql-codegen-llvm` / `ql-driver` 的 staticlib 成功链路
- 已在 `ql-driver` 放开 public async build 子集：`staticlib` 现在允许已被 backend 支持的 async library body + scalar/task-handle/tuple/array/struct/void `await` + `spawn` task-handle 绑定/await 路径通过构建，并对 fixed-array iterable 打开首个 `for await` lowering；`dylib` 也已开放最小 library-style async body 子集，只要公开导出面仍收敛在同步 `extern "c"` C ABI，且 fixed-array iterable 的 `for await` 同样可通过。async program entry、更广义 `dylib` surface 与更广义 async iteration 仍保持保守拒绝
- 已补充 `ql-driver` / `ql-cli` 的 mixed-surface 回归：当前 `staticlib` 的 async library subset 已被黑盒锁住可与 `extern "c"` export header sidecar 共存，保证异步内部 helper 不会污染公开 C header surface
- 已在 `ql-resolve` / `ql-typeck` 打开首个显式 task-handle 类型面：当前 `Task[T]` 会作为保留类型根被接受，并映射到内部 `Task[...]` 句柄类型；direct `async fn` call、spawned task、局部绑定 handle 与 sync helper 返回值现在都能统一落到这条句柄语义上
- 已在 `ql-typeck` 把 `spawn` 的消费模型对齐到 task-handle 语义：`spawn task` 与 `spawn schedule()`（其中 `schedule() -> Task[T]`）现在都可保守通过；非 task operand 会给出稳定诊断，而不是继续把 `spawn` 限死在“直接 async call”形态
- 已在 `ql-typeck` 移除 direct async call 的“必须立刻 `await` / `spawn`”限制：`let task = worker(); await task`、`forward(worker())`、`return worker()` 到 `Task[T]` helper 等路径现在都可保守通过；当 direct async call 最终被放进非 `Task[T]` 上下文时，会自然退化成普通类型不匹配（例如 `Task[Int]` vs `Int`），不再依赖单独的特判诊断
- 已在 `ql-borrowck` 扩展 task-handle 生命周期边界：direct-local `Task[T]` 当前会在 `await` / `spawn`、静态可判定的 helper `Task[T]` 形参传递、以及 direct-local `return task` 时被视为 consume，后续复用会给出稳定的 use-after-move / maybe-moved 诊断，而重赋值仍可把 local 恢复为可用状态；projected task-handle operand 当前也已纳入这条边界，并会把 `await pair[0]` / `await tasks[0]` / `await pair.task` 这类消费精确记到 projection path，而不是退化成整个 base local，因此 awaited aggregate payload 中的 sibling task-handle projection 也能稳定共存；当前数组路径仍只承诺 fixed-array literal index 的只读 consume，当前这条边界也已显式覆盖 conditional cleanup 的 reinit/consume、helper-consume/reinit 与 reverse-branch helper-consume/reinit 这三类零尺寸 `Task[Wrap]` 回归
- 已补充 `ql-codegen-llvm` / `ql-driver` / `ql-cli` 的 helper-argument end-to-end 回归：`let task = worker(); let forwarded = forward(task); await forwarded` 与 `spawn forward(task)` 现在都被显式锁住，避免 task-handle helper 形参路径在后续 backend / driver 收口中回退
- 当前仍保持 conservative 类型策略：`ql-typeck` 目前只开放 `Task[T]` 这一显式 task-handle 类型面，`await` 暂不引入 Future/effect 全类型建模，也不开放更广义的任务调度/cancellation 类型协议
- 当前仍不引入 first-class async callable type；`await` / `spawn` 先只接受可静态识别为 `async fn` 的调用路径，后续再结合 runtime/effect 设计决定是否放宽
- 当前 direct `async fn` call 已可直接作为 `Task[T]` 句柄值参与局部绑定、helper 参数/返回值与后续 `await`，但这仍不代表更广义的 async effect/type inference 已完成；更宽的 task-handle 生命周期与调度语义仍待后续设计
- 当前 borrowck 只对 direct-local task handle 建立最小 consume 合同；`await` / `spawn`、静态可判定的 helper `Task[T]` 参数传递和 direct-local `return task` 已经接入，但更广义的 helper 返回/drop 边界、place-sensitive handle move/drop 与更广义提交协议仍待后续切片；零尺寸 `Task[Wrap]` 的 conditional cleanup 族只是在当前合同内被锁住回归，不代表更一般的 async drop 协议已开放
- 当前 runtime crate 仍刻意不承诺 polling、cancellation、scheduler hints 或 Rust `Future` 绑定，只固定最小执行器接口
- 当前 hook ABI skeleton 已冻结第一版 LLVM-facing contract string，但仍只使用 `ptr` 级 opaque 形态；真实内存布局、结果传递协议和更细粒度调用约定仍未冻结
- 当前 Windows toolchain UX 已做 first-pass 收口：`ql-driver` 在 PATH / 显式 `QLANG_CLANG` / `QLANG_AR` 之外，还会 best-effort 探测常见 LLVM 安装路径（例如 Scoop 与标准 LLVM 安装目录），并在缺失诊断中附带候选路径；这改善的是 discover/hint 体验，不代表完整 linker family discovery 已完成
- 当前 backend/driver 对这些 hook 已进入“declaration + async body wrapper + frame hydration + scalar/task-handle/tuple/array/struct/void await lowering（含递归 nested aggregate-carried task-handle payload）+ task-handle-aware spawn lowering + fixed-array literal index projected consume + fixed-array `for await` lowering + `staticlib` / 最小 `dylib` library 子集开放”阶段，但这仍不代表更广义的 async iteration 协议、任务结果协议、frame 生命周期管理或调度语义已经进入可执行阶段
- 当前 fixed-array literal lowering 也只打开了“已有具体 expected array type”的保守路径：direct temp 与 tuple / array / struct 聚合字面量内部的 `[]` 已可在 `[T; N]` 上下文中工作，但没有期望数组类型的裸 `[]` 仍不开放
- 当前 zero-sized async parameter 只在现有递归可加载 frame 模型内被视为合法：这锁住的是 `[Int; 0]`、`Wrap { values: [Int; 0] }` 与 `[[Int; 0]; 1]` 这类 frame 参数稳定性，不代表更广义的 capture/frame ABI 或 generic layout substitution 已经打开
- 当前 zero-sized async result 只在现有 loadable result 模型内被视为合法：这锁住的是 `[Int; 0]` 与递归 zero-sized aggregate 的 await/staticlib 稳定性，不代表更广义的 layout substitution、result transport 协议或 drop/cancellation 语义已经打开
- 当前 parameterless `async fn` wrapper 仍只依赖 `async-task-create` hook；带参数的 `async fn` 现在还会显式要求 `async-frame-alloc` hook 已接入，但这仍只是最小 heap-frame scaffold，不代表更完整的 frame/capture/result 设计已经冻结
- 当前 `async-iteration` 已不再只是纯失败合同：library-mode async body 内的 fixed-array `for await` 已走通首个 lowering 竖切片；但共享 runtime hook / capability 仍主要承担 ABI 预留语义，不代表通用 async iterator 调度协议已经冻结。P7.4 的 docs-first 评估结论也已明确：本轮不冻结 `qlrt_async_iter_next`，不开放 dynamic-array `for await`；若未来要扩面，优先考虑 compiler-driven 的 `Slice[T]` / span-like fixed-shape view
- 当前仍未引入完整 CFG 级 must-return / 全路径控制流分析；本轮只把有序表达式求值、显式字面量 `if true` / `if false`、显式字面量 `match true/false`、非字面量 `Bool` / enum `match` 上的字面量 guard、`loop { return ... }`、显式字面量 `while true` / `while false` 与 break-sensitive loop body 纳入 conservative 收口，一般 `while` / `for` 的更强迭代推理、更广义的常量传播、更一般的 guard-sensitive `match` 与 unreachable 细化仍待后续切片
- 当前 loop-control 已具备 analysis/LSP 的只读桥接，但还未扩展到公开 editor 协议 capability；继续保持低风险桥接策略

## 分阶段实现建议

### P7.1 语义层收口

- 在现有 MIR async operator lowering 合同之上，继续把 `await` / `spawn` 当前“必须调用 `async fn`”的约束下沉到 runtime/codegen 接口契约（仍保持 conservative）
- 在 `ql-resolve` / `ql-analysis` 增补 async 语义查询契约
- 保持 conservative 策略，不提前承诺完整 effect 系统

### P7.2 MIR 与 ownership 规则扩展

- 为 async 边界补最小可验证 lowering 规则
- 定义 `spawn` 的 capture 与 escape 约束
- 先覆盖 deterministic 子集，再讨论更复杂调度

### P7.3 Runtime 与 executor 抽象

- 已落地最小 `Executor` trait 与单线程 `InlineExecutor`
- 把 runtime 调度边界隔离在独立 crate
- 与 codegen 的调用约定通过明确定义对齐

### P7.4 Rust 互操作闭环

- 在已提交 `examples/ffi-rust` 的基础上继续扩展构建矩阵与宿主场景
- 固化 C ABI 映射和错误输出格式
- 文档中给出可复现的构建矩阵

## 测试策略

- 单元测试锁语义规则和诊断文本
- 集成测试覆盖 CLI、FFI、样例工程
- 失败快照优先于“盲目支持更多语法”
- 每个切片必须包含边界回归用例

## 非目标

- 当前阶段不做 project-wide async optimizer
- 当前阶段不做完整 actor runtime
- 当前阶段不做跨平台高性能网络栈

## 出口标准

- `async fn` / `await` / `spawn` 在语义层有稳定结果与诊断
- 至少一条 Rust 混编路径可在 CI 复现
- 文档、测试、实现三者保持同一事实面
