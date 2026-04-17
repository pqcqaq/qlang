# 开发计划

关联文档：

- [当前支持基线](/roadmap/current-supported-surface)：今天真实可依赖的实现边界
- [P1-P8 阶段总览](/roadmap/phase-progress)：阶段状态
- [`/plans/`](/plans/)：阶段设计稿与切片记录
- [工具链设计](/architecture/toolchain)：build / codegen / toolchain 细节

本页只回答四件事：当前判断、优先级顺序、当前 checkpoint、交付规则。

## 当前判断

- Phase 1 到 Phase 6 的编译器地基已经足够稳定，当前主矛盾不再是 parser / typeck / MIR 缺骨架。
- 现有 Phase 8 已经补出最小 package / workspace / `.qi` / `build-run-test` smoke 闭环，并且 project-aware `build/run/test` 已打通 direct dependency public `extern "c"` 调用这条窄路径；但这还不是“成熟的真实项目开发闭环”，也不是完整跨包 Qlang 语义。
- 当前 CLI 真实入口已覆盖 `ql check`、`ql build <file-or-dir>`、`ql run <file-or-dir>`、`ql test <file-or-dir>`、`ql project ...`、`ql fmt`、`ql mir`、`ql ownership`、`ql runtime`、`ql ffi`；project-aware `build/run/test` 与 target/测试发现已落地，`ql build` / `ql run` 已补上最小 target selector（`--package`、`--lib|--bin|--target` / `--bin|--target`），而 `ql build`、`ql test` 这轮也都已补上第一版机器可消费 `--json` 输出（`ql.build.v1` / `ql.test.v1`）；与此同时，`ql project lock` 也已补出第一版 `qlang.lock`，会锁 resolved local package graph、effective default profile 与 target 输入面，并支持 `--check` 做非改写校验。其中 `ql.build.v1` 现在已覆盖成功构建结果、project-aware preflight failure（当前已覆盖 `project-context` / `manifest-load` / `target-discovery` / `target-selection` / `output-planning`，以及 selected package 无 discovered targets 的 `build-plan` 校验）、build-plan recursion failure、dependency/interface prep failure、per-target target-prep failure、build-side `emit-interface` failure，以及 actual target build failure；actual target build failure 继续使用 `diagnostics` / `invalid-input` / `io` / `toolchain`，preflight failure、build-plan recursion failure、dependency/interface prep failure、per-target target-prep failure 与 build-side `emit-interface` failure 则额外导出 stage-aware `error_kind` / detail。`ql.test --json` 当前覆盖 `listed / ok / failed / no-tests / no-match` 五类测试结果。`ql test` 这轮同时也已补上 `--package`、共享 `--list` / `--filter`、第一版项目内 `tests/ui/**/*.ql` snapshot target；`build/run/test` 现在也已开放 `--profile <debug|release>`，并会在 project-aware 路径上按 `CLI override > package profile > workspace profile > debug` 的顺序消费默认 profile。 但整体仍只是最小测试 harness + 最小 reproducible-input contract，不是完整的工程测试矩阵，也不是完整的 registry/version/hashing 级可复现工作流；下一步主线也不应再让更宽的测试 target family 抢在 manifest / 依赖模型前面。
- 当前 `qlang.toml` 真实模型仍然很薄：`[package].name`、`[workspace].members`、`[references].packages`、`[dependencies]` 本地路径依赖声明（当前只接受字符串或 `{ path = "..." }`），以及这轮补上的第一版 `[profile].default = "debug" | "release"`。现在 profile 已可同时写在 package manifest 和 workspace 根 manifest 上，并按 `CLI override > package profile > workspace profile > debug` 的顺序参与 project-aware `build/run/test` 默认 profile 决策；`ql build/run/test` 也都已经开始真实消费 package 级本地依赖闭包，而不再只做 `.qi` 预备或 workspace member 排序。这已经足够支撑 graph / `.qi`、最小依赖装载、workspace 级 `targets/build/run/test` 的本地依赖排序，以及真实 workspace 的基础 profile 继承。但它仍不足以支撑 driver 级 dependency-aware build、完整依赖解析、更完整的 profile 模型和可复现工作流。
- 因此接下来的主线不再是优先继续扩 async/runtime 或语言表面，而是先把“真实项目肯定要用”的工程能力补齐，再把语言扩面往后压。
- 计划页继续只保留方向、优先级和退出标准；逐轮实现细节仍放在测试、设计稿和 archive，不回到长流水账。

