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
- landed Phase 5 C FFI, header projection, and host-integration foundation
- landed Phase 6 query, rename, completion, and semantic-token editor foundation
- started conservative Phase 7 async/runtime/staticlib and Rust interop foundation

Documentation lives in the VitePress subproject under [`docs/`](./docs).
Merged phase plans live under [`docs/plans/`](./docs/plans/), and raw dated slice notes are archived under [`docs/plans/archive/`](./docs/plans/archive/).

Phase summary:

- [`docs/roadmap/phase-progress.md`](./docs/roadmap/phase-progress.md)
- [`docs/plans/index.md`](./docs/plans/index.md)
- [`docs/plans/archive/index.md`](./docs/plans/archive/index.md)

## Docs

Online docs:

- https://qlang.zust.online/

Local development:

```bash
cd docs
npm install
npm run dev
```

## Interop Example

A committed Rust host interop example now lives under [`examples/ffi-rust/`](./examples/ffi-rust/).

It demonstrates the current conservative Rust workflow:

- Cargo `build.rs` invokes `ql build --emit staticlib`
- Rust links the generated Qlang static library through the stable C ABI
- Qlang calls back into a Rust-provided `extern "C"` host function

## Current Status

Current Rust workspace status:

- `crates/ql-span`: spans and source location helpers
- `crates/ql-ast`: frontend AST definitions
- `crates/ql-lexer`: tokenization
- `crates/ql-parser`: modular parser for the current Phase 1 slice
- `crates/ql-fmt`: formatter for the current frontend slice
- `crates/ql-diagnostics`: shared semantic and parser diagnostics model plus text renderer
- `crates/ql-analysis`: shared parse/HIR/resolve/typeck analysis entry plus runtime-requirement summaries and same-file query, completion, rename, and semantic-token scaffolding for CLI and LSP
- `crates/ql-lsp`: minimal `qlsp` server for hover, go-to-definition, same-file find-references, conservative same-file completion/rename, semantic tokens, and diagnostics over stdio
- `crates/ql-hir`: AST -> HIR lowering with stable IDs and semantic normalization
- `crates/ql-mir`: Phase 3 structural MIR with explicit CFG, cleanup actions, and textual dumps
- `crates/ql-borrowck`: Phase 3 ownership facts and explicit `move self` consumption diagnostics
- `crates/ql-resolve`: Phase 2 scope graph and conservative name resolution
- `crates/ql-typeck`: current Phase 2 semantic baseline checks
- `crates/ql-runtime`: Phase 7 minimal runtime/executor abstraction with deterministic inline execution
- `crates/ql-driver`: Phase 4/P5 build orchestration boundary for source loading, analysis handoff, native artifacts, and generated C headers
- `crates/ql-codegen-llvm`: Phase 4 textual LLVM IR backend foundation over a narrow MIR subset
- `crates/ql-cli`: `ql` CLI with `check`, `build`, `ffi`, `fmt`, `mir`, `ownership`, and `runtime`

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
- `ql-analysis` now also exposes minimal position-based semantic queries for symbol lookup, hover, go-to-definition, same-file find-references, conservative same-file completion/rename style tooling, and source-backed semantic tokens
- `ql-analysis` now also exposes source-ordered runtime requirements for the current async surface, so later driver/codegen/runtime work can share one stable truth source for `async fn`, `spawn`, `await`, and `for await`
- `ql-driver` now also consumes that runtime requirement surface as the shared gate for still-unsupported async capabilities, so async executable builds, non-fixed-array `for await`, and other closed surfaces keep stable build-time diagnostics while the supported `staticlib` subset and minimal library-style async `dylib` subset expand conservatively
- `ql-runtime` now also defines the shared runtime hook ABI skeleton for the async surface, including `qlrt_async_frame_alloc`, `qlrt_async_task_create`, and the first task-result release hook `qlrt_task_result_release`; `ql runtime <file>` renders the deduped hook plan alongside capability requirements, `ql-codegen-llvm` reuses that shared ABI contract for runtime hook declarations, the backend now emits unified `async fn` body wrappers that always receive a frame pointer, can heap-materialize parameterized async frames conservatively for recursively loadable aggregate params including zero-sized fixed arrays and recursive zero-sized aggregates, lowers recursively loadable tuple/array/plain-struct values by value in LLVM IR, precomputes loadable async task-result payload layouts for `Void`, scalar builtins, and recursively loadable aggregate results including zero-sized fixed arrays, recursive zero-sized aggregates, and recursively nested fixed-shape aggregate payloads that themselves continue carrying `Task[T]`, lowers backend-local loadable `await` through `qlrt_task_await` + `qlrt_task_result_release`, lowers nested read-only struct field / tuple index / array index projections through a shared place-projection path, and now also back-propagates concrete expected fixed-array types through direct temp locals and expected aggregate literals so `return []`, direct call arguments like `take([])`, and nested aggregate forms such as `( [], 1 )`, `Wrap { values: [] }`, and `[[]]` lower when a concrete `[T; N]` context is already known while bare `[]` still stays rejected; `ql-driver` now allows that supported async library subset through `staticlib` builds plus the minimal library-style async `dylib` subset, with fixed-array iterable `for await` enabled on both library emit paths, fixed-array literal-index assignment enabled on mutable roots, conservative `Task[T]` dynamic array path tracking in borrowck, sibling-safe dynamic task-handle consume, and a new stable-immutable-index-path precise consume/reinit slice so `await tasks[index]; tasks[index] = worker(); await tasks[index]` and `await tasks[slot.value]; tasks[slot.value] = worker(); await tasks[slot.value]` can succeed when the dynamic index expression is rooted in the same immutable stable path, while arbitrary dynamic overlap/reinit, async executables outside the current subset, non-fixed-array `for await`, non-`Task[T]` projections, and broader async payload paths stay conservative, and `ql-typeck` plus `ql-resolve` now expose a provisional `Task[T]` handle type surface so direct `async fn` calls, spawned tasks, and sync helpers can all flow through the same task-handle model before a later `await`, including helper-returned handles that are locally rebound before `await` or `spawn`
- `ql-borrowck` now also treats direct-local task handles as consumed by `await`, `spawn`, statically-known helper `Task[T]` argument passing, and direct-local `return task`, so reusing the same `Task[T]` local after submission, helper transfer, returning, or awaiting surfaces stable moved / maybe-moved diagnostics while reassignment still reinitializes the slot
- local import aliases that point at same-file enum items now also forward variant-token hover / definition / references / completion / same-file rename / semantic tokens through that shared query layer, without pretending that a real module graph already exists
- local import aliases that point at same-file struct items now also forward struct-literal field checking plus explicit/shorthand field-label query and rename follow-through through the shared type/query layers, while same-file struct or enum pattern roots also canonicalize those aliases back to the underlying local item when that is semantically safe
- local import aliases that point at same-file function / const / static items now also reuse the shared type-checking value/call surface, so callable argument mismatches and non-callable value diagnostics no longer silently degrade to `unknown`
- assignment target diagnostics now explicitly reject non-writable or not-yet-supported targets: `const` / `static` / function / import bindings produce stable same-file errors, while writable projections stay conservatively limited to members, tuple indices, and fixed-array literal indices until a broader place-sensitive assignment model exists
- ambiguous method member access now also reports stable type-check diagnostics instead of silently degrading to `unknown`, while ambiguous completion/rename/query surfaces stay deliberately closed until a richer member truth model exists
- known-invalid projection receivers now also report stable type-check diagnostics: member access on builtins/arrays/tuples/callables/pointers and indexing on clearly non-indexable known types no longer silently degrade, while unknown/generic/deeper import-module cases remain conservative
- invalid deeper path-like calls on known-invalid member receivers no longer reuse root function/import signatures, so `ping.scope(true)` now stops at the projection error instead of cascading into fake call-argument mismatches from the root callable
- known-invalid struct-literal roots now also report stable type-check diagnostics: builtin roots, generic roots, enum roots without a struct-style variant constructor, tuple/unit variants, and same-file import aliases that canonicalize to those unsupported roots no longer silently degrade
- known-invalid struct/tuple pattern roots now also report stable type-check diagnostics: struct items matched with tuple-style patterns, enum roots without an explicit variant, tuple/unit variants matched with struct-style patterns, struct/unit variants matched with tuple-style patterns, and same-file import aliases that canonicalize to those unsupported roots no longer silently degrade
- known-invalid bare path-pattern roots now also report stable type-check diagnostics: struct items, enum roots without an explicit variant, non-unit enum variants, and same-file import aliases that canonicalize to those unsupported roots no longer silently degrade, while unit variants stay accepted
- bare path patterns that resolve to same-file `const` / `static` items or their same-file import aliases are now also rejected explicitly instead of silently passing through a deferred gap
- same-file two-segment enum variant lookup now also produces explicit unknown-variant diagnostics across struct literals plus tuple/struct/path patterns, including same-file import aliases canonicalized back to local enums; deeper multi-segment module-like paths and cross-file variant semantics stay deliberately conservative
- unsupported or still-deferred struct literal roots now also fall back to `unknown` instead of leaking a fake concrete item type into later return/assignment mismatches
- deferred multi-segment type paths now also stay source-backed instead of canonicalizing same-file local items/import aliases too early, so query/type surfaces no longer pretend `Cmd.Scope.Config` is already the concrete local type `Command`
- deferred multi-segment `impl` / `extend` targets are now also regression-locked away from concrete local receiver surfaces, so member typing/completion no longer risks leaking fake methods from paths like `Counter.Scope.Config` onto the real same-file item `Counter`
- member-name spans and method declaration spans now stay precise through AST -> HIR, so methods inside the same trait/impl/extend block no longer collapse onto one shared query scope anchor
- named-path segment spans now also stay precise through parser -> resolver -> query indexing, so enum variant uses in patterns and constructors can anchor to the variant token itself instead of collapsing onto the enum root
- explicit struct literal and struct pattern field labels now also participate in the shared field query surface; shorthand field tokens intentionally stay bound to the local/binding symbol on that token, and same-file rename now rewrites those shorthand sites into explicit labels both for source-backed field renames and for binding renames launched from the shorthand token itself when the underlying symbol is renameable, including local / parameter / import / function / const / static item-value cases
- free-function shorthand binding rename is now also explicit on the LSP side, so a shorthand literal token like `Ops { add_one }` keeps preserving the field label while renaming the bound function declaration and call sites instead of relying only on analysis-side coverage
- same-file rename for type-namespace items is now also regression-locked end to end: `type`, `opaque type`, `struct`, `enum`, and `trait` continue to reuse the same shared query surface in both analysis and LSP without introducing cross-file or module-graph behavior
- same-file rename for root function/const/static symbols is now also regression-locked end to end: direct call/use sites keep reusing the same shared query surface in both analysis and LSP prepare-rename/rename instead of only being covered through aggregate analysis tests or shorthand-binding regressions
- the same type-namespace item surface now also has explicit references / semantic-token parity coverage, so `type`, `opaque type`, `struct`, `enum`, and `trait` continue to resolve through one same-file truth source instead of drifting between query and editor highlighting behavior
- hover / definition parity for that same type-namespace item surface is now also regression-locked, so `type`, `opaque type`, `struct`, `enum`, and `trait` keep one same-file identity across navigation, references, rename, and semantic highlighting
- same-file type-namespace item parity is now also aggregate-regression-locked, so `type`, `opaque type`, `struct`, `enum`, and `trait` keep matching one ordered truth surface across hover / definition / references / semantic-token projection instead of relying on isolated per-kind tests
- same-file global value items now also have explicit query parity coverage: `const` and `static` keep one shared `QueryIndex` identity across hover / definition / references / semantic tokens in both analysis and LSP, instead of letting item definitions and value uses drift apart
- `extern` callable surfaces now also have explicit same-file parity coverage: `extern` block members, top-level `extern "c"` declarations, and top-level `extern "c"` function definitions all keep one shared `Function` identity across hover / definition / references / rename / semantic tokens in both analysis and LSP
- extern callable value completion now also has explicit parity coverage across analysis and LSP, so `extern` block members, top-level `extern "c"` declarations, and top-level `extern "c"` function definitions continue to surface as `FUNCTION` completion items with stable detail and text-edit mapping instead of only being covered through ordinary free-function completion
- ordinary free functions now also have explicit same-file query parity coverage: direct call sites keep one shared `Function` identity across hover / definition / references in both analysis and LSP, instead of only being covered through completion, rename, or aggregate root-binding tests
- ordinary free functions now also have explicit same-file semantic-token parity coverage: declarations and direct call sites keep matching the shared function-token surface in both analysis and LSP, instead of only being covered through aggregate semantic-token snapshots
- same-file callable surface parity is now also aggregate-regression-locked, so `extern` block callables, top-level `extern "c"` declarations, top-level `extern "c"` definitions, and ordinary free functions keep matching one ordered callable truth surface across hover / definition / references / semantic-token projection instead of only being covered by isolated per-kind tests
- lexical semantic symbols now also have explicit same-file parity coverage: `generic`, `parameter`, `local`, `receiver self`, and `builtin type` keep one stable behavior split across analysis and LSP; builtin types intentionally remain non-source-backed, so they expose hover / references / semantic tokens but still have no definition or rename surface
- lexical rename behavior is now also regression-locked end to end: `generic`, `parameter`, and `local` keep their existing same-file rename surface in both analysis and LSP, while `receiver self` and `builtin type` remain deliberately closed for rename
- import aliases now resolve as source-backed bindings with precise declaration spans, so hover / definition / same-file references / same-file rename / semantic tokens all reuse the same semantic identity instead of a string-only pseudo symbol
- plain import alias symbols now also have explicit same-file parity coverage across analysis and LSP for hover / definition / references / semantic tokens, which keeps source-backed import bindings aligned with editor navigation and highlighting behavior
- plain import alias type-context completion now also has explicit parity coverage across analysis and LSP, so lexical type completion continues to surface `import` candidates and the LSP bridge keeps mapping them to `MODULE` completion items with stable text edits
- free function lexical value completion now also has explicit parity coverage across analysis and LSP, so same-file value completion continues to surface callable declarations and the LSP bridge keeps mapping them to `FUNCTION` completion items with stable text edits
- plain import alias lexical value completion now also has explicit parity coverage across analysis and LSP, so same-file value completion continues to surface source-backed `import` bindings and the LSP bridge keeps mapping them to `MODULE` completion items with stable text edits
- builtin type and local struct type-context completion now also have explicit parity coverage across analysis and LSP, so same-file type completion keeps surfacing source-backed type candidates while the LSP bridge preserves the expected `CLASS`/`STRUCT` completion item mappings
- same-file type alias type-context completion now also has explicit parity coverage across analysis and LSP, so `QueryIndex` continues to surface `type alias` candidates while the LSP bridge preserves the expected `CLASS` completion item mapping and stable text edits
- same-file `opaque type` type-context completion now also has explicit parity coverage across analysis and LSP, so `TypeAlias`-backed opaque aliases continue to surface `opaque type ...` detail while the LSP bridge preserves the existing `CLASS` completion item mapping and stable text edits
- same-file generic type-context completion now also has explicit parity coverage across analysis and LSP, so lexical type completion continues to surface generic candidates while the LSP bridge preserves the expected `TYPE_PARAMETER` completion item mapping, detail rendering, and stable text edits
- same-file enum type-context completion now also has explicit parity coverage across analysis and LSP, so lexical type completion continues to surface enum candidates while the LSP bridge preserves the expected `ENUM` completion item mapping, detail rendering, and stable text edits
- same-file trait type-context completion now also has explicit parity coverage across analysis and LSP, so lexical type completion continues to surface trait candidates while the LSP bridge preserves the expected `INTERFACE` completion item mapping, detail rendering, and stable text edits
- stable receiver field completion now also has explicit parity coverage across analysis and LSP, so same-file member completion continues to surface field candidates while the LSP bridge preserves the expected `FIELD` completion item mapping, detail rendering, and stable text edits
- stable receiver unique-method completion now also has explicit parity coverage across analysis and LSP, so same-file member completion continues to surface unique method candidates while the LSP bridge preserves the expected `FUNCTION` completion item mapping, detail rendering, and stable text edits
- same-file const and static value completion now also has explicit parity coverage across analysis and LSP, so lexical value completion continues to surface these item candidates while the LSP bridge preserves the expected `CONSTANT` completion item mapping, detail rendering, and stable text edits
- same-file local value completion now also has explicit parity coverage across analysis and LSP, so lexical value completion continues to surface `local` candidates while the LSP bridge preserves the expected `VARIABLE` completion item mapping, detail rendering, and stable text edits
- same-file parameter value completion now also has explicit parity coverage across analysis and LSP, so lexical value completion continues to surface parameter candidates while the LSP bridge preserves the expected `VARIABLE` completion item mapping, detail rendering, and stable text edits
- same-file lexical value candidate-list parity is now also explicit on both sides, so import / const / static / extern callable / free function / local / parameter candidates keep matching the ordered `QueryIndex` value list, detail rendering, and replacement text-edit projection instead of only being covered through scattered per-kind checks
- same-file enum variant completion now also has explicit parity coverage across analysis and LSP, so parsed enum path completion continues to surface `variant` candidates while the LSP bridge preserves the expected `ENUM_MEMBER` completion item mapping, detail rendering, and stable text edits
- same-file import-alias variant completion now also has explicit parity coverage across analysis and LSP, so local import aliases that point at same-file enum items continue to surface `variant` candidates while the LSP bridge preserves the expected `ENUM_MEMBER` completion item mapping, detail rendering, and stable text edits
- same-file import-alias struct-variant completion now also has explicit parity coverage across analysis and LSP, so local import aliases that point at same-file enum items continue to surface struct-style `variant` candidates while the LSP bridge preserves the expected `ENUM_MEMBER` completion item mapping, detail rendering, and stable text edits
- remaining same-file variant-path completion contexts now also have explicit parity coverage across analysis and LSP, so direct struct-literal paths plus direct/import-alias pattern paths continue to surface existing `variant` candidates with stable `ENUM_MEMBER` mapping, detail rendering, and text edits
- same-file variant-path candidate-list parity is now also explicit on both sides, so enum-root / struct-literal / pattern paths and their same-file import-alias mirrors keep matching the ordered `variant` candidate list, detail rendering, and replacement text-edit projection already implied by `QueryIndex`
- deeper variant-like member chains now stay closed on both sides: only the first projection off a root enum item or same-file import alias can reuse variant truth, so `Command.Retry.more` / `Cmd.Retry.more` no longer fabricate variant hover / definition / references or `ENUM_MEMBER` completion candidates from the root enum
- deeper struct-literal and pattern variant-like paths now also stay closed on both sides: only strict two-segment `Root.Variant` paths reuse variant truth, so `Command.Scope.Config { ... }` / `Cmd.Scope.Retry(...)` no longer fabricate variant hover / definition / references / rename / semantic tokens or `ENUM_MEMBER` completion candidates from the root enum
- deeper struct-like paths now also stay closed on both sides for field semantics: only strict root struct paths keep field truth, so `Point.Scope.Config { x: ... }` / `P.Scope.Config { x: ... }` no longer fabricate field hover / definition / references / rename / semantic tokens from the root struct
- deeper struct-like shorthand tokens now also have explicit same-file parity coverage: when field semantics stay closed on paths like `Point.Scope.Config { x }` / `Point.Scope.Config { source }`, the shorthand token still follows its lexical local / binding / import identity across hover / definition / references / semantic tokens, and same-file rename keeps those edits raw instead of fabricating unsupported field-label expansions
- same-file completion filtering parity is now also explicit on the LSP side, so lexical shadowing/visibility and impl-preferred member completion keep matching the already-locked analysis behavior instead of only being covered indirectly through single-candidate prefix tests; the lexical value visibility aggregate now also locks import/function/local detail and replacement text-edit projection, and the impl-preferred member aggregate now also locks the surviving candidate count plus stable detail/text-edit projection
- same-file completion candidate-list parity is now also explicit on the LSP side, so full type-context and stable-member candidate lists keep matching the already-locked analysis ordering and namespace boundaries instead of only being covered through per-item mapping tests; the type-context matrix is regression-locked across builtin/import/struct/type alias/opaque type/enum/trait/generic candidates, and the stable-member aggregate list now also locks method/field detail plus replacement text-edit projection
- shorthand struct field token query parity is now also explicit on the LSP side, so editor-facing hover/definition keeps matching the intentionally conservative analysis rule that shorthand tokens stay on the local binding surface instead of silently drifting onto the field surface
- direct same-file variant/field-label query parity is now also explicit on the LSP side, so direct enum variant tokens and direct explicit struct field labels keep matching the already-locked analysis definition/reference surface instead of only being covered through import-alias follow-through tests
- direct same-file variant/field-label semantic-token parity is now also explicit on both the analysis and LSP sides, so direct enum variant tokens and direct explicit struct field labels keep matching the existing occurrence-based highlighting surface instead of only being covered through aggregate or import-alias semantic-token tests
- same-file direct symbol surface parity is now also aggregate-regression-locked, so direct enum variant tokens and direct explicit struct field labels keep matching one ordered truth surface across hover / definition / references / semantic-token projection instead of relying on isolated per-kind tests
- direct stable-member query parity is now also explicit on the LSP side, so direct field members and unique method members keep matching the already-locked analysis hover/definition/reference surface instead of only being covered through hover-only or import-alias-style tests
- direct stable-member semantic-token parity is now also explicit on both the analysis and LSP sides, so direct field members and unique method members keep matching the existing occurrence-based highlighting surface instead of only being covered through aggregate semantic-token tests
- same-file direct member surface parity is now also aggregate-regression-locked, so direct field members and unique method members keep matching one ordered truth surface across hover / definition / references / semantic-token projection instead of relying on isolated per-kind tests
- impl-preferred member query parity is now also explicit on the analysis and LSP sides, so direct member queries that intentionally choose an `impl` method over an `extend` method keep matching the already-locked hover/definition/reference surface instead of drifting at the bridge layer
- same-file rename now reuses that shared `QueryIndex`, validates new identifier text against lexer rules, and currently only enables symbol kinds whose reference surface is already considered stable enough to edit safely, including source-backed import aliases, local struct fields, and unique method symbols
- same-file completion now also distinguishes semantic labels from source insert text, so escaped identifiers like `` `type` `` continue to round-trip as legal edits instead of inserting raw keyword text
- Phase 4 foundation currently includes:
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
  - deferred multi-segment source-backed types now also stay preserved in backend and FFI unsupported diagnostics, so codegen/header errors report `Cmd.Scope.Config` instead of collapsing that path to a fake same-file concrete type like `Command`
  - native artifact emission currently requires clang on PATH, a common Windows LLVM install that `ql-driver` can auto-discover, or an explicit `QLANG_CLANG` override
  - static library emission currently requires an archive tool on PATH, a common Windows LLVM install that `ql-driver` can auto-discover, or an explicit `QLANG_AR` override
  - on Windows, `QLANG_CLANG` should point to an invocable binary or `.cmd` wrapper rather than a raw `.ps1` script path
  - on Windows, `QLANG_AR` should point to an invocable archive binary such as `llvm-lib.exe`, `lib.exe`, or a `.cmd` wrapper
  - when `QLANG_AR` points to a wrapper whose filename does not imply the archive flavor, `QLANG_AR_STYLE=ar|lib` can pin the expected CLI style
  - on Windows, `ql-driver` now also probes common LLVM install directories such as Scoop `llvm/current/bin`, `%LOCALAPPDATA%\Programs\LLVM\bin`, `%ProgramFiles%\LLVM\bin`, and `%ProgramFiles(x86)%\LLVM\bin`, and missing-tool diagnostics now include concrete candidate paths
  - toolchain failures preserve intermediate `.codegen.ll` and, when linking or archiving fails, intermediate `.codegen.obj` / `.codegen.o` files for debugging
  - `crates/ql-cli/tests/codegen.rs` now provides black-box codegen snapshots for `llvm-ir`, `obj`, `exe`, `dylib`, `staticlib`, library-mode `extern "c"` direct-call lowering, `extern "c"` definition exports, and build-time unsupported diagnostics
  - `crates/ql-cli/tests/ffi.rs` now provides real C-host integration smoke tests for static-library linking, shared-library runtime loading, and imported-host staticlib callbacks when a clang-style toolchain is available, and those tests now reuse `ql-driver` toolchain discovery so their skip logic matches the real build pipeline
  - imported-host staticlib fixtures now cover both `extern "c" { ... }` and top-level `extern "c" fn ...` declarations, and can opt into `exports|imports|both` generated headers through per-fixture `.header-surface` metadata
  - `ql ffi header <file>` now emits deterministic C headers for exported, imported, or combined `extern "c"` surfaces; exports remain the default and still write `target/ql/ffi/<stem>.h`, while imports and combined surfaces default to `target/ql/ffi/<stem>.imports.h` and `target/ql/ffi/<stem>.ffi.h`
  - `ql build <file> --emit dylib|staticlib` now also supports build-side header sidecars through `--header`, `--header-surface`, and `--header-output`; when no header output is specified, the header is written next to the built library artifact but keeps the source stem, for example `libffi_export.so` + `--header` -> `ffi_export.h`
  - build-side header generation reuses the same analysis snapshot as codegen, is rejected for non-library emits, rejects primary-artifact/header path collisions up front, and removes the just-built library artifact if sidecar generation fails so the CLI does not leave a half-success state behind
  - `crates/ql-cli/tests/ffi_header.rs` now locks export/import/both header surfaces plus failing-signature and invalid-surface regressions with black-box snapshots, including source-backed deferred multi-segment type names in unsupported signature diagnostics
