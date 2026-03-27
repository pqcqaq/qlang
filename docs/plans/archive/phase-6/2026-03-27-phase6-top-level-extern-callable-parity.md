# Phase 6: Top-Level Extern Callable Parity

## Goal

Lock the already-supported same-file callable query surface for top-level `extern "c"` declarations so it stays aligned with the existing `extern` block coverage.

## Scope

- add explicit analysis regressions for hover / definition / references on top-level `extern "c"` declarations
- add explicit analysis regressions for same-file prepare-rename / rename on top-level `extern "c"` declarations
- add explicit analysis regressions for semantic tokens on top-level `extern "c"` declarations
- add explicit LSP bridge regressions for the same hover / definition / references / prepare-rename / rename / semantic-token surface
- update roadmap and architecture docs so `extern callable parity` no longer reads as `extern block`-only

## Non-goals

- no module-graph or cross-file extern lookup
- no ABI-surface expansion beyond existing `extern "c"` declarations
- no special-casing for foreign-library indexing or host header discovery
- no change to top-level `extern "c"` function-definition semantics

## Verification

- `cargo fmt --all`
- `cargo test -p ql-analysis --test queries top_level_extern_function_queries_follow_callable_declarations -- --exact`
- `cargo test -p ql-analysis --test queries rename_queries_follow_top_level_extern_function_symbols -- --exact`
- `cargo test -p ql-analysis --test queries semantic_tokens_follow_top_level_extern_function_surface -- --exact`
- `cargo test -p ql-lsp --test bridge hover_definition_and_references_bridge_follow_top_level_extern_function_symbols -- --exact`
- `cargo test -p ql-lsp --test bridge rename_bridge_supports_top_level_extern_function_symbols -- --exact`
- `cargo test -p ql-lsp --test bridge semantic_tokens_bridge_maps_top_level_extern_function_surface -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build`
