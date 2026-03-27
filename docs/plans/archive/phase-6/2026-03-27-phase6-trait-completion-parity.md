# Phase 6: Trait Completion Parity

## Goal

Lock the already-supported same-file `trait` type-context completion surface into the explicit analysis/LSP parity matrix.

## Scope

- add an analysis regression for same-file `trait` type-context completion
- add an LSP bridge regression for the same `trait` completion mapping
- document that `trait` candidates remain part of the current same-file type-context completion surface

## Non-goals

- no new trait semantics or trait solving
- no cross-file or module-graph completion
- no expansion of method/member completion behavior
- no change to the existing `SymbolKind::Trait -> CompletionItemKind::INTERFACE` mapping

## Verification

- `cargo fmt --all`
- `cargo test -p ql-analysis --test queries completion_queries_surface_trait_type_candidates_by_prefix -- --exact`
- `cargo test -p ql-lsp --test bridge completion_bridge_maps_trait_type_candidates -- --exact`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
