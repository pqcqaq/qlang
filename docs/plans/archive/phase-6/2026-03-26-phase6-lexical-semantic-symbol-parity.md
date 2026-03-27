# 2026-03-26 Phase 6: Lexical Semantic Symbol Parity

## Why This Slice Exists

Phase 6 already had pieces of lexical-symbol coverage scattered across the stack:

- analysis definition / reference coverage for `generic`, `parameter`, and `local`
- analysis hover coverage for `receiver self` and `builtin type`
- a minimal LSP hover regression for one parameter case
- semantic-token plumbing for lexical symbol kinds

What was missing was a single regression-locked statement that these lexical semantic symbols continue to behave consistently across analysis and LSP.

## What Changed

Added coverage only. No new semantic capability or protocol-local heuristic was introduced.

Analysis regressions added:

- `lexical_semantic_symbol_queries_follow_same_file_identity`
- `semantic_tokens_follow_lexical_semantic_symbol_surface`

LSP bridge regressions added:

- `hover_definition_and_references_bridge_follow_lexical_semantic_symbols`
- `semantic_tokens_bridge_maps_lexical_semantic_symbol_surface`

These lock same-file parity for:

- `generic`
- `parameter`
- `local`
- `receiver self`
- `builtin type`

across:

- hover
- go to definition
- find references
- semantic tokens

## Important Boundary

`builtin type` is intentionally different from the other lexical symbols:

- it participates in hover
- it participates in same-file references
- it participates in semantic tokens
- it does **not** have a source-backed definition span
- it does **not** participate in rename

The regression now explicitly locks that split behavior.

## Boundary

This slice stays conservative:

- no cross-file/module-graph behavior
- no new symbol kinds
- no rename-surface expansion
- no LSP-local special casing

Lexical symbols continue to flow through the existing `QueryIndex` truth surface.

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
- `cargo test -p ql-analysis --test queries lexical_semantic_symbol_queries_follow_same_file_identity -- --exact`
- `cargo test -p ql-analysis --test queries semantic_tokens_follow_lexical_semantic_symbol_surface -- --exact`
- `cargo test -p ql-lsp --test bridge hover_definition_and_references_bridge_follow_lexical_semantic_symbols -- --exact`
- `cargo test -p ql-lsp --test bridge semantic_tokens_bridge_maps_lexical_semantic_symbol_surface -- --exact`

## Recommended Next Direction

Keep Phase 6 conservative. Prefer remaining parity gaps on already-supported symbol identities, especially where current docs still under-describe real behavior.
