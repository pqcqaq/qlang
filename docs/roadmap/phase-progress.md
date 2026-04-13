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
- package/workspace 已支持 manifest 加载、成员遍历、`.qi` 生成/校验/同步和状态展示；`ql project graph`、`ql project emit-interface --check` 现在会解释 `stale` 原因，并给 `invalid` / `unreadable` 补 detail；package 级和 workspace member 级的 `ql project emit-interface --check` 失败现在也会补 `failing package manifest`；workspace 根 `ql project graph` 在单个 member manifest 无法加载时也会继续输出其余 members，并把坏 member 标成 `package: <unresolved>` + `member_error`；`ql project graph` 里的 `reference_interfaces` 对 `unresolved-manifest` / `unresolved-package` 现在也会带 `detail`，并且每条 reference 现在都会显式带出对应 manifest 路径；如果 direct dependency 下面还有更深层坏引用，graph 现在也会补 `transitive_reference_failures` 计数和 `first_transitive_failure_manifest`；workspace 根 `ql project emit-interface` 在单个 member 发射失败时也会继续输出其余成功 members，并在末尾汇总失败成员数；只有多失败场景才额外补 `first failing member manifest`。如果某个 member 是因为 package 源码错误而发射失败，workspace emit 现在也会当场先补该 member 的 failing package manifest、`failing workspace member manifest`，再给直接重跑 hint；如果某个 member manifest 自身无法加载，workspace emit 和 workspace `emit-interface --check` 的局部错误块现在也都会立即补 `failing workspace member manifest` 和针对该 member manifest 的直接 rerun hint；如果某个 member manifest 能加载但没有 `[package].name`，workspace emit / workspace `emit-interface --check` 的局部 error line 现在也会保留真实命令标签（如 `--changed-only` / `--changed-only --check`），而 workspace `emit-interface --check` 也不再在这里提前退出；package 级 `ql project emit-interface` 在同一个 package 里遇到多个坏源码时也不再在第一处 source diagnostics 就停止，而会继续打印后续 diagnostics，最后统一汇总 failing source file 数；只有多失败场景才额外补 `first failing source file`，并且 direct package emit 失败时现在也会补 failing package manifest + 直接重跑 hint；`ql project emit-interface --changed-only` 的 emit 失败局部 hint 现在也会保留 `--changed-only`，不再退回全量重发；direct package `ql project emit-interface --output <path>` 如果失败，stderr 里的重跑 hint 现在也会保留同一个 `--output <path>`，不再退回默认 `.qi` 路径；如果 direct package emit、`ql build --emit-interface` 或 `ql check --sync-interfaces` 是因为默认 `.qi` 输出路径本身写不进去而失败，stderr 现在也会明确补 `failing interface output path`，并改成先修输出路径再重跑，不再误导成 package 源码错误；`ql build --emit-interface` 在 build 阶段就因为目标 package 源码 diagnostics 失败时，现在也会补 `failing package manifest`，并按最终 build 选项重建直接重跑 hint，不再只剩裸 diagnostics；workspace 根 `ql project emit-interface --check` 在单个 member manifest 无法加载时也会继续检查其余 members，并统一汇总 failing members；只有多失败场景才额外补 `first failing member manifest`；如果 member 的默认 `.qi` 自身 `missing` / `invalid` / `unreadable` / `stale`，局部错误块现在也会先补 `failing package manifest`、`failing workspace member manifest`，再给修复 hint；`ql project emit-interface --changed-only --check` 对已 `valid` 的默认 `.qi` 会报告 `up-to-date interface` 而不写文件，失败时的局部重建 hint 也会保留 `--changed-only`；workspace 根路径上的 `ql check` 也不再在首个 failing member 处停止，而会继续打印后续 member 的依赖/源码错误并给出汇总；这些局部失败块现在也会立即补 `failing workspace member manifest`，而当 member manifest 自身无法加载时也会补针对该 member manifest 的直接 rerun hint，并且只在多失败场景的最终汇总里补 `first failing member manifest`；如果 workspace member manifest 能加载但没有 `[package].name`，workspace `ql check` / `ql check --sync-interfaces` 现在也会在局部错误块里保留真实命令标签并补针对该 member manifest 的直接 rerun hint，而不会先掉回泛化 project error 再继续；package 级 `ql check` 现在也不再在首个坏引用处停止，而会继续汇总多个 direct / transitive failing references，并且只在多失败场景的最终汇总里补 `first failing reference manifest`；普通 `ql check` 对 dependency `.qi` 的 `missing` / `invalid` / `unreadable` / `stale` 失败现在也会在局部错误块里补 `failing referenced package manifest`，再补 owner manifest + reference 文本上下文，而不再只留下依赖包名和 artifact 路径；这些 dependency `.qi` 状态失败块现在也和 package/workspace 一样固定为 `error -> detail/reason -> manifest/context -> hint` 顺序；`ql check --sync-interfaces` 在同一 package 里遇到部分可同步、部分不可修复的引用时，也会保留成功写出的 `.qi` 输出，再汇总剩余 failing references；当直接依赖缺失 `.qi` 但其更深层引用仍损坏时，sync 路径现在也会先补当前可同步的上游 `.qi`，再汇总剩余 transitive failures，并且同样只在多失败场景的最终汇总里补 `first failing reference manifest`；如果某个依赖在 sync 阶段因为自身源码错误或默认输出路径失败而无法发射 `.qi`，stderr 现在会先补 failing package manifest、局部原因和 owner manifest / reference 上下文，再给统一的直接重跑 hint，而不再把 hint 提前到 owner/reference 上下文前面；`.qi` 维护链路上的 source/member/reference/stale/hint 路径显示现在也统一做了规范化，不再把 `../` 形式和直达路径混在同一条失败面里，并且这些 `first failing *` 指针只在多失败场景保留；`ql check` / `ql check --sync-interfaces` 对坏的引用 manifest 现在也会补 `detail`、`failing reference manifest` 和 owner/reference 修复提示，而不再只打印裸 project error；`ql build --emit-interface` 在 build 成功但 package 级接口发射失败时，也会继承这套 source 级汇总；只有多失败场景才额外补 `first failing source file`，然后再补 failing package manifest，并明确 build artifact 已保留。
- direct package `ql check` / `ql check --sync-interfaces` 在目标 manifest 自身无效时，现在也会保留真实命令标签，并立即补 `failing package manifest` 与针对该 manifest 的直接 rerun hint。
- direct package `ql check` / `ql check --sync-interfaces` 在 package `src/` 目录缺失时，现在也会保留真实命令标签，并立即补 `failing package manifest`、`failing package source root` 与针对该 manifest 的直接 rerun hint。
- direct package `ql check` / `ql check --sync-interfaces` 在 package `src/` 目录存在但没有任何 `.ql` 源文件时，现在也会保留真实命令标签，并立即补 `failing package manifest`、`failing package source root` 与针对该 manifest 的直接 rerun hint。
- direct package `ql check` / `ql check --sync-interfaces` 在 package 源码本身报 diagnostics 时，现在也会在 diagnostics 之后补 `failing package manifest` 与针对该 manifest 的直接 rerun hint。
- direct package `ql check` / `ql check --sync-interfaces` 在 package 因 reference failure 失败时，现在也会在局部 reference diagnostics 之后补 `failing package manifest` 与针对该 manifest 的直接 rerun hint。
- workspace 根 `ql check` / `ql check --sync-interfaces` 在 member package 的 `src/` 目录缺失时，现在也会保留真实命令标签，并立即补 `failing package manifest`、`failing workspace member manifest`、`failing package source root` 与针对该 member manifest 的直接 rerun hint。
- workspace 根 `ql check` / `ql check --sync-interfaces` 在 member package 的 `src/` 目录存在但没有任何 `.ql` 源文件时，现在也会保留真实命令标签，并立即补 `failing package manifest`、`failing workspace member manifest`、`failing package source root` 与针对该 member manifest 的直接 rerun hint。
- workspace 根 `ql check` / `ql check --sync-interfaces` 在 member manifest 自身加载或解析失败时，现在也会保留真实命令标签，并立即补 `failing workspace member manifest` 与针对该 member manifest 的直接 rerun hint。
- workspace 根 `ql check` / `ql check --sync-interfaces` 在 member package 源码本身报 diagnostics 时，现在也会补 `failing workspace member manifest` 与针对该 member manifest 的直接 rerun hint。
- workspace 根 `ql check` / `ql check --sync-interfaces` 在 member package 因 reference failure 失败时，现在也会在局部 reference diagnostics 之后补 `failing workspace member manifest` 与针对该 member manifest 的直接 rerun hint。
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
