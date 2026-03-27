# 2026-03-27 Phase 6 Callable Surface Aggregate Refresh

## Goal

Strengthen the already-supported same-file callable editor surface without expanding semantics.

## Scope

- keep `QueryIndex` as the single truth source
- keep behavior same-file only
- keep module graph, cross-file lookup, and foreign symbol semantics closed
- add aggregate regression coverage for the current callable surface:
  - `extern` block callable declarations
  - top-level `extern "c"` declarations
  - top-level `extern "c"` definitions
  - ordinary free functions

## Planned Work

1. Add an analysis-side aggregate regression that locks callable hover, definition, and references across the supported same-file callable surface.
2. Add an LSP bridge aggregate regression that locks the same hover / definition / references projection.
3. Add aggregate semantic-token regressions on both sides so callable declarations and call sites stay mapped to the stable `Function` token surface.
4. Refresh roadmap and architecture docs so the callable surface hardening is explicit.

## Verification

- `cargo fmt --all`
- `cargo test -p ql-analysis --test queries callable_queries_follow_same_file_callable_surface -- --exact`
- `cargo test -p ql-analysis --test queries semantic_tokens_follow_same_file_callable_surface -- --exact`
- `cargo test -p ql-lsp --test bridge hover_definition_and_references_bridge_follow_same_file_callable_surface -- --exact`
- `cargo test -p ql-lsp --test bridge semantic_tokens_bridge_maps_same_file_callable_surface -- --exact`
- `cargo test`
- `npm run build`
