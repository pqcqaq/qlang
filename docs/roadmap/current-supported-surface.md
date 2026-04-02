# 当前支持基线

截至 2026-04-02，本页只记录仓库里已经进入“实现、回归、文档”一致状态的能力，不记录尚未开放的目标面。后续开发请优先更新本页，再同步 `README`、`/docs/index.md` 与路线图入口。

如果你需要看设计过程、切片动机和阶段收口细节，请继续阅读：

- [Phase 7 并发、异步与 Rust 互操作](/plans/phase-7-concurrency-and-rust-interop)
- [开发计划](/roadmap/development-plan)
- [工具链设计](/architecture/toolchain)

## 已形成稳定地基

- Phase 1 到 Phase 6 已经形成稳定主干：lexer、parser、formatter、diagnostics、HIR、name resolution、type checking、MIR、borrow checking、LLVM 文本后端、driver、CLI、same-file LSP/query、FFI header projection。
- 当前 CLI 已提供真实可用入口：`ql check`、`ql build`、`ql ffi header`、`ql fmt`、`ql mir`、`ql ownership`、`ql runtime`。
- 当前构建产物已经覆盖 `llvm-ir`、`obj`、`exe`、`dylib`、`staticlib`；`dylib` / `staticlib` 还支持 build-side C header sidecar。
- 当前互操作稳定边界仍是 C ABI；Rust 互操作通过 `staticlib + header + Cargo build.rs` 路径落地。

## 当前已开放的构建表面

- `ql build --emit llvm-ir|obj|exe|dylib|staticlib` 已是仓库内真实支持能力，不再只是设计目标。
- fixed-array iterable 的 sync `for` 已开放在当前 program / library build surface 内，不依赖 async runtime hook。
- 当前普通表达式与 `if` / `while` 条件里，也已开放 same-file foldable `const` / `static` item value 及其 same-file `use ... as ...` alias 的直接 materialize；当前已支持的 tuple / fixed-array / plain-struct literal 子集会复用同一条 const-evaluation lowering。
- 最小 literal `match` lowering 已开放在当前 program / library build surface 内：
  - `Bool` scrutinee：unguarded `true` / `false` + `_` 或单名 binding catch-all arm 会直接 lower 成 LLVM branch。
  - `Int` scrutinee：unguarded integer literal + `_` 或单名 binding catch-all arm 会 lower 成稳定 compare-chain。
- 当前 build surface 也已开放普通表达式、`if` / `while` 条件里的 `Bool` 短路 `&&` / `||`，以及通用 `Bool` unary `!`。
- 上述两条子集现在都额外支持 literal `if true` / `if false` guard、same-file foldable `const` / `static`-backed `Bool` guard 和其 same-file `use ... as ...` 别名，以及包裹当前 bool guard 子集的一层 unary `!`；`if false` arm 会在 lowering 时被裁剪，`if true` arm 会按普通 arm 处理。
- 上述两条 `match` pattern 子集现在也接受 same-file foldable `const` / `static` bare path 及其 same-file local import alias，只要该 bare path 能折叠成 `Bool` / `Int` literal；`Bool` 这条链现在也接受由当前支持的 `!`、`&&`、`||`、`==`、`!=` 与整数比较子集构成的 computed same-file foldable `const` / `static` `Bool` expression；`Int` 这条链现在也接受 unary-negated literal / const / static 值，以及 same-file foldable `const` / `static` `Int` arithmetic expression 的折叠结果；不能折叠成 `Bool` / `Int` 的 bare path pattern 仍维持显式拒绝。
- 上述两条子集现在也额外支持当前简单 scalar comparison guard 子集：`Bool` `==` / `!=`，以及由 integer literal、unary-negated supported `Int` operand、same-file foldable `const` / `static`-backed `Int` 与其 same-file `use ... as ...` 别名、由当前支持的 `Int` operand 继续递归组成的最小整数算术表达式（`+` / `-` / `*` / `/` / `%`）、以 local / parameter / `self` 为根的 read-only scalar projection operand、以及能经由 struct field / tuple literal-index / fixed-array index 折叠成 scalar 的 same-file foldable `const` / `static`-backed aggregate projection 及其 same-file `use ... as ...` 别名组成的 `Int` `==` / `!=` / `>` / `>=` / `<` / `<=`；tuple / fixed-array 的 index operand 现在也接受同一条可折叠的 same-file const/static-backed `Int` arithmetic 子集。
- 上述 fixed-array 只读 guard projection 现在也接受当前 runtime `Int` scalar 子集作为 index expression，例如 `values[index + 1]` 与 `values[current + 1]`；tuple index 仍保持 foldable-const-only。
- `Bool` scrutinee 子集现在还额外支持 direct bool-valued guard：same-scope `Bool` local / parameter、以 local / parameter / `self` 为根的 read-only bool scalar projection，以及能折叠成 `Bool` 的 same-file foldable `const` / `static`-backed aggregate projection 及其 same-file `use ... as ...` 别名；但当前只开放在后续 arm 仍提供 guaranteed fallback coverage 的 ordered 子集内。
- `Int` scrutinee 子集现在还额外支持 integer-literal arm 与 guarded catch-all arm 上的同一组 direct bool-valued guard，但当前只开放在后续存在 unguarded catch-all fallback 的 ordered 子集内。
- `match guard` 现在也支持对当前 bool guard 子集做 runtime `&&` / `||` 组合；也就是说 literal / const/static-backed / direct-bool / scalar-comparison 这些已列出的 bool-valued guard，可以继续被 `!`、`&&`、`||` 组合后进入 lowering。
- 当前 arm 的单名 binding catch-all 变量现在也可作为 direct scalar operand 参与当前 guard 子集；例如 `match flag { state if state && enabled => ... }` 与 `match value { current if current > limit => ... }` 这类路径已进入 lowering。它们现在也可作为 fixed-array 只读投影里的 dynamic index operand，例如 `match value { current if values[current] < values[2] => ... }`；但基于当前 arm 新绑定名本身继续做 member projection、或把它继续当作 tuple/fixed-array projection root，仍未开放。
- async public build 当前已开放两类受控子集：
  - library build 子集：`staticlib` 与最小 async `dylib`，要求公开导出面仍保持同步 `extern "c"` C ABI。
  - program build 子集：`BuildEmit::LlvmIr`、`BuildEmit::Object`、`BuildEmit::Executable` 下的最小 `async fn main`。
