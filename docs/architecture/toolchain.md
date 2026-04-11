# 工具链设计

## 目标

工具链围绕统一 CLI 与共享 analysis 边界组织。实现分层见 [实现算法与分层边界](/architecture/implementation-algorithms)。

## 命令入口

统一入口命令为 `ql`。当前源码已实现的子命令如下：

- `ql check`
- `ql fmt`
- `ql mir`
- `ql ownership`
- `ql runtime`
- `ql build`
- `ql project`
- `ql ffi`

`new`、`init`、`run`、`test`、`doc`、`bench`、`clean` 等更宽命令面仍属于规划，不在本页按已实现能力展开。

## 子工具

### `ql build`

P4/P5 地基已经落地；当前一边保守扩展 Phase 7 async library/program build 子集，一边推进 Phase 8 的 package/`.qi` 工具链入口。现阶段 `ql build` 的职责是：

- 读取单个 `.ql` 文件
- 复用 `ql-analysis` 完成 parse / HIR / resolve / typeck / MIR
- 在无语义错误时调用 `ql-codegen-llvm`
- 输出文本 LLVM IR、对象文件、动态库、静态库和基础可执行文件

当前实现边界：

- `ql-cli` 只负责参数解析与诊断渲染
- `ql-driver` 负责 build orchestration
- `ql-codegen-llvm` 负责 MIR 子集 -> LLVM IR

当前可用命令形态：

- `ql build <file>`
- `ql build <file> --emit llvm-ir`
- `ql build <file> --emit obj`
- `ql build <file> --emit exe`
- `ql build <file> --emit dylib`
- `ql build <file> --emit staticlib`
- `ql build <file> --release`
- `ql build <file> -o <output>`
- `ql build <file> --emit dylib --header`
- `ql build <file> --emit staticlib --header-surface imports`
- `ql build <file> --emit dylib --header-output <output>`

当前默认输出路径：

- `target/ql/debug/<stem>.ll`
- `target/ql/release/<stem>.ll`
- `target/ql/debug/<stem>.obj` / `target/ql/debug/<stem>.o`
- `target/ql/release/<stem>.obj` / `target/ql/release/<stem>.o`
- `target/ql/debug/<stem>.exe` / `target/ql/debug/<stem>`
- `target/ql/release/<stem>.exe` / `target/ql/release/<stem>`
- `target/ql/debug/<stem>.dll` / `target/ql/debug/lib<stem>.so` / `target/ql/debug/lib<stem>.dylib`
- `target/ql/release/<stem>.dll` / `target/ql/release/lib<stem>.so` / `target/ql/release/lib<stem>.dylib`
- `target/ql/debug/<stem>.lib` / `target/ql/debug/lib<stem>.a`
- `target/ql/release/<stem>.lib` / `target/ql/release/lib<stem>.a`

当 `--emit` 是 `dylib` 或 `staticlib` 且启用了 build-side header 时：

- `--header` 生成默认 `exports` surface
- `--header-surface exports|imports|both` 会隐式启用 header sidecar
- `--header-output <output>` 也会隐式启用 header sidecar
- 未显式指定 header 输出路径时，header 会写到主 library artifact 同目录，但使用源码 stem 而不是 library 文件名：
  - `target/ql/debug/libffi_export.so` + `--header` -> `target/ql/debug/ffi_export.h`
  - `target/ql/debug/math.lib` + `--header-surface imports` -> `target/ql/debug/math.imports.h`

当前支持矩阵刻意收窄为：

