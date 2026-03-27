# 2026-03-27 Phase 6: Generic Completion Parity

## Why This Slice Exists

Phase 6 already had same-file type-context completion on the analysis side, and broader coverage already proved that generic parameters could appear in lexical type completion.

The LSP bridge also already knew how to project `SymbolKind::Generic` into `CompletionItemKind::TYPE_PARAMETER`.

But there was still no explicit parity slice that locked the already-supported generic candidate shape end to end in the same style as the recent import-alias, free-function, builtin/struct, and type-alias completion hardening.

Without that coverage, generic candidates could remain present in `QueryIndex` while prefix filtering, detail rendering, or editor-facing completion projection quietly drifted.

## What Changed

Added explicit completion parity coverage only. No new semantic behavior or wider completion surface was introduced.

Analysis regression added:

- `completion_queries_surface_generic_type_candidates_by_prefix`

LSP bridge regression added:

- `completion_bridge_maps_generic_type_candidates_by_prefix`

These tests lock the existing behavior that same-file generic type candidates:

- appear in type-context completion
- keep `SymbolKind::Generic` on the analysis side
- map to `CompletionItemKind::TYPE_PARAMETER` on the LSP side
- preserve stable `detail` rendering and replacement edits

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
- `cargo test -p ql-analysis --test queries completion_queries_surface_generic_type_candidates_by_prefix -- --exact`
- `cargo test -p ql-lsp --test bridge completion_bridge_maps_generic_type_candidates_by_prefix -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`

## Recommended Next Direction

Keep Phase 6 conservative. Continue only on already-supported same-file semantic identities where behavior already exists but parity coverage or doc precision still lags.
