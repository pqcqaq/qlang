# Stdlib Generics and Collections Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Turn `stdlib` from a narrow concrete helper set into a real reusable standard library by unblocking generic package execution, collection-style APIs, and a clear migration path away from fixed-arity helper duplication.

**Architecture:** `stdlib` remains an ordinary Qlang workspace, not a compiler prelude. Compiler and project support must come first for generic public APIs, because a standard library API that cannot pass `ql check/build/run/test` through local dependencies is not usable. Variadic syntax is treated as a later language feature; near-term stdlib should prefer arrays, iterable helpers, and generic APIs over adding more `foo3/foo4/foo5` variants.

**Tech Stack:** Rust crates `ql-parser`, `ql-ast`, `ql-hir`, `ql-resolve`, `ql-typeck`, `ql-project`, `ql-driver`, `ql-codegen-llvm`, `ql-cli`, plus Qlang packages under `stdlib/packages/*`.

---

## Current Assessment

- `stdlib` is real production-facing code: downstream projects can depend on `std.core`, `std.option`, `std.result`, `std.array`, and `std.test` through local `[dependencies]`.
- The current API shape is still too concrete because backend and dependency bridge support only cover a narrow generic function import/call slice. Generic public type execution is partially open for explicit `struct` / `enum` instantiations in concrete signatures and contextual aggregate usage.
- `IntOption` / `BoolOption`, `IntResult` / `BoolResult`, and `sum3_int` / `sum4_int` / `sum5_int` are transitional compatibility surfaces, not the desired long-term library design.
- Generic `Option[T]` / `Result[T, E]` should move into the P0 stdlib unblock path once generic package execution is stable.
- Variadic parameters can remove fixed-arity duplication, but they affect parser, typeck, ABI, lowering, LSP, and docs. Do not block stdlib on variadic syntax; first make collection APIs good enough.

## Non-Negotiable Rules

- Do not add more fixed-arity helpers unless they unlock an immediate downstream smoke path.
- Do not expose generic stdlib APIs as "supported" until they pass package-aware `ql check/build/run/test` from a downstream package.
- Keep concrete `Int*` / `Bool*` APIs until generic replacements are executable and templates have migrated.
- Every stdlib feature must include package-local tests and at least one downstream consumer test.
- If stdlib exposes a compiler/backend limitation, fix the compiler/backend path instead of lowering the library design.

## Task 1: Stdlib Contract and Downstream Harness

**Files:**
- Modify: `stdlib/README.md`
- Modify: `docs/roadmap/development-plan.md`
- Modify: `crates/ql-cli/tests/project_stdlib.rs` or nearest existing project-init/std-lib test file
- Test: `stdlib/packages/*/tests/*.ql`

**Steps:**

1. Add a concise contract that marks existing concrete APIs as transitional compatibility APIs.
2. Add or tighten a downstream package/workspace test that runs `ql project init --stdlib <path>` and then `ql test` against the generated project.
3. Ensure that generated smoke code imports `std.core`, `std.option`, `std.result`, `std.array`, and `std.test` through normal local dependencies.
4. Run the smallest CLI tests covering stdlib project init and generated smoke execution.
5. Commit as `docs: clarify stdlib roadmap` if this remains documentation-only, or `test: strengthen stdlib consumer smoke` if tests are added.

## Task 2: Generic Public API Execution

Status: fourth execution slice landed. Direct local dependencies can now expose public generic `struct` / `enum` declarations, use explicit instantiations such as `Box[Int]` / `Maybe[Int]` in non-generic public function signatures consumed by a root project, and build contextual generic struct literals plus field projection when the expected type carries concrete args. Typeck also substitutes those args through struct/enum patterns and enum unit/tuple/struct variant construction. `.qi` emission has regression coverage for generic enum and generic function declarations, and library-mode codegen no longer fails just because a generic function/method declaration exists but is never instantiated. Direct local dependency and package-under-test bridges now support one concrete instantiation of a public generic free function when every generic parameter can be inferred from direct `Int` / `Bool` / `String` literal call arguments, e.g. `identity[T](value: T) -> T` called as `identity(7)`. Uninferred, multi-instantiation, method, escaping value, named-argument, and complex generic helper cases still report `dependency-function-unsupported-generic`. Full generic function monomorphization, generic aliases, and generic stdlib `Option[T]` / `Result[T, E]` helper execution remain open.

**Files:**
- Modify: `crates/ql-project/src/lib.rs`
- Modify: `crates/ql-driver/src/build/*`
- Modify: `crates/ql-codegen-llvm/src/lib.rs`
- Modify: `crates/ql-typeck/src/*` only if instantiated generic checking needs adjustment
- Test: `crates/ql-cli/tests/project_interface.rs`
- Test: `crates/ql-cli/tests/codegen.rs` or nearest project-aware codegen test file

**Steps:**

1. Write failing tests for a direct local dependency exporting `pub fn identity[T](value: T) -> T`.
2. Write failing tests for a direct local dependency exporting `pub struct Box[T] { value: T }` and a public function returning `Box[Int]`.
3. Write failing tests for a direct local dependency exporting `pub enum Maybe[T] { Some(T), None }` and a public function returning `Maybe[Int]`.
4. Verify current failure is the existing generic backend/dependency bridge rejection, not a parser or resolver failure.
5. Preserve generic parameters and instantiated type arguments through `.qi` rendering/parsing and project dependency bridge preparation.
6. Add minimal monomorphization or specialization for instantiated generic functions/types used by the root target.
7. Keep unsupported generic cases explicit with diagnostics rather than falling back to `Unknown`.
8. Run focused project/interface/codegen tests.
9. Commit as `feat: support generic dependency APIs`.

