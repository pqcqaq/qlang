# 当前支持基线

> 最后同步：2026-04-04

这页只保留“今天真实可依赖的能力边界”。  
详细切片过程、逐轮回归记录与旧版长文已归档到 [路线图归档](/roadmap/archive/index)。

## 真相源

当前基线以这几类文件为准：

- 实现：`crates/*`
- executable 真运行矩阵：`crates/ql-cli/tests/executable_examples.rs`
- library build / codegen pass 矩阵：`crates/ql-cli/tests/codegen.rs`
- sync 样例：`ramdon_tests/executable_examples/`
- async 样例：`ramdon_tests/async_program_surface_examples/`

如果文档和这些文件不一致，以代码与回归矩阵为准，再回头修正文档。

## 一页结论

- Phase 1 到 Phase 6 地基已经落地：lexer、parser、formatter、diagnostics、HIR、resolve、typeck、MIR、borrowck、LLVM backend、driver、CLI、same-file LSP/query、FFI header projection 都已进入真实工程主干。
- 当前活跃主线是保守推进的 Phase 7：async/runtime/task-handle lowering、library/program build surface、Rust interop。
- 外部稳定互操作边界仍是 C ABI；Rust 继续走 `build.rs + staticlib + header` 路线。
- async 已经不是“只有语法”，而是有真实 build、真实样例和真实回归的受控子集；但 broader async ABI、broader runtime semantics 仍然刻意关闭。

## 当前已开放的构建表面

### CLI 与产物

- `ql check`
- `ql build --emit llvm-ir|obj|exe|dylib|staticlib`
- `ql ffi header`
- `ql fmt`
- `ql mir`
- `ql ownership`
- `ql runtime`

### sync build 子集

当前 sync build surface 已稳定覆盖：

- 顶层 free function
- `unsafe fn` body
- `extern "c"` 顶层声明、extern block、顶层导出定义
- `main` 程序入口
- 标量整数 / `Bool` / `Void`
- direct call 与 named arguments
- same-file `use ... as ...` function alias call
- fixed-shape `for`
  - fixed-array
  - homogeneous tuple
  - projected root / call-root / nested call-root / inline projected root
  - same-file `const` / `static` root 及其 same-file alias
- 赋值表达式的当前可运行子集
  - mutable local
  - tuple literal index projection，以及 same-file `const` / `static` / `use ... as ...` alias、branch-selected const `if` / 最小 literal `match` item value、direct inline foldable `if` / `match` integer expression，和 immutable direct local alias 复用驱动的 foldable integer constant expression tuple index
  - struct field
  - fixed-array literal index projection
  - projected-root / nested projected-root / call-root nested projected-root / import-alias call-root nested projected-root / inline nested projected-root tuple / struct-field / fixed-array literal-index chains
- 动态数组索引赋值的当前可运行子集
  - non-`Task[...]` element arrays
  - nested dynamic array projections
  - projected-root dynamic array projections
  - direct-root / projected-root / call-root nested projected-root / import-alias call-root nested projected-root / inline nested projected-root assignment-expression result form
  - nested projected-root assignment-expression result form
- 普通表达式与 `if` / `while` 条件里的 same-file foldable `const` / `static`，包括 computed/projected item value，以及 foldable const `if` / 最小 literal `match` 选出的 branch-selected item value
- `Bool` `&&` / `||` / unary `!`
- 最小 literal `match` lowering
  - `Bool` / `Int` literal-path 子集
  - 其他 current-loadable scrutinee 的 catch-all-only 子集
  - 当前 bool/scalar-comparison guard 子集
  - direct resolved sync guard call 子集
  - inline aggregate guard-call arg / inline projection-root 子集
  - call-root / nested call-root guard 子集

### async library build 子集

当前 async library build 已稳定开放：

- `staticlib`
- 最小 async `dylib`
  - 仍要求公开导出面保持同步 `extern "c"` C ABI

