# qlang VS Code Extension

Repository-local VS Code thin client for Qlang.

It:

- registers `.ql` files as `qlang`
- starts `qlsp` over stdio
- warns when the extension and server versions do not match

Semantic behavior lives in `crates/ql-lsp` and its tests.

## Development

```powershell
cargo build -p ql-lsp

cd editors/vscode/qlang
npm install
npm run compile
npm run test:grammar
```

Open this directory in VS Code and run `Run qlang`.

Server lookup order:

1. `qlang.server.path`
2. `<repo>/target/debug/qlsp`
3. `<repo>/target/release/qlsp`
4. `qlsp` from `PATH`

## Local Install

There is no bundled server or Marketplace release yet. Build matching artifacts from the same checkout.

```powershell
cd editors/vscode/qlang
npm install
npm run package:vsix
code --install-extension dist/qlang-<package.json version>.vsix
```

Check the server:

```powershell
qlsp --version
```

## Settings

- `qlang.server.path`
- `qlang.server.args`

## Scope

- TextMate grammar mirrors the current lexer keyword set.
- `qlsp` provides hover, semantic tokens, completion, signature help, inlay hints, folding, selection, formatting, code actions, code lenses, hierarchy, references and rename.
- The reliable surface is still conservative and follows `docs/roadmap/current-supported-surface.md`.
