# 当前支持基线

> 最后同步：2026-05-18

这页记录当前可依赖能力。更细节的行为以代码和回归测试为准。

## 真相源

- 实现：`crates/*`、`stdlib/packages/*`
- CLI / project 回归：`crates/ql-cli/tests/*`
- analysis / LSP 回归：`crates/ql-analysis/tests/*`、`crates/ql-lsp/tests/*`
- executable smoke：`ramdon_tests/*`

## 已支持

### 编译器和运行

- `lexer -> parser -> HIR -> resolve -> typeck -> MIR -> LLVM` 主链路可用。
- LLVM 输出覆盖 `llvm-ir`、`asm`、`obj`、`exe`、`dylib`、`staticlib`。
- `if` 表达式要求分支类型合流；语句形式只检查分支内部语义。
- async/runtime 只有最小保守子集。

### CLI 和项目

- `ql check/fmt/mir/ownership/runtime/build/run/test/project/ffi` 已实现。
- `ql project init/add/remove/status/dependencies/dependents/targets/graph/lock/emit-interface` 已可维护本地 workspace。
- `ql build/run/test/check` 支持 project-aware 入口和常用 `--json`、`--list`、`--package`、`--target`；`ql build/run --list --json --package --target` 已覆盖包内相对 target 和 selection failure，`ql run --list --json` 已覆盖 target selector miss selection failure；`ql project targets --json` 已覆盖 target selector miss selection failure；`ql test --package` 已覆盖 workspace root/member directory/member file 的文本、JSON、`--list --json`、`--filter`、`--target` 和 selector 错误入口；`ql test` 文本路径已覆盖 package/workspace/member-file default profile 与显式 `--profile` override，以及 package/workspace/direct project test file 的显式 `--release` override；`ql test --list --json` 已覆盖 standalone package path、workspace package UI listing、workspace package UI selector miss、package/workspace/member-file/direct-file default profile 与显式 profile override、direct project smoke/UI file listing 和 direct file `--package` / `--target` / `--filter` selector listing；`ql test --json` 已覆盖 manifest-load、target-discovery（含 selected package）、direct-source/workspace/package-path/direct-file selector preflight、package/direct project smoke/ui file success、workspace/package/direct project UI snapshot failure、workspace package UI selector success、package/workspace/direct-file path 的显式 `--release` override、direct project test file 的显式 `--release` override、package/workspace/member-file/direct-file default profile 与显式 `--profile` override、direct smoke/ui file combined selector success，以及 no-tests / target miss / filter miss selection failure；`ql check --package` 已覆盖 workspace root/member source/member directory 的文本、JSON、missing selector 和 `--sync-interfaces` 入口。
- `ql build/run --package <name> --target src/bin/foo.ql` 和 `ql test --package <name> --target tests/foo.ql` 支持包内相对 target；仍保留 workspace-relative 和 absolute target 匹配。
- `ql build/run/test/check` 已共享 project context / request-root resolution；`build/run` 复用 project source target selector，`test` 保留 project-aware test-file 语义，且都有真实 CLI 回归覆盖。
- `ql build/run` 已覆盖 package/workspace default profile 与显式 `--profile` / `--release` override 的文本和 JSON 合同，并覆盖 workspace member source default profile（`ql build --json`、`ql run` 文本）及 direct project source 的文本/JSON `--release` override；`ql build` 已覆盖 workspace `--package` JSON dependency-closure 输出；`run/test` 已覆盖 workspace `--package` JSON 关键路径；`ql run --json` 已覆盖 manifest-load、target-discovery 和 target-selection（无 runnable / 多 runnable）preflight JSON；`ql build/run --json` 已覆盖 source-selector conflict 和 `--package --target` selector miss JSON。
- `ql project graph/targets/status/emit-interface` 已覆盖 workspace root、workspace member path 和 workspace `--package` selector；`project graph/targets/status --json` 已覆盖 manifest-load failure，`project graph/targets/status --json` 已覆盖 standalone package root/source path，`project emit-interface` 已覆盖 standalone package source path 的普通、`--check`、`--changed-only` 和 `--changed-only --check` 组合，以及 workspace member source/directory；`project graph/targets/status --package --json` 已覆盖 member source/directory 入口，且 graph/status package selector failure 已输出 selection failure JSON；`ql project target add` 支持 workspace root `--package` 和 member directory 入口。
- `ql project dependencies/dependents` 已覆盖 standalone package root/source path 的文本和 JSON 输出；`dependencies` 支持 standalone package local dependencies 和 `--name` selector，`dependents` 支持 standalone package 空结果、source path JSON 和 `--name` selector；workspace member source/directory 派生包名和 package selector failure 也有 JSON 回归。
- `ql project lock` 已覆盖 workspace root、workspace member source/directory 写入 workspace lockfile；standalone package root/source path 已覆盖 `--json` 写入和 `--check --json` up-to-date 检查；`--json` 下 manifest-load 和 lockfile-render 失败输出稳定 failure JSON。
- `ql project add-dependency/remove-dependency` 已覆盖 workspace member source/directory 和 workspace root `--package` selector，依赖编辑逻辑已从 CLI 入口拆出。
- `ql run` 已用真实 smoke 覆盖 dependency public functions/values/types/methods/traits、direct dependency generic public functions、workspace `--package` dependency generic、workspace `--package --target` 包内相对 binary、workspace `--package` dependency generic JSON、transitive generic wrapper/helper specialization 和 dependency generic JSON 输出。
- 单文件 `ql build/run/test file.ql` 可复用本地 generic free function direct-call specialization。
- `ql project init --stdlib` 从 `stdlib/examples/starter` 复制 starter，生成依赖 `std.core`、`std.option`、`std.result`、`std.array`、`std.test` 的 package/workspace 项目，并用 `check/run/test`、`project emit-interface`、`project emit-interface --check`、package `ql check/build/run/test --json`、package `ql project graph/status --json`、package `ql project dependencies` 文本、JSON 和 `--name` JSON、package `ql project dependents --name --json`、workspace `ql check --json`、workspace `ql project graph/status --json --package`、workspace `ql project dependencies --name` 文本和 JSON、以及 `ql build/run/test --json --package` 直接覆盖 generic option/result assertions、数组 equality/reverse assertions、length-generic array helpers、重复数组和 starter `.qi` 产物；check JSON 断言 stdlib interface 加载，graph/status/dependencies JSON 断言生成接口和 stdlib local dependencies 可见，build JSON 断言 stdlib dependency closure、selected lib/bin 和 interface。