当前 library-mode async 子集已有真实 pass matrix 覆盖：

- scalar / tuple / array / struct / nested aggregate `await`
- `Task[T]` flow、payload、projection consume / submit
- projected reinit、stable-dynamic path、guard-refined path
- fixed-shape `for await`
  - fixed-array
  - homogeneous tuple
  - task-array / task-tuple auto-await
  - projected / call-root / awaited-aggregate / import-alias / inline / nested call-root
- 普通标量赋值表达式的当前可运行子集
  - mutable local
  - tuple literal index projection，以及 same-file `const` / `static` / `use ... as ...` alias、branch-selected const `if` / 最小 literal `match` item value、direct inline foldable `if` / `match` integer expression，和 immutable direct local alias 复用驱动的 foldable integer constant expression tuple index
  - struct field
  - fixed-array literal index projection
  - projected-root / nested projected-root / call-root nested projected-root / import-alias call-root nested projected-root / inline nested projected-root tuple / struct-field / fixed-array literal-index chains
- 动态 `Task[...]` 数组索引赋值的当前可运行子集
  - generic direct-root write-before-consume success path
  - generic projected-root write-before-consume success path
- async 普通标量动态数组索引赋值的当前可运行子集
  - direct-root non-`Task[...]` arrays
  - projected-root non-`Task[...]` arrays
  - direct-root / projected-root / call-root nested projected-root / import-alias call-root nested projected-root / inline nested projected-root assignment-expression result form
  - nested projected-root assignment-expression result form
- 最小 async `match` family
  - direct-call guard
  - projection guard
  - aggregate guard-call arg
  - inline aggregate / inline projection
  - nested call-root families

### async executable build 子集

当前 async executable 只开放：

- `BuildEmit::LlvmIr`
- `BuildEmit::Object`
- `BuildEmit::Executable`
- 程序入口限定为最小 `async fn main`
- `async unsafe fn` body 会沿当前最小 `async fn main` 子集一起 lowering

当前 program-mode async 子集真实覆盖：

- `Task[T]` 类型面
- direct async call / helper-returned task handle / `spawn` / `await`
  - regular-size helper-returned / forwarded task-handle flow
  - bound local task-handle `spawn`
  - regular-size aggregate params on direct `await` / `spawn`
  - zero-sized helper-returned / forwarded task-handle flow
  - zero-sized aggregate params on direct `await` / `spawn`
  - recursive aggregate params on direct `await` / `spawn`
- scalar 与 fixed-shape aggregate payload
  - tuple / fixed-array / non-generic struct
  - regular-size direct / spawn aggregate result family
  - zero-sized aggregate
  - direct / spawn recursive fixed-shape aggregate result
  - aggregate 内继续携带 `Task[T]`
  - regular-size tuple / array / nested aggregate task-handle payload family
  - nested task-handle payload
  - regular-size returned / nested / struct-carried task-handle shapes
  - zero-sized returned / nested / struct-carried task-handle shapes
- projected task-handle consume
  - tuple index
  - fixed-array literal index
  - struct field
  - regular-size fixed-array projected reinit / conditional reinit
  - zero-sized tuple / fixed-array / struct projection `await` / `spawn`
  - zero-sized tuple / fixed-array / struct projection reinit
  - direct call-root / awaited-aggregate / import-alias / inline / nested-call-root zero-sized consume
  - direct call-root / nested call-root / awaited-aggregate / inline aggregate
- conditional task-handle control flow
  - regular-size branch-local `spawn` + reinit
  - regular-size conditional async-call `spawn`
  - regular-size conditional helper-task `spawn`
  - guard-refined arithmetic static alias-sourced composed-dynamic forwarded helper `await` / direct queued `spawn`
  - zero-sized branch-local `spawn` + reinit
  - zero-sized conditional async-call `spawn`
  - zero-sized conditional helper-task `spawn`
