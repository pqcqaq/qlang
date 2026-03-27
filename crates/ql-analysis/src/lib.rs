mod query;

use std::path::Path;

use ql_borrowck::{
    BorrowckResult, analyze_module as analyze_borrowck, render_result as render_borrowck_result,
};
use ql_diagnostics::{Diagnostic, Label, render_diagnostics};
use ql_hir::{ExprId, LocalId, PatternId, lower_module};
use ql_mir::{MirModule, lower_module as lower_mir, render_module as render_mir_module};
use ql_parser::{ParseError, parse_source};
use ql_resolve::{ResolutionMap, resolve_module};
use ql_typeck::{Ty, TypeckResult, analyze_module as analyze_types};
use query::QueryIndex;
pub use query::{
    AsyncContextInfo, AsyncOperatorKind, CompletionItem, DefinitionTarget, HoverInfo,
    ReferenceTarget, RenameEdit, RenameError, RenameResult, RenameTarget, SemanticTokenOccurrence,
    SymbolKind,
};

/// Parsed-and-lowered semantic analysis snapshot shared by CLI and future LSP work.
#[derive(Clone, Debug)]
pub struct Analysis {
    ast: ql_ast::Module,
    hir: ql_hir::Module,
    mir: MirModule,
    resolution: ResolutionMap,
    typeck: TypeckResult,
    borrowck: BorrowckResult,
    index: QueryIndex,
    diagnostics: Vec<Diagnostic>,
}

impl Analysis {
    pub fn ast(&self) -> &ql_ast::Module {
        &self.ast
    }

    pub fn hir(&self) -> &ql_hir::Module {
        &self.hir
    }

    pub fn mir(&self) -> &MirModule {
        &self.mir
    }

    pub fn resolution(&self) -> &ResolutionMap {
        &self.resolution
    }

    pub fn typeck(&self) -> &TypeckResult {
        &self.typeck
    }