### stdlib 和依赖桥接

- 跨包执行仍是保守切片：public const/static、受限 free function、`extern "c"`、部分 type/value bridge、受限 receiver method。
- public/local generic free function 支持 direct-call 多实例 specialization。
- 数组长度泛型参数可在函数体内作为 `Int` 值读取。
- dependency generic specialization 能递归处理同依赖模块内 generic helper 直调，以及 dependency generic body 内对直接依赖 generic helper 的导入调用；也能从 named/expression args、generic carrier、返回类型上下文、零参数泛型显式上下文和外层调用参数推断 direct-call specialization。未使用的 direct dependency generic import 不再触发 bridge 合成失败。
- `ql test` 已用真实 smoke 覆盖 dependency public functions、public values、generic public functions、generic wrapper/helper specialization、public struct/type alias/method/trait bridge；workspace package、package 和直接 project test 文件覆盖 UI snapshot JSON success/failure；workspace package UI listing 覆盖 `--list --json --package --target --filter` 成功和 selector miss 合同；package/workspace/member-file default profile 和显式 `--profile` override 的文本合同已覆盖，package/workspace path 的显式 `--release` 文本和 JSON 合同也已覆盖，direct project test file 的显式 `--release` 文本和 JSON 合同也已覆盖，package/workspace/member-file/direct-file 的 JSON 合同也已覆盖；package path、workspace 全量 member path 和直接 project test file 都覆盖 local generic specialization 与 package-under-test public bridge 组合路径，且 workspace 全量 member path 同时覆盖文本和 JSON 成功合同；直接 project test file 还覆盖 direct dependency bridge、dependency generic public function bridge 和 dependency generic wrapper/helper bridge，其中 dependency generic bridge 已覆盖 `--json` 成功报告，直接 smoke/UI 文件 listing、`--package`、`--target`、`--filter` selector、组合 selector listing 和组合 selector 执行已覆盖 JSON 合同。
- repo stdlib workspace 的 source-only 复制 fixture 已覆盖 `ql check --sync-interfaces --json`、后续 `ql check --json`、全量 `ql test` 和 `ql test --json`；仓库内 stdlib 已覆盖 `ql build --json`、starter `ql run --json`、`project emit-interface --check`、`project status --json`、`project graph --json`、`project targets --json`、starter `project dependencies --json` 和 `std.option` / `std.core` 的 `project dependents --json`；6 个 package/example smoke、接口同步产物、构建产物与可执行产物存在性作为 downstream 可用性 gate。
- `Option[T]`、`Result[T, E]`、`std.core` scalar/predicate/bool helpers、`std.core` / `std.array` length-generic aggregate/order/median helpers、`std.test` 泛型 equality/array/option/result assertions 和 `merge_statuses[N]` 已有真实 smoke。
- stdlib package-local smoke 已改用 length-generic 状态数组聚合，不再保留测试内 `sum4` / `sum6` 固定 arity helper。
- 语言级重复数组字面量 `[value; N]` 已支持整数字面量长度和数组长度泛型；`std.option` / `std.result` concrete carrier API、`std.array` 固定长度 helper 和 `std.test` typed facade 已删除。

