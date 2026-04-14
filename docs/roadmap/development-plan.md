# 开发计划

关联文档：

- [当前支持基线](/roadmap/current-supported-surface)：当前实现边界
- [P1-P8 阶段总览](/roadmap/phase-progress)：阶段状态
- [`/plans/`](/plans/)：分阶段设计与切片记录

本页只回答三件事：当前判断、最近主线、交付原则。

## 当前判断

- Phase 1 到 Phase 6 已经形成稳定地基，不再是“随时可能推倒重来”的探索阶段。
- 当前并行主线有两条：Phase 7 继续扩 async/runtime/task-handle/build/interop，Phase 8 继续扩 package/workspace、`.qi` 和 dependency-backed cross-file tooling。
- 当前优先级是把已存在的分析与 lowering 边界继续推到用户可见能力，而不是回头重写基础设施。
- 文档、测试和实现必须保持同一事实面；入口页保留短版，详细增量放到计划文档和归档。

## 总体原则

### 1. 尽早形成真实闭环

- 一个功能至少要有 CLI 路径、diagnostics、回归测试和文档入口。
- “能解析”不等于“可交付”；用户可见 build/tooling surface 才算闭环。

### 2. 一层只维护一份真相源

- AST、HIR、resolve、typeck、MIR、codegen、runtime、LSP 各自维护自己的事实。
- CLI、LSP、FFI 和文档不重复实现语义，只消费共享结果。

### 3. 先把失败模型做对，再扩公开能力

- 保守拒绝优于错误支持。
- 只有已经被语义、borrowck、codegen、黑盒回归共同证明的 surface 才进入“当前支持”。

### 4. 只补必要测试

- 新能力至少补一条 blocker 回归和一条用户可见路径回归。
- coverage-only 的相邻变体不再作为主线目标，除非它直接保护新功能或刚修掉的回归。

### 5. C ABI 仍是当前稳定互操作边界

- Rust host 继续走 `Rust <-> C ABI <-> Qlang`。
- 更深的 Rust/C++ 互操作后置，不在当前主线里抢优先级。

### 6. 文档跟着实现走

- README、roadmap、plans、示例和测试结果必须同步。
- 当前支持页写事实，计划页写方向，旧状态进 archive。

## 已完成阶段（P0-P6）

| 阶段 | 状态 | 已形成的稳定边界 |
| --- | --- | --- |
| Phase 0 | 已完成 | 语言定位、设计原则、仓库结构、阶段划分 |
| Phase 1 | 已完成 | lexer / parser / formatter / CLI 前端闭环 |
| Phase 2 | 已完成 | HIR / resolve / typeck / diagnostics / 最小 query / 最小 LSP |
| Phase 3 | 已完成 | 结构化 MIR / ownership facts / cleanup-aware 分析 |
| Phase 4 | 已完成 | `ql build`、LLVM IR、`obj` / `exe` / `staticlib` / `dylib` |
| Phase 5 | 已完成 | C ABI、header projection、host 集成 |
| Phase 6 | 已完成 | same-file query / rename / completion / semantic tokens / LSP parity |

## 当前主线

### Phase 7：并发、异步与 Rust 互操作

当前重点：

- 扩 async/runtime/task-handle/build 子集，但保持 borrowck、lowering、黑盒回归一致。
- 继续补 program-mode 与 library-mode 的 build parity；当前 program-mode artifact 已覆盖 `llvm-ir`、`asm`、`obj`、`exe`。
- 保持 runtime hook、executor、task handle 与 C ABI 的边界清晰。

当前不做：

- 不承诺完整 async 语义。
- 不把 Rust 专用 ABI 当成当前互操作主线。
- 不用文档描述代替真实 build 支持。

### Phase 8：package / workspace / `.qi` / dependency-backed tooling

当前重点：

