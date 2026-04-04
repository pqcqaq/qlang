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
- cleanup lowering 已不再是“全量关闭”：首个 `defer` + cleanup branch/match + 透明 `?` wrapper lowering 子集已进入真实 build 回归，并已接通 callable-value cleanup callee 与 cleanup guard-call 子路径的最小间接调用；broader cleanup control flow 仍保持保守拒绝。
- 普通 `?` lowering 已接入当前 codegen 路径，并已流入当前 shipped cleanup 子集；当前 user-facing build blocker 不再包含 `return helper()?` 或 `defer helper()?` 这类透明 question-mark 表达式。

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
- `let` / `var` 局部绑定；当前已支持 statement-level 显式类型标注 `let name: Type = value` / `var name: Type = value`，以及 tuple / struct destructuring（叶子当前限 binding / `_`）
- direct call 与 named arguments
- same-file `use ... as ...` function alias call
- 最小 first-class sync function value 子集
  - same-file sync function item
  - same-file `use ... as ...` function alias
  - transparently resolve 到 same-file sync function item 的 callable `const` / `static`，以及它们的 same-file `use ... as ...` alias
  - non-capturing sync closure-backed callable `const` / `static`，以及它们的 same-file `use ... as ...` alias；当前 public regression 先锁定 ordinary positional indirect call 子集
  - non-capturing sync closure value；当前 public regression 已锁定 ordinary positional indirect call 的最小子集：zero-arg 形态、显式 typed closure parameter 形态、由 statement-level local callable type annotation 驱动的 parameterized local 形态，以及由 call-site positional argument 反推参数类型的 parameterized local/immutable-alias 形态；当前 shipped cleanup / guard-call 子路径也已显式锁定 direct local non-capturing closure 的最小子集，capturing closure 仍保持关闭
  - ordinary call 可 direct call，或先绑定到 local 后再做 positional indirect call
  - ordinary `match` guard，以及当前 shipped cleanup call / guard-call 子路径，也可通过 function-item-backed callable local / callable `const` / `static` / same-file alias 进入 positional indirect call；当前 public regression 也已显式锁定 direct closure-backed callable `const` guard + closure-backed callable `static` cleanup，以及 direct local non-capturing closure cleanup + guard 的最小子集
- 最小 first-class async function value 子集
  - same-file async function item
  - same-file `use ... as ...` async function alias
  - transparently resolve 到 same-file async function item 的 callable `const` / `static`，以及它们的 same-file `use ... as ...` alias
  - 当前 public regression 已锁定 `async fn` 内 ordinary direct call 或 ordinary local positional indirect call + `await` 子集
  - capturing closure value，以及 cleanup / guard-call 上的 async callable path 仍保持关闭
- fixed-shape `for`
  - fixed-array
  - homogeneous tuple
  - `binding` / `_` / tuple destructuring / struct destructuring loop pattern
  - projected root / direct call-root / same-file import-alias call-root / nested call-root / same-file import-alias nested call-root
  - block-valued / assignment-valued / runtime `if` / `match` valued projected root
  - parenthesized / unparenthesized inline projected root
  - same-file `const` / `static` root 及其 same-file alias
- 赋值表达式的当前可运行子集
  - mutable local
  - tuple literal index projection，以及 same-file `const` / `static` / `use ... as ...` alias、branch-selected const `if` / 最小 literal `match` item value、direct inline foldable `if` / `match` integer expression，和 immutable direct local alias 复用驱动的 foldable integer constant expression tuple index
  - struct field
  - fixed-array literal index projection
  - projected-root / nested projected-root / call-root nested projected-root / import-alias call-root nested projected-root / inline nested projected-root tuple / struct-field / fixed-array literal-index chains
  - assignment expr value form：当前已覆盖 direct call arg 与 valued block tail
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
- bool guard path 现也接受同一批 ordinary local / param / `self` root 的 bool assignment expr value；当前已锁定 shipped cleanup `if` condition 公开回归
- direct resolved sync guard call 子集
  - guard-call arg value path 现也接受同一批 ordinary local / param / `self` root 的 assignment expr value，包括当前 loadable guard-call arg 子集
  - guard-call arg value path 现也接受最小 runtime `if` value 子集：当前已锁定 loadable guard-call arg 的 `if cond { ... } else { ... }` 形态
  - guard-call arg value path 现也接受最小 runtime `match` value 子集：当前已锁定 bool/int scrutinee + 既有 guard-match arm 子集上的 loadable guard-call arg 形态
  - guard-call callee root 现也接受最小 runtime `if` / `match` callable value 子集：当前已锁定由 same-file function item 选出的 indirect callee 形态
  - callable-value positional indirect guard call 子集：当前已覆盖 callable local / callable `const` / `static` / same-file alias
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
  - same-file scalar `const` / `static` root、same-file scalar item alias，以及 scalar item-backed read-only projected root
  - same-file task-producing `const` / `static` root，以及 same-file task item alias root
  - projected / block-valued projected / assignment-valued projected / runtime `if` / `match` valued projected / call-root / awaited-aggregate / import-alias / inline / nested call-root
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
  - source-root reinit 后的 same-file arithmetic item / same-file `use ... as ...` alias 驱动 direct alias-root、projected-root、alias-sourced composed-dynamic 与 guard-refined alias-sourced composed-dynamic 形态，包括 bundle-alias-forwarded、bundle-alias-inline-forwarded、queued-root-forwarded、queued-root-inline-forwarded、queued-root-alias-forwarded、queued-root-alias-inline-forwarded、queued-root-chain-forwarded、queued-root-chain-inline-forwarded、queued-local-alias、queued-local-chain、queued-local-forwarded、queued-local-inline-forwarded、bundle-chain-forwarded 与 bundle-chain-inline-forwarded await/spawn
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
  - projected / block-valued projected / assignment-valued projected / runtime `if` / `match` valued projected / call-root / awaited-aggregate / import-alias / inline / nested call-root
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