    pub fn borrowck(&self) -> &BorrowckResult {
        &self.borrowck
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn has_errors(&self) -> bool {
        !self.diagnostics().is_empty()
    }

    pub fn expr_ty(&self, expr_id: ExprId) -> Option<&Ty> {
        self.typeck.expr_ty(expr_id)
    }

    pub fn pattern_ty(&self, pattern_id: PatternId) -> Option<&Ty> {
        self.typeck.pattern_ty(pattern_id)
    }

    pub fn local_ty(&self, local_id: LocalId) -> Option<&Ty> {
        self.typeck.local_ty(local_id)
    }

    /// Return the smallest indexed semantic symbol that covers `offset`.
    ///
    /// This currently stays conservative on purpose:
    /// - expression queries cover root bindings plus struct fields, unique method members, and
    ///   enum variant tokens, but still do not model deeper member chains or module-path semantics
    /// - builtin types can hover but have no source definition span
    /// - import aliases are now source-backed within the current file, and local aliases that
    ///   point at local enum items can forward variant-token queries, but deeper module graphs
    ///   still are not resolved beyond the imported root binding
    pub fn symbol_at(&self, offset: usize) -> Option<HoverInfo> {
        self.index.symbol_at(offset)
    }

    /// Return hover-ready semantic data for the symbol covering `offset`.
    pub fn hover_at(&self, offset: usize) -> Option<HoverInfo> {
        self.symbol_at(offset)
    }

    /// Return async semantic context for `await` / `spawn` / `for await` at `offset`.
    pub fn async_context_at(&self, offset: usize) -> Option<AsyncContextInfo> {
        self.index.async_context_at(offset)
    }

    /// Return the definition site for the symbol covering `offset`, when the target lives in source.
    pub fn definition_at(&self, offset: usize) -> Option<DefinitionTarget> {
        self.index.definition_at(offset)
    }

    /// Return every indexed occurrence for the symbol covering `offset` within the current file.
    pub fn references_at(&self, offset: usize) -> Option<Vec<ReferenceTarget>> {
        self.index.references_at(offset)
    }

    /// Return rename metadata when the symbol under `offset` is safe for same-file renaming.
    pub fn prepare_rename_at(&self, offset: usize) -> Option<RenameTarget> {
        self.index.prepare_rename_at(offset)
    }

    /// Return same-file rename edits for the symbol covering `offset`.
    pub fn rename_at(
        &self,
        offset: usize,
        new_name: &str,
    ) -> Result<Option<RenameResult>, RenameError> {
        self.index.rename_at(offset, new_name)
    }

    /// Return visible completion candidates at `offset`.
    ///
    /// This currently stays conservative on purpose:
    /// - only same-file completion is supported
    /// - lexical-scope completion currently covers value and type positions already represented in HIR
    /// - member completion currently covers already-parsed member tokens whose receiver type is
    ///   stable, plus same-file parsed enum variant paths, including local import aliases that
    ///   point at local enum items
    /// - ambiguous member surfaces, parse-error tolerant completion, and cross-file project indexing
    ///   are still intentionally deferred
    pub fn completions_at(&self, offset: usize) -> Option<Vec<CompletionItem>> {
        self.index.completions_at(offset)
    }

    /// Return source-backed semantic-token occurrences for the current file.
    ///
    /// This intentionally reuses the same conservative query surface as hover/definition/references:
    /// only tokens with stable source-backed semantic identity are emitted.
    pub fn semantic_tokens(&self) -> Vec<SemanticTokenOccurrence> {
        self.index.semantic_tokens()
    }

    pub fn render_diagnostics(&self, path: &Path, source: &str) -> String {
        render_diagnostics(path, source, self.diagnostics())
    }

    pub fn render_mir(&self) -> String {
        render_mir_module(&self.mir, &self.hir)
    }

    pub fn render_borrowck(&self) -> String {
        render_borrowck_result(&self.borrowck, &self.mir)
    }
}

/// Analyze one source string. Parse failures are returned as diagnostics directly.
/// Resolution and type diagnostics are stored on the returned [`Analysis`] even when errors exist.
pub fn analyze_source(source: &str) -> Result<Analysis, Vec<Diagnostic>> {
    let ast = parse_source(source).map_err(parse_errors_to_diagnostics)?;
    let hir = lower_module(&ast);
    let resolution = resolve_module(&hir);
    let mir = lower_mir(&hir, &resolution);
    let typeck = analyze_types(&hir, &resolution);
    let borrowck = analyze_borrowck(&hir, &resolution, &typeck, &mir);
    let index = QueryIndex::build(source, &hir, &resolution, &typeck);
    let mut diagnostics = resolution.diagnostics.clone();
    diagnostics.extend(typeck.diagnostics().iter().cloned());
    diagnostics.extend(borrowck.diagnostics().iter().cloned());

    Ok(Analysis {
        ast,
        hir,
        mir,
        resolution,
        typeck,
        borrowck,
        index,
        diagnostics,
    })
}

pub fn parse_errors_to_diagnostics(errors: Vec<ParseError>) -> Vec<Diagnostic> {
    errors
        .into_iter()
        .map(|error| Diagnostic::error(error.message).with_label(Label::new(error.span)))
        .collect()
}

#[cfg(test)]
mod tests {
    use ql_hir::{ExprKind, ItemKind, StmtKind};

    use super::analyze_source;

    #[test]
    fn keeps_semantic_diagnostics_on_successful_analysis() {
        let analysis = analyze_source(
            r#"
fn main() -> Int {
    return "oops"
}
"#,
        )
        .expect("type errors should still yield an analysis snapshot");

        assert!(analysis.has_errors());
        assert!(analysis.diagnostics().iter().any(|diagnostic| {
            diagnostic.message == "return value has type mismatch: expected `Int`, found `String`"
        }));
    }

