# Qlang

Qlang is a research repository for a new LLVM-based compiled programming language.

Current scope:

- language design and philosophy
- compiler and toolchain architecture
- interop strategy for C, C++, and Rust
- repository layout, feature inventory, and phased execution plan
- completed Phase 1 frontend baseline in Rust workspace form
- landed Phase 2 semantic foundation, name resolution, and diagnostics hardening
- landed Phase 3 MIR and ownership-analysis foundation
- landed Phase 4 LLVM backend and native-artifact foundation

Documentation lives in the VitePress subproject under [`docs/`](./docs).

Phase summary:

- [`docs/roadmap/phase-progress.md`](./docs/roadmap/phase-progress.md)

## Docs

Online docs:

- https://qlang.zust.online/

Local development:

```bash
cd docs
npm install
npm run dev
```

## Current Status

Current Rust workspace status:

- `crates/ql-span`: spans and source location helpers
- `crates/ql-ast`: frontend AST definitions
- `crates/ql-lexer`: tokenization
- `crates/ql-parser`: modular parser for the current Phase 1 slice
- `crates/ql-fmt`: formatter for the current frontend slice
- `crates/ql-diagnostics`: shared semantic and parser diagnostics model plus text renderer
- `crates/ql-analysis`: shared parse/HIR/resolve/typeck analysis entry plus same-file query/rename scaffolding for CLI and LSP
- `crates/ql-lsp`: minimal `qlsp` server for hover, go-to-definition, same-file find-references, conservative same-file rename, and diagnostics over stdio
- `crates/ql-hir`: AST -> HIR lowering with stable IDs and semantic normalization
- `crates/ql-mir`: Phase 3 structural MIR with explicit CFG, cleanup actions, and textual dumps
- `crates/ql-borrowck`: Phase 3 ownership facts and explicit `move self` consumption diagnostics
- `crates/ql-resolve`: Phase 2 scope graph and conservative name resolution
- `crates/ql-typeck`: current Phase 2 semantic baseline checks
- `crates/ql-driver`: Phase 4/P5 build orchestration boundary for source loading, analysis handoff, native artifacts, and generated C headers
- `crates/ql-codegen-llvm`: Phase 4 textual LLVM IR backend foundation over a narrow MIR subset
- `crates/ql-cli`: `ql` CLI with `check`, `build`, `ffi`, `fmt`, `mir`, and `ownership`

Current implemented syntax slice:

- package / use / const / static / type / opaque type
- fn / trait / impl / extend / extern
- struct / data struct / enum
- generics on declarations and type position, `where`, callable types, tuple return
- escaped identifiers, underscore-prefixed bindings, and raw pointer types in signatures
- closures with `=>` and `move`
- `unsafe fn` and `unsafe { ... }`
- `if` / `match` expressions
- `while` / `loop` / `for` / `for await`
- pattern-based bindings and richer match patterns

Current semantic baseline in `ql check`:

