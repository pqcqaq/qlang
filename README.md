# Qlang

Qlang is a research repository for a new LLVM-based compiled programming language.

Current scope:

- language design and philosophy
- compiler and toolchain architecture
- interop strategy for C, C++, and Rust
- repository layout, feature inventory, and phased execution plan
- early Rust workspace bootstrap for Phase 1 frontend development

Documentation lives in the VitePress subproject under [`docs/`](./docs).

## Docs

```bash
cd docs
npm install
npm run dev
```

## Frontend Bootstrap

Current Rust workspace status:

- `crates/ql-span`: spans and source location helpers
- `crates/ql-ast`: frontend AST definitions
- `crates/ql-lexer`: tokenization
- `crates/ql-parser`: modular parser for the current Phase 1 slice
- `crates/ql-fmt`: formatter for the current frontend slice
- `crates/ql-cli`: `ql` CLI with `check` and `fmt`

Current implemented syntax slice:

- package / use / fn / struct / data struct / enum / impl
- generics in type position, callable types, tuple return
- closures with `=>` and `move`
- `if` / `match` expressions
- `while` / `loop` / `for` / `for await`
- pattern-based bindings and richer match patterns

Quick start:

```bash
cargo test
cargo run -p ql-cli -- check fixtures/parser/pass/basic.ql
cargo run -p ql-cli -- fmt fixtures/parser/pass/basic.ql
cargo run -p ql-cli -- check fixtures/parser/pass/control_flow.ql
```