## 优先级顺序

| 优先级 | 主题 | 当前策略 |
| --- | --- | --- |
| P0 | 收口当前 package / workspace 工作流 | 已先补 package/workspace 的 `build/run/test`、target 发现、`build/run` target selector、smoke harness、`ql test --package/--list/--filter` 与第一版项目内 UI snapshot target；当前先收口现有 contract、脚手架和文档对齐，不继续把更宽 test target family 当主线 |
| P1 | manifest / 依赖工程化 | 在已落地 local-path `[dependencies]` 输入的基础上，把 `qlang.toml` 从最小 graph 描述升级为真实工程 manifest，优先补 target、依赖、profile 与真实 build/test/run 图 |
| P2 | 基础 LSP、可复现构建与工具链分发 | 把 project-scale 跳转/高亮/workspace symbol 做到可依赖，同时补 JSON/lockfile/CI，以及 `ql` / `qlsp` / VSIX 的版本与分发 contract |
| P3 | 高级 IDE 与语言 / 运行时扩面 | cross-file rename / workspace edits / code actions，以及 async/runtime/Rust interop、新语法和更远类型系统能力后置，只接受建立在 P0-P2 之上的扩展 |

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

### Checkpoint A：补齐 package / workspace 的开发闭环

**目标**

让一个真实 Qlang 项目能在 package 或 workspace 根目录完成“构建、运行、测试”，并且脚手架、README、primer 与当前支持页不再和现状分叉。

**为什么先做这个**

- 这是当前离“能做真实项目”最近、也是最缺的一层。
- `ql project init`、workspace 级 `ql build` / `ql run` / `ql test` 与默认 `src/main.ql` / `tests/smoke.ql` 已经存在；现在缺的是把这些入口收口成稳定的用户合同，而不是继续停留在“功能有了，但 onboarding 文档没跟上”。
- 这条线直接兑现功能清单里已经列成 P0 的 `ql build/run/check/test` 与 workspace 支持，也能避免 Checkpoint A 处于“看起来已经完成、实际上还没交付给用户”的假完成状态。

**当前关注点**

- 对齐 `README`、primer、当前支持页、工具链设计里的最小项目工作流，明确 `ql project init` 会生成什么、以及 package/workspace 根目录能直接跑什么。
- 稳定当前 `lib` / `bin` / fallback `source` 的 target 发现、selector、默认输出目录和 profile 规则，先把今天已经开放的 project-aware `build/run/test` 合同写准。
- 让脚手架生成物、当前 `src/lib.ql` / `src/main.ql` / `tests/smoke.ql` 约定，以及项目内 `tests/ui/**/*.ql` snapshot harness 形成一致的最小工作流。
- 在真实 manifest 落地前，不继续把 doc / integration / benchmark 或更宽的 `ql test` target family 扩面当成主线。

**真相源**

- `crates/ql-cli/src/main.rs`
- `crates/ql-driver/src/build.rs`
- `crates/ql-project/src/*`
- `README.md`
- `docs/getting-started/compiler-primer.md`
- `docs/architecture/toolchain.md`
- `docs/roadmap/current-supported-surface.md`
- `docs/roadmap/feature-list.md`

**退出标准**

- `README`、primer、当前支持页与工具链设计，对 `ql project init` 生成物和 package/workspace 根目录 `build/run/test` 的最小用法一致对齐。
- demo package / workspace 能直接在根目录运行 `ql build`、`ql run`、`ql test`，且不需要手写源码文件路径。
- 当前 `lib` / `bin` / fallback `source` 的 target 发现、selector、默认输出目录与 smoke/UI test 合同都有明确文档入口和回归保护。

