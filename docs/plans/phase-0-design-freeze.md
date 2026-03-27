# Phase 0 设计冻结与语言定位

## 目标

Phase 0 的任务是把“想做一门语言”收敛成一组不会频繁摇摆的基础决策：

- 语言定位
- 语法风格
- 类型与并发方向
- LLVM / FFI / 工具链边界
- 仓库结构与阶段路线图

## 关键设计结论

### 语言定位

Qlang 的目标不是做一门“语法上很特别”的语言，而是做一门：

- 面向 LLVM 的编译型语言
- 以安全、可维护、工程体验为先
- 允许和 C / C++ / Rust 现实协作
- 把简单留给程序员，把复杂留给编译器和工具链

### 语法方向

语法风格明确吸收了 Rust、Kotlin、Go、TypeScript 的优点，但避免直接复制：

- Rust：表达式导向、代数数据类型、trait / impl、强语义边界
- Kotlin：友好的 API 风格、smart cast 倾向、工程可读性
- Go：简洁、明确、工具优先、工程落地感
- TypeScript：开发体验优先、编辑器能力、易理解的语义反馈

收敛后的原则是：

- 默认显式
- 默认不可变
- 语义优先于语法技巧
- 不为了炫技引入独立多返回语法或过多局部魔法

### 运行时与互操作

Phase 0 明确把这些方向放进主线：

- LLVM 作为主要后端
- C ABI 作为第一层现实互操作桥梁
- Rust 通过 C ABI 稳定协作
- C++ 通过 shim 工作流接入，而不是一开始就做全量直接绑定

### 工程策略

Phase 0 还冻结了后续开发方式：

- compiler 永远测试驱动
- 文档和实现必须同步
- parser / semantics / MIR / codegen / LSP 必须分层
- 同一个 truth surface 不能在 CLI、LSP、bridge 三处重复实现

## 对后续阶段的影响

这一步最重要的成果不是某条语法规则，而是把后续阶段的方向锁住了：

- P1 先做前端闭环
- P2 先做语义地基，而不是上来追求完整类型系统
- P3 先做 MIR 和 ownership foundation
- P4/P5 先做真实产物和最小 FFI 闭环
- P6 再把 editor-facing truth surface 做扎实

## 当前仍有效的边界

Phase 0 的这些判断到现在仍然成立：

- 不提前引入宏系统
- 不提前引入复杂效果系统
- 不提前引入直接 C++ 绑定生成
- 不为了语法“省字”引入过多新符号机制
- 不把跨文件/project-wide IDE 能力冒进到 P2/P6 的 same-file truth surface 里

## 归档

原始设计稿已归档到：

- [`/plans/archive/phase-0/2026-03-25-qlang-design`](/plans/archive/phase-0/2026-03-25-qlang-design)
