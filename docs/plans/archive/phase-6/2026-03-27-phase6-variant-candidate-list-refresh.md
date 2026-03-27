# 2026-03-27 Phase 6 Variant Candidate-List Refresh

## Goal

Strengthen the already-supported same-file variant-path completion surface without expanding semantics.

## Scope

- keep `QueryIndex` as the single truth source
- keep behavior same-file only
- keep module graph, cross-file lookup, and foreign import alias semantics closed
- add aggregate regression coverage for all currently supported variant-path completion contexts:
  - enum item root
  - struct-literal path
  - pattern path
  - same-file import-alias enum root
  - same-file import-alias struct-literal path
  - same-file import-alias pattern path

## Planned Work

1. Add an analysis-side aggregate regression that locks ordered candidate lists, `insert_text`, `SymbolKind`, and detail strings for the supported variant-path contexts.
2. Add an LSP bridge aggregate regression that locks ordered completion items, `ENUM_MEMBER` projection, detail strings, and replacement `text_edit` ranges for the same contexts.
3. Refresh roadmap and architecture docs so the declared Phase 6 contract matches the strengthened regression surface.

## Verification

- `cargo fmt --all`
- `cargo test -p ql-analysis --test queries completion_queries_surface_variant_candidate_lists_across_supported_paths -- --exact`
- `cargo test -p ql-lsp --test bridge completion_bridge_surfaces_variant_candidate_lists_across_supported_paths -- --exact`
- `cargo test`
- `npm run build`
