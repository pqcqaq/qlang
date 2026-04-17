# qlang VS Code Extension

This extension is the repository-local VS Code client for Qlang.

It does two things:

- registers the `qlang` language for `.ql` files
- starts the existing `qlsp` language server over stdio

The extension does not ship its own compiler or semantic engine. It is a thin client over [`crates/ql-lsp`](../../../crates/ql-lsp).

## Prerequisites

Build the language server first:

```powershell
cargo build -p ql-lsp
```

By default the extension tries these server locations in order:

1. `qlang.server.path` from VS Code settings
2. `<repo>/target/debug/qlsp`
3. `<repo>/target/release/qlsp`
4. `qlsp` from `PATH`

## Development

Install dependencies and compile the extension:

```powershell
cd editors/vscode/qlang
npm install
npm run compile
```

Then open `editors/vscode/qlang` in VS Code and run the `Run qlang` launch configuration.

## Settings

- `qlang.server.path`: explicit path to the `qlsp` executable
- `qlang.server.args`: extra arguments passed to `qlsp`

Changing either setting restarts the client.

## Current Scope

This extension intentionally stays thin:

- no bundled `qlsp` binary
- no Marketplace packaging flow yet
- no TextMate grammar yet; semantic coloring comes from `qlsp`

The current editor surface follows whatever `qlsp` already exposes: diagnostics, hover, definition, declaration, type definition, references, completion, document symbols, workspace symbols, semantic tokens, and conservative rename support.
