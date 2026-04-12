# P1-P8 阶段总览

> 最后同步：2026-04-12

这页只保留阶段级结论。详细增量记录已移出主阅读路径。

## 总体结论

- Phase 1 到 Phase 6 的基础设施已经落地：lexer、parser、formatter、diagnostics、HIR、resolve、typeck、MIR、borrowck、LLVM build、CLI 与 same-file LSP/query 都在代码里持续演进。
- Phase 7 已有可用子集：`async fn`、`await`、`spawn`、`for await`、最小 `ql-runtime`、task-handle lowering、保守的 program/library async build surface。
- Phase 8 已进入真实交付面：`qlang.toml` package/workspace loader、`ql project graph`、`.qi` emit/load、`ql build --emit-interface`、`ql check --sync-interfaces`、首批 dependency-backed cross-file tooling。
- 当前主线是沿既有边界扩面，不是回头重写前六阶段。
- 真相源以 `crates/*`、`crates/ql-cli/tests/*`、`crates/ql-lsp/tests/*`、`crates/ql-analysis/tests/*` 和 [当前支持基线](/roadmap/current-supported-surface) 为准。

## 阶段状态

| 阶段 | 状态 | 当前结论 |
| --- | --- | --- |
| Phase 1 | 已完成 | 前端最小闭环：lexer / parser / formatter / CLI |
| Phase 2 | 已完成 | HIR / resolve / typeck / diagnostics / 最小 query / 最小 LSP |
| Phase 3 | 已完成 | 结构化 MIR / ownership facts / cleanup-aware 分析 |
| Phase 4 | 已完成 | `ql build`、LLVM IR、`obj` / `exe` / `staticlib` / `dylib` 主路径 |
| Phase 5 | 已完成 | C ABI、header projection、真实 host 集成 |
| Phase 6 | 已完成 | same-file hover / definition / rename / completion / semantic tokens / document symbol |
| Phase 7 | 进行中 | async/runtime/task-handle/build/interop 保守扩面 |
| Phase 8 | 进行中 | package/workspace、`.qi`、dependency-backed cross-file tooling 扩面 |

## 各阶段一句话总结

- Phase 1：把源码稳定变成 AST，并保证 formatter 与基础 CLI 可用。
- Phase 2：把“能解析”推进到“能做名称解析、类型检查和最小 IDE 查询”。
- Phase 3：建立 MIR 和 ownership/cleanup 分析，为后续 lowering 提供稳定中层。
- Phase 4：打通 `ql build` 和 LLVM backend，让受支持子集可以产出真实 artifact。
- Phase 5：把 C ABI 固定成当前稳定互操作边界，并生成 header sidecar。
- Phase 6：把 same-file 编辑器语义收口到共享 analysis/query truth surface。
- Phase 7：继续扩 async/runtime/task-handle/build/interop，但只开放已被语义、borrowck、codegen 和黑盒回归共同证明的子集。
- Phase 8：把 package/workspace、`.qi` 和 dependency-backed tooling 做成共享边界，避免 CLI 和 LSP 各写一套依赖模型。

## 当前进度对账

- 编译器主路径稳定为 AST -> HIR -> resolve -> typeck -> MIR -> LLVM IR。
- CLI 当前已实现 `ql check`、`ql fmt`、`ql mir`、`ql ownership`、`ql runtime`、`ql build`、`ql project`、`ql ffi`；`ql build --emit` 当前稳定面为 `llvm-ir|asm|obj|exe|dylib|staticlib`。
- package/workspace 已支持 manifest 加载、成员遍历、`.qi` 生成/校验/同步和状态展示；`ql project graph`、`ql project emit-interface --check` 现在会解释 `stale` 原因，并给 `invalid` / `unreadable` 补 detail；workspace 根 `ql project graph` 在单个 member manifest 无法加载时也会继续输出其余 members，并把坏 member 标成 `package: <unresolved>` + `member_error`；`ql project graph` 里的 `reference_interfaces` 对 `unresolved-manifest` / `unresolved-package` 现在也会带 `detail`，并且每条 reference 现在都会显式带出对应 manifest 路径；workspace 根 `ql project emit-interface` 在单个 member 发射失败时也会继续输出其余成功 members，并在末尾汇总失败成员数；workspace 根 `ql project emit-interface --check` 在单个 member manifest 无法加载时也会继续检查其余 members，并统一汇总 failing members；`ql project emit-interface --changed-only --check` 对已 `valid` 的默认 `.qi` 会报告 `up-to-date interface` 而不写文件；workspace 根路径上的 `ql check` 也不再在首个 failing member 处停止，而会继续打印后续 member 的依赖/源码错误并给出汇总；package 级 `ql check` 现在也不再在首个坏引用处停止，而会继续汇总多个 direct / transitive failing references；`ql check --sync-interfaces` 在同一 package 里遇到部分可同步、部分不可修复的引用时，也会保留成功写出的 `.qi` 输出，再汇总剩余 failing references；当直接依赖缺失 `.qi` 但其更深层引用仍损坏时，sync 路径现在也会先补当前可同步的上游 `.qi`，再汇总剩余 transitive failures；`ql check` / `ql check --sync-interfaces` 对坏的引用 manifest 现在也会补 `detail` 和 owner/reference 修复提示，而不再只打印裸 project error；普通 `ql check` 仍会在分析前显式拒绝本地依赖包的 `missing` / `invalid` / `unreadable` / `stale` `.qi` 并给出统一修复提示。
- dependency-backed cross-file tooling 已有首批可用合同：import path completion、dependency symbol hover/definition/declaration/references、enum variant completion/typeDefinition、显式 struct field-label completion、direct dependency struct literal value-root query/`typeDefinition`、同构 inline tuple / array destructured dependency locals 的 value-root query 与 named-local member `typeDefinition`、direct dependency iterable call tuple / array destructured locals 的 value-root query 与 member field / method query，以及语法局部可恢复 receiver、direct indexed iterable receiver（含 `config.maybe_children()?[0].value` / `get()`、`kids()?[0].value` / `get()`、对应的 value-root query/`typeDefinition`、`config.maybe_children()?[0].leaf` / `kids()?[0].leaf` 这类 member `typeDefinition`，以及 direct `if` / `match` structured question-indexed receiver与对应的 member `typeDefinition`）和 indexed bracket target 的最小 value-root/member/query/typeDefinition；这条 bracket-target value-root 现在也覆盖 direct structured question-indexed `(if ...)[0]` / `(match ...)[0]`。
- `ramdon_tests/` 已提交为 executable smoke 基线；目录仍在 `.gitignore` 中，开发者本地可以继续追加忽略样例。

## 当前最值得继续推进的方向

- 扩 Phase 7 中“前端已支持、后端仍保守拒绝”的 async/runtime/build 缺口。
- 扩 Phase 8 中 dependency-backed completion/query/typeDefinition 的 receiver slice。
- 继续收紧 `.qi` 生命周期，优先补 CLI / LSP 共享事实面里的真实缺口，而不是只追加同一 receiver slice 的锁行为。
- 保持文档入口短版，把逐轮回归明细继续放在归档和测试里。

## 继续阅读

- [当前支持基线](/roadmap/current-supported-surface)
- [开发计划](/roadmap/development-plan)
