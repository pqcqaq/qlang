# Phase 6: Type-Context Candidate-List Refresh

## Goal

Refresh the existing same-file completion candidate-list parity regressions so the type-context aggregate list matches the current already-supported type surface.

## Scope

- strengthen the analysis type-context candidate-list regression to include builtin / import / struct / `type` / `opaque type` / `enum` / `trait` / generic
- strengthen the LSP bridge candidate-list regression to verify the same aggregate list, ordering, kinds, details, and import text edit
- update docs so the candidate-list parity description matches the broadened regression coverage

## Non-goals

- no new completion semantics
- no cross-file or module-graph completion
- no receiver-self completion opening
- no member completion broadening

## Verification

- `cargo fmt --all`
- `cargo test -p ql-analysis --test queries completion_queries_follow_type_contexts -- --exact`
- `cargo test -p ql-lsp --test bridge completion_bridge_surfaces_type_context_candidates -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
