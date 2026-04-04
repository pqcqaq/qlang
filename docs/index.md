---
layout: home

hero:
  name: Qlang
  text: 给程序员简单，把复杂留给编译器
  tagline: 一门独立设计的编译型语言；当前编译器用 Rust 实现，但语言表面、语义与工程目标不以 Rust 为模板。
  actions:
    - theme: brand
      text: 查看阶段总览
      link: /roadmap/phase-progress
    - theme: alt
      text: 查看开发计划
      link: /roadmap/development-plan

features:
  - title: 开发者优先
    details: 默认不可空、默认不可变、结果类型与模式匹配、结构化并发、强诊断与自动修复建议。
  - title: 系统能力优先
    details: LLVM 后端、值语义、推断优先的所有权模型、可控共享、稳定 ABI 与多语言链接。
  - title: 工具链优先
    details: 编译器、LSP、格式化器、文档生成、测试框架、包管理和工作区模型从第一天一起设计。
  - title: 混编优先
    details: C 为一等互操作层，Rust 通过稳定 C ABI 集成，C++ 分阶段推进，先稳后广。
---

## 当前结论

这个仓库已经不是“只有设计稿的预研空壳”，而是一个真实在推进的语言与工具链仓库。当前编译器主实现使用 Rust，但 Qlang 的语言身份、语法方向、类型系统和工程目标以语言设计文档为准，而不是以 Rust 语法或 Rust 生态习惯为准；活跃主线仍是保守推进的 Phase 7：async、runtime、task-handle lowering 与互操作。

当前文档给出四类结论：

- Qlang 的语言定位、设计原则与核心语法方向
- 类型系统、内存模型、并发模型与 FFI 方案
- 编译器、LSP、格式化器、文档系统与仓库结构
- 细化到阶段出口标准的功能清单与执行路线图

当前实现状态可以概括为：

- Phase 1 到 Phase 6 的基础能力已经落地
- Phase 7 已建立最小 runtime/executor、task-handle 类型面、共享 runtime hook ABI skeleton，以及受控的 async library/program build 子集
- 当前真实支持面与未支持边界已集中收口到 [当前支持基线](/roadmap/current-supported-surface)
- 历史长文与逐轮记录已迁移到 [路线图归档](/roadmap/archive/index)，当前入口页只保留可依赖结论
- executable smoke harness 当前锁定 `60` 个 sync 示例与 `222` 个 async 示例；这两个数字已按目录与测试代码重新核对
- sync/async tuple assignment executable surface 现也已覆盖 same-file `const` / `static` item 名与 same-file `use ... as ...` alias 驱动的元组索引写入

建议先看：

- [编译器、术语与生态入门](/getting-started/compiler-primer)
- [当前支持基线](/roadmap/current-supported-surface)
- [P1-P7 阶段总览](/roadmap/phase-progress)
- [开发计划](/roadmap/development-plan)
- [路线图归档](/roadmap/archive/index)
- [Phase 7 并发、异步与 Rust 互操作](/plans/phase-7-concurrency-and-rust-interop)
- [实现算法与分层边界](/architecture/implementation-algorithms)

## 核心判断

1. 对开发者最友好的系统级语言，不应该把复杂度直接外露成一堆生命周期标注、模板噪声和脚手架样板。
2. 真正难的工作应该由编译器承担，包括所有权推断、逃逸分析、区域分配、诊断建议和增量分析。
3. 混编不是附加功能，而是语言能否在真实工程中落地的核心能力，所以 ABI、链接、绑定生成和调试体验必须前置设计。
4. 语言规范、编译器架构、工具链和文档站必须一起设计；先写编译器再补工具链，后面一定返工。

## 语言边界

- Qlang 不是 Rust 方言，也不是“更简单的 Rust”。
- Rust 目前只是编译器与工具链的宿主实现语言，以及当前互操作路径之一。
- Qlang 的语法、类型系统、并发模型和互操作边界，统一以 `/design/` 和 `/vision` 下的文档为准。
- 如果某项实现或文档让 Qlang 在视觉、语义或叙事上看起来像 Rust 子集，应优先修正文档与实现，而不是继续沿错误方向累积。
