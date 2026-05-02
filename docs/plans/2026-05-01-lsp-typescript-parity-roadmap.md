# LSP TypeScript Parity Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Bring Qlang editor experience close to the TypeScript language server baseline by fixing visible syntax UX first, then expanding backend LSP capability in small verified slices.

**Status 2026-05-02:** The first rich-editor slices are implemented: VS Code grammar now mirrors the lexer keyword set, `qlsp` declares and serves keyword hover, keyword/snippet completion, `completionItem/resolve` documentation/detail enrichment, semantic tokens full/range with lexical tokens and modifiers, signature help, local type inlay hints, folding ranges, selection ranges, range formatting, on-type formatting, `codeAction/resolve`, and `source.organizeImports`. Remaining TypeScript-parity work is still code lens, richer source actions, call/type hierarchy, file-operation hooks, and a deeper AST/type-driven implementation of signature/inlay/folding/selection.

**Architecture:** Keep `ql-analysis` as the semantic truth source and keep `ql-lsp` as the protocol bridge. VS Code lexical coloring stays in the extension TextMate grammar; semantic coloring, hover, navigation, diagnostics, actions, and workspace intelligence stay behind `qlsp`.

**Tech Stack:** Rust `tower-lsp`, `ql-lexer`, `ql-analysis`, `ql-package`, VS Code `vscode-languageclient`, TextMate grammar JSON, TypeScript reference from `D:\Projects\_reference\typescript-language-server`.

---

## Reference Baseline

The TypeScript reference is `D:\Projects\_reference\typescript-language-server`. It is an LSP wrapper around `tsserver`, not the compiler itself. Its important capability shape is:

- Incremental document sync.
- Completion with triggers `.`, `"`, `'`, `/`, `@`, `<`, plus `completionItem/resolve`.
- Code actions with advertised kinds, resolve support, organize imports, remove unused imports, add missing imports, fix all, and refactor commands.
- Definition, implementation, type definition, references, hover, document highlight, document symbol, workspace symbol.
- Formatting, range formatting, folding range, selection range, signature help, inlay hints, code lens.
- Semantic tokens full and range with semantic symbol token types plus modifiers.
- Workspace file operation hooks for rename.

Important correction: TypeScript keyword coloring is not mainly produced by LSP semantic tokens. VS Code gets keyword/operator/string/comment coloring from grammar/tokenization. Qlang must therefore improve both the VS Code grammar and `qlsp`.

## Original Qlang Gap

Current `qlsp` already declares diagnostics through publish, hover, definition, declaration, typeDefinition, implementation, references, documentHighlight, documentSymbol, workspaceSymbol, completion, codeAction with resolve, document/range/on-type formatting, semanticTokens full/range, prepareRename, and rename.

The visible UX gap that motivated this plan was:

- `editors/vscode/qlang/syntaxes/qlang.tmLanguage.json` misses lexer keywords such as `package`, `loop`, `where`, `is`, `as`, `satisfies`, and `none`.
- `crates/ql-lsp/src/bridge.rs` semantic token legend only has symbol categories. It has no token modifiers and no range support.
- `crates/ql-lsp/src/backend.rs` has no keyword hover path. Hover only works for semantic symbols and dependency-backed symbols.
- `qlsp` does not yet declare or implement code lens, richer source actions, workspace execute commands, file-operation hooks, call hierarchy, or type hierarchy.
- Diagnostics are current-document first with package preflight fallback; there is no workspace diagnostics pipeline.
- Code actions are useful but narrow: unresolved-symbol auto import, missing dependency quick fixes, idempotent resolve, and top-level organize imports only.

## Implementation Order

### Task 1: Fix Visible Syntax Baseline

**Files:**
- Modify: `editors/vscode/qlang/syntaxes/qlang.tmLanguage.json`
- Modify: `editors/vscode/qlang/README.md`
- Modify: `docs/getting-started/vscode-extension.md`
- Test: add grammar smoke coverage under `editors/vscode/qlang`

**Steps:**

1. Add the full lexer keyword set from `crates/ql-lexer/src/lib.rs` to the TextMate grammar.
2. Split keyword scopes into control, declaration, storage type, modifier, operator-like keywords, constants, and receiver keywords.
3. Add tests that load representative `.ql` snippets and assert each keyword is classified by the intended scope.
4. Run `npm run compile` in `editors/vscode/qlang`.
5. Commit as `fix: complete vscode keyword grammar`.

