# 开发计划

关联文档：

- [当前支持基线](/roadmap/current-supported-surface)
- [阶段总览](/roadmap/phase-progress)
- [工具链设计](/architecture/toolchain)
- [`/plans/`](/plans/)

这页只保留当前判断、优先级和 checkpoint，不再记录长流水账。

## 当前判断

- Phase 1 到 Phase 6 的编译器地基已经够用，当前不是“语言前端还没搭起来”，而是“做不出真实可用项目”。
- 如果语言现在还无法稳定支撑小型真实项目，继续扩语法、类型系统或 runtime 表面价值很低。
- 从现在开始，主线改为“先把语言做到可真实使用，再恢复语言扩面”；P0 未完成前，不再把新语法和更宽语言能力当主线。
- 从现在开始，每一轮功能迭代必须优先落到生产代码；只有测试或文档改动，不再计作一轮功能推进。

## 优先级

| 优先级 | 主题 | 当前目标 |
| --- | --- | --- |
| P0 | 可用性 MVP | 让 qlang 能稳定支撑小型本地 workspace 项目开发 |
| P1 | dependency-aware build/backend | 把跨包执行和依赖装载从窄的 demo slice 扩到真实项目需要的核心路径 |
| P2 | 基础 LSP / VSCode 可用性 | 把真实 workspace 里的导航、高亮、补全做到可依赖 |
| P3 | 分发 / CI / 团队接入 | 补齐安装、锁文件、JSON、CI、VSIX 分发 |
| P4 | 高级 IDE 与语言扩面 | cross-file rename / workspace edits / 更完整 code actions，以及更宽 async/runtime/语言能力后置 |

## P0 完成定义

只有同时满足下面几条，才算“语言开始可真实使用”：

- 能从 `ql project init` / `add` 建出 workspace，并直接从 workspace 根目录执行 `check/build/run/test`。
- 本地路径依赖不再只停在窄的 public free function / `extern "c"`；至少要覆盖真实项目常见的 public value/type/member 使用路径。
- VSCode 中打开真实 workspace 时，definition / references / hover / completion / semantic tokens / `workspace/symbol` 不再只在理想样例里工作。
- `ql`、`qlsp`、VSIX 的安装和版本绑定有明确、稳定、可复现的路径。
- README、支持页、开发计划三者描述一致，不再出现“文档说可用，但真实项目一碰就碎”。

## 当前 Checkpoints

### A. 可真实运行的项目闭环

目标：

- 让 package/workspace 根目录能直接稳定执行 `ql check`、`ql build`、`ql run`、`ql test`。
- 让脚手架、README、primer、支持页、VSCode 使用文档保持一致。

完成标准：

- 新脚手架能开箱进入 `graph/check/build/run/test`。
- 已创建的 workspace 能继续用 `ql project add/remove/target add/add-dependency/remove-dependency/dependencies/dependents` 维护成员、targets 和本地依赖；新增 scaffold、纳管现有 package、补新 bin target、从 workspace 根直接给指定 member 补/减依赖、按包名批量清理全部 dependents、查询正反向依赖，以及安全或级联移除 member 都不必手改 manifest。
- target 发现、graph/package 聚焦、selector 过滤、member 目录/源码路径入口语义、profile 规则、默认输出目录和测试入口都有明确文档和回归保护。

### B. 真实项目依赖后端

目标：

- 把当前最小 manifest 升级成真实工程 manifest。
- 让本地依赖不只服务 `.qi` 和排序，也真实参与 build/test/run 图。
- 把跨包执行从当前窄的 top-level free function / `extern "c"` 扩到真实项目常见路径。

完成标准：

- package/workspace 只靠 manifest 就能完成 target 发现和依赖装载。
- dependency-aware build 不再只停留在 direct dependency public `extern "c"` 或极窄 free function 路径。
- 至少补齐真实项目最常见的 public function / value / type / method 使用路径，优先覆盖本地路径依赖。

### C. 基础 IDE 可用性

目标：

- 在真实 workspace 里把 definition / references / hover / completion / `workspace/symbol` / semantic tokens 做到可依赖。
- 继续坚持 analysis / project 单一事实面，不让 LSP 自己发明第二套语义。

完成标准：

- healthy workspace 下基础导航和高亮稳定工作。
- 同名本地依赖、broken-source、workspace member 入口这些真实项目高频场景有明确保护，而不是只在单文件 happy path 里工作。
- VSCode 文档、支持页和插件 README 与实现边界一致。

说明：

- same-file rename 继续保留。
- cross-file rename / workspace edits 等高级重构要等项目模型更稳之后再做。

### D. 安装、分发与 CI

目标：

- 补齐 `qlang.lock`、JSON 输出、CI 入口和工具链分发约定。
- 让项目能进入脚本和团队协作，而不只是本地试验。

完成标准：

- workspace 级 `check/build/test` 可稳定进入 CI。
- `ql` / `qlsp` / VSIX 的安装与版本绑定有清晰文档。
- 仓库外用户可以按文档完成 CLI 安装、LSP 连接和 VSIX 安装，而不是必须读源码猜流程。

## 下一轮（已排定）

- LSP：继续把 `textDocument/implementation` 从 trait/type surface、trait method definition、已唯一解析的 concrete method call，扩到更多 workspace root/source-backed concrete member call surface。
- backend：继续扩 direct local dependency 下真实项目高频的 public value/type/member 调用面，优先补会直接阻断 workspace `build/run/test` 的路径。
- 文档：继续只保留入口结论、支持边界和最近 checkpoint；不再追加长流水账。

## 明确后置

- cross-file rename / workspace edits / 更完整 code actions
- 更宽的 async/runtime/Rust interop 扩面
- 新语法糖和更远的类型系统能力
- registry / publish workflow

## 交付规则

- 入口文档只写结论和边界，不写长流水账。
- 任何用户可见能力都必须同时具备：实现、回归、文档入口。
- 没有 `crates/*/src/*.rs` 的生产代码改动，不再计作一轮功能迭代；测试和文档只能跟随真实实现收口。
- 同一组 project-aware 命令的 workspace member 目录/源码路径入口语义必须保持一致；补一条入口时，要同时审计 `check/build/run/test` 和 `ql project *` 的相邻命令。
- 文档与实现冲突时，先修正文档，不在入口页预告“即将支持”。
