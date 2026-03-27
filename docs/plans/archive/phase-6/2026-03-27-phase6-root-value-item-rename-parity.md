# 2026-03-27 Phase 6: Root Value-item Rename Parity

## Why This Slice Exists

Phase 6 already had conservative same-file rename support for root value-like items:

- free functions
- `const`
- `static`

But the regression coverage was still uneven:

- analysis had aggregate rename coverage for functions
- shorthand-binding rename regressions indirectly exercised `const` / `static`
- the LSP bridge still lacked one explicit end-to-end parity regression for direct prepare-rename / rename on these root symbols

That left the editor-facing rename path weaker than the shared analysis surface.

## What Changed

Added explicit same-file rename regressions on both layers:

- `rename_queries_follow_function_const_and_static_symbols`
- `rename_bridge_supports_function_const_and_static_symbols`

These tests lock that direct call/use sites for root `function`, `const`, and `static` symbols:

- produce the expected prepare-rename range and placeholder
- rename the source-backed declaration and all same-file uses
- keep reusing the same shared `QueryIndex` symbol identity in both analysis and LSP

## Boundary

This slice stays conservative:

- no new rename surface
- no cross-file rename
- no ambiguous method expansion
- no bridge-local heuristics

`ql-analysis::QueryIndex` remains the only truth source for rename grouping.

## Docs Updated

- `README.md`
- `docs/.vitepress/config.mts`
- `docs/architecture/compiler-pipeline.md`
- `docs/architecture/implementation-algorithms.md`
- `docs/architecture/toolchain.md`
- `docs/roadmap/development-plan.md`
- `docs/roadmap/phase-progress.md`

## Verification

- `cargo test -p ql-analysis --test queries rename_queries_follow_function_const_and_static_symbols -- --exact`
- `cargo test -p ql-lsp --test bridge rename_bridge_supports_function_const_and_static_symbols -- --exact`

## Recommended Next Direction

Keep Phase 6 conservative. Continue only on already-supported same-file query / completion / rename / semantic-token behavior whose LSP-facing parity is still weaker than the shared analysis surface.
