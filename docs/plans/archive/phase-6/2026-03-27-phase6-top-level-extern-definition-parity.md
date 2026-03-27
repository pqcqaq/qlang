# Phase 6: Top-Level Extern Definition Parity

## Goal

Lock the already-supported same-file callable query surface for top-level `extern "c"` function definitions with bodies, so it stays aligned with the existing extern-declaration parity coverage.

## Scope

- add explicit analysis regressions for hover / definition / references on top-level `extern "c"` function definitions
- add explicit analysis regressions for same-file prepare-rename / rename on top-level `extern "c"` function definitions
- add explicit analysis regressions for semantic tokens on top-level `extern "c"` function definitions
- add explicit LSP bridge regressions for the same hover / definition / references / prepare-rename / rename / semantic-token surface
- update roadmap and architecture docs so `extern callable parity` clearly includes top-level extern definitions

## Non-goals

- no cross-file or module-graph extern lookup
- no ABI-surface expansion beyond the existing top-level `extern "c"` definition support
- no special treatment for exported symbol visibility or host-linker metadata
- no change to backend lowering for exported C symbols

## Verification

- `cargo fmt --all`
- `cargo test -p ql-analysis --test queries top_level_extern_function_definition_queries_follow_callable_symbols -- --exact`
- `cargo test -p ql-analysis --test queries rename_queries_follow_top_level_extern_function_definition_symbols -- --exact`
- `cargo test -p ql-analysis --test queries semantic_tokens_follow_top_level_extern_function_definition_surface -- --exact`
- `cargo test -p ql-lsp --test bridge hover_definition_and_references_bridge_follow_top_level_extern_function_definition_symbols -- --exact`
- `cargo test -p ql-lsp --test bridge rename_bridge_supports_top_level_extern_function_definition_symbols -- --exact`
- `cargo test -p ql-lsp --test bridge semantic_tokens_bridge_maps_top_level_extern_function_definition_surface -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build`
