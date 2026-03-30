# 阶段设计稿总览

这组文档不是路线图，而是对已经落地开发工作的设计归档。

整理原则：

- `docs/roadmap/` 负责回答“现在做到了什么”
- `docs/plans/` 负责回答“这些阶段最初是怎么设计和收敛的”
- 同一阶段的分散切片稿已经合并成 phase 级文档，便于后续会话恢复和新 agent 接手
- 原始切片稿保留在 [`/plans/archive/index`](/plans/archive/index) 中，作为审计与回溯材料

## 当前合并文档

- [Phase 0 设计冻结与语言定位](/plans/phase-0-design-freeze)
- [Phase 2 语义与类型检查地基](/plans/phase-2-semantic-and-typing)
- [Phase 3 MIR 与所有权分析地基](/plans/phase-3-mir-and-ownership)
- [Phase 4 LLVM 后端与原生产物地基](/plans/phase-4-backend-and-artifacts)
- [Phase 5 C FFI 与宿主互操作地基](/plans/phase-5-ffi-and-c-abi)
- [Phase 6 LSP 与编辑器语义收口](/plans/phase-6-lsp-and-editor-experience)
- [Phase 7 并发、异步与 Rust 互操作](/plans/phase-7-concurrency-and-rust-interop)

## 如何使用

如果你想快速恢复项目状态，建议按这个顺序阅读：

1. [P1-P7 阶段总览](/roadmap/phase-progress)
2. [开发计划](/roadmap/development-plan)
3. 本页对应阶段的合并设计稿
4. 需要追溯某个具体切片时，再进入 [`/plans/archive/index`](/plans/archive/index)

## 当前结论

目前仓库已经不是“预研空文档”，而是已经形成以下稳定边界：

- 前端基线：lexer / parser / AST / formatter / CLI
- 语义地基：HIR / resolve / typeck / diagnostics / query / minimal LSP
- 中端地基：MIR / ownership facts / cleanup-aware analysis / closure groundwork
- 后端地基：LLVM IR / obj / exe / staticlib / dylib / driver / toolchain boundary
- FFI 地基：extern C import/export、header projection、真实 C-host integration
- 编辑器地基：same-file query / rename / completion / semantic tokens / LSP parity

这意味着后续开发的主任务已经不是“重新搭骨架”，而是沿着现有分层继续扩展。

当前最活跃的主线是 [Phase 7 并发、异步与 Rust 互操作](/plans/phase-7-concurrency-and-rust-interop)。如果你要接着推进 async/runtime 相关工作，优先从该文档恢复上下文。

基于 2026-03-28 Serena 记忆整理的近期执行优先级清单见 [Phase 7 近期优先级计划](/plans/2026-03-28-phase-7-next-priorities)（Tasks 1-5 均已落地，P7.1 完成）。

P7.1 完成后的下一步可执行切片计划见 [Phase 7 P7.2 Runtime 合同扩展与 Rust 互操作](/plans/2026-03-29-phase-7-p7.2-runtime-and-interop)。

当前推荐的下一份执行计划见 [Phase 7 P7.4 Next Execution Implementation Plan](/plans/2026-03-30-phase-7-p7.4-next-execution)。