- 顶层 free function
- `extern "c"` 顶层声明、extern block 声明与顶层函数定义
- `main` 入口
- 标量整数 / `Bool` / `Void`
- direct function call
- fixed-shape iterable 的 sync `for`（当前覆盖 fixed-array 与 homogeneous tuple）
- 普通表达式与 `if` / `while` 条件里可直接 materialize same-file foldable `const` / `static` item value 及其 same-file `use ... as ...` alias；当前已支持的 tuple / fixed-array / plain-struct literal 子集、能经由 struct field / tuple index / fixed-array index 回收到同一条 foldable source 的 computed/projected item value，以及由 foldable bool condition 或最小 literal `match` arm 选择出的 branch-selected item value，都会复用同一条 const-evaluation lowering；当前 const `match` 只折叠 `Bool` / `Int` literal-or-path pattern + catch-all 与可折叠 bool guard 子集
- 普通表达式、`if` / `while` 条件里的 `Bool` 短路 `&&` / `||`
- 最小 literal `match` lowering：`Bool` scrutinee 仅支持 unguarded `true` / `false` / same-file foldable `const` / `static`-backed bare path pattern（含 same-file local import alias）+ `_` 或单名 binding catch-all arm，并额外接受 literal `if true` / `if false` guard、same-file foldable `const` / `static`-backed `Bool` guard 与其 same-file `use ... as ...` 别名；这条 `Bool` const folding 现也覆盖由当前支持的 `!`、`&&`、`||`、`==`、`!=` 与整数比较子集构成的 computed same-file foldable `const` / `static` `Bool` expression；当 guard 恰好就是当前 `Bool` scrutinee 自身、其一层 unary `!`，或“当前 scrutinee 与可折叠 `Bool` literal/const/static/alias 做 `==` / `!=` 比较”时，literal `true` / `false` arm 现也会在 lowering 前直接折叠成 always/skip，从而收回 `match flag { true if flag => ...; false => ... }` 与 `match flag { true if flag == ON => ...; false => ... }` 这类此前误判成“缺少 fallback”的最小 ordered 子集；同一条 direct bool-valued guard 子集现在也不再额外要求 later arm 提供 guaranteed fallback coverage，因此 `match flag { true if enabled => ...; false => ... }` 这类 partial dynamic guard 也能直接进入 lowering；包裹当前 bool guard 子集的一层 unary `!`、由当前 bool guard 子集继续组合出的 runtime `&&` / `||`、当前 arm 单名 binding catch-all 变量作为 direct scalar operand 参与 guard，也可作为 fixed-array 只读投影里的 dynamic index operand 参与 guard，并且现在也可直接作为只读 struct-field / tuple literal-index / fixed-array index projection root 参与 guard；fixed-array 只读 guard projection 的 index 现也接受当前 runtime `Int` scalar 子集，不只用于 local / parameter / `self` / 当前 arm binding root，也用于 same-file `const` / `static` aggregate root 及其 same-file `use ... as ...` alias，例如 `values[index + 1]`、`values[current + 1]`、`VALUES[index + 1]` 与 `INPUT[state.offset]`，但 tuple index 仍要求 foldable constant；当前简单 scalar comparison guard 子集（`Bool` `==` / `!=`，以及由 integer literal、unary-negated supported `Int` operand、same-file foldable `const` / `static`-backed `Int` 与其 same-file `use ... as ...` 别名、由当前支持的 `Int` operand 继续递归组成的最小整数算术表达式（`+` / `-` / `*` / `/` / `%`）、以 local / parameter / `self` 或当前 arm 单名 binding 为根的 read-only scalar projection operand、same-file `const` / `static` aggregate root 上沿当前 runtime `Int` index 子集的 fixed-array projection operand、以及能经由 struct field / tuple literal-index / fixed-array index 折叠成 scalar 的 same-file foldable `const` / `static`-backed aggregate projection 及其 same-file `use ... as ...` 别名组成的 `Int` `==` / `!=` / `>` / `>=` / `<` / `<=`；其中 tuple / fixed-array index operand 现也接受同一条可折叠的 same-file const/static-backed `Int` arithmetic 子集），以及 direct bool-valued guard 子集（same-scope `Bool` local / parameter、以 local / parameter / `self` 或当前 arm 单名 binding 为根的 read-only bool scalar projection、以及能折叠成 `Bool` 的 same-file foldable `const` / `static`-backed aggregate projection 及其 same-file `use ... as ...` 别名）；`Int` scrutinee 仅支持 unguarded integer literal / unary-negated integer literal / same-file foldable `const` / `static`-backed bare path pattern（含 same-file local import alias）+ `_` 或单名 binding catch-all arm，并额外接受同一组 literal / const/static-backed / scalar-comparison guard 子集，以及 integer-literal arm 或 guarded catch-all arm 上的同一组 direct bool-valued guard 子集；对于其他当前可加载 scrutinee，backend 现在也开放保守的 catch-all-only 子集：`_` / 单名 binding catch-all arm，以及这些 catch-all arm 上的同一组 bool guard 子集；这条 integer direct-bool dynamic guard 现在也不再额外要求 later unguarded catch-all fallback，因此 `match value { 1 if enabled => ...; 2 => ... }` 这类 partial dynamic guard 也能直接进入 lowering；不能折叠成 `Bool`/`Int` 的 bare path pattern 仍显式拒绝
- 上述 `match` guard 标量子集现在也包含 direct resolved sync guard calls：返回 `Bool` 的 direct/local-import-alias 调用可直接作为 guard，返回 `Int` 的同类调用可参与当前 `Int` 标量比较与命名实参重排子集；同一条 direct/sync 路径也已开放 loadable tuple / non-generic struct / fixed-array 返回值作为只读 projection root，以及 local / current-binding / item-root / projection-root / call-root / inline aggregate 组成的当前聚合实参子集；async guard call 与更广的调用表达式仍未开放。
- 保守的 async `staticlib` 子集：async library body、当前最小 `match` lowering、scalar/tuple/array/struct/void `await`、task-handle-aware `spawn`、以及 fixed-shape iterable 的 `for await`（当前覆盖 fixed-array 与 homogeneous tuple）
- 最小 async `dylib` 子集：在仍通过同步 `extern "c"` 顶层导出暴露公开 ABI 时，内部 async helper / 当前最小 `match` lowering / `await` / 已支持的 task-handle lowering 也可进入 library build，并对 fixed-shape iterable 打开同一条 `for await` lowering
- `crates/ql-cli/tests/codegen.rs` 现已把 `fixtures/codegen/pass/async_library_for_await_tuple.ql` 与 `fixtures/codegen/pass/ffi_export_async_for_await_tuple.ql` 收进 `staticlib` / `dylib` pass matrix，显式锁住 homogeneous tuple `for await` 在两条 library build surface 上都会稳定通过；此前遗留的 `unsupported_async_for_await_library_build` / `unsupported_async_for_await_dylib_build` 旧失败合同已删除
- `crates/ql-cli/tests/codegen.rs` 现也已把 `fixtures/codegen/pass/async_library_task_array_for_await.ql` 与 `fixtures/codegen/pass/ffi_export_async_task_tuple_for_await.ql` 收进 `staticlib` / `dylib` pass matrix，显式锁住 direct task-array auto-await `for await` 与 direct task-tuple auto-await `for await` 在当前两条 library build surface 上都会稳定通过
- `crates/ql-cli/tests/codegen.rs` 现进一步补齐 direct task-backed `for await` 的 library 对称矩阵：`fixtures/codegen/pass/async_library_task_tuple_for_await.ql` 与 `fixtures/codegen/pass/ffi_export_async_task_array_for_await.ql` 也已分别进入 `staticlib` / `dylib` pass matrix，因此当前 direct task-array 与 direct task-tuple auto-await `for await` 都已在两条 library build surface 上有显式 contract
- `crates/ql-cli/tests/codegen.rs` 现也已把 aggregate param/result family 锁进两条 library surface：新增 `fixtures/codegen/pass/async_library_aggregate_param_result_families.ql` 与 `fixtures/codegen/pass/ffi_export_async_aggregate_param_result_families.ql`，并用这对 family fixture 合并掉此前分散的 `async_library_recursive_aggregate_params.ql`、`async_library_zero_sized_aggregate_results.ql`、`async_library_zero_sized_aggregate_params.ql` 三条 `staticlib` only case；当前显式覆盖 recursive aggregate 参数、zero-sized array/struct result，以及 zero-sized aggregate 参数在 `staticlib` / `dylib` 上的对称成功构建合同
- `crates/ql-cli/tests/codegen.rs` 现也已把 async library `match` family 锁进两条 library surface：`fixtures/codegen/pass/async_library_match_families.ql` 与 `fixtures/codegen/pass/ffi_export_async_match_families.ql` 现已在同一对 fixture 里显式覆盖 awaited scalar scrutinee + direct-call guard、awaited aggregate scrutinee + projection guard、awaited aggregate scrutinee + aggregate guard-call arg / call-backed aggregate guard-call arg、same-file import-alias rooted awaited helper / guard helper variants、awaited scrutinee + item/import-alias-backed inline guard combos、awaited scrutinee + direct call-backed guard combos、awaited scrutinee + inline aggregate guard-call arg combos、awaited scrutinee + inline projection-root guard combos、awaited scalar scrutinee + nested call-root runtime projection guard variants、awaited scalar scrutinee + nested call-root inline projection/inline aggregate guard-call combos、awaited scrutinee + item-backed nested call-root guard variants、awaited scrutinee + call-backed nested call-root guard variants、awaited scrutinee + alias-backed nested call-root guard variants，以及 awaited aggregate current-binding / read-only projection-root backed nested call-root guard variants；因此当前 library-mode async 子集已不再只公开 `await` / `spawn` / `for await`，也已公开既有最小 `match` lowering 在 async helper body 内的更完整稳定构建合同
- `crates/ql-cli/tests/codegen.rs` 现也已把 aggregate await / projected spawn family 锁进两条 library surface：`fixtures/codegen/pass/async_library_aggregate_await_families.ql` 与 `fixtures/codegen/pass/ffi_export_async_aggregate_await_families.ql` 已在同一对 fixture 里显式覆盖 scalar/tuple/struct/array/nested aggregate `await`，tuple projection `await|spawn`，zero-sized task-array projection `await`，以及 struct-field projection `await|spawn`；此前分散的 `async_library_scalar_await.ql`、`async_library_tuple_await.ql`、`async_library_struct_await.ql`、`async_library_array_await.ql`、`async_library_nested_aggregate_await.ql`、`async_library_projected_tuple_await.ql`、`async_library_projected_tuple_spawn.ql`、`async_library_projected_array_await.ql`、`async_library_projected_struct_field_await.ql`、`async_library_projected_struct_field_spawn.ql` 这组 `staticlib` only case 现已被 family 吸收删除，因此当前 library-mode async 子集已不再把这些 aggregate await / projected submit 路径留在离散单点合同里
- `crates/ql-cli/tests/codegen.rs` 现也已把 aggregate-carried / nested task-handle payload family 锁进两条 library surface：`fixtures/codegen/pass/async_library_task_handle_payload_families.ql` 与 `fixtures/codegen/pass/ffi_export_async_task_handle_payload_families.ql` 已在同一对 fixture 里显式覆盖 async helper 返回 task-array payload、task-tuple payload、含 task field 的 nested aggregate payload，以及 `Task[Wrap]` result 的 chained `await`；此前单点的 `async_library_task_handle_tuple_payload.ql`、`async_library_task_handle_array_payload.ql`、`async_library_nested_aggregate_task_handle_payload.ql` 与更早的 `async_library_nested_task_handle.ql` 也已被 family 吸收删除，因此当前 library-mode async 子集已不再只锁定 task-handle flow 与 projection consume，也锁住了 aggregate-carried payload contract
- `crates/ql-cli/tests/codegen.rs` 现也已把 direct/helper task-handle flow family 锁进两条 library surface：`fixtures/codegen/pass/async_library_task_handle_flow_families.ql` 与 `fixtures/codegen/pass/ffi_export_async_task_handle_flow_families.ql` 已在同一对 fixture 里显式覆盖 direct call `await`、direct local-handle `await`、helper-returned handle `await`、helper-bound handle `await`、local-return handle `await`、forwarded handle `await`、direct call `spawn`、direct-bound `spawn`、helper `spawn`、helper-bound `spawn` 与 `spawn forward(task)`，并同时覆盖 `Task[Int]` 与 `Task[Wrap]`；此前零散的 `async_library_task_handle.ql`、`async_library_bound_task_handle.ql`、`async_library_local_return_task_handle.ql`、`async_library_zero_sized_task_handle.ql`、`async_library_bound_zero_sized_task_handle.ql`、`async_library_local_return_zero_sized_task_handle.ql`、`async_library_forward_task_handle.ql`、`async_library_forward_zero_sized_task_handle.ql`、`async_library_direct_handle.ql`，以及对应的 `spawn_*` 单点 fixture 也已被 family 吸收删除，因此当前 library-mode async 子集已不再只锁定 projection consume，也锁住了最常见的 task-handle 流转 contract
- `crates/ql-cli/tests/codegen.rs` 现也已把 stable-dynamic task-handle 成功路径锁进两条 library surface：`fixtures/codegen/pass/async_library_dynamic_task_handle_paths.ql` 与 `fixtures/codegen/pass/ffi_export_async_dynamic_task_handle_paths.ql` 已在同一对 fixture 里显式覆盖 projected-root dynamic reinit、aliased projected-root dynamic reinit、composed dynamic reinit、alias-sourced composed dynamic reinit、guard-refined literal reinit、generic dynamic array assignment，以及 dynamic sibling-safe `spawn` + fallback consume；此前零散的 `async_library_dynamic_task_handle_array_assignment.ql`、`async_library_dynamic_task_handle_spawn_sibling.ql`、`async_library_composed_dynamic_task_handle_reinit.ql` 与 `async_library_alias_sourced_composed_dynamic_task_handle_reinit.ql` 也已被 family 吸收删除，因此当前 library-mode async 子集已不再只锁定 literal/projection consume，也锁住了更完整的稳定动态路径成功 contract
- `crates/ql-cli/tests/codegen.rs` 现也已把 aliased projected-root repackage family 锁进两条 library surface：新增 `fixtures/codegen/pass/async_library_aliased_projected_root_repackage_families.ql` 与 `fixtures/codegen/pass/ffi_export_async_aliased_projected_root_repackage_families.ql`，在同一对 fixture 里显式覆盖 source-root reinit 之后经 tuple、struct 与 nested aggregate 重新包装的 task-handle `await` 路径；此前零散的 `async_library_aliased_projected_root_task_handle_tuple_repackage_reinit.ql` 与已被 dynamic family 吸收的 `async_library_aliased_projected_root_dynamic_task_handle_reinit.ql` 也已删除，因此当前 library-mode async 子集已不再只在 program-mode 合同里承载 alias-root aggregate repackage
- `crates/ql-cli/tests/codegen.rs` 现也已把 aliased projected-root spawn family 锁进两条 library surface：新增 `fixtures/codegen/pass/async_library_aliased_projected_root_spawn_families.ql` 与 `fixtures/codegen/pass/ffi_export_async_aliased_projected_root_spawn_families.ql`，在同一对 fixture 里显式覆盖 source-root reinit 之后经 nested struct aggregate、fixed-array aggregate、nested fixed-array aggregate，以及 helper-forwarded nested fixed-array aggregate 重包装后再 `spawn -> await running` 的路径，因此当前 library-mode async 子集也已不再只在 program-mode 合同里承载 alias-root aggregate submit
- 对应的 program-mode public surface 本轮也继续补上 same-file arithmetic item / same-file `use ... as ...` alias 变体：`199_async_main_aliased_projected_root_repackage_families.ql` 现已显式覆盖 `alias[ARITH_INDEX_ALIAS]`、`alias[ARITH_SLOT_ALIAS.value]`，以及 `alias[alias_slots[row]]` 这类 arithmetic alias-sourced composed-dynamic 路径在普通与 guard-refined 两种形态下进入 tuple / nested aggregate repackage 后再 `await` 的路径，并继续推进到 guarded bundle-alias-forwarded、bundle-alias-inline-forwarded、queued-root-forwarded、queued-root-inline-forwarded、queued-root-alias-forwarded、queued-root-alias-inline-forwarded、queued-root-chain-forwarded、queued-root-chain-inline-forwarded、queued-local-alias、queued-local-chain、queued-local-forwarded、queued-local-inline-forwarded、bundle-chain-forwarded 与 bundle-chain-inline-forwarded await；`200_async_main_aliased_projected_root_spawn_families.ql` 则继续覆盖同一批 arithmetic alias-root / projected-root / alias-sourced composed-dynamic 路径在普通与 guard-refined 两种形态下进入 fixed-array / helper-forwarded nested fixed-array repackage 后再 `spawn -> await` 的路径，并继续推进到 guarded bundle-alias-forwarded、bundle-alias-inline-forwarded、queued-root-forwarded、queued-root-inline-forwarded、queued-root-alias-forwarded、queued-root-alias-inline-forwarded、queued-root-chain-forwarded、queued-root-chain-inline-forwarded、queued-local-alias、queued-local-chain、queued-local-forwarded、queued-local-inline-forwarded、bundle-chain-forwarded 与 bundle-chain-inline-forwarded spawn
- program-mode public surface 现也单独补上 guarded arithmetic forwarded helper flow：`225_async_main_guarded_arithmetic_forwarded_task_handle_flows.ql` 显式覆盖 same-file `use ... as ...` alias 包裹的 arithmetic static source 在 guard-refined alias-sourced composed-dynamic path 上先经 `forward(...)` 再 direct `await`、以及经 direct queued `spawn -> await` 的路径
- `crates/ql-cli/tests/codegen.rs` 现也已把 guard-refined dynamic path family 锁进两条 library surface：新增 `fixtures/codegen/pass/async_library_guard_refined_dynamic_path_families.ql` 与 `fixtures/codegen/pass/ffi_export_async_guard_refined_dynamic_path_families.ql`，在同一对 fixture 里显式覆盖 direct dynamic guard refine、projected dynamic guard refine、aliased projected-root guard refine、const-backed alias-root guard refine，以及 static-alias-backed alias-root guard refine；此前零散的 `async_library_guard_refined_dynamic_task_handle_reinit.ql` 已被 family 吸收删除，因此当前 library-mode async 子集已不再只靠单点 `staticlib` case 承载 equality-guard refined dynamic path
- `crates/ql-cli/tests/codegen.rs` 现也已把 projected reinit family 锁进两条 library surface：新增 `fixtures/codegen/pass/async_library_projected_reinit_families.ql` 与 `fixtures/codegen/pass/ffi_export_async_projected_reinit_families.ql`，在同一对 fixture 里显式覆盖 fixed-array literal projected reinit、conditional literal reinit、stable projected dynamic reinit、conditional projected dynamic reinit，以及 projected-root dynamic reinit；此前零散的 `async_library_projected_array_reinit.ql`、`async_library_projected_array_conditional_reinit.ql`、`async_library_projected_dynamic_array_reinit.ql`、`async_library_projected_dynamic_array_conditional_reinit.ql` 与 `async_library_projected_root_dynamic_task_handle_reinit.ql` 也已被 family 吸收删除，因此当前 library-mode async 子集已不再把 projected reinit 路径留在 `staticlib` only 合同里
- `crates/ql-cli/tests/codegen.rs` 现也已把 projected task-handle consume / submit 的主干 family 一次锁进两条 library surface：`fixtures/codegen/pass/async_library_task_handle_consume_families.ql` 与 `fixtures/codegen/pass/ffi_export_async_task_handle_consume_families.ql` 已在同一对 fixture 里显式覆盖 direct call-root、same-file import-alias call-root、direct nested call-root、same-file import-alias nested call-root、direct awaited-aggregate、same-file import-alias awaited-aggregate、direct inline aggregate 与 same-file import-alias inline aggregate 上的 projected task-handle `await` / `spawn`，因此当前 library-mode async 子集已不再只停留在 local projection consume
- `crates/ql-cli/tests/codegen.rs` 现也已把 unparenthesized inline aggregate projected fixed-shape `for await` 锁进两条 library surface：`fixtures/codegen/pass/async_library_inline_without_parens_for_await.ql` 与 `fixtures/codegen/pass/ffi_export_async_inline_without_parens_for_await.ql` 已在同一对 fixture 里显式覆盖 direct unparenthesized inline projected scalar array/tuple、direct unparenthesized inline projected task tuple 与 deeper projected task-array root，以及同一批路径在 same-file import alias helper 参与下的 alias-backed inline variant，因此当前 library-mode `for await` 已能直接消费无需额外包裹括号的 inline aggregate iterable head
- `crates/ql-cli/tests/codegen.rs` 现也已把 parenthesized inline aggregate projected fixed-shape `for await` 锁进两条 library surface：`fixtures/codegen/pass/async_library_inline_for_await.ql` 与 `fixtures/codegen/pass/ffi_export_async_inline_for_await.ql` 已在同一对 fixture 里显式覆盖 direct inline projected scalar array/tuple、direct inline projected task tuple 与 deeper projected task-array root，以及同一批路径在 same-file import alias helper 参与下的 alias-backed inline variant，因此当前 library-mode `for await` 已能直接消费 parenthesized inline aggregate iterable head
- `crates/ql-cli/tests/codegen.rs` 现也已把 same-file import-alias awaited-aggregate fixed-shape `for await` 锁进两条 library surface：`fixtures/codegen/pass/async_library_import_alias_awaited_for_await.ql` 与 `fixtures/codegen/pass/ffi_export_async_import_alias_awaited_for_await.ql` 已显式覆盖 import-aliased awaited projected scalar array/tuple root 与 import-aliased awaited projected task tuple/array root，因此当前 library-mode `for await` 已能直接消费由 same-file import alias 包裹的 async helper aggregate root
- `crates/ql-cli/tests/codegen.rs` 现也已把 same-file import-alias fixed-shape `for await` 锁进两条 library surface：`fixtures/codegen/pass/async_library_import_alias_for_await.ql` 与 `fixtures/codegen/pass/ffi_export_async_import_alias_for_await.ql` 已在同一对 fixture 里显式覆盖 import-aliased direct call-root scalar array/tuple、import-aliased direct task tuple 与 projected task-array root，以及 import-aliased nested call-root scalar array/tuple、nested task tuple 与 deeper projected task-array root，因此当前 library-mode `for await` 已能直接消费 same-file import alias 包裹的 call-root / nested call-root iterable expression
- `crates/ql-cli/tests/codegen.rs` 现也已把 nested call-root projected fixed-shape `for await` 锁进两条 library surface：`fixtures/codegen/pass/async_library_nested_call_root_for_await.ql` 与 `fixtures/codegen/pass/ffi_export_async_nested_call_root_for_await.ql` 已显式覆盖 nested call-root scalar array/tuple 与 deeper projected task-array root，因此当前 library-mode `for await` 已能直接消费嵌套 call-root aggregate projection
- `crates/ql-cli/tests/codegen.rs` 现也已把 awaited-aggregate projected fixed-shape `for await` 锁进两条 library surface：`fixtures/codegen/pass/async_library_awaited_projected_for_await.ql` 与 `fixtures/codegen/pass/ffi_export_async_awaited_projected_for_await.ql` 已显式覆盖 awaited projected scalar array/tuple root 与 awaited projected task tuple/array root，因此当前 library-mode `for await` 已能直接消费当前受支持 async helper 返回的 aggregate root
- `crates/ql-cli/tests/codegen.rs` 现也已把 direct call-root fixed-shape `for await` 锁进两条 library surface：`fixtures/codegen/pass/async_library_call_root_for_await.ql` 与 `fixtures/codegen/pass/ffi_export_async_call_root_for_await.ql` 已显式覆盖 direct call-root scalar array/tuple 与 direct call-root task array/tuple，因此当前 library-mode `for await` 已从 local/direct root 扩到 direct call-root iterable expression
- `crates/ql-cli/tests/codegen.rs` 现继续把 projected fixed-shape `for await` 的 library 合同补齐到两条公开 surface：`fixtures/codegen/pass/async_library_projected_for_await.ql` 与 `fixtures/codegen/pass/ffi_export_async_projected_for_await.ql` 已分别锁住 projected scalar array/tuple root 与 projected task array/tuple root 在 `staticlib` / `dylib` 下都会稳定通过，因此当前 library-mode `for await` 已不再局限于 direct iterable root
- 最小 async executable 子集：`BuildEmit::Executable` 下的 `async fn main`、已接入的 task-handle / aggregate payload lowering，以及 fixed-shape iterable 的 `for await`
- sync/async `unsafe fn` body 现也会沿当前普通 function / `async fn main` lowering 路径继续进入 executable build surface，而不再被 LLVM backend 预先拒绝
- projected task-handle operand：tuple index / fixed-array literal index / struct-field 只读投影，也包括它们的递归嵌套组合，以及 direct call-root / nested call-root consume（例如 `await pair[0]`、`await tasks[0]`、`spawn pair.task`、`await pending[0].task`、`await tuple_tasks(10)[0]`、`spawn pair_tasks(11).left`、`await bundle_tasks(20).tasks[0]`）
- sync/async executable surface 现也显式锁住 ordinary assignment-expression 子集：mutable local、tuple literal-index、same-file `const` / `static` / `use ... as ...` alias 驱动并支持 immutable direct local alias 复用的 foldable integer constant expression tuple index、struct-field 与 fixed-array literal-index assignment 继续走既有 statement lowering，而且当前本地 executable smoke contract 也已继续推进到 projected-root / nested projected-root / call-root nested projected-root / import-alias call-root nested projected-root / inline nested projected-root tuple / struct-field / fixed-array literal-index 链式写入，让“赋值表达式结果值可继续参与后续普通标量计算”这条用户面不只停留在 direct-root 形态
- executable surface 现也显式锁住 dynamic array assignment 子集：sync program mode 已公开 non-`Task[...]` element dynamic array / nested dynamic array assignment，并继续推进到 projected-root dynamic array projection；同一条 sync surface 现在也已公开这些 dynamic array write 的 assignment-expression result form，并继续推进到 nested projected-root、call-root nested projected-root、import-alias call-root nested projected-root 与 inline nested projected-root 组合。async `fn main` 也已公开 direct-root `Task[...]` dynamic array write-before-consume success path，并继续推进到 projected-root write-before-consume success path；同时普通非 `Task[...]` 标量数组写入也已推进到 async executable surface，覆盖 direct-root、projected-root、call-root nested projected-root、import-alias call-root nested projected-root 与 inline nested projected-root 这几条 runtime-index path，并补上 assignment-expression result form
- dynamic fixed-array `Task[...]` 子集：generic dynamic path 继续保持 sibling-safe consume/spawn 与 maybe-overlap write/reinit，而 same immutable stable source path 会获得 precise consume/reinit；这条稳定化路径现在也已覆盖 same-file `const` / `static` item 及其 same-file `use ... as ...` alias，因此 `tasks[index]`、`tasks[slot.value]`、`tasks[INDEX]`、`tasks[STATIC_INDEX]`、`tasks[INDEX_ALIAS.value]` 都会尽量回收到同一条 stable 或 literal/projection path，而不是无差别退化成 generic dynamic overlap
- 同一条 dynamic fixed-array `Task[...]` 稳定化路径现在也接受 foldable integer arithmetic expression，因此 `tasks[1 - 1]`、`tasks[slot.value + 0]` 与 `tasks[ARITH_INDEX]` 这类 consume/reinit 也会折回 concrete literal/projection lifecycle
- 同一条 dynamic fixed-array `Task[...]` 稳定化路径现在也接受 direct inline foldable `if` / 最小 literal `match` integer expression，因此 `tasks[if true { 0 } else { 1 }]` 与 `tasks[match 1 { 1 => 0, _ => 1 }]` 这类 consume/reinit 会直接折回 concrete literal lifecycle
- 同一条 dynamic fixed-array `Task[...]` 稳定化路径现在也接受 branch-selected `const` / `static` item value，因此 item 里经由 foldable `if` / 最小 literal `match` 选出的整数值或聚合投影值，也能在 `tasks[SELECTED]` 与 `tasks[SELECTED_SLOT.value]` 这类 consume/reinit 上折回 concrete literal/projection lifecycle
- 上述 branch-selected item-value 路径现在也已显式锁进 same-file `use ... as ...` alias public surface，因此 `tasks[SELECTED_INDEX_ALIAS]` 与 `tasks[SELECTED_SLOT_ALIAS.value]` 这类 executable consume/reinit 不再只依赖内部 canonicalization 假设
- 同一条 arithmetic-backed item-value 路径现在也已显式锁进 same-file `use ... as ...` alias public surface，因此 `tasks[ARITH_INDEX_ALIAS]` 与 `tasks[ARITH_SLOT_ALIAS.value]` 这类 executable consume/reinit 也不再只依赖内部 canonicalization 假设
- 同一条 equality-guard refinement 现在也接受 arithmetic-backed refined source，以及这些 source 的 same-file `use ... as ...` alias 包裹形态，因此 `if ARITH_INDEX == 0 { await tasks[ARITH_INDEX]; tasks[0] = ... }`、`if ARITH_INDEX_ALIAS == 0 { await tasks[ARITH_INDEX_ALIAS]; tasks[0] = ... }` 与 `if ARITH_SLOT_ALIAS.value == 0 { await alias[ARITH_SLOT_ALIAS.value]; pending.tasks[0] = ... }` 这类 guarded consume/reinit 也会回收到 concrete literal/projection lifecycle
- arithmetic / compare / `Bool` unary `!` / short-circuit `&&` / `||` / branch / return
- `.ll` 文本产物始终可用
- `.obj` / `.o` / 基础 `.exe` 产物依赖 clang-style compiler
- `.dll` / `.so` / `.dylib` 产物依赖 clang-style compiler
- `.lib` / `.a` 产物依赖 clang-style compiler 与 archive tool
- codegen 会在 program mode 下把 Qlang 用户入口 lower 成内部符号，并额外生成宿主 `main` wrapper
- `dylib` 和 `staticlib` 都走 library mode，因此当前单文件库不要求顶层 `main`
- async public build 当前已开放两类受控子集：`staticlib` 与 `dylib` 都支持已接入 backend 的 async library body，并为 fixed-shape iterable 打开 `for await` lowering；`BuildEmit::LlvmIr` / `BuildEmit::Object` / `BuildEmit::Executable` 也已开放 `async fn main` 的最小程序入口生命周期与 fixed-shape iterable `for await` 子集。其中 `dylib` 仍只开放不暴露 async ABI 的最小 library-style 子集；更广义的 async program/bootstrap surface，以及更广义的 dynamic/generalized iterable 仍保持保守拒绝
- `crates/ql-cli/tests/executable_examples.rs` 当前编码了最小 async executable program subset 的 smoke contract；当前仓库已经提交 `ramdon_tests/async_program_surface_examples/` 基线目录，并会按当前注册的 `222` 个 async case 执行
- 下文若继续引用 `ramdon_tests/...` 文件名，它们指向当前仓库已提交的 smoke corpus；该目录仍在 `.gitignore` 中，因此开发者本地也可继续追加样例
- `174_async_main_spawned_aggregate_results.ql` 到 `225_async_main_guarded_arithmetic_forwarded_task_handle_flows.ql` 现进一步把 regular-size / spawned / zero-sized / recursive aggregate result family、regular-size / zero-sized helper-returned / forwarded task-handle flow、regular-size tuple / array / nested aggregate task-handle payload family、regular-size / zero-sized / recursive aggregate param family、zero-sized projected task-handle `await` / `spawn` consume、zero-sized projected reinit、zero-sized conditional `spawn/await/reinit` family、regular-size conditional task-handle flow、bound local task-handle `spawn` flow、regular-size returned / nested / struct-carried task-handle shape family、regular-size projected reinit family、stable-dynamic path family、guard-refined dynamic path family、static-alias-backed projected-root dynamic reinit family、aliased projected-root repackage/spawn family、guarded arithmetic forwarded helper flow surface、`async unsafe fn` body surface、ordinary assignment-expression surface、direct-root dynamic task-array assignment surface、tuple literal-index assignment-expression surface、projected-root / nested projected-root / call-root nested projected-root / import-alias call-root nested projected-root / inline nested projected-root tuple / struct-field / fixed-array literal-index assignment-expression surface、projected-root dynamic task-array assignment surface、async mutable-local assignment-expression surface、async scalar dynamic non-`Task[...]` array assignment surface，以及 direct-root / projected-root / nested projected-root / call-root nested projected-root / import-alias call-root nested projected-root / inline nested projected-root dynamic array assignment-expression surface 推进到 `async fn main` 的真实 build-and-run surface
- 其中 `ramdon_tests/async_program_surface_examples/107_async_main_import_alias_named_calls.ql`、`108_async_main_import_alias_direct_submit.ql`、`109_async_main_import_alias_aggregate_submit.ql`、`110_async_main_import_alias_array_submit.ql`、`111_async_main_import_alias_tuple_submit.ql`、`112_async_main_import_alias_forward_submit.ql`、`113_async_main_import_alias_helper_task_submit.ql`、`114_async_main_import_alias_helper_forward_submit.ql`、`121_async_main_import_alias_awaited_aggregate_task_array_for_await.ql`、`127_async_main_import_alias_helper_task_tuple_for_await.ql`、`131_async_main_import_alias_awaited_aggregate_task_tuple_for_await.ql` 与 `135_async_main_import_alias_task_tuple_for_await.ql` 进一步把 same-file function import alias call lowering 推进到 `async fn main` 的真实 build-and-run surface，覆盖 alias-root direct call、direct `await`、direct `spawn`、aggregate-carried、fixed-array-carried、tuple-carried、helper-forwarded、helper-returned task-handle、helper-returned-task + helper-forwarded submit，以及 direct/helper-returned/awaited-aggregate/inline-awaited fixed-array task-array `for await`、direct/helper-returned homogeneous task-tuple `for await`、以及 import-aliased awaited-aggregate task-tuple `for await` 中对每个元素自动 `await` 后再绑定循环变量的组合路径；`115_async_main_import_alias_task_array_for_await.ql`、`116_async_main_import_alias_helper_task_array_for_await.ql`、`117_async_main_projected_task_array_for_await.ql`、`118_async_main_void_task_array_for_await.ql`、`119_async_main_awaited_aggregate_task_array_for_await.ql`、`120_async_main_awaited_nested_aggregate_task_array_for_await.ql`、`122_async_main_inline_awaited_task_array_for_await.ql`、`123_async_main_awaited_tuple_projected_task_array_for_await.ql`、`124_async_main_awaited_array_projected_task_array_for_await.ql`、`125_async_main_tuple_for_await.ql`、`126_async_main_task_tuple_for_await.ql`、`128_async_main_projected_task_tuple_for_await.ql`、`129_async_main_awaited_aggregate_task_tuple_for_await.ql`、`130_async_main_awaited_tuple_projected_task_tuple_for_await.ql`、`132_async_main_inline_awaited_task_tuple_for_await.ql`、`133_async_main_awaited_nested_aggregate_task_tuple_for_await.ql`、`134_async_main_awaited_array_projected_task_tuple_for_await.ql`、`135_async_main_import_alias_task_tuple_for_await.ql` 与 `136_async_main_void_task_tuple_for_await.ql` 还分别锁住了 helper-returned fixed-array task root、projected fixed-array task root、`Task[Void]` wildcard、awaited aggregate task root、awaited nested aggregate task root、inline-awaited fixed-array task root、awaited tuple-projected task root、awaited array-projected task root、homogeneous tuple iterable root、task-tuple auto-await iterable root、projected task-tuple root、awaited aggregate task-tuple root、awaited tuple-projected task-tuple root、inline-awaited task-tuple root、awaited nested aggregate task-tuple root、awaited array-projected task-tuple root、direct import-alias task-tuple root，以及 `Task[Void]` task-tuple wildcard 的 executable public surface
- 其中 `137_async_main_const_tuple_for_await.ql`、`138_async_main_static_array_for_await.ql`、`139_async_main_import_alias_projected_const_tuple_for_await.ql`、`140_async_main_import_alias_projected_static_array_for_await.ql`、`141_async_main_import_alias_const_tuple_for_await.ql` 与 `142_async_main_import_alias_static_array_for_await.ql` 进一步把 same-file `const` / `static` fixed-shape root 及其 same-file import-alias projected/direct root 的 scalar `for await` lowering 推进到 `async fn main` 的真实 build-and-run surface；这条 public slice 现在已经显式覆盖 direct const tuple root、direct static array root、import-aliased projected const tuple root、import-aliased projected static array root、import-aliased direct const tuple root，以及 import-aliased direct static array root
- `143_async_main_call_root_projected_task_handle_consumes.ql` 则进一步把 direct call-root / projected call-root task-handle consume 推进到 `async fn main` 的真实 build-and-run surface，显式覆盖 direct call-root tuple projection await、direct call-root struct-field projection spawn，以及 direct projected call-root fixed-array projection await/spawn
- `144_async_main_call_root_fixed_shape_for_await.ql` 则继续把 direct call-root / projected call-root fixed-shape iterable `for await` 推进到 `async fn main` 的真实 build-and-run surface，显式覆盖 direct call-root fixed-array scalar iterable、direct call-root homogeneous tuple scalar iterable、direct call-root task-tuple auto-await iterable，以及 projected call-root task-array iterable
- `145_async_main_import_alias_call_root_fixed_shape_for_await.ql` 则继续把 same-file import-alias call-root / projected call-root fixed-shape iterable `for await` 推进到 `async fn main` 的真实 build-and-run surface，显式覆盖 import-aliased direct fixed-array scalar iterable、import-aliased direct homogeneous tuple scalar iterable、import-aliased direct task-tuple auto-await iterable，以及 import-aliased projected task-array iterable
- `146_async_main_nested_call_root_fixed_shape_for_await.ql` 则继续把 nested call-root projected fixed-shape iterable `for await` 推进到 `async fn main` 的真实 build-and-run surface，显式覆盖 nested call-root scalar fixed-array iterable、nested call-root scalar homogeneous tuple iterable、nested call-root task-tuple auto-await iterable，以及更深一层 nested call-root projected task-array iterable
- `147_async_main_import_alias_nested_call_root_fixed_shape_for_await.ql` 则继续把 same-file import-alias nested call-root projected fixed-shape iterable `for await` 推进到 `async fn main` 的真实 build-and-run surface，显式覆盖 import-aliased nested call-root scalar fixed-array iterable、import-aliased nested call-root scalar homogeneous tuple iterable、import-aliased nested call-root task-tuple auto-await iterable，以及 import-aliased deeper projected task-array iterable
- `148_async_main_import_alias_call_root_projected_task_handle_consumes.ql` 则继续把 same-file import-alias call-root projected task-handle consume 推进到 `async fn main` 的真实 build-and-run surface，显式覆盖 import-aliased call-root tuple projection await、import-aliased call-root struct-field projection spawn，以及 import-aliased call-root projected task-array await/spawn
- `149_async_main_import_alias_nested_call_root_projected_task_handle_consumes.ql` 则继续把 same-file import-alias nested call-root projected task-handle consume 推进到 `async fn main` 的真实 build-and-run surface，显式覆盖 import-aliased nested call-root tuple projection await、import-aliased nested call-root struct-field projection spawn，以及 import-aliased deeper projected task-array await/spawn
- `150_async_main_import_alias_awaited_aggregate_projected_task_handle_consumes.ql` 则继续把 same-file import-alias awaited-aggregate projected task-handle consume 推进到 `async fn main` 的真实 build-and-run surface，显式覆盖 import-aliased awaited-aggregate tuple projection await、import-aliased awaited-aggregate struct-field projection spawn，以及 import-aliased awaited-aggregate deeper projected task-array await/spawn
- `151_async_main_awaited_aggregate_projected_task_handle_consumes.ql` 则继续把 direct awaited-aggregate projected task-handle consume 推进到 `async fn main` 的真实 build-and-run surface，显式覆盖 direct awaited-aggregate tuple projection await、direct awaited-aggregate struct-field projection spawn，以及 direct awaited-aggregate deeper projected task-array await/spawn
- `152_async_main_inline_projected_fixed_shape_for_await.ql` 则继续把带括号的 inline aggregate projected fixed-shape `for await` 推进到 `async fn main` 的真实 build-and-run surface，显式覆盖 inline projected fixed-array scalar iterable、inline projected homogeneous tuple scalar iterable、inline projected task-tuple auto-await iterable，以及更深一层 inline projected task-array iterable
- `153_async_main_inline_projected_task_handle_consumes.ql` 则继续把带括号的 inline aggregate projected task-handle consume 推进到 `async fn main` 的真实 build-and-run surface，显式覆盖 inline projected tuple task-handle await、inline projected struct-field task-handle spawn，以及更深一层 inline projected task-array await/spawn
- `154_async_main_import_alias_inline_projected_task_handle_consumes.ql` 则继续把 same-file import-alias inline aggregate projected task-handle consume 推进到 `async fn main` 的真实 build-and-run surface，显式覆盖 alias-backed inline projected tuple task-handle await、alias-backed inline projected struct-field task-handle spawn，以及 alias-backed inline projected task-array await/spawn
- `155_async_main_import_alias_inline_projected_fixed_shape_for_await.ql` 则继续把 same-file import-alias inline aggregate projected fixed-shape `for await` 推进到 `async fn main` 的真实 build-and-run surface，显式覆盖 alias-backed inline projected fixed-array scalar iterable、alias-backed inline projected homogeneous tuple scalar iterable、alias-backed inline projected task-tuple auto-await iterable，以及 alias-backed inline projected task-array iterable
- `156_async_main_awaited_aggregate_projected_fixed_shape_for_await.ql` 则继续把 awaited-aggregate projected fixed-shape `for await` 推进到 `async fn main` 的真实 build-and-run surface，显式覆盖 awaited projected fixed-array scalar iterable、awaited projected homogeneous tuple scalar iterable、awaited projected task-tuple auto-await iterable，以及 awaited projected task-array iterable
- `157_async_main_import_alias_awaited_aggregate_projected_fixed_shape_for_await.ql` 则继续把 same-file import-alias awaited-aggregate projected fixed-shape `for await` 推进到 `async fn main` 的真实 build-and-run surface，显式覆盖 alias-backed awaited projected fixed-array scalar iterable、alias-backed awaited projected homogeneous tuple scalar iterable、alias-backed awaited projected task-tuple auto-await iterable，以及 alias-backed awaited projected task-array iterable
- `158_async_main_nested_call_root_projected_task_handle_consumes.ql` 则继续把 direct nested call-root projected task-handle consume 推进到 `async fn main` 的真实 build-and-run surface，显式覆盖 direct nested call-root tuple projection await、direct nested call-root struct-field projection spawn，以及 direct deeper projected task-array await/spawn
- `159_async_main_inline_projected_fixed_shape_for_await_without_parens.ql` 则继续把不带括号的 inline aggregate projected fixed-shape `for await` 推进到 `async fn main` 的真实 build-and-run surface，显式覆盖 unparenthesized inline projected fixed-array scalar iterable、unparenthesized inline projected homogeneous tuple scalar iterable、unparenthesized inline projected task-tuple auto-await iterable，以及 unparenthesized inline projected task-array iterable
- `160_async_main_awaited_match_guards.ql` 到 `173_async_main_awaited_match_nested_call_root_inline_combos.ql` 则继续把 awaited `match` guard surface 推进到 `async fn main` 的真实 build-and-run surface，显式覆盖 awaited scalar scrutinee + direct-call guard、awaited aggregate scrutinee + projection guard、awaited aggregate scrutinee + aggregate guard-call arg / call-backed aggregate guard-call arg、same-file import-alias rooted awaited helper / guard helper 组合、awaited scrutinee + item/import-alias-backed inline guard 组合、awaited scrutinee + direct call-backed guard 组合、awaited scrutinee + inline aggregate guard-call arg 组合、awaited scrutinee + inline projection-root guard 组合、awaited scalar scrutinee + nested call-root runtime projection guard 组合、awaited scalar scrutinee + nested call-root inline projection/inline aggregate guard-call 组合、awaited scrutinee + item-backed / call-backed / alias-backed nested call-root guard 组合，以及 awaited aggregate current-binding / read-only projection-root backed nested call-root guard 组合
- `39_sync_unparenthesized_inline_projected_control_flow_heads.ql` 则继续把不带括号的 inline aggregate projected `if` / `while` / `match` 头推进到真实 sync build-and-run surface，显式覆盖 unparenthesized inline projected bool `if` condition、unparenthesized inline projected bool `while` condition，以及 unparenthesized inline projected `match` scrutinee
- `crates/ql-cli/tests/codegen.rs` 当前也已把 direct resolved sync guard-call `match`、call-projection-root guard `match`、aggregate-call-arg guard `match`、inline-aggregate-call-arg guard `match`、inline-projection-root guard `match`、item/import-alias-backed inline guard combo `match`、call-backed inline guard combo `match`、call-root nested runtime projection `match`、nested-call-root inline combo `match`、item-backed nested-call-root combo `match`、call-backed nested-call-root combo `match`、alias-backed nested-call-root combo `match`、binding-backed nested-call-root combo `match` 与 projection-backed nested-call-root combo `match` 显式锁进 CLI 的 `llvm-ir` / `obj` / `exe` pass matrix：`fixtures/codegen/pass/match_guard_direct_calls.ql` 覆盖 `enabled()` 这类 bool guard call 与 `offset(delta: 2, value: current) == 22` 这类命名实参重排后的 integer guard-call 组合，`fixtures/codegen/pass/match_guard_call_projection_roots.ql` 继续覆盖 `pair(current)[1]`、`state(current).value` 与 `values(current)[1]` 这类 call-root projection guard，`fixtures/codegen/pass/match_guard_aggregate_call_args.ql` 显式锁住 `enabled(current)`、`matches(pair(current), 22)` 与 `contains(values(current), 4)` 这类 loadable aggregate 直接流入 guard-call 参数的路径，`fixtures/codegen/pass/match_guard_inline_aggregate_call_args.ql` 继续把 `enabled(State { ready: true })`、`matches((0, current), 22)` 与 `contains([current, current + 1, current + 2], 4)` 这类 inline aggregate literal 直接作为 guard-call 实参的路径纳入公开 CLI 合同，`fixtures/codegen/pass/match_guard_inline_projection_roots.ql` 则进一步把 `(0, current)[1]`、`State { value: current }.value` 与 `[current, current + 1, current + 2][1]` 这类 inline aggregate literal projection-root guard 纳入公开 CLI 合同，`fixtures/codegen/pass/match_guard_item_backed_inline_combos.ql` 继续把 `enabled(extra: true, state: state)`、`(INPUT[0], current)[1]` 与 `[INPUT[0], current + 1, INPUT[2]][current - 2]` 这类 same-file item/import-alias-backed inline guard 组合也纳入公开 CLI 合同，`fixtures/codegen/pass/match_guard_call_backed_combos.ql` 则继续把 `enabled(extra: ready(true), state: State { ready: ready(true) })`、`matches((seed(0), current), 22)` 与 `items(current)[slot(current)]` 这类 direct sync call-backed inline guard 组合纳入公开 CLI 合同，`fixtures/codegen/pass/match_guard_call_root_nested_runtime_projection.ql` 则继续把 `pack(current).values[offset(current)]`、`ready(pack(current).values[offset(current)])` 与 `check(expected: 4, value: pack(current).values[offset(current)])` 这类 call-root nested runtime projection guard 组合纳入公开 CLI 合同，`fixtures/codegen/pass/match_guard_nested_call_root_inline_combos.ql` 则继续把 `[pack(current)[slot(current)], current + 1, 6][0]`、`contains([pack(3)[slot(3)], current, 9], 4)` 与 `pair(left: pack(current)[slot(current)], right: 8)[0]` 这类 nested-call-root inline aggregate / projection / guard-call 组合纳入公开 CLI 合同，`fixtures/codegen/pass/match_guard_item_backed_nested_call_root_combos.ql` 则继续把 `enabled(extra: INPUT[0] == bundle(3)[offset(3)], state: state(bundle(3)[offset(3)] == 4))`、`[bundle(current)[offset(current)], INPUT[1], INPUT[2]][0]` 与 `check(expected: INPUT[0], value: [bundle(current)[offset(current)], 8, 9][0])` 这类 item-backed nested-call-root guard 组合纳入公开 CLI 合同，`fixtures/codegen/pass/match_guard_call_backed_nested_call_root_combos.ql` 则继续把 `enabled(extra: flag(pack(3)[slot(3)] == 4), state: state(flag(pack(3)[slot(3)] == 4)))`、`[pack(current)[slot(current)], seed(8), seed(9)][0]` 与 `check(expected: seed(4), value: [pack(current)[slot(current)], seed(8), 9][0])` 这类 call-backed nested-call-root guard 组合纳入公开 CLI 合同，`fixtures/codegen/pass/match_guard_alias_backed_nested_call_root_combos.ql` 则继续把 `allow(extra: flag(pack(3)[slot(3)] == literal(4)), state: make(flag(pack(3)[slot(3)] == literal(4))))`、`[pack(current)[slot(current)], literal(8), literal(9)][0]` 与 `check(expected: literal(4), value: [pack(current)[slot(current)], literal(8), 9][0])` 这类 alias-backed nested-call-root guard 组合纳入公开 CLI 合同，`fixtures/codegen/pass/match_guard_binding_backed_nested_call_root_combos.ql` 则继续把 `enabled(extra: bundle(current.value)[offset(current.value)] == 4, state: current)`、`[bundle(current.value)[offset(current.value)], current.value + 5, 9][0]` 与 `matches(expected: 4, value: [bundle(current.value)[offset(current.value)], current.value, 9][0])` 这类 binding-backed nested-call-root guard 组合纳入公开 CLI 合同，而 `fixtures/codegen/pass/match_guard_projection_backed_nested_call_root_combos.ql` 则继续把 `enabled(extra: bundle(config.slot.value)[offset(config.slot.value)] == 4, state: state(bundle(config.slot.value)[offset(config.slot.value)] == 4))`、`[bundle(config.slot.value)[offset(config.slot.value)], current + 5, 9][0]` 与 `matches(expected: 4, value: [bundle(config.slot.value)[offset(config.slot.value)], current, 9][0])` 这类 projection-backed nested-call-root guard 组合纳入公开 CLI 合同，不再只依赖 driver 单测和 sync executable 样例间接证明
- `crates/ql-cli/tests/codegen.rs` 现在也把 direct call-root fixed-shape `for` 显式锁进 CLI 的 `llvm-ir` / `obj` / `exe` pass matrix：`fixtures/codegen/pass/for_call_root_fixed_shapes.ql` 继续覆盖 `for value in array_values(10)`、`for value in tuple_values(7)` 与 `for value in make_payload(3).values` 这三条 direct call-root / projected call-root fixed-array / homogeneous tuple iterable 路径，不再只依赖 sync executable 样例间接证明
- `crates/ql-cli/tests/codegen.rs` 现在也把 same-file import-alias call-root fixed-shape `for` 显式锁进 CLI 的 `llvm-ir` / `obj` / `exe` pass matrix：`fixtures/codegen/pass/import_alias_call_root_fixed_shapes.ql` 继续覆盖 `for value in values(10)`、`for value in pairs(7)` 与 `for value in payload(3).values` 这三条 alias-call canonicalization 后的 call-root / projected call-root fixed-array / homogeneous tuple iterable 路径，不再只依赖 sync executable 样例间接证明
- `crates/ql-cli/tests/codegen.rs` 现在也把 nested call-root fixed-shape `for` 显式锁进 CLI 的 `llvm-ir` / `obj` / `exe` pass matrix：`fixtures/codegen/pass/nested_call_root_fixed_shapes.ql` 继续覆盖 `for value in array_env(10).payload.values`、`for value in tuple_env(7).payload.values` 与 `for value in deep_env(3).outer.payload.values` 这三条 nested projected call-root fixed-array / homogeneous tuple iterable 路径，不再只依赖 sync executable 样例间接证明
- `crates/ql-cli/tests/codegen.rs` 现在也把 same-file import-alias nested call-root fixed-shape `for` 显式锁进 CLI 的 `llvm-ir` / `obj` / `exe` pass matrix：`fixtures/codegen/pass/import_alias_nested_call_root_fixed_shapes.ql` 继续覆盖 `for value in arrays(10).payload.values`、`for value in tuples(7).payload.values` 与 `for value in deep(3).outer.payload.values` 这三条 alias-call canonicalization 后的 nested projected call-root fixed-array / homogeneous tuple iterable 路径，不再只依赖 sync executable 样例间接证明
- sync executable smoke contract 现也覆盖本地忽略目录中的代表性样例名 `ramdon_tests/executable_examples/04_sync_static_item_values.ql` 到 `39_sync_unparenthesized_inline_projected_control_flow_heads.ql`：除了 same-file foldable `const` / `static` item value 普通表达式 / bool 条件 lowering、direct named call argument lowering、same-file function import alias call lowering、direct fixed-array `for`、direct homogeneous tuple `for`、projected fixed-shape `for`、same-file `const` / `static` fixed-shape root 与 same-file `use ... as ...` const alias 的 sync `for` lowering、direct call-root / projected call-root fixed-shape `for` lowering、same-file import-alias call-root / projected call-root fixed-shape `for` lowering、nested call-root projected fixed-shape `for` lowering、same-file import-alias nested call-root projected fixed-shape `for` lowering、带括号 inline aggregate projected fixed-shape `for` lowering、same-file import-alias 带括号 inline aggregate projected fixed-shape `for` lowering、不带括号的 inline aggregate projected fixed-shape `for` lowering，以及不带括号的 inline aggregate projected `if` / `while` / `match` 头 lowering 之外，现在也已经把 bool `match` scrutinee self-guard folding、scrutinee-bool-comparison guard folding、bool partial dynamic guard lowering、integer partial dynamic guard lowering、当前 arm binding 作为 read-only struct/tuple/fixed-array projection root 的 guard lowering、非 `Bool` / `Int` current-loadable scrutinee 的单名 binding catch-all lowering、same-file `const` / `static` / import-alias aggregate root 上带运行时数组索引的 guard lowering、direct resolved sync scalar guard-call lowering、direct resolved sync aggregate guard-call projection-root lowering、direct resolved sync aggregate guard-call argument lowering、direct resolved sync inline aggregate-literal guard-call argument lowering、inline aggregate-literal projection-root guard lowering、same-file item/import-alias-backed inline aggregate 组合 guard lowering、direct sync call-backed guard 组合 lowering、direct sync call-root nested runtime projection lowering、nested call-root inline guard 组合 lowering、same-file item-backed nested call-root 组合 guard lowering、call-backed nested call-root 组合 lowering、alias-backed nested call-root 组合 lowering、binding-backed nested call-root 组合 lowering，以及 projection-backed nested call-root 组合 guard lowering 一并锁进真实 `--emit exe` 合同
- `dylib` 当前要求模块里至少存在一个 public 顶层 `extern "c"` 函数定义，避免生成没有明确导出面的共享库
- direct `extern "c"` 调用现在会在 program mode 和 library mode 下都 lower 成 LLVM `declare @symbol` + `call @symbol`
- 顶层 `extern "c"` 函数定义现在会 lower 成稳定 C 符号名，例如 `define i64 @q_add(...)`
- Windows 上 `dylib` 链接会为这些稳定导出符号显式追加 `/EXPORT:<symbol>`，确保 DLL 导出表和 Qlang 的 exported C surface 保持一致
- `dylib` / `staticlib` 现在还可以在构建成功后直接附带 C header sidecar，而不需要额外再跑一次 `ql ffi header`
- build-side header 会复用同一份 analysis 结果，而不是重新 parse / resolve / typeck
- build-side header 只允许出现在 `dylib` / `staticlib` 上；对 `llvm-ir` / `obj` / `exe` 会直接拒绝
- 如果显式 `--header-output` 与主 artifact 路径相同，driver 会在真正构建前直接报错，避免 header 覆盖库文件
- 如果 library 已成功产出但 sidecar header 生成失败，driver 会回收刚生成的主 artifact，避免留下“库成功、头文件失败”的半成功状态
- program mode 的入口 `main` 仍必须使用默认 Qlang ABI；如果需要导出稳定 C 符号，应定义独立的 `extern "c"` helper
- `extern` callable 现在有共享 callable identity，因此 extern block 调用也能稳定参与参数类型检查与代码生成
- first-class function value 不会再把后端打崩，而是返回结构化 unsupported diagnostics
- Windows 上如果使用 `QLANG_CLANG` 覆盖路径，建议指向 `clang.exe` 或 `.cmd` wrapper，而不是裸 `.ps1`
- 如果使用 `QLANG_AR` 覆盖路径，建议指向 `llvm-ar` / `ar` / `llvm-lib.exe` / `lib.exe` 或对应 `.cmd` wrapper
- 如果 `QLANG_AR` 指向的 wrapper 文件名本身看不出是 `ar` 风格还是 `lib` 风格，可以额外设置 `QLANG_AR_STYLE=ar|lib`
- Windows 下如果 PATH 中没有 clang / archiver，`ql-driver` 现在还会 best-effort 探测常见 LLVM 安装目录，包括 Scoop 的 `llvm/current/bin`、`%LOCALAPPDATA%\\Programs\\LLVM\\bin`、`%ProgramFiles%\\LLVM\\bin` 与 `%ProgramFiles(x86)%\\LLVM\\bin`
- 当这些位置也没有找到工具时，`ToolchainError::NotFound` 会把候选路径直接放进 hint，减少“只知道要配环境变量，但不知道应该指向哪里”的恢复成本

