# 2026-03-27 Phase 6: Unique Method Completion Parity

## Why This Slice Exists

Phase 6 already had same-file member completion on the analysis side for stable receiver types.

That meant uniquely resolved method symbols were already available as member completion candidates through `QueryIndex`, and the LSP bridge already knew how to map:

- `SymbolKind::Method` -> `CompletionItemKind::METHOD`

But there was still no explicit parity regression proving that already-supported unique method candidates stayed aligned end to end for detail rendering, kind projection, and editor-facing replacement edits.

Without that coverage, stable receiver member completion could drift so that unique method candidates remained present in analysis while the LSP-facing projection silently changed.

## What Changed

Added explicit completion parity coverage only. No new semantic behavior or wider completion surface was introduced.

Analysis regression added:

- `completion_queries_surface_unique_method_candidates_on_stable_receiver_types`

LSP bridge regression added:

- `completion_bridge_maps_unique_method_candidates_on_stable_receiver_types`

These tests lock the existing behavior that stable-receiver unique method candidates:

- appear in same-file member completion
- keep `SymbolKind::Method` on the analysis side
- map to `CompletionItemKind::METHOD` on the LSP side
- preserve stable detail rendering and replacement edits

## Boundary

This slice stays conservative:

- no cross-file/module-graph behavior
- no new completion namespaces
- no ambiguous member expansion
- no new method-resolution rules
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
- `cargo test -p ql-analysis --test queries completion_queries_surface_unique_method_candidates_on_stable_receiver_types -- --exact`
- `cargo test -p ql-lsp --test bridge completion_bridge_maps_unique_method_candidates_on_stable_receiver_types -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`

## Recommended Next Direction

Keep Phase 6 conservative. Continue only on already-supported same-file semantic identities where behavior already exists but parity coverage or doc precision still lags.
