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