- `qlsp` now consumes that shared analysis layer to provide LSP hover, go-to-definition, same-file find-references, same-file completion, same-file semantic tokens, same-file prepare/rename, and live diagnostics for open documents
- Phase 3 foundation currently includes:
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
- conservative resolution diagnostics now cover `self` misuse plus clearly bare single-segment unresolved value/type roots; multi-segment import/module/prelude unresolved strictness is still deferred
- first-pass typing now covers:
  - return-value checking
  - bool conditions in `if` / `while` / match guards
  - callable argument arity and argument-type checking
  - unique impl / extend method call argument-type checking through member selection
  - tuple-based multi-return destructuring
  - direct closure checking against expected callable types
  - struct and enum struct-variant literal field checking and missing-field validation, including same-file local import alias paths that canonicalize back to local enum items
  - source-level fixed array type expressions `[T; N]`, including lexer-style length literals that lower to semantic lengths
  - homogeneous array-literal inference plus expected fixed-array context guidance for first-pass array item type checking
  - conservative tuple/array indexing: array element projection, constant tuple indexing with lexer-style integer literals, array-index type checks, and tuple out-of-bounds diagnostics
  - positional-after-named call ordering diagnostics
  - equality-operand compatibility checks
  - comparison-operand compatible-numeric checks
  - struct member existence checks
  - pattern root / literal compatibility checks in destructuring and `match`
  - unknown struct-pattern field diagnostics, including same-file local import alias paths that canonicalize back to local items
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
- multi-segment import / module / prelude unresolved-name strictness is still intentionally deferred
- general index-protocol semantics are still intentionally deferred: source-level fixed arrays, inferred arrays, and constant tuple indexing are typed today, but a broader index protocol is not
- semantic queries are still intentionally conservative: they now cover root bindings plus struct-field / unique method member tokens and enum variant tokens, including local import aliases that point at same-file enum items or same-file struct items, but not full module-path or ambiguous method semantics yet
- `qlsp` is intentionally minimal in P2/P6: hover / definition / same-file references / same-file rename / same-file lexical-scope completion / parsed member-token completion / same-file parsed enum variant-path completion / same-file semantic tokens / diagnostics are live, and local import aliases to same-file enum items now also participate in variant-path query / completion / rename / semantic-token projection while local import aliases to same-file struct items now also participate in explicit/shorthand struct-field query and rename follow-through; completion now also preserves escaped-identifier insert text for keyword-named symbols, and same-file binding renames launched from shorthand struct tokens now preserve field semantics by expanding to explicit labels, while ambiguous-member completion, parse-error-tolerant dot-trigger completion, import-graph/module-path deeper completion, ambiguous method rename, field-symbol rename semantics from shorthand-field tokens themselves, and cross-file rename are still future work
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
cargo run -p ql-cli -- check fixtures/codegen/pass/minimal_build.ql
cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_build.ql --emit llvm-ir
cargo run -p ql-cli -- build fixtures/codegen/pass/extern_c_build.ql --emit llvm-ir
cargo run -p ql-cli -- ffi header tests/ffi/pass/extern_c_export.ql
cargo run -p ql-cli -- ffi header tests/ffi/header/extern_c_surface.ql --surface imports
cargo run -p ql-cli -- fmt fixtures/parser/pass/basic.ql
cargo run -p ql-lsp --bin qlsp
```

Parser fixtures under `fixtures/parser/pass/` remain the parser/formatter regression surface. Some of them intentionally keep placeholder symbols such as `tick`, `IoError`, or `parse_int`, so they are not all semantic-clean inputs for the current `ql check` pipeline.

When clang is available:

```bash
cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_build.ql --emit obj
cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_build.ql --emit exe
cargo run -p ql-cli -- build tests/ffi/pass/extern_c_export.ql --emit dylib --header
cargo run -p ql-cli -- build fixtures/codegen/pass/minimal_library.ql --emit staticlib
cargo run -p ql-cli -- build fixtures/codegen/pass/extern_c_library.ql --emit staticlib --header-surface imports
cargo run -p ql-cli -- build tests/ffi/pass/extern_c_export.ql --emit staticlib --header-output target/ql/debug/extern_c_export.h
cargo run -p ql-cli -- runtime tests/codegen/fail/unsupported_async_fn_build.ql
```
