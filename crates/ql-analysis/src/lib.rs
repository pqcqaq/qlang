use std::path::Path;

use ql_diagnostics::{Diagnostic, Label, render_diagnostics};
use ql_hir::{ExprId, LocalId, PatternId, lower_module};
use ql_parser::{ParseError, parse_source};
use ql_resolve::{ResolutionMap, resolve_module};
use ql_typeck::{Ty, TypeckResult, analyze_module as analyze_types};

/// Parsed-and-lowered semantic analysis snapshot shared by CLI and future LSP work.
#[derive(Clone, Debug)]
pub struct Analysis {
    ast: ql_ast::Module,
    hir: ql_hir::Module,
    resolution: ResolutionMap,
    typeck: TypeckResult,
    diagnostics: Vec<Diagnostic>,
}

impl Analysis {
    pub fn ast(&self) -> &ql_ast::Module {
        &self.ast
    }

    pub fn hir(&self) -> &ql_hir::Module {
        &self.hir
    }

    pub fn resolution(&self) -> &ResolutionMap {
        &self.resolution
    }

    pub fn typeck(&self) -> &TypeckResult {
        &self.typeck
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

    pub fn render_diagnostics(&self, path: &Path, source: &str) -> String {
        render_diagnostics(path, source, self.diagnostics())
    }
}

/// Analyze one source string. Parse failures are returned as diagnostics directly.
/// Resolution and type diagnostics are stored on the returned [`Analysis`] even when errors exist.
pub fn analyze_source(source: &str) -> Result<Analysis, Vec<Diagnostic>> {
    let ast = parse_source(source).map_err(parse_errors_to_diagnostics)?;
    let hir = lower_module(&ast);
    let resolution = resolve_module(&hir);
    let typeck = analyze_types(&hir, &resolution);
    let mut diagnostics = resolution.diagnostics.clone();
    diagnostics.extend(typeck.diagnostics().iter().cloned());

    Ok(Analysis {
        ast,
        hir,
        resolution,
        typeck,
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
}
