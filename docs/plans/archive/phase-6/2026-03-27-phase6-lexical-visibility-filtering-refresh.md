# Phase 6: Lexical Visibility Filtering Refresh

## Goal

Strengthen the existing same-file lexical visibility/shadowing completion regressions so they lock the current aggregate projection for visible import/function/local candidates, not just labels and partial kind checks.

## Scope

- strengthen the analysis lexical visibility aggregate regression to verify import/function/local detail and insert text
- strengthen the LSP bridge lexical visibility aggregate regression to verify the same list, kinds, details, and replacement text edits
- update docs so completion filtering parity wording matches the stronger aggregate coverage

## Non-goals

- no new lexical completion semantics
- no scope-graph changes
- no cross-file completion
- no member completion broadening

## Verification

- `cargo fmt --all`
- `cargo test -p ql-analysis --test queries completion_queries_follow_visible_value_bindings_and_shadowing -- --exact`
- `cargo test -p ql-lsp --test bridge completion_bridge_surfaces_visible_value_bindings_and_shadowing -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
