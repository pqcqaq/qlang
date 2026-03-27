# 2026-03-26 Phase 6: Lexical Rename Parity

## Why This Slice Exists

By this point, same-file lexical rename behavior already existed in analysis:

- `generic`
- `parameter`
- `local`

And two lexical surfaces were intentionally closed:

- `receiver self`
- `builtin type`

However, that split behavior was not explicitly regression-locked end to end at the LSP bridge level. The implementation likely worked because LSP rename already forwards analysis rename results, but there was no direct regression proving that supported lexical rename flows stayed open while unsupported lexical surfaces stayed closed.

## What Changed

Added coverage only. No new semantic capability or protocol-local heuristic was introduced.

Analysis regression added:

- `rename_queries_follow_lexical_supported_and_closed_symbols`

LSP bridge regression added:

- `rename_bridge_supports_lexical_semantic_symbols_and_keeps_closed_surfaces_closed`

These tests lock same-file rename behavior for:

- supported: `generic`, `parameter`, `local`
- intentionally closed: `receiver self`, `builtin type`

## Boundary

This slice stays conservative:

- no cross-file/module-graph behavior
- no new rename kinds
- no LSP-local special casing

The bridge continues to forward analysis rename truth directly.

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
- `cargo test -p ql-analysis --test queries rename_queries_follow_lexical_supported_and_closed_symbols -- --exact`
- `cargo test -p ql-lsp --test bridge rename_bridge_supports_lexical_semantic_symbols_and_keeps_closed_surfaces_closed -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`

## Recommended Next Direction

Keep Phase 6 conservative. Continue only on already-supported same-file symbol identities, especially where analysis behavior already exists but the LSP bridge still lacks explicit regression coverage.