- parser -> HIR lowering -> resolve -> semantic checks share one CLI pipeline
- parser diagnostics and semantic diagnostics share one renderer
- `ql-analysis` now centralizes parse -> HIR -> resolve -> typeck orchestration
- `ql-analysis` now also lowers structural MIR after resolution so later ownership and codegen passes share one stable mid-level snapshot
- `ql-analysis` now also runs the first ownership-facts pass and exposes rendered ownership state for CLI and future IDE tooling
- `ql-analysis` now also exposes minimal position-based semantic queries for symbol lookup, hover, go-to-definition, same-file find-references, and conservative same-file rename style tooling
- member-name spans and method declaration spans now stay precise through AST -> HIR, so methods inside the same trait/impl/extend block no longer collapse onto one shared query scope anchor
- named-path segment spans now also stay precise through parser -> resolver -> query indexing, so enum variant uses in patterns and constructors can anchor to the variant token itself instead of collapsing onto the enum root
- explicit struct literal and struct pattern field labels now also participate in the shared field query surface; shorthand field tokens intentionally stay bound to the local/binding symbol on that token, but same-file field rename now rewrites those shorthand sites into explicit labels when a source-backed field symbol is renamed
- import aliases now resolve as source-backed bindings with precise declaration spans, so hover / definition / same-file references / same-file rename all reuse the same semantic identity instead of a string-only pseudo symbol
- same-file rename now reuses that shared `QueryIndex`, validates new identifier text against lexer rules, and currently only enables symbol kinds whose reference surface is already considered stable enough to edit safely, including source-backed import aliases
- Phase 4 has now started with a backend foundation slice:
  - `ql-driver` keeps build orchestration out of `ql-cli`
  - `ql-codegen-llvm` lowers a controlled MIR subset into textual LLVM IR, with explicit program-mode and library-mode entry behavior
  - `ql build <file>` now writes `.ll` output, defaulting to `target/ql/<profile>/<stem>.ll`
  - `ql build <file> --emit obj`, `--emit exe`, `--emit dylib`, and `--emit staticlib` now lower through compiler/archive toolchain boundaries into native artifacts
  - emitted LLVM IR now contains an internal Qlang entry plus a host `main` wrapper, so the same IR can back `.ll`, `.obj`, and `.exe`
  - `--emit staticlib` uses library-mode codegen, so single-file libraries no longer require a top-level `main`
  - `--emit dylib` also uses library-mode codegen and currently requires at least one public top-level `extern "c"` function definition so the produced shared library has an intentional exported C surface
  - current codegen support is intentionally narrow: top-level free functions, `extern "c"` declarations and `extern "c"` function definitions, scalar integer/bool/void types, direct function calls, arithmetic, simple branching, and return
  - direct `extern "c"` declarations now flow through resolve/typeck/MIR/codegen with a shared callable identity, so both program-mode and library-mode extern calls participate in argument checking and lower to LLVM `declare` + `call`
  - top-level `extern "c"` function definitions with bodies now lower to stable exported symbol names such as `@q_add`, which gives P5 a first real C-export path on top of the P4 artifact pipeline
  - on Windows, `--emit dylib` forwards `/EXPORT:<symbol>` directives for those exported C symbols so the resulting DLL exposes the intended ABI instead of producing a no-export artifact
  - program-mode entry `main` is still required to use the default Qlang ABI; exported C ABI entrypoints must use a separate helper function
  - unsupported first-class function values now fail with structured diagnostics instead of panicking the backend
  - unsupported backend features currently fail with structured diagnostics instead of silent partial lowering
  - native artifact emission currently requires clang on PATH or an explicit `QLANG_CLANG` override
  - static library emission currently requires an archive tool on PATH or an explicit `QLANG_AR` override
  - on Windows, `QLANG_CLANG` should point to an invocable binary or `.cmd` wrapper rather than a raw `.ps1` script path
  - on Windows, `QLANG_AR` should point to an invocable archive binary such as `llvm-lib.exe`, `lib.exe`, or a `.cmd` wrapper
  - when `QLANG_AR` points to a wrapper whose filename does not imply the archive flavor, `QLANG_AR_STYLE=ar|lib` can pin the expected CLI style
  - toolchain failures preserve intermediate `.codegen.ll` and, when linking or archiving fails, intermediate `.codegen.obj` / `.codegen.o` files for debugging
  - `crates/ql-cli/tests/codegen.rs` now provides black-box codegen snapshots for `llvm-ir`, `obj`, `exe`, `dylib`, `staticlib`, library-mode `extern "c"` direct-call lowering, `extern "c"` definition exports, and build-time unsupported diagnostics
  - `crates/ql-cli/tests/ffi.rs` now provides real C-host integration smoke tests for static-library linking, shared-library runtime loading, and imported-host staticlib callbacks when a clang-style toolchain is available
  - imported-host staticlib fixtures now cover both `extern "c" { ... }` and top-level `extern "c" fn ...` declarations, and can opt into `exports|imports|both` generated headers through per-fixture `.header-surface` metadata
  - `ql ffi header <file>` now emits deterministic C headers for exported, imported, or combined `extern "c"` surfaces; exports remain the default and still write `target/ql/ffi/<stem>.h`, while imports and combined surfaces default to `target/ql/ffi/<stem>.imports.h` and `target/ql/ffi/<stem>.ffi.h`
  - `ql build <file> --emit dylib|staticlib` now also supports build-side header sidecars through `--header`, `--header-surface`, and `--header-output`; when no header output is specified, the header is written next to the built library artifact but keeps the source stem, for example `libffi_export.so` + `--header` -> `ffi_export.h`
  - build-side header generation reuses the same analysis snapshot as codegen, is rejected for non-library emits, rejects primary-artifact/header path collisions up front, and removes the just-built library artifact if sidecar generation fails so the CLI does not leave a half-success state behind
  - `crates/ql-cli/tests/ffi_header.rs` now locks export/import/both header surfaces plus failing-signature and invalid-surface regressions with black-box snapshots
- `qlsp` now consumes that shared analysis layer to provide LSP hover, go-to-definition, same-file find-references, same-file prepare/rename, and live diagnostics for open documents
- Phase 3 has started with a structural MIR slice:
  - function bodies lower into explicit basic blocks, statements, terminators, locals, scopes, and cleanup actions
  - `defer` is now represented as registered cleanup plus explicit run-cleanup steps on scope exits
  - `if` / `while` / `loop` / `break` / `continue` / block tail values lower into CFG form
  - `match` and `for` / `for await` are preserved as structural high-level MIR terminators for now instead of being prematurely forced into a lossy pseudo-lowered form
  - `ql mir <file>` now renders this layer for debugging and future borrow/drop work
  - `ql-borrowck` now performs a first forward ownership-facts pass on MIR locals
  - direct local receivers consumed by a unique `move self` method now produce use-after-move / maybe-moved diagnostics
  - deferred cleanup now participates in ownership analysis through `RunCleanup` evaluation
  - deferred cleanup ordering can now surface use-after-move / maybe-moved diagnostics at scope exit
  - `move self` consumption now happens after argument evaluation instead of before it
  - `move` closures now consume current-body direct-local captures when the closure value is created
  - non-move closures now treat captured locals as real reads in the ownership pass
  - closure capture facts are now materialized directly in MIR, so ownership/debugging no longer need to re-derive them from HIR on the hot path
  - MIR closures now carry stable identities, so later escape/drop work has a real anchor instead of ad hoc statement matching
  - `ql ownership <file>` now renders block entry/exit ownership states, read/write/consume events, and first-pass closure may-escape facts
