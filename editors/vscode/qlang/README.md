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

## Package VSIX

Build a distributable VSIX from the extension directory:

```powershell
cd editors/vscode/qlang
npm install
npm run package:vsix
```

The package is written to:

```text
editors/vscode/qlang/dist/qlang.vsix
```

You can install it in VS Code with:

- `Extensions: Install from VSIX...`
- or `code --install-extension editors/vscode/qlang/dist/qlang.vsix`

## Settings

- `qlang.server.path`: explicit path to the `qlsp` executable
- `qlang.server.args`: extra arguments passed to `qlsp`

Changing either setting restarts the client.

## Current Scope

This extension intentionally stays thin:

- no bundled `qlsp` binary
- local VSIX packaging flow exists, but Marketplace publish flow is not added yet
- ships a minimal TextMate grammar fallback for base syntax coloring

The current editor surface follows whatever `qlsp` already exposes: diagnostics, hover, definition, declaration, type definition, references, document highlight, completion, document symbols, workspace symbols, semantic tokens, and conservative rename support.

That does not mean all of those surfaces are already project-grade.

- The most reliable path today is still diagnostics plus conservative same-file semantics.
- Workspace-root-driven symbol search is now wired up, and package/workspace imports prefer workspace source definitions when a unique source target exists.
- Current-document occurrence highlighting now also reuses the same-file and package-aware references surface through `textDocument/documentHighlight`.
- Workspace-scale navigation and highlighting are still incomplete beyond that conservative slice.
- Semantic highlighting quality still depends on `qlsp`; the fallback grammar only guarantees basic syntax coloring.