**当前不做**

- manifest 里的更完整 target / dependency / profile 声明
- 更宽的 integration / doc / benchmark test family
- 远程 registry / publish
- 深入 async/runtime 新语义
- 更宽的 Rust 专用 ABI

### Checkpoint B：把 `qlang.toml` 升级成真实工程 manifest

**目标**

把当前最小 `name/members/references + local path dependencies` 模型升级为真实项目可依赖的 manifest，支撑 target、依赖、profile 和构建图，并把当前只覆盖 public `extern "c"` 的窄跨包执行路径继续推进到更完整的 dependency-aware build。

**为什么紧接着做**

- 没有真实 manifest，Checkpoint A 只能继续靠约定和猜测，工作流会很快再次分叉。
- 当前 `ql-project` 的 manifest 结构还停留在最小 graph 层，远不足以承载项目级 build/run/test 决策。
- 在 manifest 没稳定前继续扩更宽的 `ql test` target family，或继续放大 project-scale IDE 语义，都会把更多逻辑叠在临时约定上，返工风险很高。

**当前关注点**

- package metadata、local dependencies、target declarations、更完整的 build profile 模型、FFI libraries。
- 这轮已经把 `[dependencies]` 从“manifest 输入面”推进到“workspace 级 `targets/build/run/test` 会真实消费”的状态，也已经落下了第一版 path-only target declarations（`[lib].path` / `[[bin]].path`）和 package-level `[profile].default`；`ql build/run/test` 现在都已补上第一版 package-level build-prep graph，让依赖关系不只影响 workspace member 顺序，也会真实参与执行前的构建准备。下一步重点转到 driver/build backend，从当前单文件路径推进到真正的 dependency-aware build。
- 先把 path/local dependency 模型做稳，再谈 registry/version 解析。
- 让 `.qi` 生命周期继续服务依赖分析，但不再充当唯一的“项目模型”。
- 在 manifest 层明确 package vs workspace 的职责，避免 CLI、driver、analysis、LSP 各自猜一套项目结构。
- 在 manifest 落地前，不继续把更宽 `ql test` target family 当成主线。

**真相源**

- `crates/ql-project/src/lib.rs`
- `crates/ql-cli/tests/project_*`
- `docs/architecture/toolchain.md`
- `docs/roadmap/current-supported-surface.md`
- `docs/roadmap/feature-list.md`

**退出标准**

- 一个 package/workspace 能仅靠 manifest 完成 target 发现和依赖图装载。
- 本地依赖不再只表现为“接口引用关系”，而能进入真实 build/test/run 图。
- 相关失败模型在 CLI、driver、analysis、LSP 之间保持同一套项目事实面。

**当前不做**

- 远程版本求解
- package registry
- publish workflow

### Checkpoint C：做实基础 LSP 与项目索引

**目标**

把 LSP 从“协议能力存在”推进到“真实 workspace 里基础跳转和高亮可依赖”，先把 project-scale definition / references / workspace symbol / semantic tokens 做实。

**为什么排在 manifest 之后**

- 基础导航和高亮依赖稳定的项目模型、依赖图与索引；没有 Checkpoint B，LSP 只能继续建在临时约定上。
- 当前 VSCode 插件仍是 `qlsp` thin client，但已经有最小 TextMate grammar fallback；真正的缺口不是语法注册，而是 project-scale 查询面还不够稳。

**当前关注点**

- definition / declaration / references / workspace symbol / semantic tokens 在“健康 workspace + 健康依赖”下形成稳定 contract。
- 当前已先补一条更接近真实项目编辑器体验的高亮入口：healthy package/workspace 下 imported dependency enum variant、显式 struct field label 与唯一 method member 已开始进入 package-aware `semantic tokens`，因此 Checkpoint C 后续重点转向把导航与剩余高亮面继续做完整，而不是再回头补协议声明本身。
- 当前已接通的 `documentHighlight` 与 TextMate grammar fallback 继续作为底线，但它们不替代 project-scale 语义本身。
- 优先补“能用”的导航和高亮，不先追求更花的 IDE 功能。

