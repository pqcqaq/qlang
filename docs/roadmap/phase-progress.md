# P1-P8 阶段总览

> 最后同步：2026-04-29

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
- typeck 的 `if` 表达式分支统一已对齐 control-flow summary：只有会继续执行的分支参与值类型统一；另一侧已经不可贯通时，不再把 `return` 等分支的 `Void`/任务值误报为分支类型不一致。
- package/workspace 基础能力已落地：`ql project init`、`add`、`targets`、`graph`、`lock`、`emit-interface`。
- `ql project add` 现在也可在创建 member 时直接写入 workspace 内本地依赖；`--existing` 还能把现有 package 或已移出的 member 重新纳入 workspace，真实 workspace 已不再只能“先手写 manifest 再接依赖图”。
- `ql project remove` 现在会先审计反向本地依赖；仍被其他 members 引用的包会直接拒绝删除，同时保留磁盘上的包目录；真实项目里 workspace 成员已形成更安全的 `init/add/remove` 闭环。
- `ql project dependents` 现在也已落地，并支持 `--json`；当 `ql project remove` 因反向依赖阻塞时，CLI 已能直接列出依赖它的 members，便于继续清理依赖边。从 package / workspace member 路径进入时也可自动推断目标包。
- `ql project dependencies` 现在也已落地，并支持 `--json`；workspace package 当前直接依赖了哪些 members 已可直接查询，正反向依赖审计不必再手读 manifest 或 `project graph`。从 package / workspace member 路径进入时也可自动推断目标包。
- `ql project targets` 现在也支持 `--package`、`--lib`、`--bin`、`--target` 过滤；真实 workspace 下排查某个 package 或单一 target 已不必再看全量列表。
- `ql project target add --bin <name>` 现在也已落地；真实项目里新增 bin target 不必再手建 `src/bin/*.ql` 和手改 `qlang.toml`，第一次显式写入 `[[bin]]` 时也会保留当前默认发现到的 targets。
- `ql project graph` 现在也支持 `--package` 聚焦到单个 workspace member 的包图；workspace 根图查询已不必再总是展开全部成员。
- `ql project add-dependency` / `remove-dependency` 现在也支持从 workspace 根配合 `--package` 直接指定目标 member；真实项目里批量查看依赖后可直接在根目录继续修改，不必先切到 member 路径。
- `ql project emit-interface` 现在也支持在 workspace 入口配合 `--package` 只发射或检查单个 member；大 workspace 下做接口增量刷新时不必继续全量扫所有包，定向发射时也可继续配合 `--output` 导出到自定义路径。
- `ql check` 现在也支持在 workspace 入口配合 `--package` 只检查单个 member；大 workspace 下做增量排查不必继续全量扫描全部包。
- `ql project remove --cascade` 现在也已落地；当目标包仍被其他 members 引用时，CLI 已可自动清理这些本地依赖边并继续移除 member。`ql project remove-dependency` 同时兼容 `[dependencies]` 和旧的 `[references].packages` 清理路径。
- `ql project add-dependency` / `remove-dependency` 现在也可直接维护已有 workspace member 的本地 `[dependencies]`；`remove-dependency --all` 还能按 package 名批量清理全部 dependents，若从依赖包自身的 package / workspace member 路径进入也可自动推断目标包名，创建后补依赖和移除依赖都不必再手改 manifest。
- project-aware `ql build` / `ql run` / `ql test` 已可在 package/workspace 根目录工作。
- `ql check` 现在也会在 workspace member 目录或源码路径入口上恢复外层 workspace 视角；真实项目里不再出现 `build/run/graph/lock` 是 workspace-aware、但 `check` 静默退回单 member package 的不一致。
- `ql build` / `ql run` 现在也可直接从 project 源码 target 路径进入 project-aware 流程；package 内源码路径和 workspace member 源码路径都不再掉回裸单文件输出语义。
- `ql build --list` / `ql run --list` 已落地，真实 workspace 里现在可以直接在命令内查看 discovered build targets；workspace member 目录或源码路径入口也会继承外层 workspace 视角；`ql run --list` 只展示 runnable targets，`--json` 复用 `ql.project.targets.v1`。
- `ql test` 新增 exact target rerun：`--target` 可精确选择已发现测试，直接运行 project `tests/` 下的单个测试文件时也会保留 project-aware 语义，workspace member 入口也不再掉回 package-only profile。
- `ql project graph` / `ql project targets` / `ql project lock` 现在也会在 workspace member 目录或源码路径入口上继承外层 workspace 上下文。
- `ql project emit-interface` 现在也支持从 workspace member 目录或 `.ql` 路径恢复外层 workspace 视角；plain、`--changed-only`、`--check` 都已覆盖，workspace 根还可继续用 `--package` 收敛到单个 member；若定向发射单个 member，也可继续配合 `--output` 走自定义导出路径。
- `qlang.toml` 已支持最小本地依赖、target path 和默认 profile。
- 第一版 `qlang.lock`、`ql.check --json`、`ql.build --json`、`ql.run --json`、`ql.test --json` 已落地。
- `ql project lock --json` 已补齐，真实项目现在可以在写锁文件和 `--check` 两条路径上稳定拿到机器可消费结果，而不必继续解析终端文本。
- project-aware `ql build/run/test` 已补上 direct local dependency 的最小执行桥接：受限 public top-level free function（非 `async` / 非 `unsafe`、无 generics / `where`、仅普通参数）的 wrapper bridge、bridgeable public `const/static` value declaration bridge、被这些 value/function 签名直接引用的 public 非泛型 `struct` / `enum` / 非 opaque `type alias` type bridge，以及这些 bridgeable public `struct` 上的受限 public receiver method forwarder。当前 root target 会按实际导入情况注入 public type/value declaration、function wrapper 与 method forwarder；value initializer 若直接命名或调用同模块 bridgeable public free function，会隐式补齐所需 function wrapper；导入的 value/function/method 签名若依赖同模块 bridgeable public `struct` / `enum` / 非 opaque `type alias`，也会隐式补齐所需 type bridge。未导入 sibling dependency 的同名符号不会再把 `ql build/run/test` 卡死在 target-prep，但实际导入的同名直依赖 type/value/function/extern 仍会分别触发 `dependency-type-conflict` / `dependency-value-conflict` / `dependency-function-conflict` / `dependency-extern-conflict`。
- 这一轮已把 public 非泛型 `enum` 的最小 LLVM 执行闭环补到真实跨包路径：direct local dependency public function 现在可以按值返回 enum，consumer 侧 `match` 已能稳定执行 unit / tuple / struct variant；tuple variant 也已补齐 `Enum.Variant(...)` 构造与 tuple pattern 解构。
- 本地与 direct local dependency 的 `impl` / `extend` / 唯一 trait `impl for` receiver method 直接调用现在都已打通到 LLVM 执行链路；`ql build` / `ql run` / `ql test` 已能真实执行 `value.read()`，以及 `let add = value.add; add(1)` 这类经不可变局部 alias 的 method value direct call。当前边界仍然很窄：更广义的 escaping / higher-order method value 仍未打通。
- healthy workspace 下的 dependency-backed LSP 已有一批可依赖能力：workspace symbol、source-preferred navigation、dependency completion、current-document `documentHighlight`、semantic tokens，以及 source-backed dependency `method / field / enum variant` workspace rename；source-preferred navigation 现在同时覆盖 workspace members 和 workspace 外本地路径依赖，definition / typeDefinition / references / `documentHighlight` / completion / workspace rename / `workspace/symbol` 都已有 open unsaved source 合同。healthy source 的 workspace import references 现在同时覆盖 value import 和 type import 的 alias/use，并会读取 open unsaved 的导出源码与其他 workspace consumer 源码；workspace root `function / const / static / struct / enum / trait / type alias` 的 references / rename 已覆盖 import/use 发起；这一轮又补齐了 import/use `prepareRename` 的 open-doc 路径。
- 这一轮把 healthy workspace root source-backed `enum variant / struct field / receiver method` 的 references 也补到了可见 analyzed workspace consumers；从导出包源码定义点或同文件使用点发起时，其他 members 里的真实成员使用现在会一起回收。
- 这一轮继续把 broken-source workspace root source-backed `enum variant / struct field / receiver method` 的 references 补到了可见 broken consumers；导出包源码侧发起 member references 时，不再只回收 healthy members。
- 这一轮把 healthy source 下的 workspace root source-backed `enum variant / struct field / receiver method` workspace rename 也正式接进了 root rename 路径；从导出包源码定义点或同文件使用点发起时，会联动当前文件与可见 workspace consumers 的真实 member uses，同时避免误改同名顶层 root import path。
- healthy workspace import `hover/definition/declaration/typeDefinition` 这一轮也补上了 open-doc 路径；未保存的导出 workspace 源码现在会直接参与导航，而不再落回磁盘旧版本。
- healthy workspace import `documentHighlight` 这一轮也补上了 open-doc 路径；当前文件 import/use 高亮现在会直接跟随未保存的导出 workspace 源码。
- workspace import semantic tokens 这一轮也补上了 open-doc 路径；healthy 与 parse-error fallback 两条着色路径都会直接跟随未保存的导出 workspace 源码。
- healthy workspace / 本地路径依赖的 source-backed dependency `method / field` 这一轮也补上了 open-doc rename 一致性；当成员只存在于未保存源码、磁盘 `.qi` 尚未更新时，`hover / definition / typeDefinition / references / documentHighlight / semantic tokens / prepareRename / workspace rename` 仍会继续命中真实源码；一旦已回到 workspace 源码定义，rename 也不会再顺手改写生成 `.qi`。
- `qlsp` 现在会声明 `.` completion trigger，VSCode 中输入成员访问和点分 dependency 路径时可直接自动弹出补全，而不必继续手动触发 completion。
- `workspace` 外本地路径依赖的 import references 现在也走源码优先路径；broken-source fallback 已补齐到这一条路径。
- `workspace/symbol` 现在也会对 workspace 外本地路径依赖做源码优先返回，并保留 `.qi` 回退；这条能力已补到 `workspace_roots` / 无打开文档入口，当前已锁住 value / method / trait / extend symbol。
- `workspace/symbol` 对 source-preferred 本地依赖的排除现在按 manifest 身份而不是 package name 执行；真实项目里即使存在同名本地依赖，也不会再把另一个依赖的 `.qi` symbol 一起过滤掉。
- 同名本地依赖的 type / enum / enum member、method / trait method / extend method `workspace/symbol` 现在也有 open-documents 与 `workspace_roots` 回归保护；`[dependencies]` 本地路径依赖入口也已锁住“源码优先返回当前依赖，同时保留兄弟依赖 `.qi` 符号”这条组合场景。
- `workspace/symbol` 现在也有真实 LSP request 回归，直接覆盖 workspace root 无打开文档、open unsaved 本地依赖源码优先、同名本地依赖隔离三条入口级合同。
- `textDocument/codeAction` 现在也有 unresolved type 的真实 LSP request 回归，直接覆盖已有依赖的 auto-import，以及 sibling workspace member 缺依赖时“插 `use` + 补 `qlang.toml`”的组合 quick fix。
- 这一轮把 `textDocument/references` 与 `textDocument/documentHighlight` 也补进了真实 `LspService` request smoke；当前已把 workspace dependency references、current-file documentHighlight 的入口级合同锁住。
- 这一轮把 `textDocument/typeDefinition`、`textDocument/documentSymbol`、`textDocument/semanticTokens/full` 也补进了真实 `LspService` request smoke；当前 same-file 显式类型名跳转、document symbol 嵌套返回、semantic token full data 的协议入口已被锁住。
- 这一轮把 `textDocument/prepareRename` / `rename` 也补进真实 `LspService` request 回归；当前 same-file rename 的 range/placeholder 与同文件 `WorkspaceEdit` 返回合同已经直接覆盖协议入口。
- source-preferred dependency definition / typeDefinition / references / current-document `documentHighlight` / completion 现在也按 manifest 身份区分同名本地依赖；真实项目里不会再把 navigation、高亮、completion 或 references 解析到另一个同名依赖实例。
- 这一轮把 `qlsp` 的 `textDocument/formatting` 也补上了；VSCode 现在可直接通过 `Format Document` 复用 `ql fmt` 背后的格式化实现做整文档格式化。当前仅对可成功解析的源码返回编辑，parse-error 文档会保守跳过并给 warning；真实 `LspService` request 测试已覆盖需要格式化、已格式化、parse-error 三条返回路径。
- 这一轮继续把 `qlsp` 的 `textDocument/implementation` 往前推；VSCode 现在除了 same-file trait/type surface、same-file 已唯一解析的 receiver method call，以及能回到打开中本地源码的 source-backed dependency method call，还补上了 broken open 源码里的 source-backed `impl` / `extend` block、trait impl method，以及 workspace root/source-backed 与 workspace / 本地路径依赖 source-preferred `struct / enum / trait` surface 的实现块聚合；implementation 搜索不再一遇到 parse-error open docs 就退回磁盘旧内容。当前 active root/source 自身处于 parse-error 时，从导出源码里的 `struct / enum / trait` 定义点和 trait method definition 发起 implementation 也会继续保守聚合真实实现；workspace root/source-backed concrete method call 在 broken open workspace consumers 存在时也仍可保守回到真实方法定义。workspace / 本地依赖路径继续优先读取 parseable open docs；更宽的全局 implementation index 继续后置。
- 这一轮又把 healthy analyzed source 的 trait-typed receiver method call `implementation` 补上了：从 workspace root/source-backed 与 source-backed dependency consumer 的 `runner.run()` 这类调用位点发起时，VSCode 现在会聚合当前包、可见 workspace members 与本地依赖源码里的匹配 `impl` methods；concrete typed `worker.run()` 仍保持唯一真实方法定义。
- 这一轮把 healthy source 下 source-preferred dependency type `implementation` 的目标解析也补齐到了 value / enum variant / struct field / method return type 这些非 import 位点：从 `built.value`、`Cmd.Retry(...)`、`holder.child`、`current.pulse()` 这类位置发起时，VSCode 现在也会回到真实 workspace dependency 源码里的 `impl` / `extend` block，而不是只在 import/type name 上生效。
- 这一轮又把这条 dependency member/type-driven `implementation` 合同补上了 open-doc 回归：当前 consumer 无论处于 healthy source 还是 parse-error，只要本地路径依赖的真实 type 只存在于未保存源码里，`current.extra` / `current.pulse()` 这类位置也会继续优先读取 open dependency 源码并回到真实 `impl` / `extend` block。
- 这一轮也把 `textDocument/implementation` 的 request 级回归继续补宽了：现在不再只靠 backend helper matrix，`LspService` 真请求路径已直接锁住 same-file trait surface、same-file type surface、same-file trait method definition 的数组结果、same-file concrete method call、same-file method declaration site 的空结果、healthy source-backed dependency value / enum variant / struct field / method return type `implementation` 定位结果、healthy source-backed dependency trait-typed method call 的 impl method 聚合数组结果、open-doc source-backed dependency trait-typed method call 的 source-preferred scalar 结果、broken-open source-backed dependency trait-typed method call 的 source-preferred scalar 结果、broken-current source-backed dependency trait-typed method call 的 source-preferred scalar 结果、workspace source-backed type surface、workspace trait import surface、broken open workspace type import surface、broken current workspace type / trait import surface、workspace root source-backed concrete method call 的 scalar 结果、workspace root source-backed trait-typed method call 的 impl method 聚合数组结果、open-doc workspace root source-backed trait-typed method call 的 source-preferred scalar 结果、workspace root source-backed method declaration site 的空结果、workspace root source-backed trait method definition 的 open-doc source-preferred scalar 结果、broken open workspace root source-backed concrete method call 的 scalar 结果、broken open workspace root source-backed trait-typed method call 的 source-preferred scalar 结果、broken current workspace root source-backed concrete method call 的 local-fallback scalar 结果、broken current workspace root source-backed trait-typed method call 的 impl method 聚合数组结果、workspace root source-backed trait surface 的 workspace impl block 聚合、broken open workspace root source-backed trait surface 的 workspace impl block 聚合数组结果、broken current workspace root source-backed trait surface 的 workspace impl block 聚合数组结果、workspace root source-backed trait method definition 的 workspace impl method 聚合、broken open workspace root source-backed trait method definition 的 workspace impl method 聚合数组结果、broken current workspace root source-backed trait method definition 的 workspace impl method 聚合数组结果、broken-open local dependency concrete method call `implementation` 的 scalar 结果、open local dependency field-driven / method-return-type `implementation` 数组结果、broken-source local dependency field-driven / method-return-type `implementation` 数组结果、healthy / broken-source same-named local dependency concrete method call `implementation` 的 scalar 结果、open same-named local dependency field-driven / method-return-type `implementation` 数组结果、open same-named local dependency type `implementation`、open same-named local dependency trait surface `implementation`、open same-named local dependency trait method call `implementation`、broken open same-named local dependency type `implementation`、broken open same-named local dependency trait surface `implementation`、broken open same-named local dependency trait method call `implementation`，以及 broken current source 下 same-named dependency type / trait surface / trait method call / field-driven / method-return-type `implementation`。
- 这一轮把 LSP request 级测试基础设施抽成 `crates/ql-lsp/tests/common/request.rs`，并新增 core editor request smoke；同一真实 `LspService` 请求链现在顺序覆盖 hover / definition / declaration / completion / implementation，避免这些入口只靠分散 helper 或单功能 request 回归间接保护。
- 这一轮开始整理 `goto_implementation` 分发：`crates/ql-lsp/src/backend.rs` 已先把 analyzed source 与 broken-source fallback 拆成独立分支，并把 source-backed dependency concrete method implementation 的 Method 过滤、源码定位和自引用抑制收敛到共享 helper；`impl` / trait-method implementation 聚合内部也已共享 package source snapshot 读取逻辑，parseable open doc、broken-open doc 和磁盘源码不再各自复制一套分支；implementation response / source-order location normalization 与 workspace type / trait-method response 构造也已集中到专用 helper，避免后续扩 index 时重复手写排序去重和 locations-to-response 包装。后续新增 implementation case 需要进入对应分支，而不是继续扩大单个总链路。
- 这一轮把 broken-open 本地依赖 concrete method call 也补齐了：当 dependency open doc 自身已经 parse-error、而磁盘源码 / `.qi` 仍旧落后时，从 `build().pulse()` 这类位点发起真实 `implementation` request，结果会继续优先回到打开中的依赖方法定义，而不是退回磁盘旧方法或直接失效。
- 这一轮顺手把 same-named 本地依赖的 concrete method call request 也显式锁住了：healthy source 和 broken-source 下，从 `build().ping()` / `build().ping(` 这类位点发起时，真实请求路径都会继续只回到匹配依赖的方法定义，不会串到兄弟依赖。
- 这一轮顺手把 source-backed dependency concrete method call 的 request 级 source-preferred 合同也显式锁住了：当本地路径依赖的方法只存在于未保存源码里时，无论当前 consumer 处于 healthy source 还是 parse-error，从 `build().pulse()` / `build().pulse(` 这类位点发起都继续优先回到 open dependency 源码里的真实方法定义。
- 这一轮顺手把 broken current same-named 本地依赖的 member/type-driven `implementation` request 也显式锁住了：从 `current.extra` / `current.pulse()` 这类位点发起时，真实请求路径会继续只回到匹配依赖的 open source `impl` / `extend` block，不会串到兄弟依赖。
- 这一轮顺手把 broken open workspace root source-backed method declaration site 的 request 级空结果也锁住了：当前 workspace consumer 处于 parse-error 时，方法声明名本身不会被当作 `implementation` 回给调用方。
- 这一轮顺手把 broken current workspace root source-backed method declaration site 的 request 级空结果也锁住了：当前导出源码处于 parse-error 时，方法声明名本身不会被当作 `implementation` 回给调用方。
- 这一轮把 broken current workspace root source-backed concrete method call 的歧义分支也对齐到了既有 helper 契约：repaired source 现在只再用于 dependency method 路径；若当前文件里有多个同名本地候选方法，request 路径会继续返回空结果，不再猜实现。
- 这一轮把 workspace root concrete trait-impl method call 的 request 级回归也补上了：concrete typed `worker.run()` 在 `LspService` 真请求路径里继续保持 scalar，不会误扩成 trait impl 聚合结果。
- 这一轮把 source-backed method declaration site 的 `implementation` 契约也对齐到了 same-file 基线：从 workspace root/source 源码里的方法声明名本身发起时，VSCode 不再把这条声明当作自己的 implementation 回给调用方；只有真实调用位点才会继续回到方法定义。
- 这一轮把 same-file `type` / `opaque type` alias 自身的 `implementation` 也接到 type surface：从 alias 定义或使用处发起时，会回到直接写在该 alias 上的 `impl` / `extend` block；`type Alias = Other` 不会透传到 `Other` 的实现，继续保持类型语义边界。
- 这一轮继续把 `type` / `opaque type` alias 的 `implementation` 从 same-file 推到 source-backed workspace/dependency：从 workspace root alias 定义点、dependency alias import/use、broken open dependency alias 源码，以及 broken current root alias 源码发起时，真实 LSP request 会继续按 alias 自身身份聚合直接写在该 alias 上的 `impl` / `extend` / trait impl block；alias 到底层类型的透传仍保持关闭。
- 这一轮把 broken-source implementation fallback 上的 same-named 本地依赖隔离也补齐了：当前 package 里存在 `alpha/beta` 这类同名导出依赖时，broken open consumers 的 type / trait implementation 聚合现在会先按真实 dependency identity 过滤 import binding，再决定可见本地名，不会再把兄弟依赖的 `extend` / `impl` 误收进来。
- 这一轮把 broken current consumer 的 trait-typed receiver method call `implementation` 也补上了：当前 active root/source 自身处于 parse-error 时，会先把 `runner.run(` 这类未闭合调用修复成最小可分析形态，再复用既有 source-backed local/dependency method implementation 路径；broken local trait call 现在会继续聚合可见 workspace impl methods，broken dependency trait call 也会继续优先读取 parseable open docs。
- 这一轮顺手把 workspace root `Find References` 也补到了 trait method definition；从导出源码里的 trait method 发起时，VSCode 现在会把可见 workspace members / 本地依赖源码里的匹配 impl methods 一起聚合进来，并优先读取 parseable open docs。
- broken-source 下，workspace import `hover/definition/typeDefinition`、direct imported-result member hover / completion / query / `documentHighlight`、dependency struct field label completion、dependency semantic tokens fallback、dependency enum variant 的 `completion/definition/typeDefinition/references/documentHighlight` fallback 已补齐到源码优先路径；workspace import references / query、dependency references / current-document `documentHighlight` / method completion 也已补上 open unsaved workspace member / local dependency source 合同；其中 import references 在补回 healthy workspace consumers 时也会读取这些 consumer 的 open docs；这一轮又补齐了 workspace root import/use `prepareRename` 与 workspace import alias rename 的 open-doc 路径。
- 同名本地依赖在这条 broken-source 路径上继续按 manifest 身份区分；`build().ping()` / `build().value`、dependency struct field label completion，以及 enum variant query / completion 都不会再串到兄弟依赖实例。
- broken-source 下的同名本地依赖 `workspace/symbol` 现在也补到了 `[dependencies]` 本地路径依赖入口；open document 和 `workspace_roots` 的顶层 type / interface / enum symbol、enum member，以及 method / trait method / extend method 都已锁住“源码优先 + 兄弟依赖 `.qi` 保留”这条组合场景。
- parse-error 下的 dependency rename 也已有保守 workspace-edit 回归保护；当前已锁住的窄 slice 包括 dependency method / struct field / enum variant 的源码定义点、源码内部引用、当前文件与同 workspace 其他使用文件联动改名；同名本地依赖上的 method / struct field / variant rename 也继续按 manifest 身份隔离。
- 这一轮继续把 `textDocument/implementation` 的 broken-source 保守面补上了：当前 consumer 处于 parse-error 时，source-backed dependency method call 仍会继续优先读取 open docs，并回到真实方法定义；当前 active root/source 自身处于 parse-error 时，从 root `struct / enum / trait` 定义点与 trait method definition 发起 implementation 也不再直接失效。
- 这一轮继续把 `textDocument/implementation` 往 workspace root/source-backed 定义点补了一步：从导出源码里的 `struct / enum / trait` 定义点发起时，现也会聚合可见 workspace members 的 `impl` / `extend` / trait `impl` block，而不再只停在 same-file。
- 这一轮把 broken current-buffer concrete method call `implementation` 也补齐了：当前 active root/source 自身处于 parse-error 时，会先尝试 source-backed dependency method 路径；若无法建立这条依赖回路，则只在同文件存在唯一候选方法定义时保守回到真实源码，避免同名本地方法歧义时误跳。
- 这一轮又把 broken current consumer 的 dependency trait `implementation` 补到了 `impl Trait for ...` header：broken-source 下的 trait import/query 现在会把 header 里的 trait 名当作有效引用上下文，workspace import definition 与 source-preferred dependency implementation 都不会再在这里静默失效。
- parse-error 下，workspace root `function / const / static / struct / enum / trait / type alias` 的 import/use references 现在也会补回当前 package 可见的 workspace members / 本地路径依赖里的其他 broken consumers；broken-source root import references 不再只看当前文件和 healthy consumers。
- parse-error 下，workspace root `function / const / static / struct / enum / trait / type alias` 现在也允许从当前 consumer 的 import/use 发起 rename（包含 alias import/use）；当前保守联动范围是当前 broken 文件、当前 package 其他源码文件、当前 package 可见的 workspace members / 本地路径依赖里的其他 consumer 源码，以及导出包源码；alias import 仍只更新导入路径。

