# 当前支持基线

> 最后同步：2026-04-12

这页只回答“今天真实可依赖的能力边界”。

## 真相源

当前基线以这些文件为准：

- 实现：`crates/*`
- build / codegen / executable：`crates/ql-cli/tests/codegen.rs`、`crates/ql-cli/tests/string_codegen.rs`、`crates/ql-cli/tests/string_compare_codegen.rs`、`crates/ql-cli/tests/executable_examples.rs`
- project / workspace / interface：`crates/ql-cli/tests/project_graph.rs`、`crates/ql-cli/tests/project_interface.rs`、`crates/ql-cli/tests/project_check.rs`
- dependency-backed editor contracts：`crates/ql-analysis/tests/*`、`crates/ql-lsp/tests/*`
- 已提交的 `ramdon_tests/executable_examples/` 与 `ramdon_tests/async_program_surface_examples/` smoke corpus

如果文档与这些实现或测试冲突，以代码和回归矩阵为准，再回头修正文档。

## 一页结论

- Phase 1 到 Phase 6 的编译器和 same-file tooling 已经进入稳定迭代阶段。
- Phase 7 已有保守 async/runtime/build 子集：`async fn`、`await`、`spawn`、`for await`、最小 `ql-runtime`、program-mode async `main`、保守 async `staticlib` / `dylib` 子集，以及 task-handle-aware lowering。
- Phase 8 已进入真实交付面：最小 `qlang.toml` package/workspace graph、`.qi` V1 emit/load、`ql project graph`、`ql project emit-interface`、`ql build --emit-interface`、`ql check --sync-interfaces`。
- dependency-backed cross-file tooling 已开放首批合同：imported dependency symbol hover / definition / declaration / references、import path completion、dependency enum variant completion / `typeDefinition`、显式 struct field-label completion，以及语法局部可恢复 receiver 的最小 dependency member/query/typeDefinition。
- 保守 `workspace/symbol` 现在可以在有 manifest 上下文时搜索当前包源码、同一 workspace 的 sibling members，以及已加载 dependency `.qi` public symbols。

## 当前已开放的构建表面

### 前端与 same-file tooling

- lexer、parser、formatter、diagnostics、HIR、resolve、typeck、MIR、borrowck 已进入主路径。
- same-file hover、definition、references、rename、completion、semantic tokens、document symbol 已接通到共享 analysis/query surface。

### CLI 与 build

当前已实现的 CLI 子命令：

- `ql check <file-or-dir> [--sync-interfaces]`
- `ql fmt <file> [--write]`
- `ql mir <file>`
- `ql ownership <file>`
- `ql runtime <file>`
- `ql build <file> [--emit llvm-ir|asm|obj|exe|dylib|staticlib] [--release] [-o <output>] [--emit-interface] [--header] [--header-surface exports|imports|both] [--header-output <output>]`
- `ql project graph [file-or-dir]`
- `ql project emit-interface [file-or-dir] [-o <output>] [--changed-only] [--check]`
- `ql ffi header <file> [--surface exports|imports|both] [-o <output>]`

当前稳定的 artifact 面：

- `llvm-ir`、`asm`（默认输出 `.s`）、`obj`、`exe`、`dylib`、`staticlib`
- `ql ffi header` 和 build-side header sidecar
- `ql runtime` 的 runtime requirement / hook ABI 输出

### project / workspace / `.qi`

