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

## 当前进度（2026-03-27）

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
- 已把 closure 视为独立 async 边界：closure body 当前不会继承外层 `async fn` 上下文，`await` / `spawn` / `for await` 会继续走非 async 诊断路径
- 已在 `ql-typeck` 修正 closure block 的显式 `return` 推断：当 closure 存在期望 callable 返回类型时，显式 `return` 会对齐 callable 签名；内层 nested closure 的 `return` 不会抬升外层 closure 返回类型
- 已在 `ql-typeck` 增补保守的 all-path return 分析：函数与 closure body 会拒绝“部分路径 `return`、部分路径 fallthrough”的情形；当前已覆盖 `if` 与最小穷尽性 `match`（`_`、`Bool true/false`、enum 全 variant），guarded arm 不计入覆盖
- 已在 `ql-typeck` 把显式常量条件的 `if` 纳入 must-return 收口：`if true { return ... }`、`if false { ... } else { return ... }` 与 closure 中同构写法现在会被接受；`if false { return ... }` 仍不会被误判成保证返回
- 已在 `ql-typeck` 增补 loop-control 上下文约束：`break` / `continue` 在非 loop body 中会给出显式诊断；closure body 不会继承外层 loop-control 语义
- 已在 `ql-typeck` 把 must-return 收口重构为有序控制流摘要：`loop { return ... }` 与 closure 中同构写法现在会被接受；`break; return ...` 和“无 break 的 loop 之后追加 return”不会再被误判成保证返回；更深层表达式子节点也会按求值顺序参与保守 return 分析
- 已在 `ql-typeck` 把显式常量条件的 `while` 纳入 must-return 收口：`while true { return ... }` 与 closure 中同构写法现在会被接受；`while true` 中的 `break; return ...` 和 `while false { return ... }` 仍不会被误判成保证返回
- 已在 `ql-analysis` / `ql-lsp` 增补 loop-control 只读查询桥接：`break` / `continue` 现在可查询当前位置是否位于 loop body；`impl` / `extend` / `trait` 方法和 closure loop-boundary 也有回归覆盖
- 已在 `ql-driver` 补充 async backend 边界回归：当语义层允许 `async fn` 时，构建流程会在 codegen 阶段稳定返回 `async fn` unsupported 诊断
- 已在 `ql-cli` codegen 黑盒快照中补充 `unsupported_async_fn_build` 用例，锁住用户侧 `ql build` 的 async backend 拒绝输出
- 已补充 `dylib` 路径上的 async backend 回归（含合法 `extern "c"` 导出存在时仍拒绝 `async fn`），锁住边界校验优先级
- 已补充 `async + generic` 并存场景回归，锁住 backend 同阶段多条 unsupported 诊断聚合行为
- 已补充 `async + unsafe fn body` 并存场景回归，锁住 backend 对函数签名级多条 unsupported 诊断的聚合与输出顺序
- 当前仍保持 conservative 类型策略：`spawn` 结果类型保留 `Unknown`，`await` 暂不引入 Future/effect 全类型建模
- 当前仍未引入完整 CFG 级 must-return / 全路径控制流分析；本轮只把有序表达式求值、显式字面量 `if true` / `if false`、`loop { return ... }`、显式字面量 `while true` / `while false` 与 break-sensitive loop body 纳入 conservative 收口，一般 `while` / `for` 的更强迭代推理、更广义的常量传播、guard-sensitive `match` 与 unreachable 细化仍待后续切片
- 当前 loop-control 已具备 analysis/LSP 的只读桥接，但还未扩展到公开 editor 协议 capability；继续保持低风险桥接策略

## 分阶段实现建议

### P7.1 语义层收口

- 在 `ql-typeck` 明确 `await` 输入输出约束和错误信息
- 在 `ql-resolve` / `ql-analysis` 增补 async 语义查询契约
- 保持 conservative 策略，不提前承诺完整 effect 系统

### P7.2 MIR 与 ownership 规则扩展

- 为 async 边界补最小可验证 lowering 规则
- 定义 `spawn` 的 capture 与 escape 约束
- 先覆盖 deterministic 子集，再讨论更复杂调度

### P7.3 Runtime 与 executor 抽象

- 先提供最小 executor trait 与单线程实现
- 把 runtime 调度边界隔离在独立 crate
- 与 codegen 的调用约定通过明确定义对齐

### P7.4 Rust 互操作闭环

- 增加 Rust host 最小示例与自动化回归
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