## 当前主线

1. 先把 qlang 做到“可真实使用的最小项目语言”，而不是继续扩语言表面。
2. 主线先做 manifest、dependency-aware build/backend、最小 `stdlib`、真实 workspace LSP、安装与分发；P0 未完成前，不再把新语法和更宽 runtime 当主线。
3. 每一轮功能推进必须先落生产代码，再补测试和文档；只有测试或文档改动，不再计作一轮功能迭代。
4. 不再按固定日期承诺完成；每轮选择当前最能提升真实项目可用性的切片，做到实现、回归、文档一起收口。

## 下一轮

- stdlib / test harness：普通 Qlang package 形态的 `stdlib` 已继续扩面，当前 `std.core` 已覆盖区间外/无序边界外、降序、容差外、边界归一化、range/bounds 距离、安全 quotient/remainder、三/四值 extrema、三值 median、3/4 项整数聚合、2/3 项均值、Bool-to-Int 与 3/4 项 Bool all/any/none 聚合 helper，`std.test` 已覆盖 5/6 路 status 合并、max/min/median、sum/product/average、sign/compare、abs/abs-diff/range-span/bounds、quotient/remainder/has-remainder/factor、Bool all/any/none/Bool-to-Int、单边/双边 clamp / range-distance 断言与降序断言；生成的 `ql project init --stdlib` consumer smoke 也会消费这些新 helper。下一步继续扩只依赖稳定语言面的基础 helper，并优先让模板覆盖真实 consumer 路径。
- build/backend：继续优先补真实项目里高频的 direct local dependency value/type/member 调用面；本轮已把 public 非泛型、非 opaque type alias 从 declaration bridge 推到普通值兼容，typeck 现在覆盖 return、call argument、assignment、数组/分支统一、pattern literal、bool/numeric/string 操作里的透明 alias target；LLVM backend 已跟进 direct lowering 所需的 alias 赋值、数组/字段值检查、二元操作和 callable 参数断言，并用 build/run/test 真实 consumer 锁住 `Count -> Score -> Int` 的跨包签名、alias 算术与 wrapper 调用。`opaque type`、泛型 alias、`impl` / `extend` 身份匹配继续保持不透明；后续若 `stdlib` 继续暴露阻塞项，优先修阻塞项而不是扩新语法。
- typeck/backend 回归清理：`ql-codegen-llvm` 中仍有一批 IR 形状耦合测试需要分组收敛。本轮已先修正真实语义问题：`if` 一侧不可贯通时不再强制与可贯通分支统一值类型；剩余失败优先改成语义级断言或更稳定的 IR helper，而不是继续追加脆弱字符串匹配。
- LSP：继续把 `textDocument/implementation` 从已完成的 trait/type surface、workspace root/source-backed type definition surface、workspace root/source-backed concrete / trait-typed method call、source-backed dependency concrete / trait-typed method call、dependency non-import type-driven positions、trait method definition，以及 broken current-buffer concrete / trait-typed method call / broken-source open dependency member-type surface，扩到更宽的 implementation index；更广的全局聚合继续后置。
- 文档：入口页继续只保留结论、边界和最近 checkpoint，不再追加流水账。

## 明确后置

- 更广义的 cross-file rename / workspace edits / 更完整 code actions
- registry / publish
- 更宽的 async/runtime/Rust interop 扩面
- 新语法和更远的类型系统设计

## 继续阅读

- [当前支持基线](/roadmap/current-supported-surface)
- [开发计划](/roadmap/development-plan)
- [工具链设计](/architecture/toolchain)
