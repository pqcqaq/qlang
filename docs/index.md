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

## 项目状态

- Phase 1 到 Phase 6 的基础能力已经落地
- Phase 7 已建立最小 runtime/executor、task-handle 类型面、共享 runtime hook ABI skeleton，以及受控的 async library/program build 子集
- Phase 8 已落地最小 `qlang.toml` manifest graph、`.qi` emit/load、`ql project graph` / `ql project emit-interface`、package-aware `ql check --sync-interfaces` 与首批 dependency-backed cross-file LSP/query 合同
- 当前真实支持面与未支持边界已集中收口到 [当前支持基线](/roadmap/current-supported-surface)
- executable smoke harness 当前锁定 `60` 个 sync 示例与 `222` 个 async 示例；这两个数字已按目录与测试代码重新核对
- sync/async tuple assignment executable surface 现也已覆盖 same-file `const` / `static` item 名与 same-file `use ... as ...` alias 驱动的元组索引写入

## 推荐阅读

- [编译器、术语与生态入门](/getting-started/compiler-primer)
- [当前支持基线](/roadmap/current-supported-surface)
- [P1-P8 阶段总览](/roadmap/phase-progress)
- [开发计划](/roadmap/development-plan)
- [Phase 7 并发、异步与 Rust 互操作](/plans/phase-7-concurrency-and-rust-interop)
- [实现算法与分层边界](/architecture/implementation-algorithms)

## 语言边界

- Qlang 是独立语言，不以 Rust 语法为模板。
- Rust 目前用于编译器与工具链实现，以及当前互操作路径之一。
- 语言设计以 `/design/` 和 `/vision` 下的文档为准。
