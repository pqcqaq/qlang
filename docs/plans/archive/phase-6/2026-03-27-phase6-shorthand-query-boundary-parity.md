# 2026-03-27 Phase 6: Shorthand Query Boundary Parity

## Why This Slice Exists

Phase 6 already had an analysis-side regression proving that shorthand struct field tokens such as `Point { x }` still resolve as the local binding on that token, not as the struct field symbol.

That boundary matters because the project intentionally keeps shorthand field tokens conservative:

- explicit field labels participate in the field query surface
- shorthand tokens themselves stay on the binding/local surface

The LSP bridge already reused the same analysis layer, but this precise shorthand query boundary was not explicitly locked on the editor-facing side.

## What Changed

Added LSP parity-only regression coverage. No new semantic behavior or wider query surface was introduced.

LSP bridge regression added:

- `hover_and_definition_bridge_keep_shorthand_struct_field_tokens_on_local_symbols`

This test locks the existing behavior that a shorthand struct literal token:

- renders hover as a `local`
- keeps the local binding detail/type surface
- navigates definition back to the local binding declaration rather than the struct field

## Boundary

This slice stays conservative:

- no new field query semantics
- no shorthand token promotion into field symbols
- no cross-file behavior
- no LSP-local heuristics

`ql-analysis::QueryIndex` remains the only query truth source.

## Docs Updated

- `README.md`
- `docs/architecture/compiler-pipeline.md`
- `docs/architecture/toolchain.md`
- `docs/roadmap/development-plan.md`
- `docs/roadmap/phase-progress.md`
- `docs/.vitepress/config.mts`

## Verification

- `cargo fmt --all`
- `cargo test -p ql-lsp --test bridge hover_and_definition_bridge_keep_shorthand_struct_field_tokens_on_local_symbols -- --exact`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`

## Recommended Next Direction

Keep Phase 6 conservative. Continue only on already-supported same-file semantic/query boundaries where the LSP-facing parity is still only indirectly covered.
