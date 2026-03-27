# 2026-03-26 Phase 6: Import Alias Completion Parity

## Why This Slice Exists

Phase 6 already exposed same-file lexical completion through the shared `ql-analysis::QueryIndex`.

That meant plain `import` aliases already participated in type-context completion as ordinary source-backed candidates. But this behavior was only indirectly covered by broader type-context tests and did not directly prove that:

- analysis still surfaced plain `import` aliases as type candidates
- the LSP bridge still mapped them to `MODULE` completion items
- the projected text edit stayed stable

Without an explicit regression, this surface could drift even while the rest of the import-alias query behavior remained green.

## What Changed

Added explicit completion parity coverage only. No new semantic behavior or wider completion surface was introduced.

Analysis regression added:

- `completion_queries_surface_plain_import_alias_candidates_in_type_contexts`

LSP bridge regression added:

- `completion_bridge_maps_plain_import_alias_type_candidates`

These tests lock the existing behavior that plain `import` aliases:

- appear in same-file type-context completion
- keep `SymbolKind::Import` on the analysis side
- map to `CompletionItemKind::MODULE` on the LSP side
- preserve stable replacement edits

## Boundary

This slice stays conservative:

- no cross-file/module-graph behavior
- no new completion namespaces
- no foreign import alias semantics
- no new rename/query surfaces
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
- `cargo test -p ql-analysis --test queries completion_queries_surface_plain_import_alias_candidates_in_type_contexts -- --exact`
- `cargo test -p ql-lsp --test bridge completion_bridge_maps_plain_import_alias_type_candidates -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`

## Recommended Next Direction

Keep Phase 6 conservative. Continue on already-supported same-file semantic identities where analysis behavior already exists but explicit parity coverage or doc precision still lags.