当前明确未完成：

- 独立 linker family discovery
- runtime startup object
- first-class function value lowering
- closure lowering
- 更广义的 projection assignment（当前已开放 tuple-index / struct-field / fixed-array literal-index write、非 `Task[...]` 元素的 dynamic array assignment，以及 `Task[...]` 动态数组的 generic maybe-overlap write/reinit + same immutable stable index path precise consume/reinit 子集）、更广义 `for` / `for await` lowering（当前已开放 sync fixed-shape `for`，以及 library-mode 与 `BuildEmit::Executable` `async fn main` 子集下的 fixed-shape iterable `for await`，其中 fixed-shape 当前覆盖 fixed-array 与 homogeneous tuple）、更广义 `match` lowering（当前只开放 `Bool` scrutinee + unguarded `true` / `false` / same-file const/static/alias bare path + `_` 或单名 binding catch-all arm、`Int` scrutinee + unguarded integer literal / same-file const/static/alias bare path + `_` 或单名 binding catch-all arm，以及其他 current-loadable scrutinee 上的 `_` / 单名 binding catch-all arm 子集；这些子集额外接受 literal `if true` / `if false` guard、same-file foldable `const` / `static`-backed `Bool` guard 与其 same-file `use ... as ...` 别名、包裹当前 bool guard 子集的一层 unary `!`、由当前 bool guard 子集继续组合出的 runtime `&&` / `||`、当前 arm 单名 binding catch-all 变量作为 direct scalar operand 参与 guard，以及当前 `Bool ==/!=` 与 `Int ==/!=/>/>=/</<=` 的简单 scalar comparison guard 子集；其中 `Int` operand 当前开放 integer literal、same-file foldable `const` / `static`-backed `Int` 与其 same-file `use ... as ...` 别名、由当前支持的 `Int` operand 继续递归组成的最小整数算术表达式（`+` / `-` / `*` / `/` / `%`）、以 local / parameter / `self` 或当前 arm 单名 binding 为根的 read-only struct field / tuple literal-index / fixed-array index projection，以及能经由这些投影折叠成 scalar 的 same-file foldable `const` / `static`-backed aggregate root 及其 same-file `use ... as ...` alias，而 direct bool guard operand 当前开放 same-scope `Bool` local / parameter、以 local / parameter / `self` 或当前 arm 单名 binding 为根的 read-only bool scalar projection，以及能折叠成 `Bool` 的 same-file foldable `const` / `static`-backed aggregate root 及其 same-file `use ... as ...` alias；`Bool` scrutinee 子集额外接受这组 direct bool guard，而 `Int` scrutinee 子集额外接受 integer-literal arm / guarded catch-all arm 上的同一组 direct bool guard；超出当前 catch-all-only non-scalar scrutinee 子集、或超出当前只读 struct-field / tuple-index / fixed-array-index 子集的 current-arm projection 用法仍显式拒绝；不能折叠成 `Bool` / `Int` 的 bare path pattern 仍显式拒绝）与更广义 aggregate / cleanup lowering
- 任意共享库 surface、exported ABI 的 linkage/visibility 控制与 richer ABI surface
- extern ABI 与 runtime glue 的其余部分