- 继续收紧 `qlang.toml`、workspace member、references 和 `.qi` 生命周期；`ql project emit-interface --changed-only --check` 已落实为对已 `valid` artifact 报 `up-to-date interface` 且不写文件，失败时的局部重建 hint 也会保留 `--changed-only`，避免把“只修坏接口”的维护路径退回全量重发；普通 `ql project emit-interface --changed-only` 的 emit 失败局部 hint 现在也会保留 `--changed-only`，不再退回全量重发；package 级和 workspace member 级的 `ql project emit-interface --check` 失败现在也会补 `failing package manifest`，workspace 根 `ql check` 已改成聚合 failing members，并在局部失败块里补 `failing workspace member manifest`，只在多失败场景的最终汇总里补 `first failing member manifest`；其中 workspace member 缺 `[package].name` 的两条 `ql check` 路径现在都已收敛成同一失败面：无论 manifest 已加载还是在加载阶段就失败，局部错误块都会补 `failing package manifest`、`failing workspace member manifest` 和“先修 package manifest 再重跑”，而且命令标签会保留 `--sync-interfaces` 等真实选项；workspace member 的 package source diagnostics 和 reference failures 现在也会补同样的 `failing package manifest` 上下文，不再只回指 workspace member manifest；package 级 `ql check` 也已改成聚合多个 direct / transitive failing references，并且只在多失败场景的最终汇总里补 `first failing reference manifest`；坏 reference manifest 的首行 `error:` 现在也会保留真实 `ql check` / `ql check --sync-interfaces` 标签，不再退回无标签的泛化报错；普通 `ql check` 对坏 dependency `.qi` 的首行 `error:` 现在也会保留真实 `ql check` 标签，不再退回无标签的 artifact 报错；package / workspace `ql project emit-interface --check` 对坏默认 `.qi` 的首行 `error:` 现在也会保留真实 `ql project emit-interface --check` / `--changed-only --check` 标签，不再退回无标签的 `interface artifact ...`，最终 failing-members 汇总也会保留真实命令标签；workspace 根 `ql project graph` 已改成容忍坏 member，`reference_interfaces` 的 unresolved 状态也已补上 detail，并且每条 reference 现在都会显式带出 manifest 路径；当 direct dependency 自己看起来可用但其更深层仍有坏引用时，graph 现在也会补 `transitive_reference_failures` 计数和 `first_transitive_failure_manifest`；普通 `ql check` 对坏 dependency `.qi` 现在也会在局部错误块里补 `failing referenced package manifest`，再补 owner manifest + reference 文本上下文，`invalid` / `unreadable` / `stale` 这几类 dependency `.qi` 失败块现在也已和 package/workspace 统一成 `error -> detail/reason -> manifest/context -> hint` 顺序；如果 direct package emit、`ql build --emit-interface` 或 `ql check --sync-interfaces` 是因为默认 `.qi` 输出路径本身写不进去而失败，stderr 现在也会补 `failing interface output path`，并把重跑 hint 改成先修输出路径；其中 build-side `ql build --emit-interface` 失败现在也会保留原始 build 命令选项，不再退回 `ql project emit-interface ...`；如果 direct package `ql project emit-interface --output <path>` 失败，stderr 里的重跑 hint 现在也会保留同一个 `--output <path>`，不再退回默认 `.qi` 路径；`ql build --emit-interface` 在 build 阶段就因为目标 package 源码 diagnostics、toolchain、build 输入路径本身无效或不可读、主 build 输出路径、`dylib` 导出面不满足要求，或 build-side header 配置本身失败（header 输出路径不可写、与主 artifact 路径冲突，header 选项搭到了非 `dylib|staticlib` 上，或请求了 `imports|both` 但源码里没有 imported `extern \"c\"` 声明）而失败时，现在也会补 `failing package manifest` 并按最终 build 选项重建直接重跑 hint；输入路径不是文件或读不到时都会额外补 `failing build input path` 并改成先修 `build input path`，主输出路径失败时会额外补 `failing build output path`，而且现在连输出目录创建阶段卡在父路径上也会继续归到最终 artifact 路径上；如果 toolchain stderr 已明确指向最终输出文件打不开，包括归档器/`lib.exe` 这类 staticlib 归档失败，也会改成先修 `build output path`，同时继续保留 intermediate artifact 提示；`dylib` 缺少公开 `extern \"c\"` 导出时会改成先修 `dylib export surface`，header 输出失败或 collision 时会额外补 `failing build header output path`，而且现在连 header 输出目录创建阶段卡在父路径上也会继续归到最终 header artifact 路径上；header/emit 不兼容时会改成先修 `build header configuration`，header import 面为空时会改成先修 `build header import surface`；其余 toolchain 失败仍按 `build toolchain` 处理，但不会误报最终 build artifact 已保留；`ql build --emit-interface` 在 build 已成功但 package 级接口发射失败时，现在也会保留原始 `ql build ... --emit-interface` 选项，而不是退回 project-only rerun；这条规则现在对 single / multi-source build-side interface failure 都已经对齐，而且如果真正失败点是 package manifest 缺 `[package].name`、package `src/` 根目录缺失，或 `src/` 存在但没有任何 `.ql` 文件，也会分别改成更具体的 `package manifest` / `package source root` 修复提示；direct package `ql project emit-interface` 在 manifest 缺 `[package].name` 或 package `src/` 根目录缺失时，现在也会改成对应的 `package manifest` / `package source root` 修复提示，不再统称为 `package interface error`；workspace member `ql project emit-interface` 在 manifest 能加载但没有 `[package].name`，或 member package `src/` 根目录缺失时，现在也会改成对应的 `package manifest` / `package source root` 修复提示，而不再掉回 generic interface-error hint；如果 workspace member 在 `ql project emit-interface` 路径的加载阶段就是因为 `[package]` 缺 `name` 而失败，局部错误块现在也会收敛成 `failing package manifest` + `failing workspace member manifest` + “先修 package manifest 再重跑”，而不再只给 workspace-member-manifest hint；workspace member `ql project emit-interface --check` 在 member manifest 能加载但没有 `[package].name`，或 member manifest 在加载阶段就是因为缺 `[package].name` 而失败时，现在也会统一改成 `failing package manifest` + `failing workspace member manifest` + “先修 package manifest 再重跑”的局部提示，而不再只给 workspace-member-manifest hint；`ql check` / `ql check --sync-interfaces` 对坏引用 manifest 也已补上 `detail`、`failing reference manifest` 和 owner/reference hint，`ql check --sync-interfaces` 在部分引用可同步时也会保留成功写出的 `.qi` 输出，并且当中间依赖缺失 `.qi` 但更深层仍有坏引用时也会先补可同步的上游 `.qi`，并且只在多失败场景的最终汇总里补 `first failing reference manifest`；当依赖同步阶段因为自身源码错误或默认输出路径失败而无法发射 `.qi` 时，stderr 现在会先补 failing package manifest、局部原因和 owner manifest / reference 指向，再给统一的 `ql project emit-interface <manifest>` 修复提示，避免 source diagnostics 或 output-path diagnostics 脱离引用上下文，也不再把 hint 提前到 owner/reference 上下文前面；这条 `.qi` 维护失败链路里的 source/member/reference/stale/hint 路径显示也已统一规范化，避免同一轮输出里混入 `../` 形式和直达路径；workspace 根 `ql project emit-interface` 和 workspace 根 `ql project emit-interface --check` 现在也都已改成容忍单个坏 member 并汇总，但只有多失败场景的最终汇总才会补 `first failing member manifest`；这些 workspace failing-members 汇总和 package source 汇总的最终 `error:` 行现在也会保留真实命令标签，不再退回 `interface emission/check found ...`；如果 workspace member 是因为 package 源码错误而发射失败，workspace emit 现在也会当场先补该 member 的 failing package manifest、`failing workspace member manifest`，再给直接重跑 hint；如果 workspace member manifest 自身无法加载，workspace emit 和 workspace `emit-interface --check` 现在也会在局部错误块里立即补 `failing workspace member manifest` 和针对该 member manifest 的直接 rerun hint；如果 workspace member manifest 能加载但没有 `[package].name`，workspace emit / workspace `emit-interface --check` 的局部 error line 现在也会保留真实命令标签，而 workspace `emit-interface --check` 也不会在这里提前退出；如果 workspace member 的默认 `.qi` 自身 `missing` / `invalid` / `unreadable` / `stale`，workspace `emit-interface --check` 现在也会在局部错误块里先补 `failing package manifest` 和 `failing workspace member manifest`，再给修复 hint；package 级 `ql project emit-interface` 和 `ql build --emit-interface` 现在也都把 package source failure 统一成同一规则：只在多失败场景的最终汇总里补 `first failing source file`，单失败则依赖局部 diagnostics 给出源码路径；两条路径继续共享 `failing package manifest` 和直接重跑 hint。
- 继续扩 `ql project graph`、`ql project emit-interface`、`ql build --emit-interface`、`ql check --sync-interfaces` 的一致性；这几轮已经补齐 direct package `ql check` / `ql check --sync-interfaces` 在目标 manifest 自身无效、manifest 缺 `[package].name`、package `src/` 目录缺失、空 `src/`、package source diagnostics、package reference failures，以及 workspace member manifest 加载/解析失败、member `src/` 目录缺失、空 `src/`、member source diagnostics、member reference failures 时的命令标签或直接 rerun hint；`ql project graph` 现在也把 direct manifest 缺 `[package].name` 和 workspace member 缺 `[package].name` 收敛到同一 `does not declare [package].name` 失败面，把 `detail` / `member_error` 里的 manifest 路径统一成 graph-relative 形式，并给 unresolved workspace members 补了结构化 `member_status`；transitive reference 摘要也会补第一个坏点的 status/detail；如果第一个坏点是 stale，还会带出对应的 stale reason。下一步继续收尾 direct package / workspace / build / sync 四条路径上剩余的 reference hint 与 failure surface 对齐。
- 继续扩 dependency-backed completion / query / `typeDefinition` 的最小 receiver slice，并在已打通的 direct dependency struct literal value-root、同构 tuple / array destructured dependency locals、direct dependency iterable call destructured locals 基础上继续扩面。

