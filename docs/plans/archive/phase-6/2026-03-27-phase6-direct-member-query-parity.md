# 2026-03-27 Phase 6: Direct Member Query Parity

## Why This Slice Exists

Phase 6 already had analysis-side regressions for the direct stable-member query surface:

- struct field member tokens
- unique method member tokens

The editor-facing bridge, however, was still unevenly covered:

- field members had hover markdown coverage
- import-alias field-label paths had stronger end-to-end query coverage
- direct unique method members did not yet have explicit hover / definition / references parity coverage

That left the direct stable-member path weaker than the underlying same-file analysis surface.

## What Changed

Added one LSP parity-only regression:

- `hover_definition_and_references_bridge_follow_direct_member_symbols`

This test locks the existing behavior that:

- direct field member tokens still hover as `field`, navigate to the field declaration, and group same-file field-member references
- direct unique method member tokens still hover as `method`, navigate to the chosen method declaration, and group same-file method-member references

## Boundary

This slice stays conservative:

- no new member-resolution semantics
- no ambiguous member expansion
- no cross-file behavior
- no bridge-local heuristics

`ql-analysis::QueryIndex` and typeck member selection remain the only truth sources.

## Docs Updated

- `README.md`
- `docs/architecture/compiler-pipeline.md`
- `docs/architecture/implementation-algorithms.md`
- `docs/architecture/toolchain.md`
- `docs/roadmap/development-plan.md`
- `docs/roadmap/phase-progress.md`
- `docs/.vitepress/config.mts`

## Verification

- `cargo fmt --all`
- `cargo test -p ql-lsp --test bridge hover_definition_and_references_bridge_follow_direct_member_symbols -- --exact`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`

## Recommended Next Direction

Keep Phase 6 conservative. Continue only on already-supported same-file query / completion / semantic-token behavior whose editor-facing parity is still weaker than the underlying shared analysis surface.