- package directory、`qlang.toml`、包内源码路径、workspace-only 根 manifest 都可进入 package-aware `check` 流程。
- `ql project graph` 会展示 package/member、references，以及默认 `.qi` 的 `valid` / `missing` / `invalid` / `stale` 状态；`stale` 会给出 `stale_reasons`，`invalid` / `unreadable` 也会给出一行 `detail`；每条 `reference_interfaces` 现在也会显式带出对应的 reference manifest 路径，方便直接定位引用项；如果 direct dependency 下面还有更深层坏引用，也会补一个保守的 `transitive_reference_failures` 计数和 `first_transitive_failure_manifest`。
- workspace 根 `ql project graph` 在单个 member manifest 无法加载时不会整张图失败；已解析 members 会继续输出，坏 member 会落成 `package: <unresolved>` + `member_error`。
- `ql project graph` 对 `reference_interfaces` 的 `unresolved-manifest` / `unresolved-package` 现在也会带 `detail`，直接说明是引用 manifest 语法坏了，还是引用目标没有 `[package].name`。
- `ql project emit-interface` 支持 package 和 workspace 批量写出；`-o/--output` 仍仅支持 package。package 模式下若同一个 package 里有多个坏源码阻塞 `.qi` 发射，CLI 现在会继续打印后续坏源码诊断，最后再汇总 failing source file 数；只有多失败场景才额外补 `first failing source file`。direct package emit 失败时现在也会补 `failing package manifest` 和可直接重跑的 `ql project emit-interface <manifest>` hint。
- direct package `ql project emit-interface --output <path>` 如果因为源码错误或目标路径不可写而失败，stderr 里的重跑 hint 现在会保留同一个 `--output <path>`，不再退回默认 `.qi` 路径。
- 如果 `ql project emit-interface` / `ql build --emit-interface` / `ql check --sync-interfaces` 是因为默认 `.qi` 输出路径本身写不进去而失败（例如同名目录占位），stderr 现在会明确补 `failing interface output path`，并把 hint 改成“先修输出路径再重跑”，不再误导成 package 源码错误。
- workspace 根 `ql project emit-interface` 在单个 member 发射失败时不会立刻中断；已成功的 members 会继续输出，最后再汇总失败成员数；只有多失败场景才额外补 `first failing member manifest`。如果某个 member 是因为 package 源码错误而发射失败，stderr 现在会当场先补该 member 的 `failing package manifest`、`failing workspace member manifest`，再给可直接重跑的 hint；如果 member manifest 自身无法加载，局部错误块现在也会立即补 `failing workspace member manifest` 和针对该 member manifest 的直接 rerun hint，不需要只靠最终汇总回看。
- `ql project emit-interface --changed-only` 在写出路径上只重发非 `valid` 接口；发射失败时的局部重跑 hint 现在也会保留同一个 `--changed-only`，不再退回全量重发；搭配 `--check` 时不会写文件，已 `valid` 的接口会报告 `up-to-date interface`，check 失败时的局部修复 hint 也同样保留 `--changed-only`。
- `ql project emit-interface --check` 只校验当前 package/workspace 的默认 `.qi` 是否都处于 `valid` 状态；若发现 `stale` 会说明原因，若遇到 `invalid` / `unreadable` 也会直接打印 detail；package 级和 workspace member 级的 check 失败现在也会补 `failing package manifest`，不再只剩 artifact 路径。
- workspace 根 `ql project emit-interface --check` 在单个 member manifest 无法加载时也不会立刻中断；已检查 members 会先输出，member manifest 加载失败的局部错误块现在也会立即补 `failing workspace member manifest` 和针对该 member manifest 的直接 rerun hint；如果 member 的默认 `.qi` 自身 `missing` / `invalid` / `unreadable` / `stale`，局部错误块现在也会先补 `failing package manifest` 和 `failing workspace member manifest`，再给修复 hint。最后统一汇总 failing members；只有多失败场景才额外补 `first failing member manifest`。
- `ql build --emit-interface` 会在成功 build 后写出当前 package 的默认 `.qi`；如果 build 已成功但 package 内其他源码导致接口发射失败，stderr 现在会先汇总所有 failing source file；只有多失败场景才额外补 `first failing source file`，随后再补 failing package manifest，并明确已经生成的 build artifact 仍保留在原输出路径。
- `.qi` 维护相关失败输出现在统一走规范化路径显示：source diagnostics、`first failing source file`、`first failing member/reference manifest`、stale reasons、owner/reference hints 都会去掉 `../` 噪音，且这些 `first failing *` 指针现在都只在多失败场景保留，避免同一条失败链路里出现多种路径写法和重复目标。
- `ql check` 现会在分析前显式拒绝本地依赖包的非 `valid` 默认 `.qi`（`missing` / `invalid` / `unreadable` / `stale`），并统一给出 `--sync-interfaces` / `ql project emit-interface` 修复提示；这些 dependency artifact 失败现在也会在局部错误块里补 `failing referenced package manifest`，再补 owner manifest + reference 文本上下文，不再只剩依赖包名和 artifact 路径；`invalid` / `unreadable` / `stale` 这几类 dependency `.qi` 失败块现在也和 package/workspace 一样固定为 `error -> detail/reason -> manifest/context -> hint` 顺序；单 package 若有多个 direct / transitive failing references，也会继续逐个报告并在末尾汇总 failing referenced package 数，多失败时再补一个 `first failing reference manifest` 指向第一处要修的 manifest。
- `ql check` / `ql check --sync-interfaces` 现在也会把坏的引用 manifest 纳入 package-aware 诊断面：会直接说明是引用 manifest 语法错误，还是引用目标没有 `[package].name`，并在局部错误块里补 `failing reference manifest` 与 owner/reference 修复提示。
- workspace 根路径上的 `ql check` 不再在首个 failing member 处停止，而会继续检查其余 members；每个失败 member 的错误块现在也会立即补一个 `failing workspace member manifest`，只有多失败场景的最终汇总才会再补 `first failing member manifest`。
- `ql check --sync-interfaces` 会在分析前递归同步本地依赖包的默认 `.qi`，避免把这些非 `valid` artifact 留到后续分析阶段才暴露；如果同一个 package 里同时存在可同步和不可修复的引用，已成功写出的 `.qi` 仍会输出，坏引用会继续逐个报告并在末尾汇总；当中间依赖自己的 `.qi` 缺失但更深层仍有坏引用时，当前可同步的上游 `.qi` 也会先写出，再汇总剩余 transitive failures；如果某个依赖在同步阶段因为自身源码错误或默认输出路径失败而无法发射 `.qi`，stderr 现在会先补 `failing package manifest`、输出路径/源码局部原因和 owner manifest / reference 上下文，再给统一的 `ql project emit-interface <manifest>` 修复提示，避免同一条失败链路里把 hint 提前到上下文前面；最终汇总只在多失败场景下补 `first failing reference manifest`。

