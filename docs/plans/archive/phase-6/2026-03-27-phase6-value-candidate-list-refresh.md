# 2026-03-27 Phase 6 Value Candidate-List Refresh

## Goal

Strengthen the already-supported same-file lexical value completion surface without expanding semantics.

## Scope

- keep `QueryIndex` as the single truth source
- keep behavior same-file only
- keep module graph, cross-file lookup, and foreign import semantics closed
- add aggregate regression coverage for the current lexical value candidate surface:
  - import alias
  - const
  - static
  - extern block callable
  - top-level extern declaration
  - top-level extern definition
  - free function
  - local
  - parameter

## Planned Work

1. Add an analysis-side aggregate regression that locks the ordered lexical value candidate list, `insert_text`, symbol kinds, and detail strings.
2. Add an LSP bridge aggregate regression that locks the ordered completion-item list, kind projection, detail strings, and replacement `text_edit` ranges.
3. Refresh roadmap and architecture docs so the declared Phase 6 completion contract matches the stronger regression surface.

## Verification

- `cargo fmt --all`
- `cargo test -p ql-analysis --test queries completion_queries_follow_value_context_candidate_lists -- --exact`
- `cargo test -p ql-lsp --test bridge completion_bridge_surfaces_value_context_candidate_lists -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `npm run build`
