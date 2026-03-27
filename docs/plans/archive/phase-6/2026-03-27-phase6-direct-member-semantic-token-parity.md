# 2026-03-27 Phase 6: Direct Member Semantic-token Parity

## Why This Slice Exists

Phase 6 already had:

- aggregate semantic-token coverage for the current query surface
- direct member query parity on the analysis side
- direct member query parity on the LSP side after the previous slice

But the direct stable-member highlighting path was still only indirectly covered through the aggregate semantic-token tests.

That left one remaining asymmetry:

- direct field member tokens were not individually locked on the semantic-token path
- direct unique method member tokens were not individually locked on the semantic-token path

## What Changed

Added parity-only semantic-token regressions on both sides of the shared analysis surface.

Analysis regression added:

- `semantic_tokens_follow_direct_member_surface`

LSP bridge regression added:

- `semantic_tokens_bridge_maps_direct_member_surface`

These tests lock the existing behavior that:

- direct field member declarations and uses stay highlighted as `Field` / `PROPERTY`
- direct unique method declarations and uses stay highlighted as `Method` / `METHOD`

## Boundary

This slice stays conservative:

- no new member-selection semantics
- no ambiguous member expansion
- no new semantic-token kinds
- no bridge-local heuristics
- no cross-file semantic-token classification

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
- `cargo test -p ql-analysis --test queries semantic_tokens_follow_direct_member_surface -- --exact`
- `cargo test -p ql-lsp --test bridge semantic_tokens_bridge_maps_direct_member_surface -- --exact`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`

## Recommended Next Direction

Keep Phase 6 conservative. Continue only on already-supported same-file query / completion / semantic-token behavior whose editor-facing parity is still weaker than the underlying shared analysis surface.