## Task 3: Generic Option and Result Packages

**Files:**
- Modify: `stdlib/packages/option/src/lib.ql`
- Modify: `stdlib/packages/option/tests/smoke.ql`
- Modify: `stdlib/packages/result/src/lib.ql`
- Modify: `stdlib/packages/result/tests/smoke.ql`
- Modify: `stdlib/packages/test/src/lib.ql`
- Modify: `stdlib/packages/test/tests/smoke.ql`
- Modify: `stdlib/README.md`

**Steps:**

1. Add generic `Option[T]` beside existing `IntOption` / `BoolOption`.
2. Add generic constructors and predicates where the compiler can execute them through local dependencies.
3. Add generic `Result[T, E]` after generic enum/function execution is stable.
4. Keep concrete wrappers as compatibility shims until templates migrate.
5. Update `std.test` to cover generic and concrete carrier paths.
6. Run `cargo run -q -p ql-cli -- check --sync-interfaces stdlib`.
7. Run `cargo run -q -p ql-cli -- test stdlib`.
8. Commit as `feat: add generic std option result`.

## Task 4: Collection-First Replacement for Fixed-Arity Helpers

Status: first executable slice landed. `std.array` now provides concrete fixed-array helpers for `Int` and `Bool`, has package-local smoke tests, has a generated `.qi`, and is consumed by `ql project init --stdlib` package/workspace templates. Next work in this task should replace the concrete `3/4/5` surface with generic array helpers only after generic public API execution is available.

**Files:**
- Create: `stdlib/packages/array/qlang.toml`
- Create: `stdlib/packages/array/src/lib.ql`
- Create: `stdlib/packages/array/tests/smoke.ql`
- Modify: `stdlib/qlang.toml`
- Modify: `stdlib/README.md`
- Modify: project-init stdlib template code in `crates/ql-cli/src/*`

**Steps:**

1. Add `std.array` only after array values can cross the package boundary in executable code.
2. Start with `Int` and `Bool` array helpers if fully generic array helpers are still blocked.
3. Prefer APIs like "sum values in an array" or "all values in an array" over adding more `sum6_int` or `all6_bool`.
4. Add package-local smoke tests.
5. Add a downstream generated-template smoke path that imports and executes at least one `std.array` helper.
6. Run stdlib package checks and CLI project-init tests.
7. Commit as `feat: add std array helpers`.

## Task 5: Variadic Function Design Gate

**Files:**
- Create: `docs/design/variadic-functions.md`
- Modify: `docs/roadmap/development-plan.md`
- Later implementation files: `crates/ql-parser/src/*`, `crates/ql-ast/src/*`, `crates/ql-hir/src/*`, `crates/ql-typeck/src/*`, `crates/ql-codegen-llvm/src/lib.rs`, `crates/ql-lsp/src/*`

**Steps:**

1. Write a design note deciding whether Qlang wants rest parameters, tuple splat, array splat, or all three.
2. Specify call-site syntax, overload rules, ABI/lowering rules, and how variadic functions appear in `.qi`.
3. Only after the design is accepted, add parser/typeck diagnostics for unsupported rest parameters behind explicit tests.
4. Implement a minimal fixed target such as `fn sum(values: ...Int) -> Int` only after lowering and LSP can represent it.
5. Migrate fixed-arity stdlib helpers to variadic wrappers only when the feature is executable through project-aware builds.
6. Commit the design separately before implementation.

## Task 6: Stdlib Package Growth Order

**Files:**
- Modify: `stdlib/qlang.toml`
- Create or modify: `stdlib/packages/math/*`
- Create or modify: `stdlib/packages/bool/*`
- Create or modify: `stdlib/packages/array/*`
- Create or modify: `stdlib/packages/string/*`
- Create or modify: `stdlib/packages/io/*`

**Steps:**

1. Keep `std.core` small and stable; do not keep growing it as a dumping ground.
2. Move new numeric APIs into `std.math` once package splitting and re-export strategy are settled.
3. Move boolean aggregate APIs into `std.bool` or collection helpers instead of adding more arity variants.
4. Add `std.string` only after string values and common operations are executable across packages.
5. Add `std.io` only after runtime-backed IO has a stable API and tests can run deterministically.
6. Keep `std.test` focused on smoke-test assertions and status aggregation.
7. Commit each package addition separately with package-local and downstream tests.

## Verification Gate

Run the smallest relevant checks for each slice:

```powershell
cargo run -q -p ql-cli -- project targets stdlib
cargo run -q -p ql-cli -- check --sync-interfaces stdlib
cargo run -q -p ql-cli -- test stdlib
```

For compiler/backend slices, also run the focused Rust integration tests that were added for the failing behavior.

## Migration Policy

- Generic APIs become primary only after they are executable through local dependencies.
- Concrete APIs remain until generated templates and downstream smoke tests have migrated.
- Fixed-arity helpers are compatibility helpers, not the direction of travel.
- Collection APIs are the near-term answer to repeated arguments.
- Variadic syntax is a language feature and must not be faked in stdlib documentation before implementation.
