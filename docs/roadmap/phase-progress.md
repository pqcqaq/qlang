# P1-P8 阶段总览

> 最后同步：2026-04-21

这页只保留阶段级结论和当前焦点。

## 阶段状态

| 阶段 | 状态 | 结论 |
| --- | --- | --- |
| Phase 1 | 已完成 | lexer / parser / formatter / 基础 CLI |
| Phase 2 | 已完成 | HIR / resolve / typeck / diagnostics / 最小 query / 最小 LSP |
| Phase 3 | 已完成 | MIR / ownership / cleanup-aware 分析 |
| Phase 4 | 已完成 | LLVM backend 与主要 artifact 路径 |
| Phase 5 | 已完成 | C ABI、header projection、host 集成 |
| Phase 6 | 已完成 | same-file hover / definition / rename / completion / semantic tokens / document symbol |
| Phase 7 | 进行中 | async / runtime / task-handle / build / interop 的保守扩面 |
| Phase 8 | 进行中 | package/workspace、`.qi`、local dependencies、dependency-backed tooling |

## 已完成的关键进展

- 编译器主路径已经稳定为 AST -> HIR -> resolve -> typeck -> MIR -> LLVM。
- package/workspace 基础能力已落地：`ql project init`、`add`、`targets`、`graph`、`lock`、`emit-interface`。
- `ql project add` 现在也可在创建 member 时直接写入 workspace 内本地依赖；真实 workspace 已不再只能“先手写 manifest 再接依赖图”。
- `ql project remove` 现在可按 package 名把 member 从 workspace manifest 里安全摘除，同时保留磁盘上的包目录；真实项目里 workspace 成员已形成 `init/add/remove` 的保守闭环。
- project-aware `ql build` / `ql run` / `ql test` 已可在 package/workspace 根目录工作。
- `ql check` 现在也会在 workspace member 目录或源码路径入口上恢复外层 workspace 视角；真实项目里不再出现 `build/run/graph/lock` 是 workspace-aware、但 `check` 静默退回单 member package 的不一致。
- `ql build` / `ql run` 现在也可直接从 project 源码 target 路径进入 project-aware 流程；package 内源码路径和 workspace member 源码路径都不再掉回裸单文件输出语义。
- `ql build --list` / `ql run --list` 已落地，真实 workspace 里现在可以直接在命令内查看 discovered build targets；workspace member 目录或源码路径入口也会继承外层 workspace 视角；`ql run --list` 只展示 runnable targets，`--json` 复用 `ql.project.targets.v1`。
- `ql test` 新增 exact target rerun：`--target` 可精确选择已发现测试，直接运行 project `tests/` 下的单个测试文件时也会保留 project-aware 语义，workspace member 入口也不再掉回 package-only profile。
- `ql project graph` / `ql project targets` / `ql project lock` 现在也会在 workspace member 目录或源码路径入口上继承外层 workspace 上下文。
- `ql project emit-interface` 现在也支持从 workspace member 目录或 `.ql` 路径恢复外层 workspace 视角；当前保守边界是不带 `--output`，并已覆盖 plain、`--changed-only`、`--check`。
- `qlang.toml` 已支持最小本地依赖、target path 和默认 profile。
- 第一版 `qlang.lock`、`ql.check --json`、`ql.build --json`、`ql.run --json`、`ql.test --json` 已落地。
- `ql project lock --json` 已补齐，真实项目现在可以在写锁文件和 `--check` 两条路径上稳定拿到机器可消费结果，而不必继续解析终端文本。
- project-aware `ql build/run/test` 已补上 direct local dependency 的四条最小执行桥接：受限 public top-level free function（非 `async` / 非 `unsafe`、无 generics / `where`、仅普通参数）的 wrapper bridge、bridgeable public `const/static` value declaration bridge、被这些 value/function 签名直接引用的 public 非泛型 `struct` type bridge，以及这些 bridgeable public `struct` 上的受限 public receiver method forwarder。当前 root target 会按实际导入情况注入 public type/value declaration、function wrapper 与 method forwarder；value initializer 若直接命名或调用同模块 bridgeable public free function，会隐式补齐所需 function wrapper；导入的 value/function/method 签名若依赖同模块 bridgeable public `struct`，也会隐式补齐所需 type bridge。未导入 sibling dependency 的同名符号不会再把 `ql build/run/test` 卡死在 target-prep，但实际导入的同名直依赖 type/value/function/extern 仍会分别触发 `dependency-type-conflict` / `dependency-value-conflict` / `dependency-function-conflict` / `dependency-extern-conflict`。
- 本地与 direct local dependency 的 `impl` / `extend` / 唯一 trait `impl for` receiver method 直接调用现在都已打通到 LLVM 执行链路；`ql build` / `ql run` 已能真实执行 `value.read()`，以及 `let add = value.add; add(1)` 这类经不可变局部 alias 的 method value direct call。当前边界仍然很窄：更广义的 escaping / higher-order method value 仍未打通。
- healthy workspace 下的 dependency-backed LSP 已有一批可依赖能力：workspace symbol、source-preferred navigation、dependency completion、current-document `documentHighlight`、semantic tokens，以及 source-backed dependency `method / field / enum variant` workspace rename；source-preferred navigation 现在同时覆盖 workspace members 和 workspace 外本地路径依赖，definition / typeDefinition / references / `documentHighlight` / completion / workspace rename / `workspace/symbol` 都已有 open unsaved source 合同。healthy source 的 workspace import references 现在同时覆盖 value import 和 type import 的 alias/use，并会读取 open unsaved 的导出源码与其他 workspace consumer 源码；workspace root `function / const / static / struct / enum / trait / type alias` 的 references / rename 已覆盖 import/use 发起；这一轮又补齐了 import/use `prepareRename` 的 open-doc 路径。
- healthy workspace import `hover/definition/declaration/typeDefinition` 这一轮也补上了 open-doc 路径；未保存的导出 workspace 源码现在会直接参与导航，而不再落回磁盘旧版本。
- healthy workspace import `documentHighlight` 这一轮也补上了 open-doc 路径；当前文件 import/use 高亮现在会直接跟随未保存的导出 workspace 源码。
- workspace import semantic tokens 这一轮也补上了 open-doc 路径；healthy 与 parse-error fallback 两条着色路径都会直接跟随未保存的导出 workspace 源码。
- healthy workspace / 本地路径依赖的 source-backed dependency `method / field` 这一轮也补上了 open-doc rename 一致性；当成员只存在于未保存源码、磁盘 `.qi` 尚未更新时，`hover / definition / typeDefinition / references / documentHighlight / semantic tokens / prepareRename / workspace rename` 仍会继续命中真实源码；一旦已回到 workspace 源码定义，rename 也不会再顺手改写生成 `.qi`。
- `qlsp` 现在会声明 `.` completion trigger，VSCode 中输入成员访问和点分 dependency 路径时可直接自动弹出补全，而不必继续手动触发 completion。
- `workspace` 外本地路径依赖的 import references 现在也走源码优先路径；broken-source fallback 已补齐到这一条路径。
- `workspace/symbol` 现在也会对 workspace 外本地路径依赖做源码优先返回，并保留 `.qi` 回退；这条能力已补到 `workspace_roots` / 无打开文档入口，当前已锁住 value / method / trait / extend symbol。
- `workspace/symbol` 对 source-preferred 本地依赖的排除现在按 manifest 身份而不是 package name 执行；真实项目里即使存在同名本地依赖，也不会再把另一个依赖的 `.qi` symbol 一起过滤掉。
- 同名本地依赖的 type / enum / enum member、method / trait method / extend method `workspace/symbol` 现在也有 open-documents 与 `workspace_roots` 回归保护；`[dependencies]` 本地路径依赖入口也已锁住“源码优先返回当前依赖，同时保留兄弟依赖 `.qi` 符号”这条组合场景。
- source-preferred dependency definition / typeDefinition / references / current-document `documentHighlight` / completion 现在也按 manifest 身份区分同名本地依赖；真实项目里不会再把 navigation、高亮、completion 或 references 解析到另一个同名依赖实例。
- broken-source 下，workspace import `hover/definition/typeDefinition`、direct imported-result member hover / completion / query / `documentHighlight`、dependency struct field label completion、dependency semantic tokens fallback、dependency enum variant 的 `completion/definition/typeDefinition/references/documentHighlight` fallback 已补齐到源码优先路径；workspace import references / query、dependency references / current-document `documentHighlight` / method completion 也已补上 open unsaved workspace member / local dependency source 合同；其中 import references 在补回 healthy workspace consumers 时也会读取这些 consumer 的 open docs；这一轮又补齐了 workspace root import/use `prepareRename` 与 workspace import alias rename 的 open-doc 路径。
- 同名本地依赖在这条 broken-source 路径上继续按 manifest 身份区分；`build().ping()` / `build().value`、dependency struct field label completion，以及 enum variant query / completion 都不会再串到兄弟依赖实例。
- broken-source 下的同名本地依赖 `workspace/symbol` 现在也补到了 `[dependencies]` 本地路径依赖入口；open document 和 `workspace_roots` 的顶层 type / interface / enum symbol、enum member，以及 method / trait method / extend method 都已锁住“源码优先 + 兄弟依赖 `.qi` 保留”这条组合场景。
- parse-error 下的 dependency rename 也已有保守 workspace-edit 回归保护；当前已锁住的窄 slice 包括 dependency method / struct field / enum variant 的源码定义点、源码内部引用、当前文件与同 workspace 其他使用文件联动改名；同名本地依赖上的 method / struct field / variant rename 也继续按 manifest 身份隔离。
- parse-error 下，workspace root `function / const / static / struct / enum / trait / type alias` 的 import/use references 现在也会补回当前 package 可见的 workspace members / 本地路径依赖里的其他 broken consumers；broken-source root import references 不再只看当前文件和 healthy consumers。
- parse-error 下，workspace root `function / const / static / struct / enum / trait / type alias` 现在也允许从当前 consumer 的 import/use 发起 rename（包含 alias import/use）；当前保守联动范围是当前 broken 文件、当前 package 其他源码文件、当前 package 可见的 workspace members / 本地路径依赖里的其他 consumer 源码，以及导出包源码；alias import 仍只更新导入路径。

## 当前主线

1. 先把 qlang 做到“可真实使用的最小项目语言”，而不是继续扩语言表面。
2. 主线先做 manifest、dependency-aware build/backend、真实 workspace LSP、安装与分发；P0 未完成前，不再把新语法和更宽 runtime 当主线。
3. 每一轮功能推进必须先落生产代码，再补测试和文档；只有测试或文档改动，不再计作一轮功能迭代。

## 明确后置

- 更广义的 cross-file rename / workspace edits / code actions
- registry / publish
- 更宽的 async/runtime/Rust interop 扩面
- 新语法和更远的类型系统设计

## 继续阅读

- [当前支持基线](/roadmap/current-supported-surface)
- [开发计划](/roadmap/development-plan)
- [工具链设计](/architecture/toolchain)
