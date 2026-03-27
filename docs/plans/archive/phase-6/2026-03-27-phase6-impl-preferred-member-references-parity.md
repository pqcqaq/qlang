# 2026-03-27 Phase 6: Impl-preferred Member References Parity

## Why This Slice Exists

Phase 6 already had direct member query parity for the existing impl-over-extend precedence rule, but that coverage was still incomplete:

- analysis explicitly locked hover / definition for the chosen `impl` method
- the LSP bridge explicitly locked hover / definition for the same direct member call
- same-file references for that precedence rule were still only indirectly covered

That left one editor-facing gap: the direct member query path could still regress on `references` without a dedicated impl-over-extend regression catching it.

## What Changed

Expanded the existing conservative regressions on both layers:

- `member_queries_prefer_impl_methods_over_extend_methods`
- `hover_and_definition_bridge_prefer_impl_methods_over_extend_methods`

These tests now also lock that a direct member call:

- groups references under the selected `impl` method declaration
- includes the `impl` declaration when `include_declaration = true`
- excludes the same-named `extend` declaration from the result set

## Boundary

This slice stays conservative:

- no new member-resolution semantics
- no ambiguous member expansion
- no semantic-token broadening
- no bridge-local heuristics

`ql-analysis::QueryIndex` plus the existing type-driven member selection remain the only truth sources.

## Docs Updated

- `README.md`
- `docs/architecture/compiler-pipeline.md`
- `docs/architecture/implementation-algorithms.md`
- `docs/architecture/toolchain.md`
- `docs/roadmap/development-plan.md`
- `docs/roadmap/phase-progress.md`
- `docs/.vitepress/config.mts`

## Verification

- `cargo test -p ql-analysis --test queries member_queries_prefer_impl_methods_over_extend_methods -- --exact`
- `cargo test -p ql-lsp --test bridge hover_and_definition_bridge_prefer_impl_methods_over_extend_methods -- --exact`

## Recommended Next Direction

Keep Phase 6 conservative. Continue only on already-supported same-file query / completion / semantic-token behavior whose editor-facing parity is still weaker than the shared analysis surface.
