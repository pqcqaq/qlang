# 2026-03-27 Phase 6: Completion Candidate List Parity

## Why This Slice Exists

Phase 6 already had analysis-side regressions for two stable same-file completion list surfaces:

- type-context completion candidates
- stable receiver member completion candidates

The LSP bridge already reused `ql-analysis::QueryIndex`, but editor-facing coverage still focused mostly on single-candidate mapping and prefix filtering. That left the full candidate list, ordering, and namespace exclusion behavior covered only indirectly.

## What Changed

Added LSP parity-only regression coverage. No new semantic behavior or wider completion surface was introduced.

LSP bridge regressions added:

- `completion_bridge_surfaces_type_context_candidates`
- `completion_bridge_surfaces_member_candidates_on_stable_receiver_types`

These tests lock the existing behavior that:

- same-file type-context completion continues to surface only type-namespace candidates in stable sorted order
- value names do not leak into type-context completion
- stable receiver member completion continues to surface the full supported member set in stable sorted order
- fields and methods preserve their existing editor-facing kinds, details, and replacement edits

## Boundary

This slice stays conservative:

- no cross-file/module-graph behavior
- no new completion namespaces
- no new member ambiguity logic
- no LSP-local heuristics

`ql-analysis::QueryIndex` remains the only completion truth source.

## Docs Updated

- `README.md`
- `docs/architecture/compiler-pipeline.md`
- `docs/architecture/toolchain.md`
- `docs/roadmap/development-plan.md`
- `docs/roadmap/phase-progress.md`
- `docs/.vitepress/config.mts`

## Verification

- `cargo fmt --all`
- `cargo test -p ql-lsp --test bridge completion_bridge_surfaces_type_context_candidates -- --exact`
- `cargo test -p ql-lsp --test bridge completion_bridge_surfaces_member_candidates_on_stable_receiver_types -- --exact`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`

## Recommended Next Direction

Keep Phase 6 conservative. Continue only on already-supported same-file semantic identities and filtering boundaries where editor-facing parity coverage is still weaker than the underlying behavior.
