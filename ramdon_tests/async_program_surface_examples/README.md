# Async Program Surface Examples

These files cover the current async `BuildEmit::Executable` surface that exists in parser/typeck/borrowck/codegen/driver tests:

- `04_async_main_basics.ql`
- `05_async_main_aggregates_and_for_await.ql`
- `06_async_main_task_handle_payloads.ql`
- `07_async_main_projection_reinit.ql`
- `08_async_main_dynamic_task_arrays.ql`
- `09_async_main_zero_sized.ql`
- `10_async_main_guard_refined_projected_root.ql`
- `11_async_main_const_backed_projected_root.ql`
- `12_async_main_aliased_projected_root.ql`
- `13_async_main_aliased_const_backed_projected_root.ql`
- `14_async_main_aliased_guard_refined_projected_root.ql`
- `15_async_main_aliased_guard_refined_const_backed_projected_root.ql`
- `16_async_main_aliased_projected_root_tuple_repackage_reinit.ql`
- `17_async_main_aliased_projected_root_struct_repackage_reinit.ql`
- `18_async_main_aliased_projected_root_nested_repackage_reinit.ql`
- `19_async_main_aliased_guard_refined_const_backed_nested_repackage_reinit.ql`
- `20_async_main_aliased_projected_root_nested_repackage_spawn.ql`
- `21_async_main_aliased_guard_refined_const_backed_nested_repackage_spawn.ql`
- `22_async_main_aliased_projected_root_array_repackage_spawn.ql`
- `23_async_main_aliased_guard_refined_const_backed_array_repackage_spawn.ql`
- `24_async_main_aliased_projected_root_nested_array_repackage_spawn.ql`
- `25_async_main_aliased_guard_refined_const_backed_nested_array_repackage_spawn.ql`
- `26_async_main_composed_dynamic_nested_array_repackage_spawn.ql`
- `27_async_main_alias_sourced_composed_dynamic_nested_array_repackage_spawn.ql`
- `28_async_main_guarded_alias_sourced_composed_dynamic_nested_array_repackage_spawn.ql`
- `29_async_main_aliased_projected_root_forwarded_nested_array_repackage_spawn.ql`
- `30_async_main_aliased_guard_refined_const_backed_forwarded_nested_array_repackage_spawn.ql`
- `31_async_main_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn_with_tail_field.ql`
- `32_async_main_guarded_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn_with_tail_field.ql`

Current status:

- They are useful examples of the implemented async executable surface.
- In this workspace, real local `ql build --emit exe` now succeeds for these files because program-mode codegen synthesizes the current minimal `qlrt_*` runtime support in-module.
- `crates/ql-cli/tests/executable_examples.rs` now builds and runs these twenty-nine examples with the real local toolchain and locks their exit codes.

Expected exit codes:

- `04_async_main_basics.ql` -> `28`
- `05_async_main_aggregates_and_for_await.ql` -> `71`
- `06_async_main_task_handle_payloads.ql` -> `39`
- `07_async_main_projection_reinit.ql` -> `45`
- `08_async_main_dynamic_task_arrays.ql` -> `70`
- `09_async_main_zero_sized.ql` -> `10`
- `10_async_main_guard_refined_projected_root.ql` -> `49`
- `11_async_main_const_backed_projected_root.ql` -> `24`
- `12_async_main_aliased_projected_root.ql` -> `17`
- `13_async_main_aliased_const_backed_projected_root.ql` -> `17`
- `14_async_main_aliased_guard_refined_projected_root.ql` -> `21`
- `15_async_main_aliased_guard_refined_const_backed_projected_root.ql` -> `25`
- `16_async_main_aliased_projected_root_tuple_repackage_reinit.ql` -> `31`
- `17_async_main_aliased_projected_root_struct_repackage_reinit.ql` -> `32`
- `18_async_main_aliased_projected_root_nested_repackage_reinit.ql` -> `33`
- `19_async_main_aliased_guard_refined_const_backed_nested_repackage_reinit.ql` -> `36`
- `20_async_main_aliased_projected_root_nested_repackage_spawn.ql` -> `34`
- `21_async_main_aliased_guard_refined_const_backed_nested_repackage_spawn.ql` -> `38`
- `22_async_main_aliased_projected_root_array_repackage_spawn.ql` -> `37`
- `23_async_main_aliased_guard_refined_const_backed_array_repackage_spawn.ql` -> `40`
- `24_async_main_aliased_projected_root_nested_array_repackage_spawn.ql` -> `41`
- `25_async_main_aliased_guard_refined_const_backed_nested_array_repackage_spawn.ql` -> `46`
- `26_async_main_composed_dynamic_nested_array_repackage_spawn.ql` -> `47`
- `27_async_main_alias_sourced_composed_dynamic_nested_array_repackage_spawn.ql` -> `48`
- `28_async_main_guarded_alias_sourced_composed_dynamic_nested_array_repackage_spawn.ql` -> `50`
- `29_async_main_aliased_projected_root_forwarded_nested_array_repackage_spawn.ql` -> `52`
- `30_async_main_aliased_guard_refined_const_backed_forwarded_nested_array_repackage_spawn.ql` -> `54`
- `31_async_main_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn_with_tail_field.ql` -> `59`
- `32_async_main_guarded_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn_with_tail_field.ql` -> `63`

Try one file directly:

```powershell
cargo run -p ql-cli -- build ramdon_tests/async_program_surface_examples/04_async_main_basics.ql --emit exe
```

Run the targeted regression:

```powershell
cargo test -p ql-cli async_program_surface_examples_build_and_run
```