这些边界当前仍保持关闭，用于控制 Phase 4 的后端范围。

### `ql ffi`

P5 当前已经落地最小可用的 C header emit slice：

- `ql ffi header <file>`
- `ql ffi header <file> --surface exports`
- `ql ffi header <file> --surface imports`
- `ql ffi header <file> --surface both`
- `ql ffi header <file> -o <output>`
- `ql build <file> --emit dylib|staticlib --header`
- `ql build <file> --emit dylib|staticlib --header-surface exports|imports|both`
- `ql build <file> --emit dylib|staticlib --header-output <output>`

当前默认输出路径：

- `target/ql/ffi/<stem>.h`
- `target/ql/ffi/<stem>.imports.h`
- `target/ql/ffi/<stem>.ffi.h`

build-side sidecar 默认输出路径：

- `<library-dir>/<source-stem>.h`
- `<library-dir>/<source-stem>.imports.h`
- `<library-dir>/<source-stem>.ffi.h`

当前 `ql ffi header` 的职责是：

- 读取单个 `.ql` 文件
- 复用 `ql-analysis` 完成 parse / HIR / resolve / typeck
- 默认投影 public 顶层 `extern "c"` 函数定义作为 exported surface
- 在 `--surface imports` 下投影顶层 `extern "c"` 声明和 `extern "c"` block 成员声明
- 在 `--surface both` 下按源码顺序合并 import/export surface
- 将当前已支持的标量 / `String` / 指针类型投影到确定性的 C declaration
- 输出 include guard、`<stdbool.h>` / `<stdint.h>`、C++ `extern "C"` wrapper
- include guard 会按最终输出头文件名生成，避免 export/import/both 三份 header 互相冲突

