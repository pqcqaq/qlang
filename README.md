# Qlang

Qlang is a compiled systems language with its own syntax, type system, ownership model, and toolchain goals.
The current compiler and tools are implemented in a Rust workspace, but Qlang is not a Rust dialect and should not drift toward Rust syntax by implementation convenience.

## Current Reality

- Language-facing source of truth lives in `docs/design/` and `docs/vision.md`.
- Compiler/runtime/build truth lives in `crates/`, `tests/`, `fixtures/`, and the regression matrix.
- If implementation convenience conflicts with Qlang language design, the design docs win and the implementation must be corrected.

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

- [`docs/vision.md`](./docs/vision.md)
- [`docs/design/principles.md`](./docs/design/principles.md)
- [`docs/design/syntax.md`](./docs/design/syntax.md)
- [`docs/roadmap/current-supported-surface.md`](./docs/roadmap/current-supported-surface.md)
- [`docs/roadmap/development-plan.md`](./docs/roadmap/development-plan.md)
- [`docs/roadmap/phase-progress.md`](./docs/roadmap/phase-progress.md)
- [`docs/roadmap/archive/index.md`](./docs/roadmap/archive/index.md)

Detailed design merges and archived slice notes live under [`docs/plans/`](./docs/plans/) and [`docs/plans/archive/`](./docs/plans/archive/).

## Repository Layout

- `crates/`: compiler, runtime, CLI, LSP, diagnostics, and supporting layers
- `docs/`: VitePress documentation site
- `fixtures/`: parser/codegen/pass-fail fixtures
- `tests/`: committed integration and host-interop test inputs
- `crates/ql-cli/tests/executable_examples.rs`: executable smoke contract; local ignored `ramdon_tests/` examples may be used when present
- `examples/ffi-c/`, `examples/ffi-c-dylib/`, `examples/ffi-rust/`: committed host interop examples

## Regression Truth

Current user-facing executable smoke contract lives in `crates/ql-cli/tests/executable_examples.rs`.

- This checkout does not commit a `ramdon_tests/` directory.
- Local ignored `ramdon_tests/executable_examples/` and `ramdon_tests/async_program_surface_examples/` directories may be used to back executable smoke runs when present.

Current library/codegen surface is locked in `crates/ql-cli/tests/codegen.rs`.

## Quick Start

```bash
cargo test
cargo run -p ql-cli -- check fixtures/codegen/pass/minimal_build.ql
cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_build.ql --emit llvm-ir
cargo run -p ql-cli -- build path/to/package/src/lib.ql --emit llvm-ir --emit-interface
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
