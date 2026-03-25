# Qlang

Qlang is a research repository for a new LLVM-based compiled programming language.

Current scope:

- language design and philosophy
- compiler and toolchain architecture
- interop strategy for C, C++, and Rust
- repository layout, feature inventory, and phased execution plan
- completed Phase 1 frontend baseline in Rust workspace form

Documentation lives in the VitePress subproject under [`docs/`](./docs).

## Docs

```bash
cd docs
npm install
npm run dev
```

## Phase 1 Frontend

Current Rust workspace status:

- `crates/ql-span`: spans and source location helpers
- `crates/ql-ast`: frontend AST definitions
- `crates/ql-lexer`: tokenization
- `crates/ql-parser`: modular parser for the current Phase 1 slice
- `crates/ql-fmt`: formatter for the current frontend slice
- `crates/ql-cli`: `ql` CLI with `check` and `fmt`

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

Quick start:

```bash
cargo test
cargo run -p ql-cli -- check fixtures/parser/pass/basic.ql
cargo run -p ql-cli -- fmt fixtures/parser/pass/basic.ql
cargo run -p ql-cli -- check fixtures/parser/pass/control_flow.ql
cargo run -p ql-cli -- check fixtures/parser/pass/phase1_declarations.ql
```
