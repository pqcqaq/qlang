# Phase 6: Extern Callable Completion Parity

## Goal

Lock the already-supported value-context completion surface for extern callables, so editor completion stays aligned with the existing query / rename / semantic-token parity coverage.

## Scope

- add explicit analysis completion regressions for `extern` block members in value contexts
- add explicit analysis completion regressions for top-level `extern "c"` declarations in value contexts
- add explicit analysis completion regressions for top-level `extern "c"` function definitions in value contexts
- add explicit LSP bridge regressions for `FUNCTION` completion item mapping, detail rendering, and text-edit projection for the same three surfaces
- update roadmap and architecture docs so extern callable parity also documents the completion surface

## Non-goals

- no new callable semantics or first-class function expansion
- no module-graph or cross-file completion
- no ABI-surface expansion beyond existing extern callable support
- no parse-error-tolerant completion widening

## Verification

- `cargo fmt --all`
- `cargo test -p ql-analysis --test queries completion_queries_surface_extern_callable_candidates_in_value_contexts -- --exact`
- `cargo test -p ql-lsp --test bridge completion_bridge_maps_extern_callable_value_candidates -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build`