    #[test]
    fn exposes_expression_and_local_types_for_queries() {
        let analysis = analyze_source(
            r#"
fn main() -> Int {
    let value = 1
    return value
}
"#,
        )
        .expect("source should analyze");

        let function = analysis
            .hir()
            .items
            .iter()
            .find_map(|&item_id| match &analysis.hir().item(item_id).kind {
                ItemKind::Function(function) if function.name == "main" => Some(function),
                _ => None,
            })
            .expect("main function should exist");
        let body = function.body.expect("main should have a body");
        let block = analysis.hir().block(body);
        let stmt_id = block.statements[0];
        let local_id = match &analysis.hir().stmt(stmt_id).kind {
            StmtKind::Let { pattern, .. } => match &analysis.hir().pattern(*pattern).kind {
                ql_hir::PatternKind::Binding(local_id) => *local_id,
                _ => panic!("expected binding pattern"),
            },
            _ => panic!("expected let statement"),
        };
        let return_expr = match &analysis.hir().stmt(block.statements[1]).kind {
            StmtKind::Return(Some(expr_id)) => *expr_id,
            _ => panic!("expected return statement with expression"),
        };

        assert_eq!(
            analysis
                .local_ty(local_id)
                .map(ToString::to_string)
                .as_deref(),
            Some("Int")
        );
        assert_eq!(
            analysis
                .expr_ty(return_expr)
                .map(ToString::to_string)
                .as_deref(),
            Some("Int")
        );

        let value_expr = match &analysis.hir().stmt(stmt_id).kind {
            StmtKind::Let { value, .. } => *value,
            _ => unreachable!(),
        };
        assert!(matches!(
            analysis.hir().expr(value_expr).kind,
            ExprKind::Integer(_)
        ));
        assert_eq!(
            analysis
                .expr_ty(value_expr)
                .map(ToString::to_string)
                .as_deref(),
            Some("Int")
        );
    }

    #[test]
    fn exposes_array_and_index_types_for_queries() {
        let analysis = analyze_source(
            r#"
fn main() -> Int {
    let values = [1, 2, 3]
    let first = values[0]
    return first
}
"#,
        )
        .expect("source should analyze");

        let function = analysis
            .hir()
            .items
            .iter()
            .find_map(|&item_id| match &analysis.hir().item(item_id).kind {
                ItemKind::Function(function) if function.name == "main" => Some(function),
                _ => None,
            })
            .expect("main function should exist");
        let body = function.body.expect("main should have a body");
        let block = analysis.hir().block(body);

        let values_local = match &analysis.hir().stmt(block.statements[0]).kind {
            StmtKind::Let { pattern, .. } => match &analysis.hir().pattern(*pattern).kind {
                ql_hir::PatternKind::Binding(local_id) => *local_id,
                _ => panic!("expected binding pattern"),
            },
            _ => panic!("expected let statement"),
        };
        let (first_local, first_value_expr) = match &analysis.hir().stmt(block.statements[1]).kind {
            StmtKind::Let { pattern, value, .. } => {
                let local_id = match &analysis.hir().pattern(*pattern).kind {
                    ql_hir::PatternKind::Binding(local_id) => *local_id,
                    _ => panic!("expected binding pattern"),
                };
                (local_id, *value)
            }
            _ => panic!("expected second let statement"),
        };
        let return_expr = match &analysis.hir().stmt(block.statements[2]).kind {
            StmtKind::Return(Some(expr_id)) => *expr_id,
            _ => panic!("expected return statement with expression"),
        };

        assert_eq!(
            analysis
                .local_ty(values_local)
                .map(ToString::to_string)
                .as_deref(),
            Some("[Int; 3]")
        );
        assert_eq!(
            analysis
                .expr_ty(first_value_expr)
                .map(ToString::to_string)
                .as_deref(),
            Some("Int")
        );
        assert_eq!(
            analysis
                .local_ty(first_local)
                .map(ToString::to_string)
                .as_deref(),
            Some("Int")
        );
        assert_eq!(
            analysis
                .expr_ty(return_expr)
                .map(ToString::to_string)
                .as_deref(),
            Some("Int")
        );
    }

    #[test]
    fn exposes_tuple_hex_index_types_for_queries() {
        let analysis = analyze_source(
            r#"
fn main() -> Int {
    let pair = (1, "ql")
    let second = pair[0x1]
    return 0
}
"#,
        )
        .expect("source should analyze");

        let function = analysis
            .hir()
            .items
            .iter()
            .find_map(|&item_id| match &analysis.hir().item(item_id).kind {
                ItemKind::Function(function) if function.name == "main" => Some(function),
                _ => None,
            })
            .expect("main function should exist");
        let body = function.body.expect("main should have a body");
        let block = analysis.hir().block(body);

        let (second_local, second_value_expr) = match &analysis.hir().stmt(block.statements[1]).kind
        {
            StmtKind::Let { pattern, value, .. } => {
                let local_id = match &analysis.hir().pattern(*pattern).kind {
                    ql_hir::PatternKind::Binding(local_id) => *local_id,
                    _ => panic!("expected binding pattern"),
                };
                (local_id, *value)
            }
            _ => panic!("expected second let statement"),
        };

        assert_eq!(
            analysis
                .expr_ty(second_value_expr)
                .map(ToString::to_string)
                .as_deref(),
            Some("String")
        );
        assert_eq!(
            analysis
                .local_ty(second_local)
                .map(ToString::to_string)
                .as_deref(),
            Some("String")
        );
    }