### dependency-backed tooling

当前已经开放的 dependency-backed 合同：

- imported dependency symbol：hover、definition、declaration、references
- `use ...` 导入路径、grouped import 位置：dependency package path / public symbol completion
- dependency enum import roots：variant completion、variant hover/query、`textDocument/typeDefinition`
- dependency struct import roots：显式 struct literal / pattern field-label completion 与字段 query；direct dependency struct literal value roots 现也支持 hover / definition / declaration / references / `typeDefinition`
- 同构 inline tuple / array destructuring 产生的 dependency locals 现也支持 value-root hover / definition / declaration / references，并可继续进入 named-local member `typeDefinition`；同一条 destructuring slice 现在也覆盖 direct dependency iterable call，例如 `let (first, second) = config.children()` 与 `let [first, second] = config.children()`，这些 locals 也已进入 dependency member field / method query
- 语法局部可恢复 receiver 的 dependency member-field / member-method 最小 completion、query 与 `typeDefinition`
- direct indexed iterable receiver 现也开放同一最小 slice；question-unwrapped direct indexed receiver 也落在同一合同内，例如 `config.children[0].value`、`config.children()[0].get()`、`config.maybe_children()?[0].value`、`config.maybe_children()?[0].get()`，以及 grouped alias 形态 `kids()?[0].value`、`kids()?[0].get()`；同轴的 member `typeDefinition` 目标如 `config.maybe_children()?[0].leaf`、`config.maybe_children()?[0].leaf()`、`kids()?[0].leaf`、`kids()?[0].leaf()`、`(if flag { maybe_children()? } else { maybe_children()? })[0].leaf`、`(match flag { ... })[0].leaf()` 也已打通；direct structured question-indexed receiver 例如 `(if flag { maybe_children()? } else { maybe_children()? })[0].value` / `.get()` 与 `(match flag { ... })[0].value` / `.get()` 也已落进同一最小 member 合同；grouped alias 与 direct structured question-indexed bracket target 的 value-root 现都支持 hover / definition / declaration / references / `typeDefinition`，例如 `kids()?[0]`、`(if flag { maybe_children()? } else { maybe_children()? })[0]`、`(match flag { ... })[0]`
- broken-source fallback 下可恢复的 dependency import/type/value/member 查询

当前 receiver slice 仍是保守开放，不等于“任意 dependency member access 都已支持”。

## 当前回归基线

- build / codegen：`crates/ql-cli/tests/codegen.rs`
- string build：`crates/ql-cli/tests/string_codegen.rs`、`crates/ql-cli/tests/string_compare_codegen.rs`
- executable smoke：`crates/ql-cli/tests/executable_examples.rs`
- project / `.qi`：`crates/ql-cli/tests/project_graph.rs`、`crates/ql-cli/tests/project_interface.rs`、`crates/ql-cli/tests/project_check.rs`
- dependency-backed query / completion：`crates/ql-analysis/tests/*`、`crates/ql-lsp/tests/*`

## 当前明确未开放

- 真实 dependency build graph / publish workflow
- cross-file rename / workspace edits
- 超出当前 syntax-local slice 的广义 dependency member completion / query
- 更完整的 workspace-wide LSP 语义
- 完整 trait solver、generic monomorphization、effect system
- 设计稿中的默认参数、`data struct`、trait object、smart-cast 等后续能力

## 推荐阅读顺序

1. [开发计划](/roadmap/development-plan)
2. [P1-P8 阶段总览](/roadmap/phase-progress)
3. [Phase 7 并发、异步与 Rust 互操作](/plans/phase-7-concurrency-and-rust-interop)
4. [工具链设计](/architecture/toolchain)
5. [实现算法与分层边界](/architecture/implementation-algorithms)
