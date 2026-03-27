# 2026-03-27 Phase 6: Type Alias Completion Parity

## Why This Slice Exists

Phase 6 already had same-file type-context completion on the analysis side, and recent completion regressions already locked:

- plain import-alias type candidates
- builtin type candidates
- local struct type candidates

But there was still no direct parity regression proving that same-file type aliases kept their editor-facing completion projection.

Without that explicit coverage, `QueryIndex` could continue surfacing `type alias` candidates while the LSP bridge silently drifted away from the expected completion-item mapping or text-edit behavior.

## What Changed

Added explicit completion parity coverage only. No new semantic behavior or wider completion surface was introduced.

Analysis regression added:

- `completion_queries_surface_type_alias_candidates_by_prefix`

LSP bridge regression added:

- `completion_bridge_maps_type_alias_type_candidates`

These tests lock the existing behavior that same-file type aliases:

- appear in type-context completion
- keep `SymbolKind::TypeAlias` on the analysis side
- map to `CompletionItemKind::CLASS` on the LSP side
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
- `cargo test -p ql-analysis --test queries completion_queries_surface_type_alias_candidates_by_prefix -- --exact`
- `cargo test -p ql-lsp --test bridge completion_bridge_maps_type_alias_type_candidates -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`

## Recommended Next Direction

Keep Phase 6 conservative. Continue only on already-supported same-file semantic identities where behavior already exists but parity coverage or doc precision still lags.
