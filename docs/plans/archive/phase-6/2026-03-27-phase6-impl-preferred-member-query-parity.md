# 2026-03-27 Phase 6: Impl-preferred Member Query Parity

## Why This Slice Exists

Phase 6 already had an analysis-side regression proving that direct member queries keep the existing precedence rule:

- prefer a matching `impl` method
- do not drift onto a same-named `extend` method

That boundary was already important elsewhere:

- completion filtering parity already locked the same precedence rule on the completion path
- rename stayed conservative and only opens unique method surfaces

But the direct editor-facing query path still lacked an explicit LSP parity regression for this precedence rule.

## What Changed

Added one LSP parity-only regression:

- `hover_and_definition_bridge_prefer_impl_methods_over_extend_methods`

This test locks the existing behavior that a direct member call:

- hovers as the `impl` method signature
- navigates definition to the `impl` method declaration
- does not drift onto the same-named `extend` method at the bridge layer

## Boundary

This slice stays conservative:

- no new member-selection semantics
- no ambiguous member expansion
- no new rename surface
- no bridge-local heuristics

Type checking member selection plus `ql-analysis::QueryIndex` remain the only truth sources.

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
- `cargo test -p ql-lsp --test bridge hover_and_definition_bridge_prefer_impl_methods_over_extend_methods -- --exact`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`

## Recommended Next Direction

Keep Phase 6 conservative. Continue only on already-supported same-file query / completion / semantic-token behavior whose editor-facing parity is still weaker than the underlying shared analysis surface.
