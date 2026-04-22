# qlang VS Code Extension

This extension is the repository-local VS Code client for Qlang.

It stays intentionally thin:

- registers the `qlang` language for `.ql` files
- starts the existing `qlsp` language server over stdio
- warns when the extension version and `qlsp` server version do not match

The semantic contract still comes from [`crates/ql-lsp`](../../../crates/ql-lsp) and the repo tests.

## Repository Development Mode

Build the language server first:

```powershell
cargo build -p ql-lsp
```

Then build the extension:

```powershell
cd editors/vscode/qlang
npm install
npm run compile
```

Open `editors/vscode/qlang` in VS Code and run the `Run qlang` launch configuration.

By default the extension tries these server locations in order:

1. `qlang.server.path`
2. `<repo>/target/debug/qlsp`
3. `<repo>/target/release/qlsp`
4. `qlsp` from `PATH`

## Installed Usage Mode

There is no prebuilt release flow or Marketplace publish flow yet. Installed usage still means building matching artifacts from the same source checkout.

Package a VSIX from the extension directory:

```powershell
cd editors/vscode/qlang
npm install
npm run package:vsix
```

The package is written to:

```text
editors/vscode/qlang/dist/qlang-<package.json version>.vsix
```

Install it with:

- `Extensions: Install from VSIX...`
- or `code --install-extension editors/vscode/qlang/dist/qlang-<package.json version>.vsix`

## Version Matching

Use matching `ql` / `qlsp` / VSIX artifacts from the same checkout.

Check the server version directly:

```powershell
qlsp --version
```

At startup the extension reads LSP `serverInfo.version`.

- matching versions continue normally
- mismatched versions trigger a warning and point you to the README or `qlang.server.path`

## Settings

- `qlang.server.path`: explicit path to the `qlsp` executable
- `qlang.server.args`: extra arguments passed to `qlsp`

Changing either setting restarts the client.

## Current Scope

- no bundled `qlsp` binary
- no Marketplace publish flow
- ships a minimal TextMate grammar fallback for base syntax coloring

The reliable editor surface is still conservative: diagnostics, same-file semantics, and the source-backed workspace/dependency slices already covered by `qlsp`.
