# P1-P8 阶段总览

> 最后同步：2026-04-06

这页只保留阶段级结论、当前已核对的数量和继续推进的方向。
逐轮切片记录、旧版长文和本轮入口整理记录已归档到 [路线图归档](/roadmap/archive/index)。

## 总体结论

- P1 到 P6 已完成，并且已经形成可持续扩展的工程主干。
- P7 正在进行，但主线不是“重新设计 async”，而是在既有 compiler/runtime/build 真相源上保守扩面。
- P8 已进入前三条工程入口切片：最小 `qlang.toml` manifest graph、`ql project graph`、`.qi` V1 emit、package-aware dependency `.qi` syntax-load、dependency public symbol index，以及 imported dependency symbol 的最小 cross-file hover / definition / references 与 `use ...` 导入路径和平铺 / grouped import 位置里的 package path segment / public symbol completion 已落地；更完整的 cross-file editor semantics 仍未开始实现。
- 当前最重要的治理要求仍然是三者一致：
  - 代码里的真实实现
  - 测试里的真实合同
  - 文档里的当前结论

## 阶段状态

| 阶段 | 状态 | 已形成的稳定边界 |
| --- | --- | --- |
| P1 | 已完成 | Rust workspace、lexer、parser、formatter、CLI 前端闭环 |
| P2 | 已完成 | HIR、resolve、first-pass typeck、统一 diagnostics、最小 query / LSP |
| P3 | 已完成 | 结构化 MIR、ownership facts、cleanup-aware 分析、closure groundwork |
| P4 | 已完成 | `ql build`、LLVM IR、`obj` / `exe` / `dylib` / `staticlib`、driver/codegen 边界 |
| P5 | 已完成 | 最小 C ABI 闭环、header projection、C/Rust host examples、FFI 集成回归 |
| P6 | 已完成 | same-file hover / definition / references / rename / completion / semantic tokens / LSP parity |
| P7 | 进行中 | 受控 async/runtime/task-handle lowering、library/program build 子集、Rust interop 扩展 |
| P8 | 启动中 | 最小 `qlang.toml` manifest graph、`ql project graph`、`.qi` V1 emit、package-aware dependency `.qi` load、后续 cross-file LSP 入口 |

## 各阶段一句话总结

### P1 前端闭环

- 解决了“仓库能不能作为真实编译器工程演进”的问题。

### P2 语义与查询地基

- HIR、resolve、typeck、diagnostics、analysis 与最小 LSP 已接到同一条语义流水线。

### P3 MIR 与所有权地基

- MIR、ownership facts、cleanup-aware analysis 与 closure groundwork 已成立，但不应误读为完整 borrow/drop 系统已经完成。

### P4 后端与产物

- `ql build` 已能真实产出 `llvm-ir`、`obj`、`exe`、`dylib`、`staticlib`，toolchain discovery 和 codegen golden test 边界已稳定。

### P5 FFI 与 C ABI

- 最小 C ABI、header projection、C/Rust host 示例和集成回归都已经进入真实工程工作流。

### P6 编辑器与语义一致性

- same-file LSP/query 的共享 truth surface 已形成，后续 editor work 默认应继续复用 analysis。

### P7 async / runtime / Rust interop

当前已形成的 P7 事实面可以压缩成下面几条：

- `Task[T]`、最小 runtime hook ABI skeleton、`ql runtime` truth surface 已成立。
- async library build 已开放 `staticlib` 和最小 async `dylib` 子集；稳定外部边界仍是同步 `extern "c"` C ABI。
- async executable build 已开放 `BuildEmit::LlvmIr` / `Object` / `Executable` 下的最小 `async fn main` 子集。
- 当前受控子集已经覆盖主要 `await` / `spawn` payload family、projected task-handle consume/reinit、stable-dynamic 与 guard-refined dynamic path、fixed-shape `for await`、awaited `match` guard，以及 sync/async assignment-expression executable surface。
- sync ordinary executable surface 已覆盖 fixed-shape `for`、assignment-expression、dynamic non-`Task[...]` array assignment、same-file foldable `const` / `static` item value materialization，以及当前受控 `match` guard family。
- 更广义的 async ABI、cleanup codegen、generalized iterable、broader `dylib` / program bootstrap，以及更广 projection-sensitive ownership precision 仍然刻意关闭。

