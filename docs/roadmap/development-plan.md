# 开发计划

关联文档：

- [当前支持基线](/roadmap/current-supported-surface)：今天真实可依赖的实现边界
- [P1-P8 阶段总览](/roadmap/phase-progress)：阶段状态
- [`/plans/`](/plans/)：阶段设计稿与切片记录
- [工具链设计](/architecture/toolchain)：build / codegen / toolchain 细节

本页只回答四件事：当前判断、优先级顺序、当前 checkpoint、交付规则。

## 当前判断

- Phase 1 到 Phase 6 已经形成稳定地基，不应再回到“边做边改架构”的状态。
- Phase 8 的 package / workspace / `.qi` / dependency-backed tooling 已经进入真实交付面，而且目前比 Phase 7 更接近可持续维护的用户面。
- Phase 7 的 async/runtime/build 仍有一批实质性 codegen 回归没有收口；在这批回归清掉之前，继续扩大公开支持面只会让文档、测试和实现再次失真。
- 因此当前更合理的开发顺序不是“两个主线平均扩面”，也不是“所有事情严格串行”，而是：
  1. 先止住会推翻当前支持基线的全局回归
  2. 同时继续推进不扩大 async 公开面、且不依赖未收口 Phase 7 红族的 Phase 8 独立稳定化切片
  3. 等 async/codegen 红族重新回到可控范围后，再按竖切片恢复新的 Phase 7 公开扩面
- 计划页只保留方向、优先级和退出标准；逐轮变更明细放到测试、设计稿和 archive，不再写成长流水账。

## 优先级顺序

| 优先级 | 主题 | 当前策略 |
| --- | --- | --- |
| P0 | 绿色基线恢复 | 先止住会让当前支持基线失真的全局回归 |
| P1 | Phase 8 稳定化 | 并行推进不扩大 async 公开面、且不依赖未收口 Phase 7 红族的独立稳定化切片 |
| P2 | Phase 7 选择性扩面 | 一次只开一条已被端到端证明的 async/runtime/build slice |
| P3 | 新工作流与更远主题 | `ql new` / `ql init` / `ql test` / `ql doc`、publish workflow、cross-file edits 等后置 |

## 计划模型

### 1. 以 checkpoint 为单位推进

- 每个 checkpoint 只解决一种类型的问题；不同 checkpoint 可以并行推进，但单个 checkpoint 内不混合“修回归”和“开新面”。
- 每个 checkpoint 都必须写清：目标、真相源、退出标准、明确不做什么。
- 只有在退出标准满足后，对应能力才允许进入“当前支持基线”。

### 2. 主计划只保留能指导取舍的信息

- 计划页负责回答“下一阶段先做什么，为什么”。
- 计划页不再枚举上百条具体错误文案、路径格式和单测变体。
- 这类细节继续由测试、切片设计稿和提交历史承载。

## 当前 Checkpoints

### Checkpoint A：恢复可信的绿色基线

**目标**

把当前实现、测试和文档重新拉回同一事实面，先解决已经暴露出来的 Phase 7 async/codegen 回归和合同分叉。

**为什么先做这个**

- 当前仓库里最危险的不是“少了一个功能”，而是“文档说已支持，但对应回归并不稳定”。
- Phase 7 继续扩面之前，必须先把已经宣称存在的 async/build 子集重新做实。

**当前关注点**

- 以失败家族而不是单个测试名为单位清理 `ql-codegen-llvm` 回归：
  - cleanup / guard lowering
  - async main task-handle forwarding / queued / forwarded families
  - callable guard control-flow roots
  - projected task-handle path lowering
  - match lowering 中的 aggregate / allocation 形状断言
- 收口 analysis / LSP / CLI 之间已经分叉的合同，避免同一能力在不同层有不同 truth surface。
- 在这一步完成前，暂停新增 async 公开能力，只接受“修回归、收口合同、修文档”类型改动。
- Checkpoint A 阻塞的是新的 Phase 7 公开扩面，不阻塞那些严格停留在已支持边界内、且不依赖当前 async/codegen 红族的 Phase 8 独立稳定化工作。

**真相源**

- `crates/ql-codegen-llvm/src/tests/*`
- `crates/ql-cli/tests/codegen.rs`
- `crates/ql-cli/tests/executable_examples.rs`
- `crates/ql-analysis/tests/*`
- `crates/ql-lsp/tests/*`

**退出标准**

- 当前文档已经写成“已进入支持面”的 Phase 7 子集，都能在对应 targeted regression 中稳定通过。
- `ql-analysis`、`ql-lsp`、`ql-project`、`ql-cli` 的关键 package/project 测试维持绿色。
- 文档中的 Phase 7 描述全部回收到“测试证明过的子集”，不再出现测试仍红但文档先放开的情况。

**当前不做**

- 新 async 语法
- 更宽的 Rust 专用 ABI
- 新的高层 CLI 工作流

### Checkpoint B：把 Phase 8 做成无惊喜的稳定边界

**目标**

把 package / workspace / `.qi` / dependency-backed tooling 从“已经可用”推进到“可以放心依赖，且维护成本可控”。

**为什么是 P1 而不是串行后置**

