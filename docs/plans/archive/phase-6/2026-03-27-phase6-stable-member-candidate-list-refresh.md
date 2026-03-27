# Phase 6: Stable Member Candidate-List Refresh

## Goal

Strengthen the existing same-file stable-member completion aggregate regressions so they lock not just ordering and kinds, but also the current detail and text-edit projection for method and field candidates.

## Scope

- strengthen the analysis stable-member candidate-list regression to verify method/field detail and insert text
- strengthen the LSP bridge stable-member candidate-list regression to verify the same list, kinds, details, and replacement text edits
- update docs so completion candidate-list parity wording matches the stronger aggregate coverage

## Non-goals

- no new member completion semantics
- no ambiguous member completion opening
- no cross-file completion
- no receiver-self completion opening

## Verification

- `cargo fmt --all`
- `cargo test -p ql-analysis --test queries completion_queries_follow_member_candidates_on_stable_receiver_types -- --exact`
- `cargo test -p ql-lsp --test bridge completion_bridge_surfaces_member_candidates_on_stable_receiver_types -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