### P8 项目级工具链与 cross-file editor 入口

- `ql-project` 现已提供最小 `qlang.toml` manifest loader。
- 当前真实 contract 已锁定在 `[package].name`、`[workspace].members`、`[references].packages`。
- `ql project graph [file-or-dir]` 已可向上发现 manifest 并输出当前 package/workspace/reference graph。
- `ql project emit-interface [file-or-dir] [-o <output>]` 已可对 package manifest 的 `src/**/*.ql` 做逐文件分析，并输出 text-based `.qi` public interface artifact。
- `ql-analysis::analyze_package` 与 package-aware `ql check <package-dir>` 现已开始加载 `[references].packages` 指向的 dependency `.qi` artifact，并在 interface 缺失时显式失败。
- 当前 dependency `.qi` load 已推进到 syntax-aware section parse：每个 `// source: ...` module section 都会进入 interface-mode AST，支持 bodyless `fn` / `impl` / `extend` 声明以及无值 `const` / `static` 接口声明；`ql-analysis::analyze_package` 现也已把公开 dependency symbols 收进 package 级 truth surface，并接通 imported dependency symbol 的 cross-file hover / definition / references 到 `.qi` declaration；这条查询链当前也已显式覆盖 grouped import alias 形态。与此同时，`use ...` 导入路径和平铺 / grouped import 位置里的 dependency package path segment / public symbol completion 也已打通，且 grouped import 的空补全位会过滤已写过的 dependency item，减少重复提示；rename、更广义 completion 与真实 dependency build graph 仍未开放。
- dependency import completion 现在还带有最小编辑期容错：当当前文档自身暂时分析失败时，LSP 仍会走 dependency-only package load 回退，继续提供 `use ...` 导入路径上的 dependency path segment / public symbol completion；该回退当前不扩大到 hover / definition / references 或其它同文件补全。
- dependency enum import alias root 的首个 non-import-path completion 也已落地：当 `use demo.dep.Command as Cmd` 这类 alias 能唯一映射到 dependency public enum 时，`Cmd.Re` 现可通过 `.qi` public surface 继续补全到 `Retry` 等 variants；struct field / broader member completion 仍未开放。

## 当前进度对账

本轮已按代码和测试重新核对当前入口文档，结果如下：

- `ramdon_tests/executable_examples/` 当前真实有 `60` 个 sync executable 样例。
- `ramdon_tests/async_program_surface_examples/` 当前真实有 `222` 个 async executable 样例。
- `crates/ql-cli/tests/executable_examples.rs` 的注册数量与目录数量一致。
- async 目录当前最新编号是 `225`，但真实样例数是 `222`；入口文档已经统一按真实文件数描述。

## 当前最值得继续推进的方向

1. 继续沿已开放的 async executable / library 子集扩真实用户可写 surface，而不是另开 ABI 或 runtime 设计。
2. 继续让 task-handle、dynamic path、`for await`、awaited `match` 这几条线共享同一份 truth source。
3. 按已固定顺序继续推进 Phase 8：dependency `.qi` 的 syntax-aware load、public symbol index、hover / definition / references，以及 `use ...` 导入路径和平铺 / grouped import 位置里的 path segment / symbol completion 已落地；下一步应继续扩真正需要的 cross-file completion/identity contract，而不是直接跳到 cross-file rename。

## 归档入口

如果你需要追溯更细的历史记录，请看：

- [路线图归档](/roadmap/archive/index)
- [Phase 7 合并设计稿](/plans/phase-7-concurrency-and-rust-interop)
- [原始 plans 归档](/plans/archive/index)
