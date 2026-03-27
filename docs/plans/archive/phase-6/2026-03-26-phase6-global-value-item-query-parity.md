# 2026-03-26 Phase 6: Global Value Item Query Parity

## Why This Slice Exists

Phase 6 had already locked same-file rename behavior for `const` / `static` through shorthand struct-field rename regressions, but it still lacked direct parity tests for the more basic query surfaces on those same global value items.

That left a gap: item definitions and ordinary value uses of `const` / `static` were expected to share one `QueryIndex` identity, yet there was no explicit regression proving that analysis queries and LSP bridge behavior stayed aligned for hover, definition, references, and semantic tokens.

## What Changed

Added coverage only. No new query surface or LSP heuristic was introduced.

Analysis regressions added:

- `global_value_item_queries_follow_same_file_identity`
- `semantic_tokens_follow_global_value_item_surface`

LSP bridge regressions added:

- `hover_definition_and_references_bridge_follow_global_value_item_symbols`
- `semantic_tokens_bridge_maps_global_value_item_surface`

These tests lock same-file parity for:

- `const`
- `static`

across:

- hover
- go to definition
- find references
- semantic tokens

## Boundary

This slice stays conservative:

- no cross-file/module-graph behavior
- no new symbol kinds
- no LSP-local special casing
- no expansion of rename semantics beyond the already-supported same-file surface

`QueryIndex` remains the single semantic truth source.

## Docs Updated

- `README.md`
- `docs/architecture/compiler-pipeline.md`
- `docs/architecture/implementation-algorithms.md`
- `docs/architecture/toolchain.md`
- `docs/roadmap/development-plan.md`
- `docs/roadmap/phase-progress.md`
- `docs/.vitepress/config.mts`

## Verification

- `cargo fmt --all`
- `cargo test -p ql-analysis --test queries global_value_item_queries_follow_same_file_identity -- --exact`
- `cargo test -p ql-analysis --test queries semantic_tokens_follow_global_value_item_surface -- --exact`
- `cargo test -p ql-lsp --test bridge hover_definition_and_references_bridge_follow_global_value_item_symbols -- --exact`
- `cargo test -p ql-lsp --test bridge semantic_tokens_bridge_maps_global_value_item_surface -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs/`

## Recommended Next Direction

Continue only on already-supported same-file semantic surfaces where `QueryIndex` already carries stable identity. The next safe candidates remain parity gaps, not new cross-file semantics.