### cleanup lowering 子集

当前 cleanup lowering 只开放首个受控子集：

- direct / call-backed `defer`
- 其中 call-backed `defer` 当前已覆盖 direct resolved callee，以及 callable local / callable `const` / `static` / same-file alias 驱动的 positional indirect callee；runtime `if` / `match` typed-value path 当前也可选出 same-file function item / same-file import alias 作为 callable cleanup callee root
- statement-sequenced block wrapper：当前接受 binding / `_`、tuple destructuring、struct destructuring（叶子仍限 binding / `_`）的最小 `let` statement、已支持 cleanup expr statement、statement-level assignment expr、statement-level `while` / `loop` / `for`，外加可选 tail；当前已覆盖 direct cleanup body、cleanup `let` binding / destructuring block、cleanup guard / scrutinee block、cleanup call-arg value block，以及 rooted in ordinary local / param / `self` place family 的 local/field/tuple-index/fixed-array-index assignment expr statement
- cleanup value path 现也接受同一批 ordinary local / param / `self` place family root 的 assignment expr value，包括 direct cleanup call arg 与 valued cleanup block tail；当前仍限 local/field/tuple-index/fixed-array-index target path
- cleanup value path 现也接受最小 runtime `if` value 子集：当前已锁定 direct cleanup call arg 的 `if cond { ... } else { ... }` 形态
- cleanup value path 现也接受最小 runtime `match` value 子集：当前已锁定 direct cleanup call arg 的 bool/int scrutinee + 既有 cleanup-match arm 子集
- cleanup value path 现也接受最小 runtime `await` value 子集：当前已锁定 async body 内 direct cleanup call arg 的 `await task` 形态
- cleanup value path 现也接受最小 runtime `spawn` value 子集：当前已锁定 async body 内 direct cleanup call arg 的 `spawn worker(...)` / `spawn task` 形态
- statement-level cleanup `while`：当前开放 bool 条件 + 已支持 cleanup block body 的最小 lowering 子集，可在 cleanup block 内重复执行 direct / callable-backed call 路径，并支持 body-local `break` / `continue`（包括经由当前已开放 cleanup `if` branch 进入的 loop-exit path）
- statement-level cleanup `loop`：当前开放已支持 cleanup block body 的最小 lowering 子集，并支持 body-local `break` / `continue`（包括经由当前已开放 cleanup `if` branch 进入的 loop-exit path）
- statement-level cleanup `for`：当前开放 fixed array / homogeneous tuple iterable + binding / `_` / tuple destructuring / struct destructuring（叶子仍限 binding / `_`）pattern 的最小 lowering 子集，iterable 当前已覆盖 direct root、same-file `const` / `static` root 及其 same-file alias、item-backed read-only projected root、direct call-root、same-file import-alias call-root、nested call-root projected root，以及 transparent `?` wrapper 下的 projected root 形态；body 内可读取当前 item，并支持 body-local `break` / `continue`（包括经由当前已开放 cleanup `if` branch 进入的 loop-exit path）
- statement-level cleanup `for await`：当前在 async body 内开放 fixed array / homogeneous tuple iterable 的最小 lowering 子集；普通元素会直接逐项绑定，`Task[...]` 元素会复用既有 `await` + result-release 路径做逐项 auto-await，并支持 body-local `break` / `continue`；当前已锁定 direct local root、same-file scalar `const` / `static` root、same-file scalar item alias、same-file task-producing `const` / `static` root、same-file task item alias root、scalar item-backed read-only projected root、direct block-valued / assignment-valued / runtime `if` / `match` / awaited direct root、direct question-mark root、read-only projected root、assignment-valued projected root、block-valued projected root、direct call-root、same-file import-alias call-root、nested call-root projected root、awaited projected root、runtime `if` / `match` aggregate projected root、transparent `?` wrapper 下的 projected root，以及 inline array/tuple task root
- cleanup aggregate value staging：cleanup `let` / valued block / projected-root materialization 现在会沿 tuple / array / struct literal 递归走 cleanup 自身的 value path；这意味着 awaited projected loadable value 现在可以先被装入 cleanup struct literal 字段，再继续被后续 cleanup `for await` / projected read 消费
- bool-guard 驱动的 call-backed `if` cleanup branch；当前 bool/int guard call 子路径也已覆盖 callable local / callable `const` / `static` / same-file alias 驱动的 positional indirect call，并接受 runtime `if` / `match` 选出的 same-file function item / same-file import alias callee root
- bool / int scrutinee + literal-or-path / wildcard-or-single-binding catch-all arms + optional bool guard 的 cleanup `match` branch；当前 arm guard、binding arm body，以及 cleanup scalar call-arg value 里的 call 子路径也已覆盖同一批 callable-value 间接调用
- 透明 `?` wrapper，可包裹当前 shipped cleanup expr / guard / scrutinee 子路径
- cleanup value path 现也会复用既有 literal-source folding：cleanup `let` value、cleanup `for` iterable、cleanup `if` bool condition，以及 cleanup call-arg scalar/value path 当前都接受可折叠回既有 literal / aggregate root 的 `if` / 最小 literal `match` 根表达式
- 当前已锁定的用户面包括 direct cleanup `obj` build、callable-const-alias cleanup `obj` build、closure-backed callable global cleanup + guard `obj` build、local non-capturing closure cleanup + guard `obj` build、cleanup `let` binding / destructuring block `obj` build、callable-guard-alias cleanup `match` `obj` build、binding-catch-all cleanup `match` `obj` build、statement-sequenced cleanup block `obj` build、statement-sequenced cleanup guard / scrutinee / call-arg value block（现含 runtime `await` / `spawn` task value）`obj` build、带 body-local `break` / `continue` 的 statement-level cleanup `while` / `loop` `obj` build、包含 tuple/struct 解构 pattern、const/static root、projected/call-root、alias call-root、nested call-root projected 与 transparent `?` wrapper projected root 形态在内的 fixed-shape statement-level cleanup `for` `obj` build、async body 内 fixed array / homogeneous tuple + task-element auto-await 子集的 cleanup `for await` `obj` build（现含 same-file scalar `const` / `static` root、same-file scalar item alias、same-file task-producing `const` / `static` root、same-file task item alias root、scalar item-backed read-only projected root、direct block-valued / assignment-valued / runtime `if` / `match` / awaited direct root、direct question-mark root、read-only projected root、assignment-valued projected root、block-valued projected root、direct call-root、same-file import-alias call-root、nested call-root projected root、awaited projected root、runtime `if` / `match` aggregate projected root、transparent `?` wrapper 下的 projected root 与 inline array/tuple task root）、guarded dynamic task-handle cleanup `staticlib` build、cleanup `match` `obj` build，以及 cleanup-internal question-mark `obj` build

