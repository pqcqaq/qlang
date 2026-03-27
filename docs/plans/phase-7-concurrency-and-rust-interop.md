# Phase 7 并发、异步与 Rust 互操作

## 目标

- 在不破坏 P1-P6 边界的前提下，把 `async fn`、`await`、`spawn` 从“语法存在”推进到“可分析、可诊断、可逐步降级”
- 给 runtime、executor、FFI 互操作建立清晰抽象，避免后续返工
- 保持测试驱动，先锁语义与失败模型，再扩执行能力

## 当前基础

- 前端已经有 `async` / `await` / `spawn` 语法节点
- MIR 已有 `for await` 与相关结构化表示
- LLVM backend 对 async 相关能力仍显式报 `unsupported`
- C ABI 与 header 投影已经稳定，可作为 Rust 混编入口

## 当前进度（2026-03-28）

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
- 已补充 `dylib` 路径上的 async backend 回归（含合法 `extern "c"` 导出存在时仍拒绝 `async fn`），锁住边界校验优先级
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
- 已在 `crates/ql-runtime` 起草第一版共享 runtime hook ABI skeleton：当前固定 `async-task-create`、`executor-spawn`、`task-await`、`async-iter-next` 的稳定符号名，并给出第一版 LLVM-facing contract string（当前统一走 `ccc` + opaque `ptr` 骨架）
- 已在 `ql-analysis` 暴露 `runtime_requirements()`：当前会按源码顺序枚举 `async fn`、`spawn`、`await`、`for await` 对应的 runtime 需求，为后续 driver/codegen 接线提供共享 truth surface
- 已补充 `crates/ql-analysis/tests/queries.rs` 的 runtime requirement 回归：覆盖 capability 顺序、精确 operator span，以及“仅声明无 body 的 async method 不计入 lowering 需求”的边界
- 已在 `ql-cli` 新增并扩展 `ql runtime <file>`：当前可直接输出该文件的 runtime requirements 与 dedupe 后的 runtime hook 计划，便于开发阶段检查 capability/hook contract 是否符合预期
- `ql-driver` 已开始保守消费这份 runtime requirement surface：当前会把 `async-function-bodies`、`task-spawn`、`task-await`、`async-iteration` 映射成稳定的 build-time unsupported 诊断，并与 backend 同类 unsupported diagnostics 做去重合并
- 当前仍保持 conservative 类型策略：`spawn` 结果类型保留 `Unknown`，`await` 暂不引入 Future/effect 全类型建模
- 当前仍不引入 first-class async callable type；`await` / `spawn` 先只接受可静态识别为 `async fn` 的调用路径，后续再结合 runtime/effect 设计决定是否放宽
- 当前 runtime crate 仍刻意不承诺 polling、cancellation、scheduler hints 或 Rust `Future` 绑定，只固定最小执行器接口
- 当前 hook ABI skeleton 已冻结第一版 LLVM-facing contract string，但仍只使用 `ptr` 级 opaque 形态；真实内存布局、结果传递协议和更细粒度调用约定仍未冻结
- 当前 `async-iteration` 已在 driver 层前移成公开 build 诊断，但仍只固定失败合同，不代表 `for await` lowering、runtime hook 或调度语义已经设计完成
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
