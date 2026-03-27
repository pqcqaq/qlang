# 2026-03-26 Phase 6: Import Alias Query Parity

## Why This Slice Exists

Phase 6 had already moved plain `import` aliases onto a source-backed symbol identity:

- hover worked
- definition worked
- same-file references worked
- same-file rename worked
- semantic tokens already included the symbol because it was source-backed

But this behavior was still only indirectly covered by a mix of older tests and broader semantic-token snapshots. There was no direct end-to-end parity slice proving that plain import-alias bindings stayed aligned across analysis queries and the LSP bridge.

That left a real maintenance risk: import aliases could drift back toward string-only pseudo-symbol behavior without an explicit regression catching it.

## What Changed

Added explicit parity coverage only. No new semantic capability, rename kind, or protocol-local heuristic was introduced.

Analysis regressions added:

- `import_alias_queries_follow_same_file_identity`
- `semantic_tokens_follow_import_alias_surface`

LSP bridge regressions added:

- `hover_definition_and_references_bridge_follow_import_alias_symbols`
- `semantic_tokens_bridge_maps_import_alias_surface`

These tests lock same-file parity for plain `import` bindings across:

- hover
- definition
- references
- semantic tokens

## Boundary

This slice stays conservative:

- no cross-file/module-graph behavior
- no foreign import alias semantics
- no new completion or rename behavior
- no LSP-local special casing

`ql-analysis::QueryIndex` remains the only semantic truth source.

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
- `cargo test -p ql-analysis --test queries import_alias_queries_follow_same_file_identity -- --exact`
- `cargo test -p ql-analysis --test queries semantic_tokens_follow_import_alias_surface -- --exact`
- `cargo test -p ql-lsp --test bridge hover_definition_and_references_bridge_follow_import_alias_symbols -- --exact`
- `cargo test -p ql-lsp --test bridge semantic_tokens_bridge_maps_import_alias_surface -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`

## Recommended Next Direction

Keep Phase 6 conservative. Continue on already-supported same-file symbol identities where behavior exists but explicit parity coverage or docs still lag the actual shared query surface.