### 透明 `?` lowering

当前透明 `?` 表达式会沿 inner operand 直接进入既有 codegen 路径：

- `match` + `?` 不再因为 question-mark 本身被 backend 拦截
- cleanup-adjacent 的 `return helper()?` / 普通 return path 也不再单独报 `?` lowering unsupported
- cleanup-internal 的 `defer helper()?` 也不再单独报 cleanup / `?` lowering unsupported

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
- 更广义的 runtime const/static/item-backed aggregate lowering，超出当前 fixed-shape `for await` / cleanup `for await` direct same-file `const` / `static` / item-alias root 子集之外仍未开放；当前 const item lowering 仍不会把 `worker(...)` 这类 runtime task-producing initializer 普遍提升为通用常量值
- broader cleanup lowering / cleanup codegen，超出当前 direct / call-backed `defer` + `if` / `match` + 透明 `?` wrapper cleanup 子集之外
- broader callable value lowering，超出当前 same-file sync function item / same-file alias / function-item-backed callable `const` / `static` 子集、closure-backed callable `const` / `static` 的 ordinary positional indirect-call 最小子集与 direct cleanup/guard item 子集、non-capturing sync closure value 的 ordinary positional indirect-call 最小子集与 direct local cleanup/guard 子集（zero-arg + explicit typed-parameter shape + statement-level local callable type-annotation shape + call-site positional-arg-inferred parameterized local/immutable-alias shape），以及 same-file async function item / alias / callable `const` / `static` / same-file alias 的 ordinary local indirect-call + `await` 子集之外；capturing closure value与 cleanup 内更广义的 async control-flow 仍未开放
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