### LSP 和 VSCode

- same-file：hover、keyword hover、definition、declaration、typeDefinition、references、documentHighlight、completion、semantic tokens、formatting、codeAction、codeLens、callHierarchy、typeHierarchy、rename。
- workspace：`workspace/symbol`、`implementation`、open-doc dependency navigation、依赖调用 signatureHelp、保守 workspace rename；`workspace/symbol` 已用真实 `stdlib/` workspace root 覆盖当前 stdlib public symbols，并在缺失接口等包级预检失败时回退索引源码符号；`codeLens` 也已在真实 workspace consumer 场景下保持可用，包括 open-doc consumer。
- dependency imports：`textDocument/documentLink` 可把已解析 dependency import 链接到对应 `.qi` 接口文件，真实 stdlib workspace 已覆盖。
- 第三方旧接口里的 stdlib 兼容 API 会在真实 `textDocument/completion`、`textDocument/hover` 和 `textDocument/semanticTokens/full/range` 请求中提示 deprecated 并带迁移 guidance；当前 stdlib 正式 API 使用 generic carrier 和 length-generic helpers。
- 真实 stdlib LSP smoke 已按 request family 拆分，覆盖 completion/resolve、hover、definition/declaration/typeDefinition、references/documentHighlight、implementation、documentSymbol、signatureHelp/inlayHint、folding/selection、formatting、codeAction/resolve、call/type hierarchy、semanticTokens full/range、rename；`diagnostics`、`codeLens`、`workspace/symbol` 由独立 smoke 覆盖，其中 `diagnostics`、`codeLens`、`workspace/symbol` 和 rich app 请求都有 real stdlib open-doc 优先级回归。
- semantic tokens 覆盖 parse-error fallback、注释 token 和 `self` keyword token。
- inlay hints 覆盖 same-file inferred local type，以及 same-file/dependency 调用参数名提示；方法调用会隐藏 receiver `self`。
- folding range 覆盖代码块、块注释和连续整行 `//` 注释；字符串内注释标记不会生成注释折叠。
- references 和 codeLens 覆盖同文件引用/实现计数，并能在 workspace package 源文件上统计可见 consumer 的引用/实现，包括 open-doc consumer。
- formatting：document/range/on-type formatting 复用 `ql fmt`。
- VSCode 插件是 thin client，不自带 `qlsp`。

## 未支持

- 完整 dependency-aware backend
- 完整 generic monomorphization、泛型 alias lowering、自动 prelude
- registry、version solving、publish workflow
- release 和 VSCode Marketplace 分发
- 完整 workspace-wide rename/refactor/code actions/references index
- 完整 trait solver、effect system、async/runtime 语言面

## 主要缺口

- `ql-cli` 主链路仍过度集中；`build/run/test/check` 的入口 request-context，以及 `project emit-interface/graph/dependencies/dependents/add/status` 的 workspace member lookup 已共享并统一了 unresolved/ambiguous member reporting；`project add-dependency/remove-dependency` 的编辑逻辑已拆出。剩余重点是继续收口 reporting 细节和真实 workspace smoke。
- `ql test` 的 package-under-test/direct-dependency bridge source override 已覆盖 package path 和直接 project test file 的 local generic 组合；剩余重点是继续抽成共享 project pipeline，并扩大到更宽 dependency-aware backend 语义。
- LSP 还不是稳定 workspace service；diagnostics、references、rename、symbols 和 rich editor hints 需要统一 workspace index。
- stdlib public API 已清掉 concrete carrier、主要固定 arity 包装和 `std.test` typed facade；`std.core` package-local smoke 已覆盖公开 scalar/predicate/bool helpers，`std.result` package-local smoke 和 `project init --stdlib` starter 已直接覆盖 generic carrier 语义与 option/result assertions。剩余重点是更完整 generic backend、共享 project pipeline 和更宽 dependency-aware backend。
- `project init --stdlib` starter 已迁到 `stdlib/examples/starter`；后续重点是让更多 stdlib examples/downstream smoke 覆盖更宽 dependency-aware backend。

## 继续阅读

- [开发计划](/roadmap/development-plan)
- [阶段总览](/roadmap/phase-progress)
- [工具链设计](/architecture/toolchain)
