# Qlang

Qlang is a compiled systems language with its own syntax, type system, ownership model, and toolchain goals.
The current compiler and tools are implemented in a Rust workspace. Qlang remains a separate language design.

## Current Reality

- Language design: `docs/design/` and `docs/vision.md`
- Implementation status: `crates/`, `tests/`, `fixtures/`, and the regression matrix
- Phase 1 through Phase 6 foundations are already landed.
- Active work is split across:
  - Phase 7: async/runtime/task-handle lowering, library/program build surface, and Rust interop
  - Phase 8: package/workspace manifests, local-path manifest dependencies, `.qi` interface artifacts, and dependency-backed cross-file tooling
- Near-term priority is to close the real project workflow gap first: package/workspace build-run-test, richer manifests/dependencies, reproducible automation, and only then broader language/runtime expansion.
- The current `qlang.toml` surface is still intentionally small: `[package].name`, `[workspace].members`, legacy `[references].packages`, local-path `[dependencies]`, and package-level `[profile].default = "debug" | "release"`; this is not yet a full dependency build graph or full profile system.
- Project-aware `ql build` / `ql run` / `ql test` now execute one narrow cross-package path: a package can call a direct local dependency's public `extern "c"` symbols through the project dependency graph. This is still a C-ABI bridge, not general cross-package Qlang free-function/member/const semantics.
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
cargo run -p ql-cli -- project init demo-workspace --workspace --name app
cargo run -p ql-cli -- project graph demo-workspace
cargo run -p ql-cli -- check demo-workspace
```

When a clang-style toolchain is available:

```bash
cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_build.ql --emit exe
cargo run -p ql-cli -- run fixtures/codegen/pass/minimal_build.ql
cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_library.ql --emit staticlib
cargo run -p ql-cli -- build tests/ffi/pass/extern_c_export.ql --emit dylib --header
cargo run -p ql-cli -- build demo-workspace
cargo run -p ql-cli -- run demo-workspace
cargo run -p ql-cli -- test demo-workspace
```

`ql project init` 生成的最小 package / workspace 脚手架现在会同时带上 `src/lib.ql`、`src/main.ql` 和 `tests/smoke.ql`，因此新项目可以直接从根目录进入 `ql project graph` / `ql check` / `ql build` / `ql run` / `ql test`。
如果 workspace/member 之间要发生真实调用，当前稳定边界仍然只覆盖 direct dependency 的 public `extern "c"` 符号；普通跨包 Qlang 语义还没有开放。

## Docs

Online docs:

- https://qlang.zust.online/

Local docs development:

```bash
cd docs
npm install
npm run dev
```
