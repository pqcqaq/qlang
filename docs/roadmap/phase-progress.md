# P1-P7 阶段总览

> 最后同步：2026-04-03

这页现在只保留阶段级结论，不再堆叠逐轮流水账。  
完整历史记录已归档到 [路线图归档](/roadmap/archive/index)。

## 总体结论

- P1 到 P6 已完成，且已经形成可持续扩展的工程主干。
- P7 正在进行，但不是“从零开始探索”，而是在既有主干上保守扩 async/runtime/library-build/program-build/Rust interop。
- 当前最重要的治理原则不是继续写长日志，而是保持三件事一致：
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

## 各阶段一句话总结

### P1 前端闭环

- 解决了“仓库能不能作为真实编译器工程开始演进”的问题。
- 当前 parser/formatter 已不再是主线风险点，后续前端变化应服务于语义和后端。

### P2 语义与查询地基

- HIR、resolve、typeck、diagnostics、analysis、最小 LSP 已接上同一条语义流水线。
- same-file 查询与 editor semantics 的主干是在这一阶段建立的。

### P3 MIR 与所有权地基

- MIR、ownership facts、cleanup-aware analysis、closure groundwork 都已成立。
- 当前所有权仍然是保守切片，不应误读成“完整 borrow/drop 系统已经完成”。

### P4 后端与产物

- `ql build` 已能真实产出 `llvm-ir`、`obj`、`exe`、`dylib`、`staticlib`。
- toolchain discovery、build orchestration、codegen golden tests 已形成稳定边界。

### P5 FFI 与 C ABI

- 最小 C ABI 已落地，header projection 与 sidecar header 也已进入真实工作流。
- 示例和回归已经覆盖 staticlib、dylib、C host、Rust host。

### P6 编辑器与语义一致性

- same-file LSP/query 的稳定边界已形成。
- 后续 editor work 默认应沿 analysis 共享真相源扩展，而不是单独做一套。

### P7 async / runtime / Rust interop

当前已形成的 P7 事实面：

- `Task[T]` 类型面已成立
- 最小 runtime hook ABI skeleton 已成立
- `staticlib` 与最小 async `dylib` 已开放受控 async library build 子集
- `BuildEmit::LlvmIr` / `Object` / `Executable` 已开放最小 `async fn main` program 子集
- fixed-shape `for await`、task-handle payload / projection consume、dynamic task-array 的保守成功路径、stable-dynamic path family、guard-refined dynamic path family、static-alias-backed dynamic reinit family、aliased projected-root repackage/spawn family、sync/async `unsafe fn` body executable surface、sync/async assignment expression executable surface、sync nested projected-root tuple assignment-expression executable surface、sync nested projected-root struct-field / fixed-array assignment-expression executable surface、sync call-root nested projected-root struct-field / fixed-array assignment-expression executable surface、sync dynamic non-`Task[...]` array assignment executable surface、sync projected-root dynamic non-`Task[...]` array assignment executable surface、sync dynamic assignment-expression executable surface、sync nested projected-root dynamic assignment-expression executable surface、async dynamic `Task[...]` array assignment executable surface、async projected-root dynamic `Task[...]` array assignment executable surface、async scalar dynamic non-`Task[...]` array assignment executable surface、async dynamic assignment-expression executable surface、async nested projected-root dynamic assignment-expression executable surface、async nested projected-root tuple assignment-expression executable surface、async nested projected-root struct-field / fixed-array assignment-expression executable surface、async call-root nested projected-root struct-field / fixed-array assignment-expression executable surface、awaited `match` guard families，以及 regular-size / spawned / zero-sized / recursive aggregate result family、regular-size / zero-sized helper task-handle flow family、regular-size task-handle payload family、regular-size / zero-sized task-handle family、regular-size / zero-sized projected reinit family、regular-size / zero-sized / recursive aggregate param family、regular-size conditional / bound / returned task-handle family，以及 zero-sized call-root/import-alias/inline/nested consume family 都已进入真实回归矩阵

## 当前进度与代码核对结果

本轮已按代码和测试重新核对当前文档入口，结论如下：

- `ramdon_tests/executable_examples/` 当前真实是 `51` 个 sync executable 样例
- `ramdon_tests/async_program_surface_examples/` 当前真实是 `211` 个 async executable 样例
- `crates/ql-cli/tests/executable_examples.rs` 与目录数量一致
- async 目录现在最新文件编号是 `214`，但真实样例数是 `211`；不要再把文件编号误当成文件数

## 当前最值得继续推进的方向

接下来仍建议按这个顺序推进：

1. 沿当前 async executable / library 已开放子集继续扩真实用户可写 surface
2. 保持 task-handle / dynamic path / `for await` / awaited `match` 四条线共享同一份 truth source
3. 每次扩面都优先补真实样例和 family fixture，而不是先写大段说明文档
4. 更广 async ABI、cleanup lowering、generalized iterable、broader ownership precision 继续延后

## 历史记录去哪里看

如果需要追溯详细流水账、每轮提交到底补了哪些 case、当时的保守约束是什么，请看：

- [路线图归档](/roadmap/archive/index)
- [Phase 7 合并设计稿](/plans/phase-7-concurrency-and-rust-interop)
- [原始 plans 归档](/plans/archive/index)
