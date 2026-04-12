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

- 继续收紧 `qlang.toml`、workspace member、references 和 `.qi` 生命周期；`ql project emit-interface --changed-only --check` 已落实为对已 `valid` artifact 报 `up-to-date interface` 且不写文件，workspace 根 `ql check` 已改成聚合 failing members，并在最终汇总里补 `first failing member manifest`，package 级 `ql check` 也已改成聚合多个 direct / transitive failing references，并在最终汇总里补 `first failing reference manifest`，workspace 根 `ql project graph` 已改成容忍坏 member，`reference_interfaces` 的 unresolved 状态也已补上 detail，并且每条 reference 现在都会显式带出 manifest 路径；当 direct dependency 自己看起来可用但其更深层仍有坏引用时，graph 现在也会补 `transitive_reference_failures` 计数和 `first_transitive_failure_manifest`；`ql check` / `ql check --sync-interfaces` 对坏引用 manifest 也已补上 detail + hint，`ql check --sync-interfaces` 在部分引用可同步时也会保留成功写出的 `.qi` 输出，并且当中间依赖缺失 `.qi` 但更深层仍有坏引用时也会先补可同步的上游 `.qi`，并在最终汇总里补 `first failing reference manifest`；当依赖同步阶段因为自身源码错误无法发射 `.qi` 时，stderr 现在会先复用统一的 failing package manifest + `ql project emit-interface <manifest>` 修复提示，再补 owner manifest / reference 指向，避免 source diagnostics 脱离引用上下文且不再重复给两套 hint；这条 `.qi` 维护失败链路里的 source/member/reference/stale/hint 路径显示也已统一规范化，避免同一轮输出里混入 `../` 形式和直达路径；workspace 根 `ql project emit-interface` 和 workspace 根 `ql project emit-interface --check` 现在也都已改成容忍单个坏 member 并汇总，同时在最终汇总里补 `first failing member manifest`；如果 workspace member 是因为 package 源码错误而发射失败，workspace emit 现在也会当场补该 member 的 failing package manifest 和直接重跑 hint；package 级 `ql project emit-interface` 现在也已改成聚合多个 failing source file，并补 `first failing source file`，而且 direct package emit 失败时也会补 failing package manifest 和直接重跑 hint；`ql build --emit-interface` 在 build 已成功但 package 级接口发射失败时，也会继承 source 级汇总，再补 failing package manifest，并明确 build artifact 仍保留，下一步继续统一其余 `.qi` 维护路径的状态、detail 和修复提示。
- 继续扩 `ql project graph`、`ql project emit-interface`、`ql build --emit-interface`、`ql check --sync-interfaces` 的一致性，下一步优先把 dependency sync 与普通 `ql check` 的 failing-reference detail / summary 顺序继续对齐，并压缩同一条 reference 在局部失败和最终汇总中的重复描述，避免剩余输出面继续分叉。
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
