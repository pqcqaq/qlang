# 阶段总览

> 最后同步：2026-05-03

## 状态

| 阶段 | 状态 | 结果 |
| --- | --- | --- |
| Phase 1 | 已完成 | lexer、parser、formatter、基础 CLI |
| Phase 2 | 已完成 | HIR、resolve、typeck、diagnostics、query、最小 LSP |
| Phase 3 | 已完成 | MIR、ownership、cleanup-aware 分析 |
| Phase 4 | 已完成 | LLVM backend、obj/exe/dylib/staticlib 路径 |
| Phase 5 | 已完成 | C ABI、header projection、host integration |
| Phase 6 | 已完成 | same-file hover、definition、rename、completion、semantic tokens、symbols |
| Phase 7 | 进行中 | async、runtime、task-handle、build、interop 的保守扩面 |
| Phase 8 | 进行中 | package/workspace、`.qi`、local dependencies、dependency-backed tooling |

## 主线

1. 先保证小型本地 Qlang workspace 可真实使用。
2. 再扩 stdlib、generic/backend、workspace LSP 和分发。
3. 高级语言能力必须跟随回归和真实项目 smoke 推进。

## 继续阅读

- [当前支持基线](/roadmap/current-supported-surface)
- [开发计划](/roadmap/development-plan)
- [设计稿总览](/plans/)