而 `ql build` 上的 header sidecar 只是复用同一套投影逻辑，但把默认输出目录改成 library artifact 同目录，并挂到 build orchestration 上统一交付。

当前支持矩阵刻意收窄为：

- 顶层 `pub extern "c" fn ... { ... }`
- 顶层 `extern "c" fn ...`
- `extern "c" { fn ... }`
- `Bool` / `Void`
- `Int` / `UInt` / `I8` / `I16` / `I32` / `I64` / `ISize`
- `U8` / `U16` / `U32` / `U64` / `USize`
- `F32` / `F64`
- `String`，当前稳定投影为 `typedef struct ql_string { const uint8_t* ptr; int64_t len; } ql_string;`
- 原始指针和多级原始指针
- `exports` / `imports` / `both` 三种 surface 选择
- 按源码顺序稳定输出 declaration

当前明确未完成：

- struct / tuple / callable / function-pointer ABI
- layout 校验与 richer diagnostics
- exported symbol 的 visibility/linkage 控制
- bridge code generation

### `ql check`

当前 `ql check` 已经进入 Phase 2 的第一层语义阶段，不再只是 parser 验证。它现在负责：

- 读取单个 `.ql` 文件并执行 lexer / parser 验证
- 将 AST lowering 到 HIR
- 在 HIR 上执行名称解析与作用域图构建
- 运行第一批 semantic checks
- 对目录执行批量检查
- 统一输出带 span 的 parser 与 semantic diagnostics

