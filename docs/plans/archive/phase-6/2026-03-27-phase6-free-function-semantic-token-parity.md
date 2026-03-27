# 2026-03-27 Phase 6: Free Function Semantic-token Parity

## Why This Slice Exists

Phase 6 already had ordinary free-function support across several same-file surfaces:

- query parity for direct call sites
- lexical value completion parity
- same-file rename parity

But semantic-token coverage was still weaker:

- the shared analysis surface already emitted `Function` tokens for free-function declarations and direct call sites
- the LSP bridge already mapped that shared token stream
- coverage still relied on aggregate semantic-token tests rather than one explicit free-function parity regression

## What Changed

Added explicit semantic-token regressions on both layers:

- `semantic_tokens_follow_free_function_surface`
- `semantic_tokens_bridge_maps_free_function_surface`

These tests lock that ordinary free-function declarations and direct call sites:

- continue to emit `Function` semantic-token occurrences in analysis
- continue to map to LSP `FUNCTION` semantic tokens through the bridge

## Boundary

This slice stays conservative:

- no new semantic-token kinds
- no new function semantics
- no bridge-local heuristics
- no project indexing or cross-file behavior

`ql-analysis::QueryIndex` and the existing occurrence export remain the only truth sources.

## Docs Updated

- `README.md`
- `docs/.vitepress/config.mts`
- `docs/architecture/compiler-pipeline.md`
- `docs/architecture/implementation-algorithms.md`
- `docs/architecture/toolchain.md`
- `docs/roadmap/development-plan.md`
- `docs/roadmap/phase-progress.md`

## Verification

- `cargo test -p ql-analysis --test queries semantic_tokens_follow_free_function_surface -- --exact`
- `cargo test -p ql-lsp --test bridge semantic_tokens_bridge_maps_free_function_surface -- --exact`

## Recommended Next Direction

Keep Phase 6 conservative. Continue only on already-supported same-file query / completion / rename / semantic-token behavior whose editor-facing parity is still weaker than the shared analysis surface.
