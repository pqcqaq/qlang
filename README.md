# Qlang

Qlang is a research repository for a new LLVM-based compiled programming language.

Current scope:

- language design and philosophy
- compiler and toolchain architecture
- interop strategy for C, C++, and Rust
- repository layout, feature inventory, and phased execution plan
- completed Phase 1 frontend baseline in Rust workspace form
- landed Phase 2 semantic foundation, name resolution, and diagnostics hardening

Documentation lives in the VitePress subproject under [`docs/`](./docs).

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
- `crates/ql-analysis`: shared parse/HIR/resolve/typeck analysis entry for CLI and future LSP
- `crates/ql-lsp`: minimal `qlsp` server for hover, go-to-definition, and diagnostics over stdio
- `crates/ql-hir`: AST -> HIR lowering with stable IDs and semantic normalization
- `crates/ql-mir`: Phase 3 structural MIR with explicit CFG, cleanup actions, and textual dumps
- `crates/ql-borrowck`: Phase 3 ownership facts and explicit `move self` consumption diagnostics
- `crates/ql-resolve`: Phase 2 scope graph and conservative name resolution
- `crates/ql-typeck`: current Phase 2 semantic baseline checks
- `crates/ql-cli`: `ql` CLI with `check`, `fmt`, `mir`, and `ownership`

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
- `ql-analysis` now also exposes minimal position-based semantic queries for symbol lookup, hover, and go-to-definition style tooling
- `qlsp` now consumes that shared analysis layer to provide LSP hover, go-to-definition, and live diagnostics for open documents
- Phase 3 has started with a structural MIR slice:
  - function bodies lower into explicit basic blocks, statements, terminators, locals, scopes, and cleanup actions
  - `defer` is now represented as registered cleanup plus explicit run-cleanup steps on scope exits
  - `if` / `while` / `loop` / `break` / `continue` / block tail values lower into CFG form
  - `match` and `for` / `for await` are preserved as structural high-level MIR terminators for now instead of being prematurely forced into a lossy pseudo-lowered form
  - `ql mir <file>` now renders this layer for debugging and future borrow/drop work
  - `ql-borrowck` now performs a first forward ownership-facts pass on MIR locals
  - direct local receivers consumed by a unique `move self` method now produce use-after-move / maybe-moved diagnostics
  - `ql ownership <file>` now renders block entry/exit ownership states plus read/write/consume events
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
- semantic queries are still intentionally conservative: they resolve root bindings and source-backed declarations, but not full member, variant, or module-path semantics yet
- `qlsp` is intentionally minimal in P2: hover / definition / diagnostics are live, but references / completion / rename / semantic tokens are still future work
- Phase 3 ownership is intentionally narrow in this slice: only direct-local `move self` consumption is diagnosed today; general call contracts, place-sensitive moves, borrow/escape analysis, and drop elaboration are still future passes on top of the current MIR foundation

Quick start:

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test
cargo run -p ql-cli -- check fixtures/parser/pass/basic.ql
cargo run -p ql-cli -- fmt fixtures/parser/pass/basic.ql
cargo run -p ql-cli -- mir fixtures/parser/pass/basic.ql
cargo run -p ql-cli -- ownership fixtures/parser/pass/basic.ql
cargo run -p ql-cli -- check fixtures/parser/pass/control_flow.ql
cargo run -p ql-cli -- check fixtures/parser/pass/phase1_declarations.ql
cargo run -p ql-lsp --bin qlsp
```
