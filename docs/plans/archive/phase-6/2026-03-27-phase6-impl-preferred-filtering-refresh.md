# Phase 6: Impl-Preferred Filtering Refresh

## Goal

Strengthen the existing same-file impl-preferred member filtering regressions so they lock the aggregate surviving-candidate projection, not just the presence of the preferred method and absence of filtered extend candidates.

## Scope

- strengthen the analysis impl-preferred member filtering regression to verify surviving candidate count, detail, and insert text
- strengthen the LSP bridge impl-preferred member filtering regression to verify the same surviving candidate count, detail, and replacement text edit
- update docs so completion filtering parity wording matches the stronger aggregate coverage

## Non-goals

- no new member completion semantics
- no ambiguous member completion opening
- no impl/extend priority changes
- no cross-file completion

## Verification

- `cargo fmt --all`
- `cargo test -p ql-analysis --test queries member_completion_prefers_impl_methods_and_skips_ambiguous_extend_candidates -- --exact`
- `cargo test -p ql-lsp --test bridge completion_bridge_prefers_impl_methods_and_skips_ambiguous_extend_candidates -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
