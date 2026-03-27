# 2026-03-27 Phase 6 Direct Member Surface Aggregate Refresh

## Goal

Strengthen the already-supported same-file direct member editor surface without expanding semantics.

## Scope

- keep `QueryIndex` as the single truth source
- keep behavior same-file only
- keep module graph, cross-file lookup, and foreign symbol semantics closed
- add aggregate regression coverage for the current direct member surface:
  - direct field members
  - unique direct method members

## Planned Work

1. Add an analysis-side aggregate regression that locks hover, definition, and references across the supported same-file direct member surface.
2. Add an LSP bridge aggregate regression that locks the same hover / definition / references projection.
3. Add aggregate semantic-token regressions on both sides so direct field/method members stay mapped to the stable direct-member token surface.
4. Refresh roadmap and architecture docs so the aggregate hardening is explicit.

## Verification

- `cargo fmt --all`
- `cargo test -p ql-analysis --test queries direct_member_queries_follow_same_file_surface_aggregate -- --exact`
- `cargo test -p ql-analysis --test queries semantic_tokens_follow_same_file_direct_member_surface -- --exact`
- `cargo test -p ql-lsp --test bridge hover_definition_and_references_bridge_follow_same_file_direct_member_surface -- --exact`
- `cargo test -p ql-lsp --test bridge semantic_tokens_bridge_maps_same_file_direct_member_surface -- --exact`
- `cargo test`
- `npm run build`
