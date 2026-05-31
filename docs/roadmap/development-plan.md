# 开发计划

> 最后同步：2026-05-28

只记录当前开发顺序和可执行约束。不写日期承诺，不写流水账。

## 当前判断

- Qlang 已有编译器、CLI、项目系统、LSP 和 stdlib 地基。
- 当前瓶颈是生产可用性，不是重做语言骨架。
- stdlib、workspace、LSP、测试和分发必须一起推进。
- 每轮交付一个可验证切片：代码、回归、文档、提交。

## 推进顺序

| 顺序 | 主题 | 目标 |
| --- | --- | --- |
| 1 | 项目可用性 | `ql project/check/build/run/test` 在真实本地 workspace 中稳定工作 |
| 2 | stdlib | 收口 `std.core`、`std.option`、`std.result`、`std.array`、`std.test` 的 public API，并扩大 downstream smoke |
| 3 | generics/backend | 补齐 generic monomorphization 和 dependency-aware backend |
| 4 | LSP/VSCode | 补齐真实 workspace 下的导航、高亮、补全、格式化、code action |
| 5 | 分发准备 | release、VSIX、CI、JSON 输出、安装文档 |
| 6 | 语言扩面 | 更宽 async/runtime、trait/effect、workspace-wide refactor |

## 当前工作项

- `ql-cli` 主入口、check/build/run/test pipeline、project selection/reporting、dependency bridge 和 build reporting 已持续模块化；dependency generic bridge 的 call/expr/binding/substitution/rendering/forwarder/instantiation 合同测试已按责任外置并使用语义化测试模块名，specialization 覆盖已拆成 status、local rewrite、import rewrite 三组，instantiation 已按 entrypoint、pattern、array、context、expression 拆分，并锁住 alias/group import 行为；`ql build --list` workspace listing、manifest/workspace profile、workspace selector/package-relative target/dependency closure、single-file/direct-source entrypoint、dependency generic build、direct dependency public API build、build failure JSON 合同已迁出到独立 integration tests，历史 `project_build.rs` 聚合文件已收口为 `project_build_core.rs`；`ql project add/remove` member mutation、`ql project add/remove-dependency` manifest mutation、`ql check` workspace/reference、workspace sync failure、package/workspace failure-rerun-hint、workspace/package selector、sync selector、sync failure 和 sync/stale、`ql project graph` manifest/preflight failure、workspace package selector、workspace context、workspace member error 和 reference/transitive summary、`ql run` package/workspace/direct-source profile、失败/selector、dependency bridge 和 direct dependency public API 合同已迁出到独立 integration tests；codegen snapshot runner 已抽到 `tests/support/codegen.rs`，core LLVM IR、async LLVM IR、async/closure LLVM IR、match/guard LLVM IR、match/guard root LLVM IR、call-root fixed-shape LLVM IR、match/guard object、async object、executable/dylib、staticlib、dynamic task-handle、projected dynamic task-handle、cleanup value、awaited cleanup、capturing closure task-handle、capturing closure root/guard、cleanup capturing closure、aggregate match catch-all、call-root match catch-all、awaited match catch-all、awaited control-flow、cleanup block basics、cleanup iteration roots、cleanup callable/match、cleanup projected roots 和 cleanup question-mark codegen 回归已迁出到独立 integration tests。下一步继续拆 CLI 大边界并外置剩余内联测试，所有移动必须保持行为不变并跑真实 workspace smoke。
- 关键输出写入已统一走同目录临时文件替换和输出路径锁，覆盖 artifact、header、source、manifest、lockfile、interface 以及 `run/test` 执行期 executable。后续并发问题继续按“先保护真实产物，再补 CLI smoke”的顺序处理。
- `project init --stdlib` starter downstream smoke 已迁出到独立 integration test；package/workspace 的 `check/build/run/test`、interface sync/check、graph/status/targets/dependencies/dependents、lock stale failure 和 JSON/listing 合同都必须持续覆盖。
- `ql test` 已覆盖 package-under-test、direct dependency 和 local generic source override 的组合路径；test discovery 的 project/direct target discovery、project test file scanning、all-member/selected-member/selected-package/selected-workspace/workspace-selector loading、package selector validation/message/reporting、project error message/reporting、member target assembly、target/filter selection、target construction、output/diagnostic path、UI test detection、listing 与 no-tests/no-match message/reporting 已独立拆分并外置合同测试，UI snapshot、standalone/direct-file/workspace-member listing project smoke、package selector success/text/JSON preflight、target/filter selection failure、profile text/JSON/listing smoke、direct dependency bridge smoke 和 current/package-under-test bridge smoke 已迁出到独立 integration tests，集中测试文件继续收缩。后续重点是继续与 build/run 的 selector 和 JSON failure 合同保持一致。
- stdlib public API 继续优先用泛型、数组长度泛型和语言能力表达；新增 API 必须进入 package-local smoke 或 starter/downstream smoke。
- LSP 已共享 request context 并拆出主要 request family；initialize capabilities 和 `qlsp` binary tests 已外置；下一步是稳定 workspace index/cache 生命周期，减少重复扫描。
- README、roadmap、stdlib、VSCode 文档必须跟实现同步；实现未落地时文档写成未支持。
- JSON 输出继续覆盖成功和 preflight/render/selection failure；新增命令路径必须同时考虑文本和 JSON 合同。

## 明确后置

- 自动 prelude
- registry、publish、version solving
- 完整 generic monomorphization
- 完整 workspace-wide rename/refactor/index
- 更宽 async/runtime/Rust interop
- 可变参数语法

## 代码整理规则

- 不新增 `foo3/foo4/foo5` 这类固定 arity API；先补泛型、数组初始化或可变参数等语言能力。
- compiler regression fixture 也遵守同一规则；用 length-generic wrapper 加具体 caller 触发实例化，不用固定 arity 假 API。
- 不在 LSP 里复制语义规则；缺能力先补 `ql-analysis` query。
- 不把测试 fixture 写进生产路径。
- 大文件拆分必须行为不变，并用现有回归证明。
- 文档发现实现缺口时，优先修实现；实现未落地时，文档必须写成未支持。
