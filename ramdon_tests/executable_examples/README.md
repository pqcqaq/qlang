# Executable Examples

These examples were verified with the real local toolchain and link successfully with `ql build --emit exe`.

Files:

- `01_sync_minimal.ql`: free functions, arithmetic, compare, `if`
- `02_sync_data_shapes.ql`: structs, tuples, arrays, nested projections, dynamic array assignment, zero-sized arrays with expected context
- `03_sync_extern_c_export.ql`: top-level `extern "c" pub fn` definition plus normal `main`

Additional async program-surface examples live in `ramdon_tests/async_program_surface_examples/`.
They now also build and run successfully with the real local toolchain because program-mode codegen synthesizes the current minimal `qlrt_*` runtime support in-module.

Expected exit codes for the sync examples:

- `01_sync_minimal.ql` -> `42`
- `02_sync_data_shapes.ql` -> `32`
- `03_sync_extern_c_export.ql` -> `42`

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
