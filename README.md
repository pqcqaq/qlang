# Qlang

Qlang is a compiled systems language with its own syntax, type system, ownership model, and toolchain goals.
The current compiler and tools are implemented in a Rust workspace. Qlang remains a separate language design.

## Current Reality

- Language design: `docs/design/` and `docs/vision.md`
- Implementation status: `crates/`, `tests/`, `fixtures/`, and the regression matrix
- Phase 1 through Phase 6 foundations are already landed.
- Active work is split across:
  - Phase 7: async/runtime/task-handle lowering, library/program build surface, and Rust interop
  - Phase 8: package/workspace manifests, `.qi` interface artifacts, and dependency-backed cross-file tooling
- The stable external interop boundary is still C ABI.
- Current async surface:
  - async library build for `staticlib` and the current minimal `dylib` subset
  - minimal program-mode `async fn main`
  - executable `unsafe fn` bodies on the current sync / async program subset
  - fixed-shape `for await`
  - task-handle payload / projection / guarded-match slices that are already regression-locked

## Key Docs

- [`docs/vision.md`](./docs/vision.md)
- [`docs/design/principles.md`](./docs/design/principles.md)
- [`docs/design/syntax.md`](./docs/design/syntax.md)
- [`docs/roadmap/current-supported-surface.md`](./docs/roadmap/current-supported-surface.md)
- [`docs/roadmap/development-plan.md`](./docs/roadmap/development-plan.md)
- [`docs/roadmap/phase-progress.md`](./docs/roadmap/phase-progress.md)

Merged phase design docs live under [`docs/plans/`](./docs/plans/). Historical archive pages stay out of the main reading path.

## Repository Layout

- `crates/`: compiler, project/workspace, runtime, CLI, LSP, diagnostics, and supporting layers
- `docs/`: VitePress documentation site
- `fixtures/`: parser/codegen/pass-fail fixtures
- `ramdon_tests/`: committed executable smoke corpus used by `crates/ql-cli/tests/executable_examples.rs`
- `tests/`: committed integration and host-interop test inputs
- `crates/ql-cli/tests/executable_examples.rs`: executable smoke contract over the committed `ramdon_tests/` baseline, with room for extra local-only ignored examples when present
- `examples/ffi-c/`, `examples/ffi-c-dylib/`, `examples/ffi-rust/`: committed host interop examples

## Regression Truth

Current user-facing executable smoke contract lives in `crates/ql-cli/tests/executable_examples.rs`.

- This checkout currently includes committed `ramdon_tests/executable_examples/` and `ramdon_tests/async_program_surface_examples/` corpora.
- The directory is still listed in `.gitignore`, so extra local-only smoke files may coexist with the committed baseline.

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
