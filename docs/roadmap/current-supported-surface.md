# 当前支持基线

截至 2026-04-03，本页只记录仓库里已经进入“实现、回归、文档”一致状态的能力，不记录尚未开放的目标面。后续开发请优先更新本页，再同步 `README`、`/docs/index.md` 与路线图入口。

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
- fixed-shape iterable 的 sync `for` 已开放在当前 program / library build surface 内，不依赖 async runtime hook；当前 fixed-shape 覆盖 fixed-array 与 homogeneous tuple，并已开放 local root、projected root、same-file `const` / `static` root 及其 same-file `use ... as ...` alias。
- 当前普通表达式与 `if` / `while` 条件里，也已开放 same-file foldable `const` / `static` item value 及其 same-file `use ... as ...` alias 的直接 materialize；当前已支持的 tuple / fixed-array / plain-struct literal 子集会复用同一条 const-evaluation lowering。
- 最小 literal `match` lowering 已开放在当前 program / library build surface 内：
  - `Bool` scrutinee：unguarded `true` / `false` + `_` 或单名 binding catch-all arm 会直接 lower 成 LLVM branch。
  - `Int` scrutinee：unguarded integer literal + `_` 或单名 binding catch-all arm 会 lower 成稳定 compare-chain。
- 当前 build surface 也已开放普通表达式、`if` / `while` 条件里的 `Bool` 短路 `&&` / `||`，以及通用 `Bool` unary `!`。
- 上述两条子集现在都额外支持 literal `if true` / `if false` guard、same-file foldable `const` / `static`-backed `Bool` guard 和其 same-file `use ... as ...` 别名，以及包裹当前 bool guard 子集的一层 unary `!`；`if false` arm 会在 lowering 时被裁剪，`if true` arm 会按普通 arm 处理。
- 上述两条 `match` pattern 子集现在也接受 same-file foldable `const` / `static` bare path 及其 same-file local import alias，只要该 bare path 能折叠成 `Bool` / `Int` literal；`Bool` 这条链现在也接受由当前支持的 `!`、`&&`、`||`、`==`、`!=` 与整数比较子集构成的 computed same-file foldable `const` / `static` `Bool` expression；`Int` 这条链现在也接受 unary-negated literal / const / static 值，以及 same-file foldable `const` / `static` `Int` arithmetic expression 的折叠结果；不能折叠成 `Bool` / `Int` 的 bare path pattern 仍维持显式拒绝。
- 上述两条子集现在也额外支持当前简单 scalar comparison guard 子集：`Bool` `==` / `!=`，以及由 integer literal、unary-negated supported `Int` operand、same-file foldable `const` / `static`-backed `Int` 与其 same-file `use ... as ...` 别名、由当前支持的 `Int` operand 继续递归组成的最小整数算术表达式（`+` / `-` / `*` / `/` / `%`）、以 local / parameter / `self` 为根的 read-only scalar projection operand、以及能经由 struct field / tuple literal-index / fixed-array index 折叠成 scalar 的 same-file foldable `const` / `static`-backed aggregate projection 及其 same-file `use ... as ...` 别名组成的 `Int` `==` / `!=` / `>` / `>=` / `<` / `<=`；tuple / fixed-array 的 index operand 现在也接受同一条可折叠的 same-file const/static-backed `Int` arithmetic 子集。
- 上述 fixed-array 只读 guard projection 现在也接受当前 runtime `Int` scalar 子集作为 index expression，例如 `values[index + 1]` 与 `values[current + 1]`；tuple index 仍保持 foldable-const-only。
- `Bool` scrutinee 子集现在还额外支持 direct bool-valued guard：same-scope `Bool` local / parameter、以 local / parameter / `self` 为根的 read-only bool scalar projection，以及能折叠成 `Bool` 的 same-file foldable `const` / `static`-backed aggregate projection 及其 same-file `use ... as ...` 别名；当前不再额外要求 later arm 提供 guaranteed fallback coverage，guard miss 会像现有其他非穷尽 `match` 一样自然回落到当前 `match` 的 `else_target`。
- `Int` scrutinee 子集现在还额外支持 integer-literal arm 与 guarded catch-all arm 上的同一组 direct bool-valued guard；当前也不再额外要求 later unguarded catch-all fallback，guard miss 会像现有其他非穷尽 `match` 一样自然回落到当前 `match` 的 `else_target`。
- `match guard` 现在也支持对当前 bool guard 子集做 runtime `&&` / `||` 组合；也就是说 literal / const/static-backed / direct-bool / scalar-comparison 这些已列出的 bool-valued guard，可以继续被 `!`、`&&`、`||` 组合后进入 lowering。
- 当前 arm 的单名 binding catch-all 变量现在也可作为 direct scalar operand 参与当前 guard 子集；例如 `match flag { state if state && enabled => ... }` 与 `match value { current if current > limit => ... }` 这类路径已进入 lowering。它们现在也可作为 fixed-array 只读投影里的 dynamic index operand，例如 `match value { current if values[current] < values[2] => ... }`；并且现在也可直接作为只读 member / tuple / fixed-array projection root，例如 `match state { current if current.slot.ready => ... }`、`match pair { current if current[1] == 2 => ... }` 与 `match values { current if current[0] == 1 => ... }` 这类路径也已进入 lowering。
- async public build 当前已开放两类受控子集：
  - library build 子集：`staticlib` 与最小 async `dylib`，要求公开导出面仍保持同步 `extern "c"` C ABI。
  - program build 子集：`BuildEmit::LlvmIr`、`BuildEmit::Object`、`BuildEmit::Executable` 下的最小 `async fn main`。
