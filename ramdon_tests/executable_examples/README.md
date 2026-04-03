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