**真相源**

- `crates/ql-analysis/src/*`
- `crates/ql-lsp/src/*`
- `crates/ql-lsp/tests/*`
- `docs/getting-started/vscode-extension.md`
- `editors/vscode/qlang/*`

**退出标准**

- VSCode 下的基础跳转、高亮、workspace symbol 在真实 workspace 中可稳定工作，不再需要用户接受“协议支持存在，但编辑器里几乎没有效果”的状态。
- LSP 用户面与 CLI/project 模型共享同一套项目事实。
- VSCode 文档、当前支持页与插件 README 对当前编辑器能力边界的描述一致。

**当前不做**

- cross-file rename / workspace edits
- code actions、inlay hints、call hierarchy 等更高阶 IDE 入口
- 脱离项目模型的 editor-only 特判

### Checkpoint D：补齐可复现构建与团队协作接口

**目标**

让 Qlang 项目能进入 CI、脚本和团队协作，而不只适合本地单人试验。

**为什么这一步要前置**

- 真实项目不只需要“能 build”，还需要“能稳定地被自动化系统消费”。
- 第一版 lockfile、机器可消费输出、稳定 exit code、deterministic test/build contract，以及匹配的工具链/编辑器分发方式，比继续扩语言语法更直接影响能否落地。

**当前关注点**

- JSON diagnostics / machine-readable project output；当前已先从 `ql project targets --json` 与 `ql project graph --json` 打出两条稳定 project schema，也已补出第一版 `ql check --json`（`ql.check.v1`）、`ql build --json`（`ql.build.v1`）与 `ql test --json`（`ql.test.v1`）；其中 `ql.check.v1` 当前只覆盖 success / source diagnostics，`ql.build.v1` 当前已覆盖成功构建结果、project-aware preflight failure（当前已覆盖 `project-context` / `manifest-load` / `target-discovery` / `target-selection` / `output-planning`，以及 selected package 无 discovered targets 的 `build-plan` 校验）、build-plan recursion failure、dependency/interface prep failure、per-target target-prep failure、build-side interface emit failure，以及 actual target build failure，`ql.test.v1` 当前覆盖 `listed / ok / failed / no-tests / no-match` 五类测试结果。actual target build failure 继续使用 `diagnostics` / `invalid-input` / `io` / `toolchain`；preflight failure、build-plan recursion failure、dependency/interface prep failure、per-target target-prep failure 与 build-side interface emit failure 则额外导出 `stage` 和场景化 detail，其中 build-plan recursion failure 当前稳定落到 `stage = build-plan`，并补 `owner_manifest_path` / `dependency_manifest_path` / `cycle_manifests` 这类递归依赖 detail；dependency/interface prep failure 当前稳定落到 `stage = dependency-interface-prep`，并补 `owner_manifest_path` / `reference_manifest_path` / `reference` / `failing_dependency_count` / `first_failing_dependency_manifest`；per-target target-prep failure 当前稳定落到 `stage = target-prep`，并补 `dependency_manifest_path` / `dependency_package` / `interface_path` / `symbol` / `first_dependency_package` / `first_dependency_manifest_path` / `conflicting_dependency_package` / `conflicting_dependency_manifest_path` / `io_path` 这类 target-prep detail；build-side interface emit failure 当前也会补 `failing_source_count` / `first_failing_source` 这类 source-summary 载荷。之前剩下的 per-target dependency extern/interface artifact prep 这块失败面，现在也已稳定留在 `ql.build.v1` 的 stdout contract 内；下一步优先级从这块退出，转去补更宽的 reproducible build / CI contract 与剩余 failure model。
- debug / release / test profile 的稳定 contract。
- 第一版 `qlang.lock` 已落地；下一步是在这个基础上继续扩更宽的可复现输入面、workspace 级 CI 入口与更完整的失败模型，而不是把 lockfile 假装成已经覆盖 registry/version/source hash。
- workspace 级 `check/build/test` 的 CI 入口和稳定失败语义。
- `ql` / `qlsp` / VSIX 的版本绑定、兼容矩阵与可重复的团队分发路径。

