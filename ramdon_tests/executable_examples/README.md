# Executable Examples

These examples were verified with the real local toolchain and link successfully with `ql build --emit exe`.

Files:

- `01_sync_minimal.ql`: free functions, arithmetic, compare, `if`
- `02_sync_data_shapes.ql`: structs, tuples, arrays, nested projections, dynamic array assignment, zero-sized arrays with expected context
- `03_sync_extern_c_export.ql`: top-level `extern "c" pub fn` definition plus normal `main`
- `04_sync_static_item_values.ql`: same-file `static` item values plus `use ... as ...` aliases in ordinary expressions and bool conditions
- `05_sync_named_call_arguments.ql`: direct named call arguments lowered in parameter order, including expected-type back-propagation for `[]`
- `06_sync_import_alias_named_call_arguments.ql`: same-file `use ... as ...` function alias calls plus named arguments, lowered as the original direct callee in parameter order
- `07_sync_for_fixed_array.ql`: direct fixed-array `for` lowering in executable mode
- `08_sync_for_tuple.ql`: homogeneous tuple `for` lowering in executable mode
- `09_sync_for_projected_fixed_shape.ql`: projected tuple/array fixed-shape `for` lowering in executable mode
- `10_sync_for_const_static_fixed_shape.ql`: same-file `const` / `static` fixed-shape `for` roots, including a `use ... as ...` const alias
- `11_sync_match_scrutinee_self_guard.ql`: bool `match` self-guard folding in executable mode, where `true if flag` can lower when the guard is the scrutinee itself
- `12_sync_match_scrutinee_bool_comparison_guard.ql`: bool `match` scrutinee-comparison guard folding in executable mode, where `true if flag == ON` can lower when the comparison is just the scrutinee against a foldable bool literal/const/alias
- `13_sync_match_partial_dynamic_guard.ql`: bool `match` partial dynamic-guard lowering in executable mode, where `true if enabled` no longer needs a later `true` fallback arm just to pass backend lowering
- `14_sync_match_partial_integer_dynamic_guard.ql`: integer `match` partial dynamic-guard lowering in executable mode, where `1 if enabled` no longer needs a later unguarded catch-all arm just to pass backend lowering
- `15_sync_match_guard_binding_projection_roots.ql`: current-arm binding guard projection roots in executable mode, where `current.slot.ready`, `current[1]`, and `current[0]` can now lower as read-only guard operands
- `16_sync_match_binding_catch_all_aggregate_scrutinees.ql`: current-loadable aggregate binding catch-all `match` lowering in executable mode, where `match state { current => ... }` and other non-`Bool`/`Int` catch-all-only binding shapes now lower directly
- `17_sync_match_guard_runtime_index_item_roots.ql`: same-file `const` / `static` / import-alias aggregate-root dynamic-index guard lowering in executable mode, where `VALUES[index + 1]` and `INPUT[state.offset]` can now lower as read-only guard operands
- `18_sync_match_guard_direct_calls.ql`: direct resolved sync scalar guard-call lowering in executable mode, where `enabled()` and `offset(delta: 2, value: current)` can now lower inside `match` guards
- `19_sync_match_guard_call_projection_roots.ql`: direct resolved sync aggregate guard-call projection-root lowering in executable mode, where tuple / struct / fixed-array results like `pair(current)[1]`, `state(current).value`, and `values(current)[1]` can now lower inside `match` guards
- `20_sync_match_guard_aggregate_call_args.ql`: direct resolved sync aggregate guard-call argument lowering in executable mode, where loadable struct / tuple / fixed-array values like `enabled(current)`, `matches(pair(current), 22)`, and `contains(values(current), 4)` can now flow into `match` guards
- `21_sync_match_guard_inline_aggregate_call_args.ql`: direct resolved sync inline aggregate-literal guard-call argument lowering in executable mode, where `enabled(State { ready: true })`, `matches((0, current), 22)`, and `contains([current, current + 1, current + 2], 4)` can now flow into `match` guards
- `22_sync_match_guard_inline_projection_roots.ql`: inline aggregate-literal projection-root guard lowering in executable mode, where `(0, current)[1]`, `State { value: current }.value`, and `[current, current + 1, current + 2][1]` can now lower inside `match` guards
- `23_sync_match_guard_item_backed_inline_combos.ql`: same-file item/import-alias-backed inline aggregate guard combos in executable mode, where `enabled(extra: true, state: state)`, `(INPUT[0], current)[1]`, and `[INPUT[0], current + 1, INPUT[2]][current - 2]` now lower through the existing guard-call / inline aggregate / projection paths
- `24_sync_match_guard_call_backed_combos.ql`: direct sync call-backed guard combos in executable mode, where `enabled(extra: ready(true), state: State { ready: ready(true) })`, `matches((seed(0), current), 22)`, and `items(current)[slot(current)]` now lower through the existing guard-call / inline aggregate / projection paths
- `25_sync_match_guard_call_root_nested_runtime_projection.ql`: direct sync call-root nested runtime projection combos in executable mode, where `pack(current).values[offset(current)]`, `ready(pack(current).values[offset(current)])`, and `check(expected: 4, value: pack(current).values[offset(current)])` now lower through the existing call-root materialize / nested projection / scalar guard-call paths
- `26_sync_match_guard_nested_call_root_inline_combos.ql`: nested call-root inline guard combos in executable mode, where `[pack(current)[slot(current)], current + 1, 6][0]`, `contains([pack(3)[slot(3)], current, 9], 4)`, and `check(expected: 4, value: pair(left: pack(current)[slot(current)], right: 8)[0])` now lower through the existing call-root materialize / runtime projection / inline aggregate / guard-call paths
- `27_sync_match_guard_item_backed_nested_call_root_combos.ql`: item-backed nested call-root guard combos in executable mode, where `enabled(extra: INPUT[0] == bundle(3)[offset(3)], state: state(bundle(3)[offset(3)] == 4))`, `[bundle(current)[offset(current)], INPUT[1], INPUT[2]][0]`, and `check(expected: INPUT[0], value: [bundle(current)[offset(current)], 8, 9][0])` now lower through the existing item-root materialize / nested call-root projection / inline aggregate / guard-call paths
- `28_sync_match_guard_call_backed_nested_call_root_combos.ql`: call-backed nested call-root guard combos in executable mode, where `enabled(extra: flag(pack(3)[slot(3)] == 4), state: state(flag(pack(3)[slot(3)] == 4)))`, `[pack(current)[slot(current)], seed(8), seed(9)][0]`, and `check(expected: seed(4), value: [pack(current)[slot(current)], seed(8), 9][0])` now lower through the existing call-root materialize / call-backed scalar / inline aggregate / guard-call paths
- `29_sync_match_guard_alias_backed_nested_call_root_combos.ql`: alias-backed nested call-root guard combos in executable mode, where `allow(extra: flag(pack(3)[slot(3)] == literal(4)), state: make(flag(pack(3)[slot(3)] == literal(4))))`, `[pack(current)[slot(current)], literal(8), literal(9)][0]`, and `check(expected: literal(4), value: [pack(current)[slot(current)], literal(8), 9][0])` now lower through the existing import-alias call canonicalization / nested call-root projection / inline aggregate / guard-call paths
- `30_sync_match_guard_binding_backed_nested_call_root_combos.ql`: binding-backed nested call-root guard combos in executable mode, where `enabled(extra: bundle(current.value)[offset(current.value)] == 4, state: current)`, `[bundle(current.value)[offset(current.value)], current.value + 5, 9][0]`, and `matches(expected: 4, value: [bundle(current.value)[offset(current.value)], current.value, 9][0])` now lower through the existing non-scalar current-binding guard path / nested call-root projection / inline aggregate / guard-call paths
- `31_sync_match_guard_projection_backed_nested_call_root_combos.ql`: projection-backed nested call-root guard combos in executable mode, where `enabled(extra: bundle(config.slot.value)[offset(config.slot.value)] == 4, state: state(bundle(config.slot.value)[offset(config.slot.value)] == 4))`, `[bundle(config.slot.value)[offset(config.slot.value)], current + 5, 9][0]`, and `matches(expected: 4, value: [bundle(config.slot.value)[offset(config.slot.value)], current, 9][0])` now lower through the existing read-only projection-root guard path / nested call-root projection / inline aggregate / guard-call paths
- `32_sync_for_call_root_fixed_shapes.ql`: direct call-root fixed-shape `for` lowering in executable mode, where `for value in array_values(10)`, `for value in tuple_values(7)`, and `for value in make_payload(3).values` now all lower through the existing fixed-array / homogeneous tuple / projected fixed-shape iterable paths
- `33_sync_import_alias_call_root_fixed_shapes.ql`: same-file import-alias call-root fixed-shape `for` lowering in executable mode, where `for value in values(10)`, `for value in pairs(7)`, and `for value in payload(3).values` now all lower through the existing alias-call canonicalization plus fixed-array / homogeneous tuple / projected fixed-shape iterable paths
- `34_sync_nested_call_root_fixed_shapes.ql`: nested call-root fixed-shape `for` lowering in executable mode, where `for value in array_env(10).payload.values`, `for value in tuple_env(7).payload.values`, and `for value in deep_env(3).outer.payload.values` now all lower through the existing nested projection plus fixed-array / homogeneous tuple iterable paths
- `35_sync_import_alias_nested_call_root_fixed_shapes.ql`: same-file import-alias nested call-root fixed-shape `for` lowering in executable mode, where `for value in arrays(10).payload.values`, `for value in tuples(7).payload.values`, and `for value in deep(3).outer.payload.values` now all lower through the existing alias-call canonicalization plus nested projection and fixed-array / homogeneous tuple iterable paths
- `36_sync_inline_projected_fixed_shapes.ql`: parenthesized inline aggregate projected fixed-shape `for` lowering in executable mode, where `for value in (ArrayPayload { values: [10, 11] }).values`, `for value in (TuplePayload { values: (7, 8) }).values`, and `for value in (DeepPayload { inner: ArrayPayload { values: [3, 3] } }).inner.values` now all lower through the existing inline aggregate materialization plus projected fixed-array / homogeneous tuple iterable paths
- `37_sync_import_alias_inline_projected_fixed_shapes.ql`: same-file import-alias parenthesized inline aggregate projected fixed-shape `for` lowering in executable mode, where `for value in (ArrayPayload { values: [make_value(10), make_value(11)] }).values`, `for value in (TuplePayload { values: (make_value(7), make_value(8)) }).values`, and `for value in (DeepPayload { inner: ArrayPayload { values: [make_value(3), make_value(3)] } }).inner.values` now all lower through the existing alias-call canonicalization plus inline aggregate materialization and projected fixed-array / homogeneous tuple iterable paths
- `38_sync_inline_projected_fixed_shapes_without_parens.ql`: unparenthesized inline aggregate projected fixed-shape `for` lowering in executable mode, where `for value in ArrayPayload { values: [10, 11] }.values`, `for value in TuplePayload { values: (7, 8) }.values`, and `for value in DeepPayload { inner: ArrayPayload { values: [3, 3] } }.inner.values` now all lower through the same inline aggregate materialization plus projected fixed-array / homogeneous tuple iterable paths without requiring an extra outer pair of parentheses
- `39_sync_unparenthesized_inline_projected_control_flow_heads.ql`: unparenthesized inline aggregate projected `if` / `while` / `match` head lowering in executable mode, where `if FlagState { ready: true }.ready`, `while FlagState { ready: false }.ready`, and `match PairPayload { values: (20, 22) }.values[1]` now all lower without requiring an extra outer pair of parentheses around the inline aggregate root
- `40_sync_unsafe_function_bodies.ql`: sync `unsafe fn` body lowering in executable mode, where ordinary unsafe-marked helper bodies now share the same arithmetic/call executable path and still exit with `5`
- `41_sync_assignment_expressions.ql`: sync assignment-expression lowering in executable mode, where mutable-local, struct-field, and fixed-array literal-index assignments can yield a result value that immediately participates in later scalar computation and still exits with `38`
- `42_sync_dynamic_array_assignments.ql`: sync dynamic array assignment lowering in executable mode, where non-`Task[...]` element arrays and nested dynamic array projections can both be assigned through runtime indices and still exit with `18`
- `43_sync_tuple_assignment_expressions.ql`: sync tuple assignment-expression lowering in executable mode, where tuple literal-index assignments yield a result value that immediately participates in later scalar computation and still exit with `19`
- `44_sync_projected_root_dynamic_array_assignments.ql`: sync projected-root dynamic array assignment lowering in executable mode, where a struct field carrying a non-`Task[...]` fixed array can be assigned through a runtime index and still exit with `8`
- `45_sync_projected_root_tuple_assignment_expressions.ql`: sync projected-root tuple assignment-expression lowering in executable mode, where a struct field carrying a tuple can be assigned through tuple literal-index expressions and still exit with `19`
- `46_sync_projected_root_assignment_expressions.ql`: sync projected-root assignment-expression lowering in executable mode, where nested `holder.pair.value = ...` and `holder.pair.values[1] = ...` writes both yield a result value that immediately participates in later scalar computation and still exit with `13`
- `47_sync_dynamic_assignment_expressions.ql`: sync dynamic assignment-expression lowering in executable mode, where both `values[index] = ...` and `wrap.values[index] = ...` on non-`Task[...]` arrays yield a result value that immediately participates in later scalar computation and still exit with `38`
- `48_sync_nested_projected_dynamic_assignment_expressions.ql`: sync nested projected-root dynamic assignment-expression lowering in executable mode, where `env.payload.values[index] = ...` yields a result value that immediately participates in later scalar computation and still exits with `16`
- `49_sync_nested_projected_tuple_assignment_expressions.ql`: sync nested projected-root tuple assignment-expression lowering in executable mode, where `env.inner.pair[0] = ...` and `env.inner.pair[1] = ...` both yield a result value that immediately participates in later scalar computation and still exit with `20`
- `50_sync_nested_projected_assignment_expressions.ql`: sync nested projected-root assignment-expression lowering in executable mode, where `env.holder.pair.value = ...` and `env.holder.pair.values[1] = ...` both yield a result value that immediately participates in later scalar computation and still exit with `14`
- `51_sync_call_root_nested_projected_assignment_expressions.ql`: sync call-root nested projected assignment-expression lowering in executable mode, where `make_env().holder.pair.value = ...` and `make_env().holder.pair.values[1] = ...` both yield a result value that immediately participates in later scalar computation and still exit with `14`
- `52_sync_import_alias_call_root_nested_projected_assignment_expressions.ql`: sync import-alias call-root nested projected assignment-expression lowering in executable mode, where `env().holder.pair.value = ...` and `env().holder.pair.values[1] = ...` both yield a result value that immediately participates in later scalar computation and still exit with `14`
- `53_sync_inline_nested_projected_assignment_expressions.ql`: sync inline nested projected assignment-expression lowering in executable mode, where `(Env { ... }).holder.pair.value = ...` and `(Env { ... }).holder.pair.values[1] = ...` both yield a result value that immediately participates in later scalar computation and still exit with `14`
- `54_sync_call_root_nested_projected_dynamic_assignment_expressions.ql`: sync call-root nested projected dynamic assignment-expression lowering in executable mode, where `make_env().payload.values[index] = ...` yields a result value that immediately participates in later scalar computation and still exits with `12`
- `55_sync_import_alias_call_root_nested_projected_dynamic_assignment_expressions.ql`: sync import-alias call-root nested projected dynamic assignment-expression lowering in executable mode, where `env().payload.values[index] = ...` yields a result value that immediately participates in later scalar computation and still exits with `12`
- `56_sync_inline_nested_projected_dynamic_assignment_expressions.ql`: sync inline nested projected dynamic assignment-expression lowering in executable mode, where `(Env { ... }).payload.values[index] = ...` yields a result value that immediately participates in later scalar computation and still exits with `12`
- `57_sync_call_root_nested_projected_tuple_assignment_expressions.ql`: sync call-root nested projected tuple assignment-expression lowering in executable mode, where `make_env().inner.pair[0] = ...` and `make_env().inner.pair[1] = ...` both yield a result value that immediately participates in later scalar computation and still exit with `20`
- `58_sync_import_alias_call_root_nested_projected_tuple_assignment_expressions.ql`: sync import-alias call-root nested projected tuple assignment-expression lowering in executable mode, where `env().inner.pair[0] = ...` and `env().inner.pair[1] = ...` both yield a result value that immediately participates in later scalar computation and still exit with `20`
- `59_sync_inline_nested_projected_tuple_assignment_expressions.ql`: sync inline nested projected tuple assignment-expression lowering in executable mode, where `(Env { ... }).inner.pair[0] = ...` and `(Env { ... }).inner.pair[1] = ...` both yield a result value that immediately participates in later scalar computation and still exit with `20`
- `60_sync_const_backed_tuple_assignment_expressions.ql`: sync tuple constant-index read/write lowering in executable mode, where same-file `const` / `static` items, struct-field projections, integer arithmetic, and same-file `use ... as ...` aliases can drive tuple indices and still exit with `20`

