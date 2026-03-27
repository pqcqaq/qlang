# 2026-03-27 Phase 6: Direct Semantic-token Parity

## Why This Slice Exists

Phase 6 already had semantic-token regressions for:

- the broad current query surface
- import-alias variant follow-through
- import-alias explicit field-label follow-through

That left one asymmetry behind:

- direct enum variant tokens were not explicitly regression-locked on the semantic-token path
- direct explicit struct field labels were not explicitly regression-locked on the semantic-token path

So the editor-facing highlighting surface was still relying on aggregate coverage and import-alias follow-through coverage more than on the simpler direct same-file path those cases depend on.

## What Changed

Added parity-only semantic-token regressions on both sides of the shared analysis surface.

Analysis regressions added:

- `semantic_tokens_follow_direct_variant_surface`
- `semantic_tokens_follow_direct_struct_field_surface`

LSP bridge regressions added:

- `semantic_tokens_bridge_maps_direct_variant_surface`
- `semantic_tokens_bridge_maps_direct_struct_field_surface`

These tests lock the existing behavior that:

- direct tuple/struct enum variants stay highlighted as `Variant` / `ENUM_MEMBER`
- direct explicit struct field declaration, literal labels, pattern labels, and member uses stay highlighted as `Field` / `PROPERTY`

## Boundary

This slice stays conservative:

- no new semantic-token kinds
- no shorthand field-token promotion into field symbols
- no new query behavior
- no cross-file semantic-token classification
- no bridge-local semantic heuristics

`ql-analysis::QueryIndex` remains the only semantic-token truth source.

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
- `cargo test -p ql-analysis --test queries semantic_tokens_follow_direct_variant_surface -- --exact`
- `cargo test -p ql-analysis --test queries semantic_tokens_follow_direct_struct_field_surface -- --exact`
- `cargo test -p ql-lsp --test bridge semantic_tokens_bridge_maps_direct_variant_surface -- --exact`
- `cargo test -p ql-lsp --test bridge semantic_tokens_bridge_maps_direct_struct_field_surface -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`

## Recommended Next Direction

Keep Phase 6 conservative. Continue only on already-supported same-file query / completion / semantic-token behavior whose editor-facing parity is still weaker than the underlying shared analysis surface.