当前不做：

- 不承诺真实 dependency build graph / publish workflow。
- 不提前开放 cross-file rename / workspace edits。
- 不把 broader dependency semantics 写成“已支持”。

## 后续阶段

P8 之后的工作仍以当前两条主线收口结果为前提，暂不单独拆新阶段承诺。可以明确排在后面的主题包括：

- 更完整的 dependency/workspace 编辑语义
- 更宽的 runtime 与 async build surface
- `ql new` / `ql init` / `ql test` / `ql doc` 等项目级工作流
- 设计稿中尚未实现的语言能力

## 每个阶段都必须交付的横向事项

- 回归测试：最小但足够保护用户面
- 文档同步：当前支持页、开发计划和相关设计页同轮更新
- CLI 行为：错误信息、默认输出路径、帮助文本与源码一致
- 示例与 smoke：必要时补到 `ramdon_tests/` 或 CLI 黑盒矩阵

## 当前最推荐的阅读顺序

1. [当前支持基线](/roadmap/current-supported-surface)
2. [P1-P8 阶段总览](/roadmap/phase-progress)
3. [Phase 7 并发、异步与 Rust 互操作](/plans/phase-7-concurrency-and-rust-interop)
4. [实现算法与分层边界](/architecture/implementation-algorithms)
5. [工具链设计](/architecture/toolchain)