### Task 2: Add Keyword Hover

**Files:**
- Modify: `crates/ql-lsp/src/backend.rs`
- Create or modify: `crates/ql-lsp/src/keyword_docs.rs`
- Test: `crates/ql-lsp/tests/hover_keywords.rs`
- Docs: `docs/getting-started/vscode-extension.md`

**Steps:**

1. Write request tests for hover on `fn`, `let`, `match`, `where`, `satisfies`, `none`, `self`, `async`, and `await`.
2. Implement a lexer-based keyword lookup before semantic hover fallback.
3. Return markdown with category, short meaning, minimal syntax example, and links or names for related keywords.
4. Ensure escaped identifiers such as `` `type` `` do not return keyword hover.
5. Run `cargo test -p ql-lsp --test hover_keywords`.
6. Commit as `feat: add qlsp keyword hover`.

### Task 3: Expand Semantic Token Protocol

**Files:**
- Modify: `crates/ql-analysis/src/query.rs`
- Modify: `crates/ql-lsp/src/bridge.rs`
- Modify: `crates/ql-lsp/src/backend.rs`
- Test: `crates/ql-lsp/tests/semantic_tokens_keywords.rs`
- Test: existing semantic token bridge/request tests

**Steps:**

1. Add token modifiers for declaration, static, readonly, async, unsafe, local, and defaultLibrary where Qlang can infer them.
2. Add lexical token occurrences for keyword, operator, string, number, comment, boolean, and builtin constants only where VS Code semantic token themes benefit.
3. Keep semantic symbol tokens dominant over lexical tokens for overlapping spans.
4. Implement `textDocument/semanticTokens/range`.
5. Run focused semantic token tests and `cargo test -p ql-lsp --test request_smoke`.
6. Commit as `feat: broaden qlsp semantic tokens`.

### Task 4: Completion Parity Layer

**Files:**
- Modify: `crates/ql-analysis/src/query.rs`
- Modify: `crates/ql-lsp/src/bridge.rs`
- Modify: `crates/ql-lsp/src/backend.rs`
- Test: `crates/ql-lsp/tests/completion_documentation.rs`
- Test: new `crates/ql-lsp/tests/completion_resolve_request.rs`

**Steps:**

1. Add keyword and snippet completion for declarations, control flow, match arms, impl/extend blocks, imports, and common stdlib roots.
2. Add trigger characters for `:`, `"`, `/`, `@`, and package path positions only where the parser can disambiguate them.
3. Add `completionItem/resolve` for expensive docs, snippets, and import edits.
4. Add label details and insert/replace ranges where supported by clients.
5. Run completion request tests.
6. Commit as `feat: add rich qlsp completion`.

### Task 5: Signature Help

**Files:**
- Modify: `crates/ql-analysis/src/query.rs`
- Modify: `crates/ql-lsp/src/backend.rs`
- Test: `crates/ql-lsp/tests/signature_help_request.rs`

**Steps:**

1. Add analysis query for callable at argument position.
2. Support free functions, extern functions, methods, constructors, enum variants, and dependency-backed calls.
3. Track active parameter across commas and nested calls.
4. Declare trigger characters `(`, `,`, `<` and retrigger `)`.
5. Run `cargo test -p ql-lsp --test signature_help_request`.
6. Commit as `feat: add qlsp signature help`.

### Task 6: Inlay Hints

**Files:**
- Modify: `crates/ql-analysis/src/query.rs`
- Modify: `crates/ql-lsp/src/backend.rs`
- Test: `crates/ql-lsp/tests/inlay_hints_request.rs`
- Docs: `docs/getting-started/vscode-extension.md`

**Steps:**

1. Add parameter-name hints for call arguments.
2. Add inferred local type hints for unannotated `let`.
3. Add closure parameter and function return hints where inference is stable.
4. Add VS Code settings to enable or disable hint categories.
5. Run focused inlay hint tests.
6. Commit as `feat: add qlsp inlay hints`.

### Task 7: Folding, Selection Range, and Document UX

