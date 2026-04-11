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
- `ql build <file> [--emit llvm-ir|obj|exe|dylib|staticlib] [--release] [-o <output>] [--emit-interface] [--header] [--header-surface exports|imports|both] [--header-output <output>]`
- `ql project graph [file-or-dir]`
- `ql project emit-interface [file-or-dir] [-o <output>] [--changed-only] [--check]`
- `ql ffi header <file> [--surface exports|imports|both] [-o <output>]`

当前稳定的 artifact 面：

- `llvm-ir`、`obj`、`exe`、`dylib`、`staticlib`
- `ql ffi header` 和 build-side header sidecar
- `ql runtime` 的 runtime requirement / hook ABI 输出

### project / workspace / `.qi`

- package directory、`qlang.toml`、包内源码路径、workspace-only 根 manifest 都可进入 package-aware `check` 流程。
- `ql project graph` 会展示 package/member、references，以及默认 `.qi` 的 `valid` / `missing` / `invalid` / `stale` 状态。
- `ql project emit-interface` 支持 package 和 workspace 批量写出；`-o/--output` 仍仅支持 package。
- `ql project emit-interface --changed-only` 只重发非 `valid` 接口。
- `ql project emit-interface --check` 只校验当前 package/workspace 的默认 `.qi` 是否都处于 `valid` 状态而不写文件。
- `ql build --emit-interface` 会在成功 build 后写出当前 package 的默认 `.qi`。
- `ql check --sync-interfaces` 会在分析前递归同步本地依赖包的默认 `.qi`，并把 `stale` 视为显式失败而不是隐式降级。

### dependency-backed tooling

当前已经开放的 dependency-backed 合同：

- imported dependency symbol：hover、definition、declaration、references
- `use ...` 导入路径、grouped import 位置：dependency package path / public symbol completion
- dependency enum import roots：variant completion、variant hover/query、`textDocument/typeDefinition`
- dependency struct import roots：显式 struct literal / pattern field-label completion 与字段 query
- 语法局部可恢复 receiver 的 dependency member-field / member-method 最小 completion、query 与 `typeDefinition`
- direct indexed iterable receiver 现也开放同一最小 slice；question-unwrapped direct indexed receiver 也落在同一合同内，例如 `config.children[0].value`、`config.children()[0].get()`、`config.maybe_children()?[0].value`、`config.maybe_children()?[0].get()`，以及 grouped alias 形态 `kids()?[0].value`、`kids()?[0].get()`；同轴的 member `typeDefinition` 目标如 `config.maybe_children()?[0].leaf`、`config.maybe_children()?[0].leaf()` 也已打通；bracket target 上的 value-root 现也支持 hover / definition / declaration / references / `typeDefinition`
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
