# 2026-03-27 Phase 6: Direct Query Parity

## Why This Slice Exists

Phase 6 already had analysis-side regressions for the direct same-file query surface on:

- enum variant tokens
- explicit struct literal / struct pattern field labels

The LSP bridge was already reusing that same analysis layer, and it already had explicit end-to-end parity on the import-alias follow-through paths for both families.

What was still missing was the simpler direct path:

- direct enum variant token -> definition / references
- direct explicit field label -> definition / references

That meant the editor-facing bridge tests were stronger on the follow-through cases than on the base same-file surface those cases depend on.

## What Changed

Added LSP parity-only regression coverage. No new semantic behavior or broader query surface was introduced.

LSP bridge regressions added:

- `definition_and_references_bridge_follow_variant_symbols`
- `definition_and_references_bridge_follow_explicit_struct_field_labels`

These tests lock the existing direct same-file behavior that:

- tuple variants still navigate back to the variant declaration and group constructor / pattern uses
- struct variants still navigate back to the variant declaration and group literal / pattern uses
- explicit struct field labels still navigate back to the field declaration and group literal / pattern / member uses

## Boundary

This slice stays conservative:

- no new query semantics
- no shorthand field-token promotion into field symbols
- no cross-file behavior
- no bridge-local heuristics

`ql-analysis::QueryIndex` remains the only query truth source.

## Docs Updated

- `README.md`
- `docs/architecture/compiler-pipeline.md`
- `docs/architecture/toolchain.md`
- `docs/roadmap/development-plan.md`
- `docs/roadmap/phase-progress.md`
- `docs/.vitepress/config.mts`

## Verification

- `cargo fmt --all`
- `cargo test -p ql-lsp --test bridge definition_and_references_bridge_follow_variant_symbols -- --exact`
- `cargo test -p ql-lsp --test bridge definition_and_references_bridge_follow_explicit_struct_field_labels -- --exact`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run build` in `docs`

## Recommended Next Direction

Keep Phase 6 conservative. Continue only on already-supported same-file query or semantic-token behavior whose editor-facing parity is still weaker than the underlying shared analysis surface.