- aliased projected-root aggregate repackage / submit
  - tuple / struct / nested aggregate repackage before `await`
  - fixed-array / nested fixed-array / helper-forwarded nested fixed-array repackage before `spawn`
  - source-root reinit 后的 same-file arithmetic item / same-file `use ... as ...` alias 驱动 direct alias-root、projected-root、alias-sourced composed-dynamic 与 guard-refined alias-sourced composed-dynamic 形态，包括 bundle-alias-forwarded、queued-root-forwarded、queued-root-inline-forwarded、queued-root-alias-forwarded、queued-root-alias-inline-forwarded、queued-root-chain-forwarded、queued-local-forwarded、queued-local-inline-forwarded 与 bundle-chain-forwarded await/spawn
- dynamic fixed-array `Task[...]` 的保守子集
  - generic dynamic sibling-safe consume / spawn
  - same immutable stable source path precise consume / reinit
  - projected-root stable dynamic reinit / conditional reinit
  - aliased / const-backed alias-root stable-dynamic reinit
  - composed / alias-sourced composed dynamic reinit
  - foldable integer arithmetic expression 回收到 concrete literal/projection path
  - direct inline foldable `if` / 最小 literal `match` integer expression 回收到 concrete literal path consume / reinit
  - direct / projected / aliased guard-refined dynamic reinit，包括 arithmetic-backed refined source 及其 same-file `use ... as ...` alias 包裹形态
  - same-file static/import-alias-backed projected-root dynamic reinit
  - same-file `const` / `static` / `use ... as ...` alias 回收到 literal/projection path，包括 computed/projected item value、branch-selected const `if` / 最小 literal `match` item value、foldable arithmetic item value，以及这些 item value 的 same-file `use ... as ...` alias 包裹形态
  - equality-guard refinement
  - projected-root / alias-root canonicalization
- fixed-shape `for await`
  - fixed-array
  - homogeneous tuple
  - task-array / task-tuple auto-await
  - projected / call-root / awaited-aggregate / import-alias / inline / nested call-root
- 普通表达式与 `if` / `while` 条件里的 same-file foldable `const` / `static`
  - 包括 computed/projected item value
  - 包括 foldable const `if` / 最小 literal `match` 选出的 branch-selected item value
- awaited `match` guard 子集
  - awaited scalar + direct-call guard
  - awaited aggregate + projection guard
  - aggregate guard-call arg / call-backed aggregate arg
  - import-alias helper family
  - inline aggregate arg / inline projection-root
  - nested call-root runtime projection family
  - nested call-root deeper inline-combo family

## 当前回归规模

截至当前代码：

- sync executable examples：`60`
- async executable examples：`222`

注意：

- async 目录文件编号从 `04` 编到 `225`，但真实 `.ql` 文件数是 `222`，不是 `225`
- `crates/ql-cli/tests/executable_examples.rs` 当前也只注册了 `222` 个 async executable case 和 `60` 个 sync executable case

## 当前明确未开放

- 更广义的 async executable / program bootstrap，除最小 `async fn main` 以外
- 更广义的 async `dylib` surface，尤其是公开 async ABI
- generalized `for await`，超出 fixed-array / homogeneous tuple 之外的 iterable
- cleanup lowering / cleanup codegen
- cancellation / polling / drop semantics
- generic async ABI / layout substitution
- arbitrary dynamic overlap precision
- 更广义的 projection-sensitive partial move / partial-place ownership
- 超出当前 minimal subset 的 `match` lowering、guard shape 与 pattern discrimination

## 推荐阅读顺序

如果你要继续开发，建议按这个顺序恢复上下文：

1. [开发计划](/roadmap/development-plan)
2. [P1-P7 阶段总览](/roadmap/phase-progress)
3. [Phase 7 设计合并稿](/plans/phase-7-concurrency-and-rust-interop)
4. [工具链设计](/architecture/toolchain)
5. [路线图归档](/roadmap/archive/index)