- fixed-array iterable 的 `for await` 已开放在当前 library build 子集和当前 `async fn main` program build 子集内。
- `dylib` 当前仍要求至少存在一个 public 顶层 `extern "c"` 函数定义，避免生成没有明确导出面的共享库。
- `ql runtime <file>` 已能输出 runtime requirement 与 dedupe 后的 runtime hook 计划，作为 async lowering 的共享 truth surface。

## 当前已支持的 async / task-handle 子集

- `Task[T]` 已作为显式 task-handle 类型面进入 `ql-resolve` / `ql-typeck`。
- direct async call、helper-returned task handle、`spawn`、`await` 已统一走同一套 task-handle 模型。
- 当前已支持的 task result / await payload 包括：
  - `Void`
  - scalar builtin
  - tuple、fixed-array、non-generic struct
  - zero-sized fixed-array 与递归 zero-sized aggregate
  - 递归 fixed-shape aggregate
  - aggregate 内继续携带 `Task[T]` 的 payload
  - nested task-handle payload，例如 `await outer(); await next`
- projected task-handle operand 已支持 tuple index、fixed-array literal index、struct field，以及这些路径的递归组合。
- dynamic fixed-array `Task[...]` 当前已开放三层保守子集：
  - generic dynamic write/reinit，仍按 maybe-overlap 处理
  - sibling-safe dynamic consume/spawn
  - same immutable stable source path 的 precise consume/reinit，例如 `index`、`slot.value`
- same-file `const` item、最小 equality guard refinement、immutable alias-source canonicalization、projected-root / alias-root canonicalization，已经进入当前 dynamic task-handle 子集，而不再只是内部假设。
- cleanup 相关的 equality-guard refinement 已进入分析层 truth surface，但 cleanup codegen 仍未开放。

## 当前可运行的真实样例

- `ramdon_tests/async_program_surface_examples/` 当前收录 69 个 async executable 样例，覆盖当前 `BuildEmit::Executable` program surface。
- `crates/ql-cli/tests/executable_examples.rs` 会在真实本地 toolchain 上构建并运行这 69 个样例，并锁定退出码。
- 这些样例不只验证“能产出 IR”，也验证当前最小 async executable 子集已经能真实链接、运行，并复用现有 task-handle / aggregate payload / fixed-array `for await` lowering。

## 当前互操作与工具边界

- `ql ffi header` 与 build-side `--header` / `--header-surface` / `--header-output` 已形成同一套 header projection 真相源。
- 仓库内已有 committed C host、C dylib host、Rust host 示例：
  - `examples/ffi-c/`
  - `examples/ffi-c-dylib/`
  - `examples/ffi-rust/`
- 当前 Rust host 路径是保守的 `Cargo build.rs -> ql build --emit staticlib -> stable C ABI`，并已有测试锁定。

## 当前明确未支持

- 更广义的 async executable / program bootstrap，除当前 `async fn main` 最小子集以外仍未开放。
- 更广义的 async `dylib` surface，以及任何需要公开 async ABI 的共享库承诺。
- 非 fixed-array iterable 的 `for` / `for await`。
- 更广义动态 guard 的 `match`（包括超出当前 `!` / `&&` / `||` + `Bool ==/!=` 与 `Int ==/!=/>/>=/</<=` 子集之外的任意表达式 guard、超出当前 read-only local / parameter / `self` root 与 same-file foldable `const` / `static`-backed aggregate root 及其 same-file `use ... as ...` alias 子集的投影 operand、基于当前 arm 新绑定名继续做 member/index projection、`Bool` scrutinee 上不具备 guaranteed fallback coverage 的 direct bool-valued guard，以及 `Int` scrutinee 上不具备 later unguarded catch-all fallback 的 direct bool-valued guard）、非 `Bool` / `Int` scrutinee `match`、以及超出 `Bool true|false|same-file const/static/alias bare path|_|single-name binding` / `Int literal|same-file const/static/alias bare path|_|single-name binding` 的更广义 match pattern lowering。
- cleanup lowering / cleanup codegen。
- cancellation / polling / drop 语义。
- generic async ABI / layout substitution。
- 更广义的 projection-sensitive partial move、arbitrary dynamic overlap precision、以及超出当前 `Task[...]` 子集的 place-sensitive 生命周期推理。

## 后续开发约束

- 长文设计与切片原因继续以 [Phase 7 并发、异步与 Rust 互操作](/plans/phase-7-concurrency-and-rust-interop) 为准。
- “今天真实支持到哪里” 以本页为准。
- 每次继续扩 async/build/interop surface 时，至少同步四处：
  - 本页
  - `README.md`
  - `/docs/index.md`
  - 对应 phase / roadmap 入口文档
- 新能力默认要求同时具备：
  - 一条真实用户入口
  - 至少一条用户可见回归
  - 对未支持边界的稳定诊断