- fixed-shape iterable 的 `for await` 已开放在当前 library build 子集和当前 `async fn main` program build 子集内；当前 fixed-shape 覆盖 fixed-array 与 homogeneous tuple，并已开放 local root、projected root、same-file `const` / `static` root 及其 same-file `use ... as ...` alias。
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
- same-file `const` / `static` item，以及指向这些 same-file item 的 `use ... as ...` alias，连同最小 equality guard refinement、immutable alias-source canonicalization、projected-root / alias-root canonicalization，已经进入当前 dynamic task-handle 子集，而不再只是内部假设；其中 current program 回归面现在也已显式锁住 `let alias = pending.tasks; await alias[INDEX_ALIAS.value]; pending.tasks[0] = ...; await alias[INDEX_ALIAS.value]`，以及 `if INDEX_ALIAS.value == 0 { await alias[INDEX_ALIAS.value]; pending.tasks[0] = ... } await alias[0]` 这类 projected-root + alias-root + static/use-alias 组合路径；前者现已进入 driver `BuildEmit::Object` 与 CLI `object` / `executable` regression matrix，后者现已进入 driver `BuildEmit::Object` 与 CLI `llvm-ir` / `object` / `executable` public regression matrix，并已有真实 executable 示例 `ramdon_tests/async_program_surface_examples/73_async_main_aliased_guard_refined_static_alias_backed_projected_root.ql` 锁定 direct build-and-run；同一条 static/use-alias + guard-refined 路径现也已推进到 nested aggregate submit 形态，新增真实 executable 示例 `ramdon_tests/async_program_surface_examples/74_async_main_aliased_guard_refined_static_alias_backed_nested_repackage_spawn.ql` 锁定 `spawn env.bundle.left` 这条更深一层的 build-and-run 行为，并继续推进到 helper-forwarded nested fixed-array submit 形态，新增 `ramdon_tests/async_program_surface_examples/75_async_main_aliased_guard_refined_static_alias_backed_forwarded_nested_array_repackage_spawn.ql` 锁定 `forward(alias[INDEX_ALIAS.value])` 先重包装再提交的 build-and-run 行为；随后继续推进到 alias-sourced composed dynamic + helper-forwarded nested fixed-array submit 形态，新增 `ramdon_tests/async_program_surface_examples/76_async_main_guarded_static_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn.ql` 锁定 `alias[alias_slots[row]]` 这条 composed stable-dynamic 路径先重包装再提交的 build-and-run 行为；本轮再继续推进到 double-root alias + alias-sourced composed dynamic 形态，新增 `ramdon_tests/async_program_surface_examples/77_async_main_guarded_static_alias_backed_double_root_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn.ql` 锁定 `let root = pending.tasks; let alias = root; forward(alias[alias_slots[row]])` 这条多一层 root alias 的 build-and-run 行为；并继续推进到 double-root double-source alias 形态，新增 `ramdon_tests/async_program_surface_examples/78_async_main_guarded_static_alias_backed_double_root_double_source_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn.ql` 锁定 `let slot_root = slots; let alias_slots = slot_root; let root = pending.tasks; let alias = root; forward(alias[alias_slots[row]])` 这条再多一层 source alias 的 build-and-run 行为；随后继续推进到 row alias 形态，新增 `ramdon_tests/async_program_surface_examples/79_async_main_guarded_static_alias_backed_double_root_double_source_row_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn.ql` 锁定 `let row_root = INDEX_ALIAS.value; let row = row_root` 这条再多一层 row alias 的 build-and-run 行为；继续推进到 slot alias 形态，新增 `ramdon_tests/async_program_surface_examples/80_async_main_guarded_static_alias_backed_double_root_double_source_row_slot_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn.ql` 锁定 `let slot = Slot { value: INDEX_ALIAS.value }; let slot_alias = slot` 这条 row/slot 双别名的 build-and-run 行为；并继续推进到 triple-root 形态，新增 `ramdon_tests/async_program_surface_examples/81_async_main_guarded_static_alias_backed_triple_root_double_source_row_slot_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn.ql` 锁定 `let root_alias = root; let alias = root_alias` 这条三层 task-root alias 的 build-and-run 行为；最后推进到 triple-source 形态，新增 `ramdon_tests/async_program_surface_examples/82_async_main_guarded_static_alias_backed_triple_root_triple_source_row_slot_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn.ql` 锁定 `let slot_alias_root = slot_root; let alias_slots = slot_alias_root` 这条三层 source-root alias 的 build-and-run 行为。
- cleanup 相关的 equality-guard refinement 已进入分析层 truth surface，但 cleanup codegen 仍未开放。

