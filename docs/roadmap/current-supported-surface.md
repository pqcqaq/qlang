# 当前支持基线

> 最后同步：2026-04-19

这页只记录今天真实可依赖的能力边界。

## 真相源

- 实现：`crates/*`
- CLI / build / project 回归：`crates/ql-cli/tests/*`
- analysis / LSP 回归：`crates/ql-analysis/tests/*`、`crates/ql-lsp/tests/*`
- executable smoke：`ramdon_tests/executable_examples/`、`ramdon_tests/async_program_surface_examples/`

如果文档与实现冲突，以代码和回归测试为准。

## 已支持

### 编译器主路径

- lexer、parser、formatter、diagnostics、HIR、resolve、typeck、MIR、borrowck 已在主路径工作。
- LLVM 产物当前稳定开放：`llvm-ir`、`asm`、`obj`、`exe`、`dylib`、`staticlib`。
- 当前稳定互操作边界仍是 C ABI。

### CLI 与项目工作流

- 已实现：`ql check`、`ql fmt`、`ql mir`、`ql ownership`、`ql runtime`、`ql build`、`ql run`、`ql test`、`ql project`、`ql ffi`。
- `ql project init` 已能生成最小 package / workspace 脚手架，并附带 `src/lib.ql`、`src/main.ql`、`tests/smoke.ql`。
- `ql project targets`、`ql project graph`、`ql project lock`、`ql project emit-interface` 已落地。
- `qlang.toml` 当前只稳定支持：
  - `[package].name`
  - `[workspace].members`
  - `[references].packages`
  - `[dependencies]` 本地路径依赖
  - `[lib].path`、`[[bin]].path`
  - `[profile].default = "debug" | "release"`
- `qlang.lock` 第一版已落地，当前锁定本地 package graph、默认 profile 和 target 输入面。
- package/workspace 根目录已经可以直接进入 project-aware `ql build` / `ql run` / `ql test`。
- `ql build` / `ql run` 对 project 内已声明 target 的单个源码文件也已支持 project-aware 入口；无论直接执行 package 自身的 `src/main.ql`、`src/lib.ql`、`src/bin/*.ql`，还是执行 workspace member 下对应源码路径，都会保留 package / workspace profile、依赖构建和 project 输出目录语义。
- `ql test` 已支持对已发现测试使用 `--target` 做精确 rerun；直接执行 package `tests/` 下的单个 `.ql` 文件，或执行 workspace member 下对应测试文件时，也会保留 package/workspace-aware smoke / UI test 语义。
- `ql project graph` / `ql project targets` / `ql project lock` 直接指向 workspace member 源码文件时，会解析外层 workspace，而不是退化成单 package 视图。
- `ql project emit-interface` 在不带 `--output` 时，直接指向 workspace member `.ql` 文件也会解析外层 workspace，并按 workspace member 集合执行发射/检查。
- `ql check` / `ql build` / `ql run` / `ql test` 都已有第一版 `--json` 输出；其中 `ql run --json` 当前稳定导出 `ql.run.v1`，包含 built target、程序参数、捕获到的 stdout/stderr 和子进程退出码。更早的 selector / project preflight 失败仍保留既有 stderr failure surface。
- 当前真正打通的跨包执行路径仍然很窄：只稳定覆盖 direct local dependency 的 public `extern "c"` 符号。
- root target 的 dependency extern 预处理现在只会注入当前源码实际导入的直依赖 `extern "c"` 符号；未导入 sibling dependency 的同名符号不会再提前打断 `ql build/run/test`。

### async / runtime

- 已有最小 async 子集：`async fn`、`await`、`spawn`、`for await`、program-mode `async main`。
- 已有最小 `ql-runtime` 和 task-handle lowering。
- async library/program build 仍是保守子集，不应按“完整 runtime”理解。

### LSP 与 VSCode

- same-file 语义已经接通：hover、definition、declaration、typeDefinition、references、documentHighlight、completion、semanticTokens、documentSymbol、rename。
- `workspace/symbol` 已落地。
- healthy package/workspace 下，dependency-backed navigation 已能提供一批可依赖能力：
  - import root / dependency value / enum variant / struct field / method member 的 hover / definition / declaration / references / typeDefinition
  - source-preferred navigation：对 workspace members 和 workspace 外本地路径依赖，能唯一回溯到源码时优先跳源码而不是 `.qi`
  - package-aware semantic tokens
- source-preferred dependency navigation 现在按 manifest 身份区分同名本地依赖；definition / typeDefinition / references / `workspace/symbol` 不会再串到另一个依赖实例。
- `workspace/symbol` 对 workspace 外本地路径依赖在源码可用时会优先返回源码里的 value / method / trait / extend symbols；源码不可用时仍回退到 `.qi`。这条行为现在也覆盖 `workspace_roots` / 无打开文档入口；同名本地依赖也不会再因为 source-preferred 排除而误丢另一个依赖的 `.qi` 符号。
- broken-source / parse-error 下，当前只保留保守子集，不等于完整恢复；workspace 外本地路径依赖的 import references fallback、direct imported-result member hover / completion / query / `documentHighlight`（如 `build().ping()` / `build().value`）、dependency struct field label completion、dependency enum variant 的 `completion/definition/typeDefinition/references/documentHighlight`、dependency value/member semantic tokens fallback 都会继续走源码优先路径；同名本地依赖按 manifest 身份区分，不会串到兄弟依赖实例。
- current-document rename 在 parse-error 下也保留了一批保守合同；当前已锁住的窄 slice 包括 `config.child()?.leaf().value` 这类 question-unwrapped method-result member field，以及 dependency enum variant rename；同名本地依赖继续按 manifest 身份区分，不会串改兄弟依赖实例。
- rename 仍然只做 same-file；cross-file rename / workspace edits 尚未开放。

## 当前明确未支持

- 普通跨包 Qlang free function / member / const 的完整 dependency-aware backend
- registry / version solving / publish workflow
- cross-file rename / workspace edits
- 更宽的 project-scale references / refactor / code actions / inlay hints
- 超出当前保守 slice 的广义 parse-error member 语义
- 完整 trait solver、完整 monomorphization、更完整 effect system

## 建议阅读

1. [开发计划](/roadmap/development-plan)
2. [阶段总览](/roadmap/phase-progress)
3. [工具链设计](/architecture/toolchain)
4. [VSCode 插件](/getting-started/vscode-extension)
