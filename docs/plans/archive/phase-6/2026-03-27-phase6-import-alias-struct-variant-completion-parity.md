# 2026-03-27 Phase 6: Import Alias Struct Variant Completion Parity

## Why This Slice Exists

Phase 6 already had same-file parsed variant-path completion for enum struct variants, and the same-file import-alias follow-through already reached those variant identities semantically.

But there was still no explicit parity regression proving that the struct-literal alias path stayed aligned end to end for:

- analysis-side variant candidate shape
- LSP `ENUM_MEMBER` projection
- detail rendering for struct-style payloads
- editor-facing replacement edits

Without that lock, `QueryIndex` could continue to resolve the alias path correctly while the editor-facing completion projection for struct variants silently drifted.

## What Changed

Added parity-only regression coverage. No new semantic behavior or wider completion surface was introduced.

Analysis regression added:

- `completion_queries_surface_variant_candidates_in_import_alias_struct_literal_paths`

LSP bridge regression added:

- `completion_bridge_maps_import_alias_struct_variant_candidates`

These tests lock the existing behavior that local import aliases pointing at same-file enum items:

- still surface struct-style `variant` completion candidates in struct-literal paths
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
- `cargo test -p ql-analysis --test queries completion_queries_surface_variant_candidates_in_import_alias_struct_literal_paths -- --exact`
- `cargo test -p ql-lsp --test bridge completion_bridge_maps_import_alias_struct_variant_candidates -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`

## Recommended Next Direction

Keep Phase 6 conservative. Continue only on already-supported same-file semantic identities where completion/query projection coverage is still weaker than the underlying capability.