当前已落地的 semantic checks 包括：

- top-level duplicate definition
- duplicate generic parameter
- duplicate function parameter
- duplicate enum variant
- duplicate method in trait / impl / extend block
- duplicate binding inside a pattern
- duplicate field in `struct` declaration
- duplicate field in `struct` pattern
- duplicate field in `struct` literal
- duplicate named call argument
- positional argument after named arguments
- invalid use of `self` outside a method receiver scope
- return-value type mismatches
- non-`Bool` conditions in `if` / `while` / match guards
- callable arity / argument type mismatches
- tuple destructuring arity mismatches
- struct literal unknown-field / missing-field / field-type mismatches
- source-level fixed array type expr `[T; N]` lowering and compatibility checks
- expected fixed-array context guided array literal item diagnostics
- equality operand compatibility mismatches
- comparison operand compatible-numeric mismatches
- unknown struct member access
- pattern root / literal type mismatches in destructuring and `match`
- calling non-callable values

当前 `ql check` 的内部边界也进一步明确了：

- `ql-analysis` 负责统一 parse / HIR / resolve / typeck 分析入口
- `ql-cli` 不再自己拼装语义流水线，而是消费 `ql-analysis`
- 这让 CLI、测试和未来 LSP 可以共享同一份分析快照，而不是各自拷贝一份流程
- `ql-analysis` 现在还额外暴露了 position-based query surface：
  - `symbol_at(offset)`
  - `hover_at(offset)`
  - `definition_at(offset)`
  - `references_at(offset)`
  - `completions_at(offset)`
  - `semantic_tokens()`
  - `prepare_rename_at(offset)`
- `rename_at(offset, new_name)`
- 这组 API 当前已经能回答 item / local / param / generic / receiver `self` / named type root / pattern root / struct literal root 的基础语义查询，并导出同文件 completion 与 semantic tokens 所需的稳定语义数据
- import alias 现在会作为 source-backed binding 进入同一份 query index，因此可以返回真实 definition span，并参与同文件 hover / references / rename / semantic tokens；builtin type 则继续作为非 source-backed stable symbol 参与 hover / references / semantic tokens，但不提供 definition / rename
- 当 import alias 的原始路径恰好是单段、且命中同文件 root struct item 时，field label query / rename 与 struct literal 字段检查也会继续沿用原 struct item / field symbol；struct 或 enum pattern root 也会沿同一条 canonicalization 回到本地 item

这一层现在还有两个明确的架构保证：

- AST 会保留 declaration name、generic param、regular param、pattern field、struct literal field、named call arg、closure param 的精确 name span
- receiver param 现在也会保留精确 span，而不是退化成整个函数 span
- HIR 会提前正规化 shorthand sugar，例如 struct pattern / struct literal 的缩写字段，后续 name resolution 和 type checking 不需要再区分“缩写”和“完整写法”

现在又额外补上了一条关键边界：

- `ql-resolve` 专门承接 lexical scope graph 与 best-effort name resolution，避免把作用域查找逻辑散落进 `ql-typeck`
- 当前 resolution 故意只做保守诊断：先落地 `self` misuse 与 bare single-segment value/type root 的 unresolved，不抢跑 multi-segment unresolved global / unresolved type 的全面报错，这样可以先把语义架构打稳，再补 import / module / prelude 规则
- `ql-typeck` 现在已经不只是 duplicate checker，而是开始承接真正的 first-pass typing；但它依然刻意保守，未知成员访问、通用索引协议结果、未建模模块语义仍然会回退成 `unknown`，避免过早把当前样例集打成错误；当前只对 source-level fixed array、inferred array，以及同文件 foldable integer constant expression / immutable direct local alias-backed constant tuple index 开了一层稳定 typing
- top-level `const` / `static` 的声明类型现在会进入后续表达式 typing，因此函数值常量的调用也能拿到参数类型诊断

当前目录扫描策略也已经收紧，避免把仓库噪音当成真实源码：

- 会跳过 `target`、`node_modules`、`dist`、`build`、`coverage`
- 会跳过隐藏目录
- 会跳过仓库里的 `fixtures/` 和临时测试目录，例如 `ramdon_tests/`
- 如果用户显式传入某个 fixture 文件或 fail fixture 目录，仍然允许直接检查