**Files:**
- Modify: `crates/ql-analysis/src/query.rs`
- Modify: `crates/ql-lsp/src/backend.rs`
- Test: `crates/ql-lsp/tests/folding_selection_request.rs`

**Steps:**

1. Add AST-backed folding ranges for blocks, imports, comments, match arms, impl/extend/trait bodies, struct/enum bodies, and extern blocks.
2. Add selection ranges from token to expression to statement to item to file.
3. Preserve parse-error tolerant lexical fallback where possible.
4. Run focused request tests.
5. Commit as `feat: add folding and selection ranges`.

### Task 8: Formatting Parity

**Files:**
- Modify: `crates/ql-lsp/src/backend.rs`
- Modify: formatter crate only if necessary
- Test: `crates/ql-lsp/tests/formatting_request.rs`

**Steps:**

1. Done: add safe range formatting by reusing `ql fmt` and returning per-line minimal edits only when each edit stays inside the requested range.
2. Done: add on-type formatting for newline, `}`, `;`, and `,` by returning per-line minimal edits only on the trigger line.
3. Done: unsupported partial changes that cannot be safely split by line return empty edits instead of silently changing unrelated text.
4. Done: `cargo test -p ql-lsp --test formatting_request --test initialize_capabilities --test request_smoke`.
5. Commit as `feat: expand qlsp formatting requests`.

### Task 9: Workspace Diagnostics and Actions

**Files:**
- Modify: `crates/ql-lsp/src/backend.rs`
- Modify: `crates/ql-lsp/src/store.rs`
- Modify: package/workspace analysis crates as needed
- Test: `crates/ql-lsp/tests/diagnostics_lifecycle_request.rs`
- Test: `crates/ql-lsp/tests/code_action_request.rs`

**Steps:**

1. Add a workspace document graph that tracks open files, manifests, package roots, local dependencies, and interface freshness.
2. Publish diagnostics for open workspace files after debounced changes.
3. Add diagnostics for stale `.qi`, missing package exports, duplicate package names, broken local dependency paths, and import resolution failures.
4. Partly done: add `source.organizeImports` for sorting/deduplicating consecutive top-level `use ...` blocks; remove unused imports, create missing package section, and regenerate interface remain open.
5. Done: add `codeAction/resolve`, advertised quickfix/source action kinds, and `context.only` filtering.
6. Done for this slice: `cargo test -p ql-lsp --test code_action_request --test initialize_capabilities --test request_smoke`.
7. Commit as `feat: add workspace diagnostics and source actions`.

### Task 10: Navigation and Refactor Parity

**Files:**
- Modify: `crates/ql-analysis/src/query.rs`
- Modify: `crates/ql-lsp/src/backend.rs`
- Modify: `crates/ql-lsp/src/store.rs`
- Test: `crates/ql-lsp/tests/references_document_highlight_request.rs`
- Test: `crates/ql-lsp/tests/rename_request.rs`
- Test: new hierarchy and code lens tests

**Steps:**

1. Add a real workspace symbol/reference index instead of ad hoc source-backed scans.
2. Expand workspace rename to every source-backed symbol that has a unique package identity.
3. Add code lens for references and implementations.
4. Add call hierarchy for functions and methods.
5. Add type hierarchy for structs, enums, traits, impls, extends, and type aliases where meaningful.
6. Add file rename support for package/source path changes.
7. Run rename, reference, implementation, and hierarchy tests.
8. Commit as `feat: add workspace refactor parity`.

## Verification Gate

Every implementation slice must pass the smallest direct tests plus the request smoke test:

```powershell
cargo test -p ql-lsp --test request_smoke
```

For VS Code extension changes:

```powershell
cd editors/vscode/qlang
npm run compile
```

Do not run broad formatting across the repository unless the touched crates require it; broad `cargo fmt -p ql-lsp` has previously been noisy on Windows path handling.

## Non-Negotiable Boundaries

- Do not duplicate compiler semantics in `ql-lsp`; add query APIs to `ql-analysis` first.
- Do not claim TypeScript parity from capability flags alone; every declared capability needs request-level tests.
- Do not depend on generated `.qi` where open source documents can provide fresher truth.
- Keep docs synchronized with user-visible capability changes.