Additional async program-surface examples live in `ramdon_tests/async_program_surface_examples/`.
They now also build and run successfully with the real local toolchain because program-mode codegen synthesizes the current minimal `qlrt_*` runtime support in-module.

Expected exit codes for the sync examples:

- `01_sync_minimal.ql` -> `42`
- `02_sync_data_shapes.ql` -> `32`
- `03_sync_extern_c_export.ql` -> `42`
- `04_sync_static_item_values.ql` -> `5`
- `05_sync_named_call_arguments.ql` -> `47`
- `06_sync_import_alias_named_call_arguments.ql` -> `49`
- `07_sync_for_fixed_array.ql` -> `42`
- `08_sync_for_tuple.ql` -> `42`
- `09_sync_for_projected_fixed_shape.ql` -> `42`
- `10_sync_for_const_static_fixed_shape.ql` -> `42`
- `11_sync_match_scrutinee_self_guard.ql` -> `42`
- `12_sync_match_scrutinee_bool_comparison_guard.ql` -> `42`
- `13_sync_match_partial_dynamic_guard.ql` -> `42`
- `14_sync_match_partial_integer_dynamic_guard.ql` -> `42`
- `15_sync_match_guard_binding_projection_roots.ql` -> `42`
- `16_sync_match_binding_catch_all_aggregate_scrutinees.ql` -> `42`
- `17_sync_match_guard_runtime_index_item_roots.ql` -> `42`
- `18_sync_match_guard_direct_calls.ql` -> `42`
- `19_sync_match_guard_call_projection_roots.ql` -> `42`
- `20_sync_match_guard_aggregate_call_args.ql` -> `42`
- `21_sync_match_guard_inline_aggregate_call_args.ql` -> `42`
- `22_sync_match_guard_inline_projection_roots.ql` -> `42`
- `23_sync_match_guard_item_backed_inline_combos.ql` -> `42`
- `24_sync_match_guard_call_backed_combos.ql` -> `42`
- `25_sync_match_guard_call_root_nested_runtime_projection.ql` -> `42`
- `26_sync_match_guard_nested_call_root_inline_combos.ql` -> `42`
- `27_sync_match_guard_item_backed_nested_call_root_combos.ql` -> `42`
- `28_sync_match_guard_call_backed_nested_call_root_combos.ql` -> `42`
- `29_sync_match_guard_alias_backed_nested_call_root_combos.ql` -> `42`
- `30_sync_match_guard_binding_backed_nested_call_root_combos.ql` -> `42`
- `31_sync_match_guard_projection_backed_nested_call_root_combos.ql` -> `42`
- `32_sync_for_call_root_fixed_shapes.ql` -> `42`
- `33_sync_import_alias_call_root_fixed_shapes.ql` -> `42`
- `34_sync_nested_call_root_fixed_shapes.ql` -> `42`
- `35_sync_import_alias_nested_call_root_fixed_shapes.ql` -> `42`
- `36_sync_inline_projected_fixed_shapes.ql` -> `42`
- `37_sync_import_alias_inline_projected_fixed_shapes.ql` -> `42`
- `38_sync_inline_projected_fixed_shapes_without_parens.ql` -> `42`
- `39_sync_unparenthesized_inline_projected_control_flow_heads.ql` -> `42`
- `40_sync_unsafe_function_bodies.ql` -> `5`
- `41_sync_assignment_expressions.ql` -> `38`
- `42_sync_dynamic_array_assignments.ql` -> `18`
- `43_sync_tuple_assignment_expressions.ql` -> `19`
- `44_sync_projected_root_dynamic_array_assignments.ql` -> `8`
- `45_sync_projected_root_tuple_assignment_expressions.ql` -> `19`
- `46_sync_projected_root_assignment_expressions.ql` -> `13`
- `47_sync_dynamic_assignment_expressions.ql` -> `38`
- `48_sync_nested_projected_dynamic_assignment_expressions.ql` -> `16`
- `49_sync_nested_projected_tuple_assignment_expressions.ql` -> `20`
- `50_sync_nested_projected_assignment_expressions.ql` -> `14`
- `51_sync_call_root_nested_projected_assignment_expressions.ql` -> `14`
- `52_sync_import_alias_call_root_nested_projected_assignment_expressions.ql` -> `14`
- `53_sync_inline_nested_projected_assignment_expressions.ql` -> `14`
- `54_sync_call_root_nested_projected_dynamic_assignment_expressions.ql` -> `12`
- `55_sync_import_alias_call_root_nested_projected_dynamic_assignment_expressions.ql` -> `12`
- `56_sync_inline_nested_projected_dynamic_assignment_expressions.ql` -> `12`
- `57_sync_call_root_nested_projected_tuple_assignment_expressions.ql` -> `20`
- `58_sync_import_alias_call_root_nested_projected_tuple_assignment_expressions.ql` -> `20`
- `59_sync_inline_nested_projected_tuple_assignment_expressions.ql` -> `20`
- `60_sync_const_backed_tuple_assignment_expressions.ql` -> `20`

Build one verified executable example:

```powershell
cargo run -p ql-cli -- build ramdon_tests/executable_examples/01_sync_minimal.ql --emit exe
```

Build all verified executable examples:

```powershell
fd -e ql . ramdon_tests/executable_examples | sort | % { cargo run -p ql-cli -- build $_ --emit exe }
```

Run the targeted sync/async executable regressions:

```powershell
cargo test -p ql-cli executable_examples_build_and_run
cargo test -p ql-cli async_program_surface_examples_build_and_run
```
