use std::fs;
use std::path::Path;

use ql_analysis::{Analysis, PackageAnalysisError, analyze_package, analyze_source};
use ql_diagnostics::Diagnostic as CompilerDiagnostic;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range, Url};

use crate::bridge::diagnostics_to_lsp;

use super::canonicalize_or_clone;

struct DiagnosticsRequestContext<'a> {
    uri: &'a Url,
    source: &'a str,
    analysis: std::result::Result<Analysis, Vec<CompilerDiagnostic>>,
    package: Option<std::result::Result<ql_analysis::PackageAnalysis, PackageAnalysisError>>,
    source_matches_disk: bool,
}

pub(super) fn document_diagnostics(uri: &Url, source: &str) -> Vec<Diagnostic> {
    let context = diagnostics_request_context(uri, source);
    match &context.analysis {
        Ok(analysis) if !analysis.diagnostics().is_empty() => {
            diagnostics_to_lsp(context.uri, context.source, analysis.diagnostics())
        }
        Ok(_) => package_diagnostics_for_document(&context).unwrap_or_default(),
        Err(diagnostics) => diagnostics_to_lsp(context.uri, context.source, diagnostics),
    }
}

fn diagnostics_request_context<'a>(uri: &'a Url, source: &'a str) -> DiagnosticsRequestContext<'a> {
    let analysis = analyze_source(source);
    let should_run_package_preflight =
        matches!(&analysis, Ok(analysis) if analysis.diagnostics().is_empty());
    let package = should_run_package_preflight
        .then(|| {
            uri.to_file_path().ok().map(|path| {
                let source_matches_disk = source_matches_disk_source(&path, source);
                (Some(analyze_package(&path)), source_matches_disk)
            })
        })
        .flatten();
    let (package, source_matches_disk) = package.unwrap_or((None, false));

    DiagnosticsRequestContext {
        uri,
        source,
        analysis,
        package,
        source_matches_disk,
    }
}

fn package_diagnostics_for_document(
    context: &DiagnosticsRequestContext<'_>,
) -> Option<Vec<Diagnostic>> {
    let path = context.uri.to_file_path().ok()?;
    match context.package.as_ref()? {
        Ok(_) => None,
        Err(PackageAnalysisError::SourceDiagnostics {
            path: diagnostic_path,
            source: diagnostic_source,
            diagnostics,
        }) => {
            let current_path = canonicalize_or_clone(&path);
            let diagnostic_path = canonicalize_or_clone(diagnostic_path);
            if current_path != diagnostic_path || !context.source_matches_disk {
                return None;
            }
            Some(diagnostics_to_lsp(
                context.uri,
                diagnostic_source,
                diagnostics,
            ))
        }
        Err(PackageAnalysisError::Project(ql_project::ProjectError::ManifestNotFound {
            ..
        })) => None,
        Err(PackageAnalysisError::Project(error)) => Some(vec![package_lsp_diagnostic(
            "package-project-error",
            format!("package analysis failed: {error}"),
        )]),
        Err(PackageAnalysisError::Read { path, error }) => Some(vec![package_lsp_diagnostic(
            "package-read-error",
            format!("failed to read `{}`: {error}", path.display()),
        )]),
        Err(PackageAnalysisError::InterfaceNotFound { package_name, path }) => {
            Some(vec![package_lsp_diagnostic(
                "package-interface-not-found",
                format!(
                    "referenced package `{package_name}` is missing interface artifact `{}`",
                    path.display()
                ),
            )])
        }
        Err(PackageAnalysisError::InterfaceParse { path, message }) => {
            Some(vec![package_lsp_diagnostic(
                "package-interface-parse-error",
                format!("invalid interface `{}`: {message}", path.display()),
            )])
        }
    }
}

fn source_matches_disk_source(path: &Path, source: &str) -> bool {
    fs::read_to_string(path)
        .map(|disk_source| normalize_line_endings(&disk_source) == normalize_line_endings(source))
        .unwrap_or(false)
}

fn normalize_line_endings(source: &str) -> String {
    source.replace("\r\n", "\n")
}

fn package_lsp_diagnostic(code: &str, message: String) -> Diagnostic {
    Diagnostic {
        range: Range::new(Position::new(0, 0), Position::new(0, 0)),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(code.to_owned())),
        code_description: None,
        source: Some("qlang".to_owned()),
        message,
        related_information: None,
        tags: None,
        data: None,
    }
}
