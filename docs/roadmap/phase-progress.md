# P1-P8 阶段总览

> 最后同步：2026-05-03

这页只保留阶段状态和当前主线。

## 阶段状态

| 阶段 | 状态 | 结论 |
| --- | --- | --- |
| Phase 1 | 已完成 | lexer / parser / formatter / 基础 CLI |
| Phase 2 | 已完成 | HIR / resolve / typeck / diagnostics / 最小 query / 最小 LSP |
| Phase 3 | 已完成 | MIR / ownership / cleanup-aware 分析 |
| Phase 4 | 已完成 | LLVM backend 与主要 artifact 路径 |
| Phase 5 | 已完成 | C ABI、header projection、host 集成 |
| Phase 6 | 已完成 | same-file hover / definition / rename / completion / semantic tokens / document symbol |
| Phase 7 | 进行中 | async / runtime / task-handle / build / interop 的保守扩面 |
| Phase 8 | 进行中 | package/workspace、`.qi`、local dependencies、dependency-backed tooling |

## 当前主线

1. 先把 Qlang 做到“可真实使用的最小项目语言”。
2. 主线继续围绕 manifest、dependency-aware build/backend、真实 `stdlib`、真实 workspace LSP、安装与分发。
3. 当前仍以实现、回归、文档同步收口为主，不再用固定日期承诺整条主线。

## 现在重点

- 收紧 `stdlib` 的 generic carrier 和 canonical array API。
- 继续补 direct local dependency 的真实项目常见路径。
- 继续把 `qlsp` 的基础 workspace 体验做稳。
- 继续把长期历史说明从入口页移到代码和回归测试。

## 明确后置

- cross-file rename / workspace edits / 更完整 code actions
- 更宽的 async/runtime/Rust interop
- 新语法和更远的类型系统设计
- registry / publish workflow

## 继续阅读

- [当前支持基线](/roadmap/current-supported-surface)
- [开发计划](/roadmap/development-plan)
- [工具链设计](/architecture/toolchain)
