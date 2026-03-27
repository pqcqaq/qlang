# 2026-03-27 Phase 6: Remaining Variant Path Completion Parity

## Why This Slice Exists

Phase 6 already had explicit completion parity coverage for direct enum item roots, plus the import-alias follow-through on enum roots and struct-literal variant paths.

There were still a few already-supported same-file variant-path contexts whose editor-facing projection was only covered indirectly:

- direct enum struct-variant literal paths
- direct enum pattern paths
- local import alias -> same-file enum item pattern paths

Those contexts already worked semantically through `QueryIndex`, but they still lacked explicit end-to-end parity regressions for:

- analysis-side candidate shape
- LSP `ENUM_MEMBER` projection
- detail rendering
- replacement text edits

## What Changed

Added parity-only regression coverage. No new semantic behavior or wider completion surface was introduced.

Analysis regressions added:

- `completion_queries_surface_variant_candidates_in_struct_literal_paths`
- `completion_queries_surface_variant_candidates_in_pattern_paths`
- `completion_queries_surface_variant_candidates_in_import_alias_pattern_paths`

LSP bridge regressions added:

- `completion_bridge_maps_struct_variant_candidates_by_prefix`
- `completion_bridge_maps_variant_candidates_in_pattern_paths`
- `completion_bridge_maps_import_alias_variant_candidates_in_pattern_paths`

These tests lock the existing behavior that all currently supported same-file variant-path contexts:

- still surface `variant` completion candidates
- keep `SymbolKind::Variant` on the analysis side
- map to `CompletionItemKind::ENUM_MEMBER` on the LSP side
- preserve stable detail rendering and replacement edits

## Boundary

This slice stays conservative:

- no cross-file/module-graph behavior
- no foreign import alias semantics
- no variant-field completion expansion
- no new query or rename surface
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
- `cargo test -p ql-analysis --test queries completion_queries_surface_variant_candidates_in_struct_literal_paths -- --exact`
- `cargo test -p ql-analysis --test queries completion_queries_surface_variant_candidates_in_pattern_paths -- --exact`
- `cargo test -p ql-analysis --test queries completion_queries_surface_variant_candidates_in_import_alias_pattern_paths -- --exact`
- `cargo test -p ql-lsp --test bridge completion_bridge_maps_struct_variant_candidates_by_prefix -- --exact`
- `cargo test -p ql-lsp --test bridge completion_bridge_maps_variant_candidates_in_pattern_paths -- --exact`
- `cargo test -p ql-lsp --test bridge completion_bridge_maps_import_alias_variant_candidates_in_pattern_paths -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`

## Recommended Next Direction

Keep Phase 6 conservative. Continue only on already-supported same-file semantic identities where analysis/LSP parity coverage is still weaker than the underlying capability.
