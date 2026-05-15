# 开发计划

> 最后同步：2026-05-15

这页只记录当前开发顺序。不写日期承诺，不写流水账。

## 当前判断

- Qlang 已有编译器、CLI、项目系统、LSP 和 stdlib 地基。
- 当前瓶颈是生产可用性，不是重新设计语言骨架。
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

- 继续抽 `ql-cli` project pipeline；`build/run/test/check` 的入口 request-context 与 `project emit-interface/graph/dependencies/dependents/add/status` 的 workspace member lookup 已共享，`project add-dependency/remove-dependency` 编辑逻辑已拆出，`emit-interface` 普通/check/output selector 已补齐 ambiguous/unresolved 回归；`ql build` 已补 workspace `--package` JSON dependency-closure、`--package --target` 包内相对 target、`--list --json --package --target`、source-selector conflict JSON、selector miss JSON/text smoke 和 `--list --json` selection failure，`ql test` 已补 manifest-load / target-discovery（含 selected package）preflight JSON、member directory/test-file `--list --json`、direct-source/workspace/package-path selector preflight JSON smoke，以及 no-tests / target miss / filter miss selection failure JSON，`ql project graph/targets/status --json` 已补 manifest-load failure，`ql project targets --json` 已补 selector miss selection failure，`ql project graph/status --json --package` 已补 selector failure JSON，`ql project graph/targets/status` 已补 workspace/member source/member directory `--package --json` smoke，`ql project target add` 已补 member directory smoke，`ql project dependencies/dependents --json` 已补 workspace member source/directory 派生包名和 package selector failure smoke，`ql project lock` 已补 workspace/member source/member directory `--json` 写入和 source/directory `--check --json` smoke，`ql project add-dependency/remove-dependency` 已补 member directory smoke，`ql run` 已补 direct dependency generic、`--json` dependency generic、workspace `--package` dependency generic/JSON、manifest-load / target-discovery / target-selection preflight JSON、source-selector conflict JSON、`--package --target` 包内相对 binary、`--list --json --package --target`、selector miss JSON/text、`--list --json` target/runnable selection failure 和 transitive wrapper/helper smoke，`ql check --package` 已补 workspace root/member source/member directory 文本、JSON、missing selector 和 sync smoke。下一步继续扩真实 workspace smoke。
- 收紧 `ql test`；`--package` 已复用共享 workspace member lookup，并覆盖 missing/invalid/ambiguous/broken member 回归、workspace root/member directory 文本/JSON smoke、member directory/test-file `--list --json` smoke，以及 member directory `--filter`/包内相对 `--target` smoke；测试 target 的 dependency/package-under-test/local-generic source override 拼接已收口；direct dependency generic consumer 已覆盖 named/expression args、carrier、result-context、zero-arg context 和 wrapper/helper smoke。下一步继续收口 dependency-aware backend。
- 建立 LSP workspace index，让 diagnostics、references、rename、symbols、semantic tokens 和 rich editor hints 走同一份分析缓存。
- 收紧 generic backend：`std.core` package-local smoke 已覆盖公开 scalar/predicate/bool helpers；继续扩大 package-local tests、downstream smoke 和 dependency consumers 对 generic public functions 的直接覆盖；继续把 direct-call specialization 扩到更宽表达式、类型和 dependency-aware backend 场景；保留未推断调用的显式错误，但不让未使用 import 触发 bridge 失败。
- 继续把 `project init --stdlib` 的真实模板、stdlib examples 和 downstream smoke 作为同一套可验证入口维护。
- 保持 README、roadmap、stdlib、VSCode 文档与实现同步。

## 明确后置

- 自动 prelude
- registry、publish、version solving
- 完整 generic monomorphization
- 完整 workspace-wide rename/refactor/index
- 更宽 async/runtime/Rust interop
- 可变参数语法

## 代码整理规则

- 不新增 `foo3/foo4/foo5` 这类固定 arity API；先补泛型、数组初始化或可变参数等语言能力。
- compiler regression fixture 也遵守同一规则；用 length-generic wrapper 加具体 caller 触发实例化，不用 `foo3/foo4/foo5` 这类假 API。
- 不在 LSP 里复制语义规则；缺能力先补 `ql-analysis` query。
- 不把测试 fixture 写进生产路径。
- 大文件拆分必须行为不变，并用现有回归证明。
- 文档发现实现缺口时，优先修实现；实现未落地时，文档必须写成未支持。
