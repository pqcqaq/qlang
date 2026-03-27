# 2026-03-27 Phase 6: Completion Filtering Parity

## Why This Slice Exists

Phase 6 already had analysis-side regressions for two conservative completion behaviors:

- lexical value completion follows same-file visibility and shadowing rules
- member completion prefers the stable impl method surface and skips ambiguous extend-only candidates

The LSP bridge already reused `ql-analysis::QueryIndex` for completion, but these behaviors were still only covered indirectly on the editor-facing side. Existing bridge tests mostly locked prefix filtering for single candidates, not the full candidate list or the ambiguity boundary.

## What Changed

Added LSP parity-only regression coverage. No new semantic behavior or wider completion surface was introduced.

LSP bridge regressions added:

- `completion_bridge_surfaces_visible_value_bindings_and_shadowing`
- `completion_bridge_prefers_impl_methods_and_skips_ambiguous_extend_candidates`

These tests lock the existing behavior that:

- lexical value completion continues to surface only currently visible same-file bindings in stable sorted order
- shadowed bindings keep their local `VARIABLE` interpretation on the editor-facing side
- generic type parameters do not leak into value completion
- stable impl methods remain eligible member candidates while ambiguous extend-only method surfaces stay filtered out

## Boundary

This slice stays conservative:

- no cross-file/module-graph behavior
- no new completion namespaces
- no new ambiguity resolution logic
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
- `cargo test -p ql-lsp --test bridge completion_bridge_surfaces_visible_value_bindings_and_shadowing -- --exact`
- `cargo test -p ql-lsp --test bridge completion_bridge_prefers_impl_methods_and_skips_ambiguous_extend_candidates -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`

## Recommended Next Direction

Keep Phase 6 conservative. Continue only on already-supported same-file semantic identities and filtering boundaries where the editor-facing parity coverage is still weaker than the underlying behavior.