    #[test]
    fn keeps_resolution_diagnostics_in_the_combined_output() {
        let analysis = analyze_source(
            r#"
fn main() -> Int {
    self
}
"#,
        )
        .expect("resolution errors should still yield an analysis snapshot");

        assert!(analysis.diagnostics().iter().any(|diagnostic| {
            diagnostic.message == "invalid use of `self` outside a method receiver scope"
        }));
    }

    #[test]
    fn exposes_rendered_mir_for_debugging() {
        let analysis = analyze_source(
            r#"
fn main() -> Int {
    let value = 1
    return value
}
"#,
        )
        .expect("source should analyze");

        let rendered = analysis.render_mir();
        assert!(rendered.contains("body 0 main"));
        assert!(rendered.contains("bind_pattern value <-"));
        assert!(rendered.contains("return"));
    }

    #[test]
    fn rendered_mir_exposes_explicit_closure_capture_facts() {
        let analysis = analyze_source(
            r#"
fn main() -> Int {
    let value = 1
    let make = move (extra) => value + extra
    return 0
}
"#,
        )
        .expect("source should analyze");

        let rendered = analysis.render_mir();
        assert!(rendered.contains("[captures: value@"));
    }

    #[test]
    fn exposes_rendered_ownership_for_debugging() {
        let analysis = analyze_source(
            r#"
struct User {
    name: String,
}

impl User {
    fn into_json(move self) -> String {
        return self.name
    }
}

fn main() -> String {
    let user = User { name: "ql" }
    user.into_json();
    return user.name
}
"#,
        )
        .expect("borrowck diagnostics should still yield an analysis snapshot");

        let rendered = analysis.render_borrowck();
        assert!(rendered.contains("ownership main"));
        assert!(rendered.contains("consume(move self into_json)"));
    }

    #[test]
    fn rendered_ownership_exposes_closure_escape_facts() {
        let analysis = analyze_source(
            r#"
fn apply(f: () -> Int) -> Int {
    return f()
}

fn main() -> Int {
    let value = 1
    let closure = move () => value
    return apply(closure)
}
"#,
        )
        .expect("source should analyze");

        let rendered = analysis.render_borrowck();
        assert!(rendered.contains("closures:"));
        assert!(rendered.contains("call-arg@"));
    }

    #[test]
    fn includes_borrowck_diagnostics_in_the_combined_output() {
        let analysis = analyze_source(
            r#"
struct User {
    name: String,
}

impl User {
    fn into_json(move self) -> String {
        return self.name
    }
}

fn main() -> String {
    let user = User { name: "ql" }
    user.into_json()
    return user.name
}
"#,
        )
        .expect("borrowck diagnostics should still yield an analysis snapshot");

        assert!(
            analysis
                .diagnostics()
                .iter()
                .any(|diagnostic| { diagnostic.message == "local `user` was used after move" })
        );
    }

    #[test]
    fn includes_deferred_cleanup_borrowck_diagnostics_in_the_combined_output() {
        let analysis = analyze_source(
            r#"
struct User {
    name: String,
}

impl User {
    fn into_json(move self) -> String {
        return self.name
    }
}

fn main() -> String {
    let user = User { name: "ql" }
    defer user.name
    defer user.into_json()
    return ""
}
"#,
        )
        .expect("deferred cleanup diagnostics should still yield an analysis snapshot");

        assert!(
            analysis
                .diagnostics()
                .iter()
                .any(|diagnostic| { diagnostic.message == "local `user` was used after move" })
        );
    }

    #[test]
    fn includes_move_closure_capture_borrowck_diagnostics_in_the_combined_output() {
        let analysis = analyze_source(
            r#"
fn main() -> Int {
    let value = 1
    let capture = move () => value
    return value
}
"#,
        )
        .expect("move closure diagnostics should still yield an analysis snapshot");

        assert!(
            analysis
                .diagnostics()
                .iter()
                .any(|diagnostic| { diagnostic.message == "local `value` was used after move" })
        );
        assert!(
            analysis
                .render_borrowck()
                .contains("consume(move closure capture)")
        );
    }
}
