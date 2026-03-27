# 2026-03-27 Phase 6 Type-Namespace Surface Aggregate Refresh

## Goal

Strengthen the already-supported same-file type-namespace editor surface without expanding semantics.

## Scope

- keep `QueryIndex` as the single truth source
- keep behavior same-file only
- keep module graph, cross-file lookup, and foreign symbol semantics closed
- add aggregate regression coverage for the current type-namespace item surface:
  - `type`
  - `opaque type`
  - `struct`
  - `enum`
  - `trait`

## Planned Work

1. Add an analysis-side aggregate regression that locks hover, definition, and references across the supported same-file type-namespace item surface.
2. Add an LSP bridge aggregate regression that locks the same hover / definition / references projection.
3. Add aggregate semantic-token regressions on both sides so type-namespace declarations and uses stay mapped to the stable item token surface.
4. Refresh roadmap and architecture docs so the aggregate hardening is explicit.

## Verification

- `cargo fmt --all`
- `cargo test -p ql-analysis --test queries type_namespace_item_queries_follow_same_file_surface_aggregate -- --exact`
- `cargo test -p ql-analysis --test queries semantic_tokens_follow_same_file_type_namespace_item_surface -- --exact`
- `cargo test -p ql-lsp --test bridge hover_definition_and_references_bridge_follow_same_file_type_namespace_item_surface -- --exact`
- `cargo test -p ql-lsp --test bridge semantic_tokens_bridge_maps_same_file_type_namespace_item_surface -- --exact`
- `cargo test`
- `npm run build`
