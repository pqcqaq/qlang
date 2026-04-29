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
- 最小 `stdlib` 属于 P0 可用性，不属于后置语言扩面；没有可依赖的 `core/test` 包，真实项目仍会退化成手写样例集合。
- 从现在开始，每一轮功能迭代必须优先落到生产代码；只有测试或文档改动，不再计作一轮功能推进。
- 不再用固定日期承诺完成 P0；节奏改为每一轮尽力交付一个可验证切片，并用回归和文档更新收口。

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
- 仓库内存在最小 `stdlib`，至少包含普通 Qlang package 形态的 `std.core` 与 `std.test`，并能被真实项目通过本地依赖消费。
- VSCode 中打开真实 workspace 时，definition / references / hover / completion / semantic tokens / `workspace/symbol` 不再只在理想样例里工作。
- `ql`、`qlsp`、VSIX 的安装和版本绑定有明确、稳定、可复现的路径。
- README、支持页、开发计划三者描述一致，不再出现“文档说可用，但真实项目一碰就碎”。

## 执行节奏

- 不按日期倒排，不承诺某个自然日完成整条主线。
- 每一轮都选择当前最能提升可用性的切片，做到实现、回归、文档一起提交。
- 每轮结束时必须能说明本轮让真实项目多走通了哪条路径，不能只报告“清理了一些代码”。
- 如果 `stdlib` 暴露出 backend / project / LSP 缺口，优先修真实阻塞项；不为绕过缺口而降低 `stdlib` 设计边界。

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

### E. 最小标准库

目标：

- 先以普通 Qlang workspace/package 形态落地 `stdlib`，不先做编译器内置 prelude。
- 让用户项目能通过本地 `[dependencies]` 显式依赖 `std.core` / `std.test`。
- 用 `stdlib` 反向驱动 dependency-aware backend、项目模板和文档收口。

完成标准：

- `stdlib/packages/core` 能被 `ql check/build/test` 验证，并提供第一批稳定基础函数。（已落地整数/布尔 helper，含符号、比较、三/四值 extrema、三值 median、3/4 项整数聚合、2/3 项均值、安全 quotient/remainder、3/4 项 Bool all/any/none 聚合、单边/双边/无序边界 clamp、边界归一化、绝对差、range span、range/bounds 距离、零值/正负/非正/非负、奇偶、闭/开区间、无序边界区间、区间外/无序边界外、升/降序判断、整除、余数、因子、容差内/外检查和基础布尔组合）
- `stdlib/packages/test` 能提供 smoke-test 友好的断言辅助，并通过 package-aware smoke test 直接导入自身 public helpers。（已落地 true/false、bool equality/ne/logic/implies、Bool all/any/none、int equality/order、zero/nonzero、max/min、sum/product/average、quotient/remainder/has-remainder/factor、2-6 路 status 组合、正负/非正/非负、区间、无序边界区间、clamp / range-distance、升/降序、奇偶、整除和容差内/外断言）
- 用户项目模板能依赖 `std.core` / `std.test` 并通过 `ql test`。（已落地 `ql project init --stdlib <path>` 的 package 与 workspace member 生成路径）
- `stdlib` API 只使用当前稳定语言面；泛型、IO、字符串、自动 prelude 和 registry 发布继续后置。

## 下一轮（已排定）

- stdlib：继续扩只依赖稳定语言面的基础 helper，并让 package / workspace 初始化模板优先覆盖真实 consumer 路径。
- backend：继续扩 direct local dependency 下真实项目高频的 public value/member 调用面；public 非泛型、非 opaque type alias 已进入 type declaration bridge，并已在普通值上下文支持 alias 透明的跨包函数签名、alias 算术与 wrapper 调用。后续仍优先修 `stdlib` / 模板暴露的真实阻塞。
- LSP：`textDocument/implementation` 的已完成基线已明显超出这里最初记录，当前准确支持面以 `current-supported-surface.md` 为准；继续扩更宽的 implementation index，但不压过 P0 stdlib / backend 阻塞项。
- 文档：继续只保留入口结论、支持边界和最近 checkpoint；不再追加长流水账。

## 明确后置

- cross-file rename / workspace edits / 更完整 code actions
- 更宽的 async/runtime/Rust interop 扩面
- 新语法糖和更远的类型系统能力
- 自动 prelude、泛型集合库、IO / 字符串完整库面
- registry / publish workflow

## 交付规则

- 入口文档只写结论和边界，不写长流水账。
- 任何用户可见能力都必须同时具备：实现、回归、文档入口。
- 没有 `crates/*/src/*.rs` 的生产代码改动，不再计作一轮功能迭代；测试和文档只能跟随真实实现收口。
- `stdlib/**/*.ql` 属于用户可见生产代码；但纯文档调整仍只算计划维护，不算功能推进。
- 同一组 project-aware 命令的 workspace member 目录/源码路径入口语义必须保持一致；补一条入口时，要同时审计 `check/build/run/test` 和 `ql project *` 的相邻命令。
- 文档与实现冲突时，先修正文档，不在入口页预告“即将支持”。
