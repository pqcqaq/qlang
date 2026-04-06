# P1-P8 阶段总览

> 最后同步：2026-04-07

这页只保留阶段级结论、当前已核对的数量和继续推进的方向。
逐轮切片记录、旧版长文和本轮入口整理记录已归档到 [路线图归档](/roadmap/archive/index)。

## 总体结论

- P1 到 P6 已完成，并且已经形成可持续扩展的工程主干。
- P7 正在进行，但主线不是“重新设计 async”，而是在既有 compiler/runtime/build 真相源上保守扩面。
- P8 已进入前三条工程入口切片：最小 `qlang.toml` manifest graph、`ql project graph`、`.qi` V1 emit、`ql build --emit-interface`、`ql check --sync-interfaces`、package-aware dependency `.qi` syntax-load、dependency public symbol index，以及 imported dependency symbol 的最小 cross-file hover / definition / declaration / references、`use ...` 导入路径和平铺 / grouped import 位置里的 package path segment / public symbol completion、dependency enum variant completion、dependency struct explicit field-label completion、dependency struct member-field / member-method completion、显式 type use 上的最小 `textDocument/typeDefinition`，以及 dependency struct field token 到 dependency public field type、dependency method token 到 dependency public return type 的最小 `textDocument/typeDefinition` 子集；这条 receiver truth surface 现已覆盖 dependency method-call result local、block tail result、结果稳定收敛到同一个 dependency struct 的 `if` / `match` 表达式结果，以及不经命名 local 的 direct dependency method-call / structured receiver。broken-source / semantic-error 场景下的 import-root / variant / struct-field / struct-member-field token / struct-member-method token 最小 hover / definition / declaration / references fallback 已落地，对应的 dependency import/type-root / dependency value root / dependency struct field token / dependency method token `typeDefinition` fallback 也都已接通；`ql project emit-interface` 现在也已支持从 workspace-only 根 manifest 批量写出成员包默认 `.qi`，`ql project graph` 现也已在 package/workspace 两条路径上展示默认 `.qi` 的路径/状态与引用 interface 状态，而 package-aware `ql check` 与 `ql check --sync-interfaces` 的入口现都已覆盖 package dir / manifest / package source file / workspace-only root manifest；更完整的 cross-file editor semantics 仍未开始实现。
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
| P6 | 已完成 | same-file hover / definition / references / rename / completion / semantic tokens / document symbols / package-rooted conservative workspace symbols / LSP parity |
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

