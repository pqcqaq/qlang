# P1-P8 阶段总览

> 最后同步：2026-04-19

这页只保留阶段级结论和当前焦点。

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

## 已完成的关键进展

- 编译器主路径已经稳定为 AST -> HIR -> resolve -> typeck -> MIR -> LLVM。
- package/workspace 基础能力已落地：`ql project init`、`targets`、`graph`、`lock`、`emit-interface`。
- project-aware `ql build` / `ql run` / `ql test` 已可在 package/workspace 根目录工作。
- `ql build` / `ql run` 现在也可直接从 project 源码 target 路径进入 project-aware 流程；package 内源码路径和 workspace member 源码路径都不再掉回裸单文件输出语义。
- `ql test` 新增 exact target rerun：`--target` 可精确选择已发现测试，直接运行 project `tests/` 下的单个测试文件时也会保留 project-aware 语义，workspace member 入口也不再掉回 package-only profile。
- `ql project graph` / `ql project targets` / `ql project lock` 现在也会在 workspace member 源码路径入口上继承外层 workspace 上下文。
- `ql project emit-interface` 现在也支持从 workspace member `.ql` 路径恢复外层 workspace 视角；当前保守边界是不带 `--output`。
- `qlang.toml` 已支持最小本地依赖、target path 和默认 profile。
- 第一版 `qlang.lock`、`ql.check --json`、`ql.build --json`、`ql.test --json` 已落地。
- healthy workspace 下的 dependency-backed LSP 已有一批可依赖能力：workspace symbol、source-preferred navigation、semantic tokens、保守 same-file rename；source-preferred navigation 现在同时覆盖 workspace members 和 workspace 外本地路径依赖，`workspace/symbol` 对本地依赖源码里的 methods / trait methods / extend methods 也已有源码优先回归保护。
- `workspace` 外本地路径依赖的 import references 现在也走源码优先路径；broken-source fallback 已补齐到这一条路径。
- `workspace/symbol` 现在也会对 workspace 外本地路径依赖做源码优先返回，并保留 `.qi` 回退；这条能力已补到 `workspace_roots` / 无打开文档入口，当前已锁住 value / method / trait / extend symbol。
- `workspace/symbol` 对 source-preferred 本地依赖的排除现在按 manifest 身份而不是 package name 执行；真实项目里即使存在同名本地依赖，也不会再把另一个依赖的 `.qi` symbol 一起过滤掉。
- parse-error 下的 current-document rename 也已有保守回归保护；最近新增的一条是 `config.child()?.leaf().value` 这类 question-unwrapped method-result member field。

## 当前主线

1. 继续把 manifest 和 dependency-aware build 做实，不再停留在窄的 `extern "c"` 跨包路径。
2. 继续把基础 LSP 做到真实项目可依赖，优先是 definition / references / workspace symbol / semantic tokens。
3. 在现有 lock / JSON 输出基础上补 CI、分发和可复现构建约定。

## 明确后置

- cross-file rename / workspace edits / code actions
- registry / publish
- 更宽的 async/runtime/Rust interop 扩面
- 新语法和更远的类型系统设计

## 继续阅读

- [当前支持基线](/roadmap/current-supported-surface)
- [开发计划](/roadmap/development-plan)
- [工具链设计](/architecture/toolchain)