这个策略不是“保守”，而是为了避免 `ql check .` 在仓库根目录误扫失败夹具、构建产物和杂项测试目录，污染真实前端回归结果。

当前仍需明确的一条状态边界：

- 默认参数仍是设计稿能力，不属于当前已经实现并验证的 `ql check` 语义范围

当前测试基建也已经进入下一层：

- `crates/ql-typeck/tests/` 继续承载 crate-local duplicates / typing / rendering 回归
- `crates/ql-analysis` 现在承载统一分析边界和查询 API 的单元测试
- 仓库根 `tests/ui/` 现在开始承载黑盒 UI diagnostics fixture
- `crates/ql-cli/tests/ui.rs` 负责驱动真实 `ql` 二进制，对 parser / resolve / semantic / type diagnostics 的最终 stderr 做 snapshot 比对

截至 2026-03-28，当前工具链与文档基线常用的验证命令包括：

- `cargo test`
- `cargo test -p ql-cli --test codegen`
- 在 clang-style compiler 与 archiver 可用时：`cargo test -p ql-cli --test ffi`
- `cargo test -p ql-cli --test ffi_header`
- `cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_build.ql --emit llvm-ir`
- `cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_library.ql --emit staticlib`
- `cargo run -p ql-cli -- build fixtures/codegen/pass/extern_c_library.ql --emit staticlib --header-surface imports`
- `cargo run -p ql-cli -- ffi header tests/ffi/pass/extern_c_export.ql`
- `cargo run -p ql-cli -- ffi header tests/ffi/header/extern_c_surface.ql --surface imports`
- 在 clang 可用或 mock toolchain 注入时：`cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_build.ql --emit obj`
- 在 clang 可用或 mock toolchain 注入时：`cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_build.ql --emit exe`
- 在 clang 与 archiver 可用或 mock toolchain 注入时：`cargo run -p ql-cli -- build tests/ffi/pass/extern_c_export.ql --emit staticlib --header`
- `npm run build` in `docs/`

当前新增的黑盒 codegen harness 位于：

- `crates/ql-cli/tests/codegen.rs`
- `tests/codegen/pass/`
- `tests/codegen/fail/`

它会直接驱动真实 `ql build`，锁定：

- LLVM IR 快照
- extern C direct-call LLVM IR 快照
- mock object / executable / static library 产物
- build-side export/import header sidecar 快照
- build 路径上的 unsupported diagnostics

当前还新增了第一版真实 FFI smoke harness：

- `crates/ql-cli/tests/ffi.rs`
- `tests/ffi/pass/`
- `tests/ffi/pass/*.header-surface`

静态库回归会在 clang-style compiler 和 archiver 可用时：

- 构建导出 `extern "c"` 符号的 Qlang `staticlib`
- 在同一次 `ql build --header-output` 里生成对应的 C 头文件
- 用包含该头文件的真实 C harness 链接该库
- 运行宿主可执行文件确认导出符号可被调用
- imported-host 夹具还会通过 `both` surface header 同时拿到 imported/exported 声明，并验证 Qlang 导出函数体内的 imported C 调用能真实命中宿主实现
- `crates/ql-cli/tests/ffi.rs` 现在直接复用 `ql-driver` 的 toolchain discover 结果，因此这些回归对 clang / archiver 的可用性判断与真实 `ql build` 路径保持一致
- 当前 imported-host staticlib 已覆盖：
  - extern block declaration
  - top-level extern declaration

共享库回归会在 clang-style compiler 可用时：

- 构建导出 `extern "c"` 符号的 Qlang `dylib`
- 在同一次 `ql build --header-output` 里生成对应的 C 头文件
- 用真实 C loader harness 编译宿主可执行文件
- 运行宿主可执行文件，并在进程内通过 `LoadLibraryA` / `dlopen` 解析并调用导出符号

当前 `.header-surface` fixture 元数据规则：

- 不存在时默认 `exports`
- 存在时内容必须是 `exports` / `imports` / `both`
- 这让 FFI harness 可以在不硬编码 case 名称的前提下，为 imported-host 夹具切换到 combined surface

### `qlsp`

LSP 服务端，复用编译器 HIR 与查询系统。长期目标支持：

- go to definition
- find references
- hover
- completion
- semantic tokens
- rename
- code action
- diagnostics

当前已经有的地基：

- `ql-analysis` 已提供最小可用的 hover / definition / references 查询面
- `ql-analysis` 现在还提供基于稳定 symbol identity 的 same-file references 查询面
- struct field 与唯一 method candidate 的 member token 现在也能直接复用同一套查询面
- explicit struct literal / struct pattern field label 现在也能直接复用同一套 field 查询面
- enum variant declaration / pattern use / constructor use 现在也能直接复用同一套查询面
- `ql-analysis` 现在也能基于同一份 occurrence 索引导出 same-file semantic tokens
- `qlsp` 的第一版已经落地在 `crates/ql-lsp`
- 当前通过 stdio 运行，复用 `ql-analysis`
- 当前已实现：
  - `textDocument/didOpen`
  - `textDocument/didChange`（full sync）
  - `textDocument/didClose`
  - `textDocument/hover`
  - `textDocument/definition`
  - `textDocument/references`（当前为 same-file）
  - `textDocument/completion`（当前为 same-file lexical scope + parsed member token + parsed enum variant path，且支持 local import alias -> local enum item 的 variant follow-through，并保留 escaped identifier 的合法 insert text）
  - `textDocument/semanticTokens/full`（当前为 same-file source-backed symbol）
  - `textDocument/prepareRename`
  - `textDocument/rename`（当前为 same-file）
  - `textDocument/publishDiagnostics`
- LSP 协议桥接已单独分层：
  - 位置 `Position <-> byte offset` 换算
  - `Span -> Range`
  - compiler diagnostics -> LSP diagnostics
  - analysis hover / definition / references / completion / semantic tokens / rename -> LSP response
