# 2026-03-27 Phase 6: Variant Completion Parity

## Why This Slice Exists

Phase 6 already had same-file parsed enum path completion on the analysis side.

That meant enum variant symbols were already available as completion candidates through `QueryIndex`, and the LSP bridge already knew how to map:

- `SymbolKind::Variant` -> `CompletionItemKind::ENUM_MEMBER`

But there was still no explicit parity regression proving that already-supported variant candidates stayed aligned end to end for detail rendering, kind projection, and editor-facing replacement edits.

Without that coverage, parsed enum path completion could drift so that variant candidates remained present in analysis while the LSP-facing projection silently changed.

## What Changed

Added explicit completion parity coverage only. No new semantic behavior or wider completion surface was introduced.

Analysis regression added:

- `completion_queries_surface_variant_candidates_on_enum_item_roots`

LSP bridge regression added:

- `completion_bridge_maps_variant_candidates_on_enum_item_roots`

These tests lock the existing behavior that same-file enum variant candidates:

- appear in parsed enum path completion
- keep `SymbolKind::Variant` on the analysis side
- map to `CompletionItemKind::ENUM_MEMBER` on the LSP side
- preserve stable detail rendering and replacement edits

## Boundary

This slice stays conservative:

- no cross-file/module-graph behavior
- no new completion namespaces
- no foreign import alias semantics
- no variant-field completion expansion
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
- `cargo test -p ql-analysis --test queries completion_queries_surface_variant_candidates_on_enum_item_roots -- --exact`
- `cargo test -p ql-lsp --test bridge completion_bridge_maps_variant_candidates_on_enum_item_roots -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`

## Recommended Next Direction

Keep Phase 6 conservative. Continue only on already-supported same-file semantic identities where behavior already exists but parity coverage or doc precision still lags.
