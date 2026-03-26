---
layout: home

hero:
  name: Qlang
  text: 给程序员简单，把复杂留给编译器
  tagline: 一个基于 LLVM 的编译型语言预研方案，目标是把安全性、可维护性、互操作性和工具链体验统一起来。
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

这个仓库当前处于预研阶段，目标不是马上写一个半成品编译器，而是先把关键设计决策收敛到一套可以持续执行的方案上。

当前文档给出四类结论：

- Qlang 的语言定位、设计原则与核心语法方向
- 类型系统、内存模型、并发模型与 FFI 方案
- 编译器、LSP、格式化器、文档系统与仓库结构
- 细化到阶段出口标准的功能清单与执行路线图

当前实现已经推进到 P4 backend foundation，并在 P5 上落地了最小可用的 C 互操作闭环：稳定 `extern "c"` 导出、真实 C 宿主集成 harness、`ql ffi header` 头文件生成、library build sidecar header，以及受约束的 `ql build --emit dylib` 共享库输出。建议先看：

- [P1-P4 阶段总览](/roadmap/phase-progress)
- [开发计划](/roadmap/development-plan)
- [实现算法与分层边界](/architecture/implementation-algorithms)

## 核心判断

1. 对开发者最友好的系统级语言，不应该把复杂度直接外露成一堆生命周期标注、模板噪声和脚手架样板。
2. 真正难的工作应该由编译器承担，包括所有权推断、逃逸分析、区域分配、诊断建议和增量分析。
3. 混编不是附加功能，而是语言能否在真实工程中落地的核心能力，所以 ABI、链接、绑定生成和调试体验必须前置设计。
4. 语言规范、编译器架构、工具链和文档站必须一起设计；先写编译器再补工具链，后面一定返工。