- same-file LSP/query 的共享 truth surface 已形成，hover / definition / references / rename / completion / semantic tokens 之外，当前同一份真相源也已接出 document symbol outline 与保守的 package-rooted `workspace/symbol` 搜索；该搜索现会从已打开 package 扩到同包源码 modules，并带上已加载 dependency `.qi` public symbols；后续 editor work 默认应继续复用 analysis。

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
- `ql project graph [file-or-dir]` 已可向上发现 manifest 并输出当前 package/workspace/reference graph；package manifest 当前会额外显示默认 `.qi` 路径/状态与引用 interface 状态，workspace-only 根 manifest 则会进一步展开 member package 的 manifest、包名、默认 `.qi` 路径/状态、references 与引用 interface 状态。
- `ql project emit-interface [file-or-dir] [-o <output>]` 已可对 package manifest 的 `src/**/*.ql` 做逐文件分析，并输出 text-based `.qi` public interface artifact。
- `ql build <file> --emit-interface` 现也可在构建成功后顺手写出当前包默认 `<package>.qi`，让声明文件生成不再只挂在单独的 project 子命令上。
- `ql check --sync-interfaces` 现会在 package-aware check 前递归同步写出本地引用包默认 `<package>.qi`，减少依赖接口需要手工预生成的摩擦；当前该开关已支持 package directory、`qlang.toml` 路径、包内源码文件路径，以及 workspace-only 根 manifest，并会对重复 dependency 写出做去重。
- `ql-analysis::analyze_package` 与 package-aware `ql check` 现已开始加载 `[references].packages` 指向的 dependency `.qi` artifact，并在 interface 缺失时显式失败；当前 package-aware `ql check` 入口已覆盖 package directory、`qlang.toml`、包内源码文件路径，以及 workspace-only 根 manifest，后者会顺序检查每个 workspace member package。
- 当前 dependency `.qi` load 已推进到 syntax-aware section parse：每个 `// source: ...` module section 都会进入 interface-mode AST，支持 bodyless `fn` / `impl` / `extend` 声明以及无值 `const` / `static` 接口声明；`ql-analysis::analyze_package` 现也已把公开 dependency symbols 收进 package 级 truth surface，并接通 imported dependency symbol 的 cross-file hover / definition / declaration / references 到 `.qi` declaration；这条查询链当前也已显式覆盖 grouped import alias 形态。与此同时，`use ...` 导入路径和平铺 / grouped import 位置里的 dependency package path segment / public symbol completion 也已打通，且 grouped import 的空补全位会过滤已写过的 dependency item，减少重复提示；rename、更广义 completion 与真实 dependency build graph 仍未开放。
- dependency-only 回退现在还带有最小编辑期容错：当当前文档自身暂时分析失败时，真实 `ql-lsp` backend 仍会走 dependency-only package load 回退，继续提供 `use ...` 导入路径上的 dependency path segment / public symbol completion、dependency enum import alias root 上的 variant completion，以及 dependency struct explicit field-label completion；同一条回退链现在也已开放 imported dependency local name 的最小 hover / definition / declaration / references 查询，覆盖 import binding token 本身，以及 parse-only 可恢复的 single-segment use site/type root；dependency enum variant token、struct-field token（含 explicit / shorthand）与 syntax-local 可恢复 receiver 上的首个 dependency struct member-field token / member-method token 现在也已在同一路径下补齐最小 hover / definition / declaration / references 回退，而同一 receiver slice 上的 dependency field / method completion 也已继续可用；该路径当前仍不扩大到 broader member query、任意表达式 receiver 或其它同文件补全。
- dependency enum import alias root 的首个 non-import-path query contract 也已落地：当 `use demo.dep.Command as Cmd` 这类 alias 能唯一映射到 dependency public enum 时，`Cmd.Re` 现可通过 `.qi` public surface 继续补全到 `Retry` 等 variants，而 `Cmd.Retry` 这类 variant token 也已接通 dependency hover / definition / declaration / references；与此同时，当当前文档仅有 same-file 语义错误时，真实 `ql-lsp` backend 现在也会继续从该 variant token 提供 dependency hover / definition / declaration / references 回退；更广义 dependency member completion 仍未开放。
- dependency struct import alias root 的首个 field contract 也已继续扩到 local value member path：当 `use demo.dep.Config as Cfg` 这类 alias 能唯一映射到 dependency public struct 时，`Cfg { fl: true }` / `let Cfg { fl: enabled } = built` 这类显式 struct literal / struct pattern 字段标签现在也已接通 dependency public field completion，并会跳过同一字面量/模式里已经写过的 sibling 字段；struct-field token 查询当前已覆盖 explicit 与 shorthand 两种写法，并继续支持 dependency hover / definition / declaration / references；与此同时，对能从语法局部恢复为 dependency struct 的 named local value，`config.value` / `built.value` / `current.value` 这类 member field token 现在也已接通同一条 dependency hover / definition / declaration / references，而 `config.va` / `built.va` / `current.va` 这类 syntax-local member prefix 也已接通 dependency field completion；当前 receiver slice 也已扩到 `let current = config.child()` 这类 dependency method-call result local，以及 `let current = if flag { config.child() } else { config.child() }`、`let current = match flag { ... }`、`let current = { config.child() }` 这类 structured result local；更广义 member completion 与任意 receiver 仍未开放。
- dependency struct local-value member path 的首个 method contract 也已落地并补齐到 completion 与 broken-source fallback：当 `use demo.dep.Config as Cfg` 这类 alias 能唯一映射到 dependency public struct，并且 `.qi` 中该 nominal struct 的 public method 仍可唯一解析（当前继续遵守 impl 优先、再看 extend 的保守规则）时，`config.get()` / `built.get()` / `current.get()` 这类 member method token 现在已在成功分析路径与 same-file semantic-error fallback 上接通 dependency hover / definition / declaration / references，而 `config.ge` / `built.ge` / `current.ge` 这类 syntax-local member prefix 也已接通 dependency method completion；当前 receiver slice 也已扩到 dependency method-call result local、block tail result，以及结果稳定收敛到同一个 dependency struct 的 `if` / `match` structured result local；这条能力当前仍只覆盖语法局部可恢复出的 named local receiver，不扩展到任意表达式 receiver。
- 显式 type-namespace 位置上的最小 `textDocument/typeDefinition` 现也已接到同一份 analysis identity truth surface，并开始小步扩到 value-position 的受控子集：same-file type use 当前覆盖 struct / enum / trait / type alias / generic 定义，same-file local import type alias use 会优先跳到底层本地类型定义，而 package-aware 成功分析路径下的 dependency import/type root（例如 `Buf[Int]`）也已可跳到 dependency `.qi` artifact 内的 public type declaration；与此同时，对能从语法局部恢复出 dependency struct 类型的 named local value，value token 本身现在也可跳到 dependency public struct declaration，而 dependency struct field token 本身现在也可在字段声明类型唯一映射到 dependency public type 时跳到对应 `.qi` public type declaration，dependency method token 本身现在也可在返回类型唯一映射到 dependency public type 时跳到对应 `.qi` public type declaration；当当前文档存在 same-file semantic errors、但语法仍可恢复且 dependency package 仍可加载时，这几条 dependency typeDefinition 都已补上 broken-source fallback；当前仍不做更广义值位推断 type jump，也不扩到任意表达式 receiver 或更广义 non-type-context fallback。

## 当前进度对账

本轮已按代码和测试重新核对当前入口文档，结果如下：

- `ramdon_tests/executable_examples/` 当前真实有 `60` 个 sync executable 样例。
- `ramdon_tests/async_program_surface_examples/` 当前真实有 `222` 个 async executable 样例。
- `crates/ql-cli/tests/executable_examples.rs` 的注册数量与目录数量一致。
- async 目录当前最新编号是 `225`，但真实样例数是 `222`；入口文档已经统一按真实文件数描述。

## 当前最值得继续推进的方向

1. 继续沿已开放的 async executable / library 子集扩真实用户可写 surface，而不是另开 ABI 或 runtime 设计。
2. 继续让 task-handle、dynamic path、`for await`、awaited `match` 这几条线共享同一份 truth source。
3. 按已固定顺序继续推进 Phase 8：dependency `.qi` 的 syntax-aware load、public symbol index、hover / definition / declaration / references，以及 `use ...` 导入路径 completion、import-root/enum-variant/struct-field-token 的最小 broken-source query fallback 已落地；下一步应继续扩真正需要的 cross-file completion/identity contract，而不是直接跳到 cross-file rename。

## 归档入口

如果你需要追溯更细的历史记录，请看：

- [路线图归档](/roadmap/archive/index)
- [Phase 7 合并设计稿](/plans/phase-7-concurrency-and-rust-interop)
- [原始 plans 归档](/plans/archive/index)
