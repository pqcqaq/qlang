---
layout: home

hero:
  name: Qlang
  text: 给程序员简单，把复杂留给编译器
  tagline: 一个基于 LLVM 的编译型语言项目，当前已完成 P1-P6 地基，并开放了 Phase 7 的受控 async build、runtime 与 Rust 互操作子集。
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

这个仓库已经不是“只有设计稿的预研空壳”，而是一个真实的 Rust 编译器与工具链工作区。当前已经形成前端、语义、中端、后端、FFI、LSP 与文档站的稳定主干，活跃主线是保守推进的 Phase 7：async、runtime、task-handle lowering 与 Rust 互操作。

当前文档给出四类结论：

- Qlang 的语言定位、设计原则与核心语法方向
- 类型系统、内存模型、并发模型与 FFI 方案
- 编译器、LSP、格式化器、文档系统与仓库结构
- 细化到阶段出口标准的功能清单与执行路线图

当前实现状态可以概括为：

- Phase 1 到 Phase 6 的基础能力已经落地
- Phase 7 已建立最小 runtime/executor、task-handle 类型面、共享 runtime hook ABI skeleton，以及受控的 async library/program build 子集
- 当前 build surface 已开放 fixed-array `for` 与最小 literal `match` lowering，其中 `Bool true|false|_|single-name binding` 与 `Int literal|_|single-name binding` 两条 `match` 子集都已可真实编译；两者都额外接受 literal / same-file const-backed `Bool` guard 及其 same-file `use ... as ...` 别名，`Bool` scrutinee 子集支持带 guaranteed fallback coverage 的 direct same-scope `Bool` local / parameter name guard，而 `Int` scrutinee 子集支持带 later unguarded catch-all fallback 的 integer-literal-arm / guarded-catch-all-arm direct same-scope `Bool` local / parameter name guard
- 当前真实支持面与未支持边界已集中收口到 [当前支持基线](/roadmap/current-supported-surface)
- 文档、测试和实现已经开始围绕同一份 phase 文档与回归矩阵收口

建议先看：

- [编译器、术语与生态入门](/getting-started/compiler-primer)
- [当前支持基线](/roadmap/current-supported-surface)
- [P1-P7 阶段总览](/roadmap/phase-progress)
- [开发计划](/roadmap/development-plan)
- [Phase 7 并发、异步与 Rust 互操作](/plans/phase-7-concurrency-and-rust-interop)
- [实现算法与分层边界](/architecture/implementation-algorithms)

## 核心判断

1. 对开发者最友好的系统级语言，不应该把复杂度直接外露成一堆生命周期标注、模板噪声和脚手架样板。
2. 真正难的工作应该由编译器承担，包括所有权推断、逃逸分析、区域分配、诊断建议和增量分析。
3. 混编不是附加功能，而是语言能否在真实工程中落地的核心能力，所以 ABI、链接、绑定生成和调试体验必须前置设计。
4. 语言规范、编译器架构、工具链和文档站必须一起设计；先写编译器再补工具链，后面一定返工。
