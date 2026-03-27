# 2026-03-27 Phase 6: Import Alias Value Completion Parity

## Why This Slice Exists

Phase 6 already had same-file lexical value completion on the analysis side.

That meant plain `import` bindings were already available as value candidates through `QueryIndex`, and the LSP bridge already knew how to map:

- `SymbolKind::Import` -> `CompletionItemKind::MODULE`

But there was still no explicit parity regression proving that already-supported import-alias value candidates stayed aligned end to end for prefix completion, detail rendering, and editor-facing projection.

Without that coverage, same-file value completion could drift so that source-backed import bindings remained present in analysis while the LSP-facing kind or replacement edits silently changed.

## What Changed

Added explicit completion parity coverage only. No new semantic behavior or wider completion surface was introduced.

Analysis regression added:

- `completion_queries_surface_plain_import_alias_candidates_in_value_contexts`

LSP bridge regression added:

- `completion_bridge_maps_plain_import_alias_value_candidates`

These tests lock the existing behavior that same-file import-alias value candidates:

- appear in lexical value completion
- keep `SymbolKind::Import` on the analysis side
- map to `CompletionItemKind::MODULE` on the LSP side
- preserve stable detail rendering and replacement edits

## Boundary

This slice stays conservative:

- no cross-file/module-graph behavior
- no new completion namespaces
- no value-semantics expansion
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
- `cargo test -p ql-analysis --test queries completion_queries_surface_plain_import_alias_candidates_in_value_contexts -- --exact`
- `cargo test -p ql-lsp --test bridge completion_bridge_maps_plain_import_alias_value_candidates -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`

## Recommended Next Direction

Keep Phase 6 conservative. Continue only on already-supported same-file semantic identities where behavior already exists but parity coverage or doc precision still lags.
