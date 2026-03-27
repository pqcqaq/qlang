# 2026-03-27 Phase 6: Function Shorthand Rename Parity

## Why This Slice Exists

Phase 6 already had conservative shorthand-binding rename behavior for renameable symbols launched from struct-literal shorthand tokens.

That behavior was already explicitly covered in analysis for a free-function binding:

- `Ops { add_one }`
- rename launched from the shorthand token
- field label preserved as `add_one: inc`

But the LSP bridge still lacked an explicit end-to-end parity regression for this free-function path, even though similar bridge coverage already existed for:

- local bindings
- import aliases
- `const`
- `static`

## What Changed

Added one LSP parity regression:

- `rename_bridge_preserves_function_shorthand_binding_sites`

This test locks that a free-function shorthand token:

- prepares rename on the shorthand token itself
- preserves the field label during rename
- renames the bound function declaration and same-file call site

## Boundary

This slice stays conservative:

- no new rename surface
- no field-symbol widening from shorthand tokens
- no cross-file rename
- no bridge-local heuristics

`ql-analysis::QueryIndex` remains the only truth source for binding selection and edit grouping.

## Docs Updated

- `README.md`
- `docs/.vitepress/config.mts`
- `docs/architecture/compiler-pipeline.md`
- `docs/architecture/implementation-algorithms.md`
- `docs/architecture/toolchain.md`
- `docs/roadmap/development-plan.md`
- `docs/roadmap/phase-progress.md`

## Verification

- `cargo test -p ql-analysis --test queries function_rename_preserves_shorthand_struct_literal_sites -- --exact`
- `cargo test -p ql-lsp --test bridge rename_bridge_preserves_function_shorthand_binding_sites -- --exact`

## Recommended Next Direction

Keep Phase 6 conservative. Continue only on already-supported same-file query / completion / rename / semantic-token behavior whose editor-facing parity is still weaker than the shared analysis surface.
