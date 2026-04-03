# Qlang

Qlang is an LLVM-based compiled language project implemented as a Rust workspace.

## Current Reality

- Phase 1 through Phase 6 foundations are already landed.
- Active work is conservative Phase 7: async/runtime/task-handle lowering, library/program build surface, and Rust interop.
- The stable external interop boundary is still C ABI.
- Async support is real, but intentionally narrow:
  - async library build for `staticlib` and the current minimal `dylib` subset
- minimal program-mode `async fn main`
- executable `unsafe fn` bodies on the current sync / async program subset
  - fixed-shape `for await`
  - task-handle payload / projection / guarded-match slices that are already regression-locked

## Source Of Truth

Read these first:

- [`docs/roadmap/current-supported-surface.md`](./docs/roadmap/current-supported-surface.md)
- [`docs/roadmap/development-plan.md`](./docs/roadmap/development-plan.md)
- [`docs/roadmap/phase-progress.md`](./docs/roadmap/phase-progress.md)
- [`docs/roadmap/archive/index.md`](./docs/roadmap/archive/index.md)

Detailed design merges and archived slice notes live under [`docs/plans/`](./docs/plans/) and [`docs/plans/archive/`](./docs/plans/archive/).

## Repository Layout

- `crates/`: compiler, runtime, CLI, LSP, diagnostics, and supporting layers
- `docs/`: VitePress documentation site
- `fixtures/`: parser/codegen/pass-fail fixtures
- `ramdon_tests/`: real executable surface examples
- `examples/ffi-c/`, `examples/ffi-c-dylib/`, `examples/ffi-rust/`: committed host interop examples

## Regression Truth

Current user-facing executable smoke surface:

- `60` sync executable examples under `ramdon_tests/executable_examples/`
- `220` async executable examples under `ramdon_tests/async_program_surface_examples/`

Current library/codegen surface is locked in `crates/ql-cli/tests/codegen.rs`.

## Quick Start

```bash
cargo test
cargo run -p ql-cli -- check fixtures/codegen/pass/minimal_build.ql
cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_build.ql --emit llvm-ir
cargo run -p ql-cli -- fmt fixtures/parser/pass/basic.ql
```

When a clang-style toolchain is available:

```bash
cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_build.ql --emit exe
cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_library.ql --emit staticlib
cargo run -p ql-cli -- build tests/ffi/pass/extern_c_export.ql --emit dylib --header
```

## Docs

Online docs:

- https://qlang.zust.online/

Local docs development:

```bash
cd docs
npm install
npm run dev
```
