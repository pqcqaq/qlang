# 当前支持基线

> 最后同步：2026-05-03

这页只记录当前真正可依赖的边界。

## 真相源

- 实现：`crates/*`
- CLI / project 回归：`crates/ql-cli/tests/*`
- analysis / LSP 回归：`crates/ql-analysis/tests/*`、`crates/ql-lsp/src/backend/tests.rs`、`crates/ql-lsp/tests/*`
- executable smoke：`ramdon_tests/executable_examples/`、`ramdon_tests/async_program_surface_examples/`

如果文档与实现冲突，以代码和回归测试为准。

## 已支持

### 编译器与运行

- 主链路 `lexer -> parser -> HIR -> resolve -> typeck -> MIR -> LLVM` 已可用。
- 现有 LLVM 输出覆盖 `llvm-ir`、`asm`、`obj`、`exe`、`dylib`、`staticlib`。
- 已有最小 async/runtime 子集，但仍不是完整 runtime。
- `if` 作为表达式使用时要求分支类型合流；作为语句使用时只检查分支内部语义，不统一被丢弃的 tail 值。

### CLI 与项目工作流

- `ql check`、`ql fmt`、`ql mir`、`ql ownership`、`ql runtime`、`ql build`、`ql run`、`ql test`、`ql project`、`ql ffi` 已实现。
- `ql project init/add/remove/status/dependencies/dependents/targets/graph/lock/emit-interface` 已能支撑 workspace 维护。
- `ql build` / `ql run` / `ql test` / `ql check` 已支持 project-aware 入口，`--json`、`--list`、`--package`、`--target` 等常用工作流已经落地。
- 单文件 `ql build file.ql` / `ql run file.ql` / `ql test file.ql` 已复用本地 generic free function direct-call specialization，可覆盖简单同文件泛型函数调用。
- `ql project init --stdlib` 可以直接生成依赖 `std.core` / `std.option` / `std.result` / `std.array` / `std.test` 的项目脚手架。

### 依赖桥接与 stdlib

- 当前跨包执行仍是保守切片：bridgeable `const/static`、受限顶层 free function、`extern "c"`、部分 type/value bridge、受限 receiver method。
- public/local generic free function 已支持 direct-call 多实例 specialization，前提是类型参数能从字面量、简单表达式、tuple / fixed-array literal、projection、显式 typed value、carrier value、单字段 enum variant constructor 或显式结果上下文推断。
- 数组长度泛型参数可在函数体内作为 `Int` 值读取，例如 `fn len[T, N](values: [T; N]) -> Int { return N }`。
- dependency generic specialization 会递归处理同依赖模块内的 generic helper 直调，允许兼容 wrapper 转调 canonical generic API。
- `std.option.Option[T]`、`std.result.Result[T, E]`、`std.array` 的 canonical length-generic helpers（含 `len_array`）、`std.test` 的普通断言和 length-generic 数组断言 helpers 已进入真实 smoke；数组固定长度 helper 只保留为兼容层。

### LSP 与 VSCode

- same-file 语义已接通：hover、definition、declaration、typeDefinition、references、documentHighlight、completion、semantic tokens、formatting、codeAction、codeLens、rename。
- `workspace/symbol`、`implementation`、open-doc 优先的 dependency 导航、基础 workspace rename 已覆盖当前支持切片。
- `textDocument/formatting`、`rangeFormatting`、`onTypeFormatting` 已可直接复用 `ql fmt`。

## 当前明确未支持

- 普通跨包 Qlang free function / member / const 的完整 dependency-aware backend
- 完整 generic monomorphization、泛型 alias lowering、自动 prelude
- registry / version solving / publish workflow
- 预编译 release 和 VSCode Marketplace 分发
- 更广义的 cross-file rename / workspace edits
- 更宽的 project-scale references / refactor / workspace-wide code actions
- 完整 trait solver、完整 effect system、更宽的 async/runtime 语言面

## 建议阅读

1. [开发计划](/roadmap/development-plan)
2. [阶段总览](/roadmap/phase-progress)
3. [工具链设计](/architecture/toolchain)
