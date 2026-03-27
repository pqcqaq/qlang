# 2026-03-27 Phase 6: Builtin / Struct Completion Parity

## Why This Slice Exists

Phase 6 already had same-file type-context completion on the analysis side, and broader tests already proved that type candidates could include:

- builtin types
- generic parameters
- local struct items
- import aliases

But the editor-facing parity coverage was still incomplete. The LSP bridge had direct completion regressions for:

- generic type candidates
- plain import-alias type candidates

There was still no direct parity regression proving that builtin types and local struct items kept their expected LSP completion-item mappings.

## What Changed

Added explicit completion parity coverage only. No new semantic behavior or wider completion surface was introduced.

Analysis regression added:

- `completion_queries_surface_builtin_and_struct_type_candidates_by_prefix`

LSP bridge regression added:

- `completion_bridge_maps_builtin_and_struct_type_candidates`

These tests lock the existing behavior that builtin types and local struct items:

- appear in same-file type-context completion
- keep `SymbolKind::BuiltinType` / `SymbolKind::Struct` on the analysis side
- map to `CompletionItemKind::CLASS` / `CompletionItemKind::STRUCT` on the LSP side
- preserve stable replacement edits

## Boundary

This slice stays conservative:

- no cross-file/module-graph behavior
- no new completion namespaces
- no type-system expansion
- no rename/query surface changes
- no LSP-local heuristics

`ql-analysis::QueryIndex` remains the only completion truth source.

## Docs Updated

- `README.md`
- `docs/architecture/compiler-pipeline.md`
- `docs/architecture/toolchain.md`
- `docs/roadmap/development-plan.md`
- `docs/roadmap/phase-progress.md`
- `docs/.vitepress/config.mts`

## Verification

- `cargo fmt --all`
- `cargo test -p ql-analysis --test queries completion_queries_surface_builtin_and_struct_type_candidates_by_prefix -- --exact`
- `cargo test -p ql-lsp --test bridge completion_bridge_maps_builtin_and_struct_type_candidates -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`

## Recommended Next Direction

Keep Phase 6 conservative. Continue on already-supported same-file semantic identities where behavior exists but explicit parity coverage or doc precision still lags.