- 这条线已经有比较完整的 CLI、LSP、测试和文档闭环。
- 相比继续在 Phase 7 上加新面，先把 Phase 8 稳住，更容易形成真正可交付的用户面。
- 这条线优先级低于 P0，但不是全局串行阻塞；只要工作不扩大 async 公开面、也不依赖未收口的 Phase 7 红族，就允许继续推进。

**当前关注点**

- 继续收紧 `qlang.toml`、workspace member、references 和 `.qi` 生命周期，让 `ql project graph`、`ql project emit-interface`、`ql build --emit-interface`、`ql check --sync-interfaces` 共享同一套事实与失败模型。
- 继续保持 package-aware tooling 的 resilience：一个坏 dependency `.qi` 或坏 manifest 不应把其余健康依赖的能力整包打空。
- dependency-backed tooling 只在共享 analysis truth surface 已经稳定时再扩面；一次只新增一个 receiver slice，不把“测试变体扩张”误当成真实进展。
- `workspace/symbol`、hover / definition / references / completion 的 package-aware 路径继续以“健康依赖保留、坏依赖隔离”为原则推进。

**真相源**

- `crates/ql-project/src/*`
- `crates/ql-cli/tests/project_graph.rs`
- `crates/ql-cli/tests/project_interface.rs`
- `crates/ql-cli/tests/project_check.rs`
- `crates/ql-analysis/tests/package_*`
- `crates/ql-lsp/tests/package_*`

**退出标准**

- project / workspace / `.qi` 相关 CLI 回归保持绿色。
- 当前支持页对 Phase 8 的描述只保留 contract 级能力，不再混入提交流水账。
- package-aware LSP/analysis 在“健康依赖存在、坏依赖并存”的情况下仍能稳定保留健康部分结果。

**当前不做**

- 真实 dependency publish / registry / lockfile workflow
- cross-file rename / workspace edits
- 更广义的 workspace-wide IDE 语义

### Checkpoint C：按竖切片恢复 Phase 7 扩面

**目标**

在 Checkpoint A 完成后，恢复 Phase 7 的能力推进，但改为“一次一条竖切片”，不再同时拉很多 async 形态。

**建议顺序**

1. 先补“文档已写、但回归仍不稳定”的 async executable / cleanup / guard 缺口。
2. 再补 program-mode 与 library-mode 对同一 task-handle / projection 家族的 parity。
3. 最后才补 Rust host 互操作上的增量，而且仍通过稳定 C ABI 进场，不直接把 Rust 专用 ABI 变成主线。

**每条竖切片都必须同时经过**

- analysis / typeck / MIR
- borrowck / cleanup-aware facts
- codegen / driver / runtime hooks
- CLI 或 LSP 的用户可见入口
- 文档同步

**退出标准**

- 每新增一条公开 slice，至少有一条 targeted regression 和一条用户可见回归保护它。
- 如果能力会进入 build surface，还要有至少一条 CLI build 或 executable smoke 证明。
- 当前支持页必须在同一轮修改里同步更新。

**当前不做**

- “完整 async 语义”承诺
- 将 Rust 专用互操作抬升为主边界
- 未经证明的大跨度 surface jump

### Checkpoint D：后置主题

以下主题明确后置，不进入当前主线争抢优先级：

- `ql new` / `ql init` / `ql test` / `ql doc`
- 真实 dependency build graph / publish workflow
- cross-file rename / workspace edits
- 更宽的 workspace-wide LSP 语义
- 设计稿中尚未进入实现闭环的语言能力

## 每个 Checkpoint 都必须遵守的交付规则

### 1. 一层只维护一份真相源

- AST、HIR、resolve、typeck、MIR、borrowck、codegen、runtime、LSP 各自维护自己的事实。
- CLI、LSP、FFI 和文档只消费共享结果，不重复发明语义。

### 2. 先修回归，再开新面

- 任何已经把工作区拉红的回归，优先级都高于同层的新能力。
- 文档只记录已经通过当前门槛的能力，不提前帮实现“预约支持”。

### 3. 新能力必须形成真实闭环

- 至少要有一个真实入口：CLI、LSP、FFI 或 executable smoke。
- 至少要有 diagnostics / failure model。
- 至少要有回归测试和文档入口。

### 4. 测试保护“家族”，不保护“噪音”

- 新增测试优先覆盖新的 failure family 或新的用户可见 slice。
- 相邻的 coverage-only 变体不是主线目标，除非它直接保护刚修掉的回归。

### 5. C ABI 仍是当前稳定互操作边界

- Rust host 继续走 `Rust <-> C ABI <-> Qlang`。
- 更深的 Rust / C++ 专用 ABI 设计后置。

### 6. 文档必须短而准

- `README`、`当前支持基线`、`开发计划`、阶段设计稿必须同步。
- 当前支持页写事实，开发计划写取舍，详细增量写设计稿和 archive。

## 当前最推荐的阅读顺序

1. [当前支持基线](/roadmap/current-supported-surface)
2. [P1-P8 阶段总览](/roadmap/phase-progress)
3. 做 package / workspace / `.qi` / dependency-backed tooling 时，先读 [Phase 8 入口文档](/plans/2026-04-05-phase8-interface-artifacts-and-cross-file-lsp)
4. 做 async/runtime/build/interop 时，先读 [Phase 7 文档](/plans/phase-7-concurrency-and-rust-interop)
5. 做 build / codegen / toolchain 问题时，再读 [工具链设计](/architecture/toolchain)