**真相源**

- `crates/ql-cli/src/main.rs`
- `crates/ql-driver/src/build.rs`
- `crates/ql-project/src/*`
- `tests/ui`
- `crates/ql-cli/tests/*`
- `docs/getting-started/vscode-extension.md`
- `editors/vscode/qlang/*`

**退出标准**

- CI 能稳定运行 workspace 级 `ql check` / `ql build` / `ql test`。
- 诊断与图信息至少有一条机器可消费出口，不再只能靠人读终端文本。
- 构建 profile 和产物目录规则在文档和实现里不再分叉。
- 团队可以按文档安装匹配版本的 `ql` / `qlsp` / VSIX，而不是只能靠本地手工凑环境。

**当前不做**

- benchmark 平台化
- binary caching
- registry 托管

### Checkpoint E：补齐高级 project-scale IDE 语义

**目标**

把编辑器支持从“基础跳转和高亮已可依赖”推进到更完整的工程化语义。

**为什么它排在基础工作流之后**

- cross-file rename、workspace edits、code actions 的正确性依赖稳定的项目索引、manifest 和依赖图。
- 在基础导航和高亮都还没做实之前先追求高级 refactor，只会继续放大“协议能力看起来很多，但编辑器里不可依赖”的落差。

**当前关注点**

- cross-file rename / workspace edits。
- project-scale references / definition / symbol index 的稳定性。
- code actions、inlay hints 等真正能降低项目维护成本的入口。
- 继续坚持 analysis truth surface 单源，不在 LSP 层临时发明第二套语义。

**真相源**

- `crates/ql-analysis/src/*`
- `crates/ql-lsp/src/*`
- `crates/ql-lsp/tests/*`
- `docs/architecture/toolchain.md`

**退出标准**

- 健康 workspace 下的 cross-file rename / workspace edits 有明确 contract 和回归保护。
- project-scale query 不再只停留在保守演示级别。
- LSP 用户面与 CLI/project 模型共享同一套项目事实。

**当前不做**

- call hierarchy 全量扩面
- 更宽的 speculative IDE heuristics
- 脱离项目模型的 editor-only 特判

### Checkpoint F：语言与运行时扩面后置

以下主题明确后置，不再与 P0-P2 争抢主线资源：

- 新 async/runtime/build 公开切片
- 更宽的 Rust 专用互操作
- 新语法糖与类型系统扩面
- 远程 registry / publish workflow
- 设计稿中尚未进入实现闭环的语言能力

## 每个 Checkpoint 都必须遵守的交付规则

### 1. 一层只维护一份真相源

- AST、HIR、resolve、typeck、MIR、borrowck、codegen、runtime 各自维护本层事实。
- CLI、LSP、FFI 和文档只消费 `ql-analysis` / `ql-project` / `ql-driver` 等共享结果，不在用户面重新发明第二套语义。

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
- 这轮 project-aware `build/run/test` 能跑通的跨包调用，也仍然沿同一条 C ABI 边界，只开放 direct dependency 的 public `extern "c"` 符号。
- 更深的 Rust / C++ 专用 ABI 设计后置。

### 6. 文档必须短而准

- `README`、`当前支持基线`、`开发计划`、阶段设计稿必须同步。
- 当前支持页写事实，开发计划写取舍，详细增量写设计稿和 archive。

## 当前最推荐的阅读顺序

1. [当前支持基线](/roadmap/current-supported-surface)
2. [P1-P8 阶段总览](/roadmap/phase-progress)
3. 先读 [工具链设计](/architecture/toolchain)，确认 build / project / LSP 的现有边界
4. 再读 [功能清单](/roadmap/feature-list)，核对 P0/P1/P2 的长期能力分层
5. 做 package / workspace / `.qi` / dependency-backed tooling 时，再读 [Phase 8 入口文档](/plans/2026-04-05-phase8-interface-artifacts-and-cross-file-lsp)
6. 做 async/runtime/build/interop 时，最后再读 [Phase 7 文档](/plans/phase-7-concurrency-and-rust-interop)
