# 2026-03-26 Phase 6: Extern Callable Parity

## Why This Slice Exists

`ql-analysis` had already exposed hover / definition / references for `extern` block function declarations, but that surface was not yet regression-locked across the rest of the Phase 6 tooling stack.

That left a conservative but real gap: extern callable declarations were expected to behave like ordinary same-file function symbols in the shared `QueryIndex`, yet there was no explicit end-to-end proof that analysis queries, same-file rename, semantic tokens, and LSP bridge behavior stayed aligned.

## What Changed

Added coverage only. No new semantic capability or protocol-layer heuristic was introduced.

Analysis regressions added:

- `rename_queries_follow_extern_block_function_symbols`
- `semantic_tokens_follow_extern_block_function_surface`

Existing analysis coverage already locked:

- `extern_block_function_queries_follow_callable_declarations`

LSP bridge regressions added:

- `hover_definition_and_references_bridge_follow_extern_block_function_symbols`
- `rename_bridge_supports_extern_block_function_symbols`
- `semantic_tokens_bridge_maps_extern_block_function_surface`

These tests lock same-file parity for:

- `extern` block function declaration / call site

across:

- hover
- go to definition
- find references
- prepare rename / rename
- semantic tokens

## Boundary

This slice stays conservative:

- no cross-file/module-graph behavior
- no new symbol kinds
- no import/export graph semantics
- no LSP-local special casing

Extern callable declarations continue to flow through the existing `Function` + `QueryIndex` truth surface.

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
- `cargo test -p ql-analysis --test queries rename_queries_follow_extern_block_function_symbols -- --exact`
- `cargo test -p ql-analysis --test queries semantic_tokens_follow_extern_block_function_surface -- --exact`
- `cargo test -p ql-lsp --test bridge hover_definition_and_references_bridge_follow_extern_block_function_symbols -- --exact`
- `cargo test -p ql-lsp --test bridge rename_bridge_supports_extern_block_function_symbols -- --exact`
- `cargo test -p ql-lsp --test bridge semantic_tokens_bridge_maps_extern_block_function_surface -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs/`

## Recommended Next Direction

Keep Phase 6 conservative. Prefer remaining same-file parity gaps on already-supported symbol identities over any new cross-file/module-graph work.
