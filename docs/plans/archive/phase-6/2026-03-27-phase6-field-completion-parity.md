# 2026-03-27 Phase 6: Field Completion Parity

## Why This Slice Exists

Phase 6 already had same-file member completion on the analysis side for stable receiver types.

That meant struct-field symbols were already present as member completion candidates through `QueryIndex`, and the LSP bridge already knew how to map:

- `SymbolKind::Field` -> `CompletionItemKind::FIELD`

But there was still no explicit parity regression proving that field candidates stayed aligned end to end for prefix completion, detail rendering, and editor-facing projection.

Without that coverage, stable receiver member completion could drift so that field candidates remained present in analysis while the LSP-facing kind or replacement edit silently changed.

## What Changed

Added explicit completion parity coverage only. No new semantic behavior or wider completion surface was introduced.

Analysis regression added:

- `completion_queries_surface_field_candidates_on_stable_receiver_types`

LSP bridge regression added:

- `completion_bridge_maps_field_candidates_on_stable_receiver_types`

These tests lock the existing behavior that stable-receiver field candidates:

- appear in same-file member completion
- keep `SymbolKind::Field` on the analysis side
- map to `CompletionItemKind::FIELD` on the LSP side
- preserve stable detail rendering and replacement edits

## Boundary

This slice stays conservative:

- no cross-file/module-graph behavior
- no new completion namespaces
- no new member-resolution rules
- no ambiguous member expansion
- no parse-error-tolerant dot-trigger completion
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
- `cargo test -p ql-analysis --test queries completion_queries_surface_field_candidates_on_stable_receiver_types -- --exact`
- `cargo test -p ql-lsp --test bridge completion_bridge_maps_field_candidates_on_stable_receiver_types -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`

## Recommended Next Direction

Keep Phase 6 conservative. Continue only on already-supported same-file semantic identities where behavior already exists but parity coverage or doc precision still lags.
