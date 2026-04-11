# Phase 7 并发、异步与 Rust 互操作

这页只保留阶段目标、当前已落地子集和下一步切片。当前事实面以 [开发计划](/roadmap/development-plan) 和 [当前支持基线](/roadmap/current-supported-surface) 为准。

## 目标

- 把 `async fn`、`await`、`spawn`、`for await` 从“语法存在”推进到“可分析、可诊断、可构建”的稳定子集。
- 固定 runtime、executor、task handle 与 hook ABI 的边界，避免后续返工。
- 维持 Rust host 通过 C ABI + generated header 调用 Qlang 的稳定路径。

## 当前基础

- 前端已经有 `async` / `await` / `spawn` / `for await` 语法节点。
- `ql-runtime` 已提供最小 runtime hook 和能力枚举；`ql runtime <file>` 可直接输出当前程序所需的 runtime requirement 与 hook ABI。
- LLVM backend 已有保守 async lowering；`ql build` 已能覆盖 program-mode async `main` 和保守 async library 子集。
- C ABI 与 header projection 已稳定，可继续作为 Rust 混编入口。

## 已落地子集

### 语义与查询

- `await`、`spawn`、`for await` 的 async 上下文诊断已接通到 `ql-typeck`。
- `ql-analysis` 与 `ql-lsp` 已暴露 async context 查询，不需要编辑器重复推导。
- runtime requirement truth surface 已接通到共享 analysis，而不是 CLI/LSP 各写一套规则。

### backend 与 build

- `ql build --emit llvm-ir|obj|exe` 已支持保守的 async `main` 子集。
- `ql build --emit staticlib|dylib` 已支持保守的 async library 子集，并继续复用同步 `extern "c"` 导出面。
- task-handle-aware `await` / `spawn`、部分 projected-root / nested-root / cleanup / control-flow 路径已经进入 codegen、driver、CLI 和 executable smoke 回归。
- build-side header 与 `ql ffi header` 仍复用同一份 analysis 结果，不单独复制语义。

### 互操作边界

- 当前稳定边界仍是 `Rust host <-> C ABI <-> Qlang runtime hooks`。
- Rust 互操作优先保证“可构建、可链接、可调用”，不提前承诺更深的语言级 ABI 融合。

## 当前下一步

- 优先补齐“前端已支持、语义已明确、backend 仍保守拒绝”的 async/runtime/build 缺口。
- 继续扩 program-mode 与 library-mode 的对称 build surface，但每次只放开一个最小可证明子集。
- 把 cleanup、control-flow、projection、task-handle reinit 等路径继续收口到共享 lowering，而不是各自长出特例。
- 保持 Phase 7 与 Phase 8 解耦：runtime 工作不直接绑定 `.qi` / package tooling。

## 测试策略

- 每个新增能力至少补一条语义回归和一条用户可见 build 或 executable 回归。
- 优先使用 family fixture、黑盒 CLI 测试和 `ramdon_tests/` smoke corpus，不再把长篇样例日志写回计划页。
- 文档与测试同轮更新，避免“代码已变、计划页还停在旧状态”。

## 非目标

- 不把 C ABI 替换成 Rust 专用 ABI。
- 不承诺完整 async 语义、通用 executor 生态或深度 C++ 互操作。
- 不用文档描述替代实际 build 支持；没有测试和实现的 surface 不写成已支持。

## 出口标准

- `ql build` 下的 async program/library 子集稳定，且 smoke corpus 可重复通过。
- `ql runtime` 输出能稳定反映当前 runtime hooks 与能力需求。
- Rust host 继续可以只依赖 C ABI + generated header 完成集成。
- 文档、回归矩阵和源码保持同一事实面。
