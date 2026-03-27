# Phase 6: Opaque Type Parity

## Goal

Lock the already-supported `opaque type` surface into the same type-namespace parity matrix used by ordinary `type` aliases.

## Scope

- add explicit analysis regressions for `opaque type` hover / definition / references
- add explicit analysis regressions for `opaque type` same-file prepare-rename / rename
- add explicit analysis regressions for `opaque type` semantic tokens
- add explicit analysis regressions for `opaque type` type-context completion
- add explicit LSP bridge regressions for the same hover / definition / references / prepare-rename / rename / semantic-token / completion surfaces
- update roadmap and architecture docs so type-namespace parity and type-alias completion parity both include `opaque type`

## Non-goals

- no new nominal typing semantics
- no expansion of `opaque type` lowering or backend behavior
- no cross-file or module-graph alias lookup
- no divergence from the existing `SymbolKind::TypeAlias` model

## Verification

- `cargo fmt --all`
- `cargo test -p ql-analysis --test queries opaque_type_queries_follow_type_namespace_item_symbols -- --exact`
- `cargo test -p ql-analysis --test queries rename_queries_follow_opaque_type_symbols -- --exact`
- `cargo test -p ql-analysis --test queries semantic_tokens_follow_opaque_type_surface -- --exact`
- `cargo test -p ql-analysis --test queries completion_queries_surface_opaque_type_candidates_by_prefix -- --exact`
- `cargo test -p ql-lsp --test bridge hover_definition_and_references_bridge_follow_opaque_type_symbols -- --exact`
- `cargo test -p ql-lsp --test bridge rename_bridge_supports_opaque_type_symbols -- --exact`
- `cargo test -p ql-lsp --test bridge semantic_tokens_bridge_maps_opaque_type_surface -- --exact`
- `cargo test -p ql-lsp --test bridge completion_bridge_maps_opaque_type_candidates_by_prefix -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build`