- precise identifier spans flow through AST -> HIR -> diagnostics for semantic hotspots
- receiver parameter spans now stay precise through AST -> HIR, which keeps diagnostics and semantic queries anchored to `self` instead of whole function spans
- shorthand struct pattern and struct literal fields are normalized during HIR lowering
- scope graph construction now covers module, callable, block, closure, match-arm, and for-loop scopes
- best-effort resolution now covers locals, params, generics, imports, builtin types, struct literal roots, and pattern path roots
- resolver now records item/function scopes so semantic queries can map bindings back to declaration sites without re-walking resolution order
- conservative resolution diagnostics currently add `self` misuse detection without eagerly rejecting unresolved globals or types
- first-pass typing now covers:
  - return-value checking
  - bool conditions in `if` / `while` / match guards
  - callable argument arity and argument-type checking
  - unique impl / extend method call argument-type checking through member selection
  - tuple-based multi-return destructuring
  - direct closure checking against expected callable types
  - struct literal field checking and missing-field validation
  - positional-after-named call ordering diagnostics
  - equality-operand compatibility checks
  - struct member existence checks
  - pattern root / literal compatibility checks in destructuring and `match`
- duplicate checks currently cover:
  - top-level definitions
  - generic parameters
  - function parameters
  - enum variants
  - trait / impl / extend methods
  - pattern bindings
  - struct / struct-pattern / struct-literal fields
  - named call arguments

Current intentional gap:

- default parameters are part of the language design docs, but they are not lowered into AST/HIR or checked yet
- import / module / prelude unresolved-name strictness is still intentionally deferred
- semantic queries are still intentionally conservative: they now cover root bindings plus struct-field / unique method member tokens and enum variant tokens, but not full module-path or ambiguous method semantics yet
- `qlsp` is intentionally minimal in P2/P6: hover / definition / same-file references / same-file rename / diagnostics are live, but completion, semantic tokens, method rename, rename from shorthand-field tokens themselves, and cross-file rename are still future work
- Phase 3 ownership is intentionally narrow in this slice: direct-local `move self` consumption and direct-local `move` closure capture are diagnosed today; general call contracts, place-sensitive moves, borrow/escape analysis, and drop elaboration are still future passes on top of the current MIR foundation
- cleanup-aware ownership is still intentionally partial: nested `defer` runtime modeling and projection-sensitive cleanup effects are future work
- closure ownership is still intentionally partial: MIR capture facts, stable closure IDs, and conservative may-escape facts exist, but closure environment lowering and full escape graph construction are still future work
- Phase 4/P5 native artifacts are still intentionally partial: basic executable, dynamic-library, and static-library emission now exist, direct `extern "c"` declarations can lower in both program-mode and library-mode module builds, top-level `extern "c"` function definitions can now export stable C symbols, `ql ffi header` can project minimal export/import/both C API surfaces, and `ql build` can now attach library-side header sidecars, but arbitrary shared-library surfaces without exported C ABI, symbol-visibility control, first-class function values, separate linker-family discovery, runtime startup glue, and richer ABI support remain follow-up work

Quick start:

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test
cargo test -p ql-cli --test ffi
cargo test -p ql-cli --test ffi_header
cargo run -p ql-cli -- check fixtures/parser/pass/basic.ql
cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_build.ql --emit llvm-ir
cargo run -p ql-cli -- build fixtures/codegen/pass/extern_c_build.ql --emit llvm-ir
cargo run -p ql-cli -- ffi header tests/ffi/pass/extern_c_export.ql
cargo run -p ql-cli -- ffi header tests/ffi/header/extern_c_surface.ql --surface imports
cargo run -p ql-cli -- fmt fixtures/parser/pass/basic.ql
cargo run -p ql-cli -- mir fixtures/parser/pass/basic.ql
cargo run -p ql-cli -- ownership fixtures/parser/pass/basic.ql
cargo run -p ql-cli -- check fixtures/parser/pass/control_flow.ql
cargo run -p ql-cli -- check fixtures/parser/pass/phase1_declarations.ql
cargo run -p ql-lsp --bin qlsp
```

When clang is available:

```bash
cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_build.ql --emit obj
cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_build.ql --emit exe
cargo run -p ql-cli -- build tests/ffi/pass/extern_c_export.ql --emit dylib --header
cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_library.ql --emit staticlib
cargo run -p ql-cli -- build fixtures/codegen/pass/extern_c_library.ql --emit staticlib --header-surface imports
cargo run -p ql-cli -- build tests/ffi/pass/extern_c_export.ql --emit staticlib --header-output target/ql/debug/extern_c_export.h
```