- 这意味着 `qlsp` 的第一版不需要重新发明一套“源码位置 -> 语义实体”的逻辑
- 当前 same-file rename 也明确只开放保守符号集：function / const / static / struct / enum / variant / trait / type alias / import / field / method（仅唯一 candidate）/ local / parameter / generic
- 这组 type-namespace item rename 现在也已经由 analysis / LSP 回归明确锁住：`type`、`opaque type`、`struct`、`enum`、`trait` 不依赖协议层特判，而是继续走统一 `QueryIndex`
- 这组 root value-item rename 现在也已经由 analysis / LSP 回归明确锁住：`function`、`const`、`static` 不依赖协议层特判，而是继续走统一 `QueryIndex`
- 同一组 type-namespace item 现在也已经有 references / semantic-token parity 回归，确保 `type`、`opaque type`、`struct`、`enum`、`trait` 的 query、LSP references 与语义高亮继续站在同一份 item occurrence 上
- 这组 item 现在还额外有 hover / definition parity 回归，确保 `type`、`opaque type`、`struct`、`enum`、`trait` 的导航与悬浮信息继续落回同一份 definition span，而不是在 bridge 层退化成字符串级猜测
- same-file type-namespace item surface 现在也已经有显式聚合回归：`type`、`opaque type`、`struct`、`enum`、`trait` 会继续共享同一组 item truth surface，因此 hover / definition / references / semantic tokens 不需要分别靠零散的单类回归来兜底
- global value item 现在也已经有 query parity 回归：`const`、`static` 的 item definition 与 value-use 会继续共享同一份 `QueryIndex` truth surface，因此 hover / definition / references / semantic tokens 不需要在 LSP 层额外补特判
- `extern` callable surface 现在也已经有 same-file parity 回归：无论是 `extern` block 成员、顶层 `extern "c"` 声明，还是带 body 的顶层 `extern "c"` 函数定义，定义点与 call site 都会继续共享同一份 `Function` truth surface，因此 hover / definition / references / rename / semantic tokens 不需要在 LSP 层额外做 extern 特判
- extern callable 的 value completion 现在也已经有显式 parity 回归：analysis 会继续把 `extern` block 成员、顶层 `extern "c"` 声明和顶层 `extern "c"` 函数定义作为 `function` 候选产出，LSP bridge 会继续把它们投影成 `FUNCTION` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- ordinary free function 现在也已经有 same-file query parity 回归：direct call site 会继续共享同一份 `Function` truth surface，因此 hover / definition / references 不需要在 LSP 层额外补自由函数特判
- ordinary free function 现在也已经有 same-file semantic-token parity 回归：declaration 与 direct call site 会继续共享同一份 `Function` truth surface，因此 semantic tokens 不需要在 LSP 层额外补自由函数特判
- same-file callable surface 现在也已经有显式聚合回归：`extern` block callable、顶层 `extern "c"` 声明、顶层 `extern "c"` 定义与 ordinary free function 会继续共享同一组 callable truth surface，因此 hover / definition / references / semantic tokens 不需要分别靠零散的单类回归来兜底
- plain import alias symbol 现在也已经有 same-file parity 回归：`import` binding 会继续作为 source-backed symbol 共享同一份 truth surface，因此 hover / definition / references / semantic tokens 不需要在 analysis 与 LSP 两层分别做例外处理
- plain import alias 的 type-context completion 现在也已经有显式 parity 回归：analysis 会继续产出 `import` 候选，LSP bridge 会继续把它投影为 `MODULE` completion item，并沿用同一份 insert-text / text-edit 语义
- free function 的 lexical value completion 现在也已经有显式 parity 回归：analysis 会继续产出 `function` 候选，LSP bridge 会继续把它投影为 `FUNCTION` completion item，并沿用同一份 insert-text / text-edit 语义
- plain import alias 的 lexical value completion 现在也已经有显式 parity 回归：analysis 会继续产出 source-backed `import` 候选，LSP bridge 会继续把它投影为 `MODULE` completion item，并沿用同一份 insert-text / text-edit 语义
- builtin type 与 local struct item 的 type-context completion 现在也已经有显式 parity 回归：analysis 会继续产出这两类 type 候选，LSP bridge 会继续把它们投影为 `CLASS` / `STRUCT` completion item，并沿用同一份 insert-text / text-edit 语义
- same-file type alias 的 type-context completion 现在也已经有显式 parity 回归：analysis 会继续产出 `type alias` 候选，LSP bridge 会继续把它投影为 `CLASS` completion item，并沿用同一份 insert-text / text-edit 语义
- same-file `opaque type` 的 type-context completion 现在也已经有显式 parity 回归：analysis 会继续产出 `TypeAlias`-backed opaque alias 候选，LSP bridge 会继续把它投影为 `CLASS` completion item，并沿用 `opaque type ...` detail 与同一份 insert-text / text-edit 语义
- same-file generic 的 type-context completion 现在也已经有显式 parity 回归：analysis 会继续产出 `generic` 候选，LSP bridge 会继续把它投影为 `TYPE_PARAMETER` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- same-file enum 的 type-context completion 现在也已经有显式 parity 回归：analysis 会继续产出 `enum` 候选，LSP bridge 会继续把它投影为 `ENUM` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- same-file trait 的 type-context completion 现在也已经有显式 parity 回归：analysis 会继续产出 `trait` 候选，LSP bridge 会继续把它投影为 `INTERFACE` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- stable receiver field completion 现在也已经有显式 parity 回归：analysis 会继续产出 `field` 候选，LSP bridge 会继续把它投影为 `FIELD` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- stable receiver unique method completion 现在也已经有显式 parity 回归：analysis 会继续产出唯一 `method` 候选，LSP bridge 会继续把它投影为 `FUNCTION` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- same-file const / static 的 value completion 现在也已经有显式 parity 回归：analysis 会继续产出 `const` / `static` 候选，LSP bridge 会继续把它们投影为 `CONSTANT` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- same-file local 的 value completion 现在也已经有显式 parity 回归：analysis 会继续产出 `local` 候选，LSP bridge 会继续把它投影为 `VARIABLE` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- same-file parameter 的 value completion 现在也已经有显式 parity 回归：analysis 会继续产出 `parameter` 候选，LSP bridge 会继续把它投影为 `VARIABLE` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- same-file lexical value candidate-list parity 现在也已经有显式回归：analysis / LSP 会继续让 import / const / static / extern callable / free function / local / parameter 这些 already-supported value surface 共享同一份有序候选列表、detail 渲染与 replacement text-edit 投影，而不是让这组 editor-facing 契约分散在单类候选测试里
- same-file enum variant completion 现在也已经有显式 parity 回归：analysis 会继续产出 parsed enum path 上的 `variant` 候选，LSP bridge 会继续把它投影为 `ENUM_MEMBER` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- same-file import alias variant completion 现在也已经有显式 parity 回归：analysis 会继续让指向同文件根 enum item 的 local import alias path 产出 `variant` 候选，LSP bridge 会继续把它投影为 `ENUM_MEMBER` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- same-file import alias struct-variant completion 现在也已经有显式 parity 回归：analysis 会继续让指向同文件根 enum item 的 local import alias struct-literal path 产出 struct-style `variant` 候选，LSP bridge 会继续把它投影为 `ENUM_MEMBER` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- remaining same-file variant-path completion contexts 现在也已经有显式 parity 回归：analysis 会继续产出 direct struct-literal path 以及 direct/local-import-alias pattern path 上既有的 `variant` 候选，LSP bridge 会继续把它们投影为 `ENUM_MEMBER` completion item，并沿用同一份 detail / insert-text / text-edit 语义
- same-file variant-path candidate-list parity 现在也已经有显式回归：analysis / LSP 会继续让 enum-root / struct-literal / pattern path 及其 same-file import-alias 镜像上下文共享同一份有序 `variant` 候选列表、detail 渲染与 replacement text-edit 投影，而不是让这组 editor-facing 契约停留在单个候选映射测试
- deeper variant-like member chain 现在也明确保持关闭：只有 root enum item 或 same-file import alias 的第一段 variant tail 还能复用 enum variant truth surface，`Command.Retry.more` / `Cmd.Retry.more` 这类更深 member chain 不会再伪造 hover / definition / references 或 `ENUM_MEMBER` completion
- deeper struct-literal / pattern variant-like path 现在也明确保持关闭：只有严格两段 `Root.Variant` path 才继续复用 enum variant truth surface，`Command.Scope.Config { ... }` / `Cmd.Scope.Retry(...)` 这类更深 path 不会再伪造 hover / definition / references / rename / semantic tokens 或 `ENUM_MEMBER` completion
- deeper struct-like path 的 field truth 现在也明确保持关闭：只有严格 root struct path 才继续复用 field query / rename / semantic-token surface，`Point.Scope.Config { x: ... }` / `P.Scope.Config { x: ... }` 这类更深 path 不会再伪造字段标签的 hover / definition / references / rename / semantic tokens
- deeper struct-like shorthand token 现在也有显式 parity 回归：当 `Point.Scope.Config { x }` / `Point.Scope.Config { source }` 这类路径仍处于 field semantics 关闭状态时，analysis 会继续把 shorthand token 保留在 local / binding / import lexical surface，LSP bridge 也会继续按该 lexical symbol 提供 hover / definition / references / semantic tokens / rename，并保持 raw binding edit 而不是伪造 `label: new_name` 扩写
- same-file completion filtering parity 现在也已经有显式回归：analysis 会继续按 lexical scope visibility/shadowing 与 impl-preferred member filtering 产出候选，LSP bridge 会继续原样投影这些结果，而不会在协议层额外扩张或放宽歧义 surface；其中 lexical value visibility 的聚合回归现在也已经显式覆盖 import / function / local 的 detail 与 text-edit 投影，而 impl-preferred member 聚合回归现在也已经显式覆盖 surviving candidate count 以及稳定 detail / text-edit 投影
- same-file completion candidate-list parity 现在也已经有显式回归：analysis 会继续按 type-context 与 stable-member 的完整候选列表产出结果，LSP bridge 会继续原样投影这些列表，而不会在协议层悄悄改变排序、命名空间边界或完整成员集合；其中 type-context 总表现在已经显式覆盖 builtin / import / struct / `type` / `opaque type` / `enum` / `trait` / generic，而 stable-member 总表也已经显式覆盖 method / field 的 detail 与 text-edit 投影
- shorthand struct field token query parity 现在也已经有显式回归：analysis 会继续把 `Point { x }` 这种 shorthand token 视为 local/binding surface，LSP bridge 会继续按这个结果提供 hover / definition，而不会在协议层把 shorthand token 误投影成 struct field
- direct same-file variant / explicit field-label query parity 现在也已经有显式回归：analysis 会继续把 direct enum variant token 与 direct explicit struct field label 的 definition / references 原样投影给 LSP，而不是只在 import-alias follow-through 路径上有端到端覆盖
- direct same-file variant / explicit field-label semantic-token parity 现在也已经有显式回归：analysis 会继续把 direct enum variant token 与 direct explicit struct field label 的 highlighting occurrence 原样投影给 LSP semantic tokens，而不是只在 import-alias follow-through 路径或聚合总表里被间接覆盖
- same-file direct symbol surface 现在也已经有显式聚合回归：direct enum variant token 与 direct explicit struct field label 会继续共享同一组 direct-symbol truth surface，因此 hover / definition / references / semantic tokens 不需要分别靠零散的单类回归来兜底
- direct stable-member query parity 现在也已经有显式回归：analysis 会继续把 direct field member 与唯一 method member 的 hover / definition / references 原样投影给 LSP，而不是只剩字段 hover 或其他间接覆盖
- direct stable-member semantic-token parity 现在也已经有显式回归：analysis 会继续把 direct field member 与唯一 method member 的 highlighting occurrence 原样投影给 LSP semantic tokens，而不是只靠聚合总表测试间接覆盖
- same-file direct member surface 现在也已经有显式聚合回归：direct field member 与唯一 method member 会继续共享同一组 direct-member truth surface，因此 hover / definition / references / semantic tokens 不需要分别靠零散的单类回归来兜底
- impl-preferred member query parity 现在也已经有显式回归：analysis 会继续把 impl-over-extend 的既有 direct member 选择结果原样投影给 LSP hover / definition / references，而不是在桥接层重新解释同名成员优先级
- lexical semantic symbol 现在也已经有 same-file parity 回归：`generic`、`parameter`、`local`、`receiver self` 与 `builtin type` 会继续共享同一份 lexical truth surface；其中 builtin type 仍没有 source-backed declaration，所以 definition / rename 保持关闭，但 hover / references / semantic tokens 已经显式回归锁住
- lexical rename surface 现在也已经有显式回归：`generic`、`parameter`、`local` 会继续沿用 analysis 的 same-file rename 结果直通到 LSP，而 `receiver self` / `builtin type` 继续返回 closed surface，不在协议层做特判补开
- 显式字段标签虽然已经能 hover / definition / references 到 struct field，shorthand `Point { x }` token 仍故意保守地继续解析为 local/binding；但从 source-backed field symbol 发起 rename 时，这些 shorthand site 会被自动扩写成显式标签，而从 shorthand token 上发起的 renameable binding rename 现在也会保持这条展开逻辑；这条回归已经明确覆盖 local / parameter / import / function / const / static；同文件 local import alias -> local struct item 的路径现在也会继续复用这条 field rename surface
- 其中 free-function shorthand binding rename 现在也已经有显式 LSP parity 回归：analysis 会继续把 shorthand token 解析为 `function` binding，bridge 会继续保留 field label 并只改 declaration / use，而不是把这类 site 退化成普通字段编辑
- 但这还不是完整 LSP 语义层：当前 completion 只做到 same-file lexical scope + parsed member token + parsed enum variant path，以及 local import alias -> local enum item 的 variant follow-through；这条 follow-through 也已经进入 hover / definition / references / same-file rename / semantic tokens；同文件 local import alias -> local struct item 现在也已经进入显式字段标签与 field-driven shorthand rename 的 query surface；struct field、显式字段标签、唯一 method candidate、enum variant token 这些精确 query 已经可复用，而唯一 method candidate 现在也已进入 same-file rename；completion 现在也会把 keyword-named symbol 写回 escaped identifier；import-graph/module-path deeper completion、foreign import alias variant semantics、ambiguous method、parse-error tolerant member completion、从 shorthand field token 本身发起的 field-symbol rename 和 cross-file rename 仍需要后续继续补齐

### `qfmt`

格式化器必须尽早做，并尽量做到：

- 输出稳定
- 风格单一
- 对 AST 变化敏感度低

现代语言生态一旦放任格式风格分裂，后面会一直付成本。

当前阶段 `qfmt` 已覆盖的语法切片包括：

- 基础声明：`const`、`static`、`type`、`opaque type`
- 可调用声明：`fn`、`trait` method、`impl`、`extend`、`extern`
- 类型表达式：named type、tuple、callable type、声明泛型、`where`
- 表达式：调用、成员访问、结构体字面量、闭包、`unsafe`、`if`、`match`
- 控制流：`while`、`loop`、`for`、`for await`
- 模式：tuple、path、tuple-struct、struct、字面量、`_`

Phase 1 结束后，`qfmt` 的下一步重点不是增加风格选项，而是跟随后续 HIR / diagnostics 演进，保持语法扩展时的稳定输出与可维护实现。

### 当前已验证命令

截至 2026-03-27，当前工具链基线已经反复验证过以下命令：

- `cargo fmt`
- `cargo test`
- `cargo run -p ql-cli -- check fixtures/codegen/pass/minimal_build.ql`
- `cargo run -p ql-cli -- check <含重复定义/重复绑定的源码>`
- 手工负例验证：`cargo run -p ql-cli -- check tests/ui/type_unknown_member.ql`
- `cargo run -p ql-cli -- fmt fixtures/parser/pass/phase1_declarations.ql`
- `npm run build` in `docs/`

说明：

- `fixtures/parser/pass/` 仍然是 parser / formatter regression surface，不再等价于“对当前完整语义流水线一定无错的输入”
- 其中部分 fixture 故意保留了 `tick`、`IoError`、`parse_int` 这类占位符符号，用来覆盖语法面，而不是充当当前 `ql check` 的 semantic-clean sample

### `qdoc`

文档生成器负责：

- 从公共 API 提取签名
- 展示效果、错误、trait 约束和 FFI 标记
- 输出静态站点内容

### 测试工具

`ql test` 不只是运行单元测试，还应逐步支持：

- UI tests
- doc tests
- integration tests
- benchmark harness

## 包与工作区

Qlang 应提供统一 manifest，例如 `qlang.toml`，支持：

- package metadata
- dependencies
- features
- build profiles
- ffi libraries
- workspace members

工作区模型必须在早期就纳入，因为编译器、标准库、示例、FFI 包和工具链本身都会依赖它。

借鉴 TypeScript 的 project references，Qlang 还应支持显式项目引用图：

- 工作区成员能声明上游接口依赖
- 增量构建优先基于接口产物判断失效范围
- LSP 可直接消费依赖包的公共 API 元数据

## 接口产物

Qlang 建议为每个包输出公共接口产物，例如 `.qi` 文件：

- 包含公共类型、函数签名、trait、effect、布局约束等元数据
- 供下游类型检查和 LSP 使用
- 避免每次都重新解析全部依赖源码

这相当于把 TypeScript 的 declaration emit 和 project references 经验，转化为适合编译型系统语言的工程能力。

## 编辑器体验

LSP 的目标不是“有就行”，而是从第一阶段就支撑日常开发：

- 补全要基于真实类型，不是纯文本猜测
- 报错位置要稳定
- rename 要有跨文件可信度
- code action 要能生成 `match` 分支、导入、trait stub

Qlang 的目标不是“有一个能用的 LSP”，而是像 TypeScript 一样，把语言服务当成语言本体的一部分来设计。

## 发布与生态

P1 之后可以逐步加入：

- package registry
- lockfile
- binary caching
- doc hosting
- template generator

但这些必须建立在前面的语义和构建基础上，而不是为了“看起来像成熟生态”提前堆功能。