## 当前可运行的真实样例

- `ramdon_tests/executable_examples/04_sync_static_item_values.ql` 现已把 same-file foldable `const` / `static` item value 及其 same-file `use ... as ...` alias 的普通表达式 / bool 条件 lowering 收口到真实 `ql build --emit exe` sync 样例。
- `ramdon_tests/executable_examples/05_sync_named_call_arguments.ql` 现已把 direct named call arguments lowering（含 `[]` 的 expected-type back-propagation）收口到真实 `ql build --emit exe` sync 样例。
- `ramdon_tests/executable_examples/06_sync_import_alias_named_call_arguments.ql` 现已把 same-file function import alias calls lowering（含 named arguments 与 `[]` 的 expected-type back-propagation）收口到真实 `ql build --emit exe` sync 样例。
- `ramdon_tests/executable_examples/07_sync_for_fixed_array.ql`、`08_sync_for_tuple.ql` 与 `09_sync_for_projected_fixed_shape.ql` 现已把 sync fixed-shape `for` lowering 收口到真实 `ql build --emit exe` sync 样例，覆盖 direct fixed-array root、direct homogeneous tuple root，以及 projected tuple/array root。
- `ramdon_tests/executable_examples/10_sync_for_const_static_fixed_shape.ql` 现已把 same-file `const` / `static` fixed-shape root 及其 same-file `use ... as ...` alias 的 sync `for` lowering 收口到真实 `ql build --emit exe` sync 样例。
- `ramdon_tests/executable_examples/11_sync_match_scrutinee_self_guard.ql` 现已把 bool `match` 的 scrutinee self-guard folding 收口到真实 `ql build --emit exe` sync 样例，因此 `match flag { true if flag => ..., false => ... }` 这类“guard 就是 scrutinee 自身”的最小 ordered 子集不再误报 backend unsupported。
- `ramdon_tests/executable_examples/12_sync_match_scrutinee_bool_comparison_guard.ql` 现已把 bool `match` 的 scrutinee-bool-comparison guard folding 收口到真实 `ql build --emit exe` sync 样例，因此 `match flag { true if flag == true => ..., false => ... }` 以及 `true if flag == ON` 这类“scrutinee 只是在和可折叠 `Bool` literal/const/static/alias 做 `==` / `!=` 比较”的最小 ordered 子集也不再误报 backend unsupported。
- `ramdon_tests/executable_examples/13_sync_match_partial_dynamic_guard.ql` 现已把 bool `match` 的 partial dynamic guard lowering 收口到真实 `ql build --emit exe` sync 样例，因此 `match flag { true if enabled => ..., false => ... }` 这类此前仅因为“没有 later true fallback arm”而被 backend 误拒的最小 ordered 子集，现在也能进入 LLVM lowering。
- `ramdon_tests/executable_examples/14_sync_match_partial_integer_dynamic_guard.ql` 现已把 integer `match` 的 partial dynamic guard lowering 收口到真实 `ql build --emit exe` sync 样例，因此 `match value { 1 if enabled => ..., 2 => ... }` 这类此前仅因为“没有 later unguarded catch-all arm”而被 backend 误拒的最小 ordered 子集，现在也能进入 LLVM lowering。
- `ramdon_tests/executable_examples/15_sync_match_guard_binding_projection_roots.ql` 现已把当前 arm binding 作为只读 struct-field / tuple-index / fixed-array-index projection root 的 guard lowering 收口到真实 `ql build --emit exe` sync 样例，因此 `current.slot.ready`、`current[1]` 与 `current[0]` 这类此前会被 backend 误拒的最小 guard projection 路径，现在也能进入 LLVM lowering。
- `ramdon_tests/async_program_surface_examples/` 当前收录 142 个 async executable 样例，覆盖当前 `BuildEmit::Executable` program surface。
- `for` / `for await` 的循环变量现在会按当前 fixed-shape iterable 的元素类型绑定，而不再一律退化成 `Unknown`；当 `for await` 遍历 `[Task[T]; N]` 或 homogeneous task tuple `(Task[T], ...)` 时，循环变量会进一步绑定到自动 `await` 后的 `T`，因此 loop body 可以直接使用 `value`，而重复写 `await value` 会稳定报“非 task operand”诊断。
- `ramdon_tests/async_program_surface_examples/107_async_main_import_alias_named_calls.ql`、`108_async_main_import_alias_direct_submit.ql`、`109_async_main_import_alias_aggregate_submit.ql`、`110_async_main_import_alias_array_submit.ql`、`111_async_main_import_alias_tuple_submit.ql`、`112_async_main_import_alias_forward_submit.ql`、`113_async_main_import_alias_helper_task_submit.ql`、`114_async_main_import_alias_helper_forward_submit.ql`、`115_async_main_import_alias_task_array_for_await.ql`、`116_async_main_import_alias_helper_task_array_for_await.ql`、`121_async_main_import_alias_awaited_aggregate_task_array_for_await.ql`、`127_async_main_import_alias_helper_task_tuple_for_await.ql`、`131_async_main_import_alias_awaited_aggregate_task_tuple_for_await.ql` 与 `135_async_main_import_alias_task_tuple_for_await.ql` 现已把 same-file function import alias calls lowering（含 named arguments、direct `await`、direct `spawn`、aggregate-carried、fixed-array-carried、tuple-carried、helper-forwarded、helper-returned task-handle、helper-returned-task + helper-forwarded submit，以及 direct/helper-returned/awaited-aggregate/inline-awaited fixed-array task-array `for await`、direct/helper-returned homogeneous task-tuple `for await`、以及 import-aliased awaited-aggregate task-tuple `for await` 自动 `await` 元素后再绑定循环变量）收口到真实 `async fn main` build-and-run 样例；`117_async_main_projected_task_array_for_await.ql`、`118_async_main_void_task_array_for_await.ql`、`119_async_main_awaited_aggregate_task_array_for_await.ql`、`120_async_main_awaited_nested_aggregate_task_array_for_await.ql`、`122_async_main_inline_awaited_task_array_for_await.ql`、`123_async_main_awaited_tuple_projected_task_array_for_await.ql`、`124_async_main_awaited_array_projected_task_array_for_await.ql`、`125_async_main_tuple_for_await.ql`、`126_async_main_task_tuple_for_await.ql`、`128_async_main_projected_task_tuple_for_await.ql`、`129_async_main_awaited_aggregate_task_tuple_for_await.ql`、`130_async_main_awaited_tuple_projected_task_tuple_for_await.ql`、`132_async_main_inline_awaited_task_tuple_for_await.ql`、`133_async_main_awaited_nested_aggregate_task_tuple_for_await.ql`、`134_async_main_awaited_array_projected_task_tuple_for_await.ql`、`135_async_main_import_alias_task_tuple_for_await.ql` 与 `136_async_main_void_task_tuple_for_await.ql` 则继续把 projected/awaited fixed-array task root、homogeneous tuple iterable root、task-tuple auto-await root，以及 projected/awaited projected task-tuple root 收口到同一条 executable public surface。
- `137_async_main_const_tuple_for_await.ql`、`138_async_main_static_array_for_await.ql`、`139_async_main_import_alias_projected_const_tuple_for_await.ql`、`140_async_main_import_alias_projected_static_array_for_await.ql`、`141_async_main_import_alias_const_tuple_for_await.ql` 与 `142_async_main_import_alias_static_array_for_await.ql` 则把 same-file `const` / `static` fixed-shape root 及其 same-file import-alias projected/direct root 的 scalar `for await` lowering 收口到真实 `async fn main` build-and-run 样例。
- `crates/ql-cli/tests/executable_examples.rs` 会在真实本地 toolchain 上构建并运行这 142 个 async 样例，并锁定退出码；同一个 harness 也会继续运行 15 个 sync executable examples，其中现已包含 `04_sync_static_item_values.ql` 到 `15_sync_match_guard_binding_projection_roots.ql`。
- 这些样例不只验证“能产出 IR”，也验证当前最小 async executable 子集已经能真实链接、运行，并复用现有 task-handle / aggregate payload / fixed-shape `for await` lowering。

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
- 更广义的 dynamic/generalized iterable，以及非 homogeneous tuple 的 `for` / `for await`。
- 更广义动态 guard 的 `match`（包括超出当前 `!` / `&&` / `||` + `Bool ==/!=` 与 `Int ==/!=/>/>=/</<=` 子集之外的任意表达式 guard、超出当前 read-only local / parameter / `self` root、当前 arm 单名 binding 的只读 struct-field / tuple-index / fixed-array-index root、以及 same-file foldable `const` / `static`-backed aggregate root 及其 same-file `use ... as ...` alias 子集的投影 operand）、非 `Bool` / `Int` scrutinee `match`、以及超出 `Bool true|false|same-file const/static/alias bare path|_|single-name binding` / `Int literal|same-file const/static/alias bare path|_|single-name binding` 的更广义 match pattern lowering。
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
