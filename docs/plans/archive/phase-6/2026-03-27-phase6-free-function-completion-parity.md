# 2026-03-27 Phase 6: Free Function Completion Parity

## Why This Slice Exists

Phase 6 already exposed same-file lexical completion for visible value bindings.

That meant free functions were already present on the analysis side as lexical value candidates. But the coverage was still asymmetric:

- analysis already proved that visible value completion could include free functions
- LSP already mapped free-function completion to `FUNCTION`
- there was still no direct parity regression proving that lexical free-function candidates stayed aligned across analysis and the LSP bridge

Without that explicit coverage, lexical completion could drift so that callable declarations still existed in `QueryIndex` while the editor-facing projection stopped behaving consistently.

## What Changed

Added explicit completion parity coverage only. No new semantic behavior or wider completion surface was introduced.

Analysis regression added:

- `completion_queries_surface_free_function_candidates_in_value_contexts`

LSP bridge regression added:

- `completion_bridge_maps_free_function_value_candidates`

These tests lock the existing behavior that free functions:

- appear in same-file lexical value completion
- keep `SymbolKind::Function` on the analysis side
- map to `CompletionItemKind::FUNCTION` on the LSP side
- preserve stable replacement edits

## Boundary

This slice stays conservative:

- no cross-file/module-graph behavior
- no new completion namespaces
- no callable-value semantic expansion
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
- `cargo test -p ql-analysis --test queries completion_queries_surface_free_function_candidates_in_value_contexts -- --exact`
- `cargo test -p ql-lsp --test bridge completion_bridge_maps_free_function_value_candidates -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`

## Recommended Next Direction

Keep Phase 6 conservative. Continue on already-supported same-file semantic identities where behavior exists but explicit parity coverage or doc precision still lags.
