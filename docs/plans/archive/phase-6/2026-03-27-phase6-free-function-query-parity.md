# 2026-03-27 Phase 6: Free Function Query Parity

## Why This Slice Exists

Phase 6 already had conservative same-file support for ordinary free functions:

- lexical value completion
- same-file rename
- aggregate root-binding query coverage

But the explicit query regressions were still uneven compared with other stable surfaces such as:

- `const` / `static`
- `extern` callable declarations
- direct member tokens

That left ordinary free function hover / definition / references weaker on the editor-facing bridge than the underlying shared analysis surface.

## What Changed

Added explicit same-file query regressions on both layers:

- `free_function_queries_follow_same_file_identity`
- `hover_definition_and_references_bridge_follow_free_function_symbols`

These tests lock that an ordinary free function direct call site:

- hovers as `function`
- navigates definition to the free-function declaration
- groups same-file references under the declaration and all direct uses

## Boundary

This slice stays conservative:

- no new function semantics
- no cross-file/project indexing
- no completion expansion
- no bridge-local heuristics

`ql-analysis::QueryIndex` remains the only truth source for ordinary free-function query grouping.

## Docs Updated

- `README.md`
- `docs/.vitepress/config.mts`
- `docs/architecture/compiler-pipeline.md`
- `docs/architecture/toolchain.md`
- `docs/roadmap/development-plan.md`
- `docs/roadmap/phase-progress.md`

## Verification

- `cargo test -p ql-analysis --test queries free_function_queries_follow_same_file_identity -- --exact`
- `cargo test -p ql-lsp --test bridge hover_definition_and_references_bridge_follow_free_function_symbols -- --exact`

## Recommended Next Direction

Keep Phase 6 conservative. Continue only on already-supported same-file query / completion / rename / semantic-token behavior whose editor-facing parity is still weaker than the shared analysis surface.
