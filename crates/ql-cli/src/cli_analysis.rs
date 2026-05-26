use ql_analysis::{PackageAnalysisError, analyze_source as analyze_semantics};
use ql_diagnostics::Diagnostic;

use crate::cli_diagnostics::print_diagnostics;

pub(crate) fn analyze_source(source: &str) -> Result<(), Vec<Diagnostic>> {
    let analysis = analyze_semantics(source)?;
    if analysis.has_errors() {
        Err(analysis.diagnostics().to_vec())
    } else {
        Ok(())
    }
}

pub(crate) fn print_package_analysis_error(error: &PackageAnalysisError) {
    match error {
        PackageAnalysisError::Project(error) => eprintln!("error: {error}"),
        PackageAnalysisError::Read { path, error } => {
            eprintln!("error: failed to read `{}`: {error}", path.display());
        }
        PackageAnalysisError::SourceDiagnostics {
            path,
            source,
            diagnostics,
        } => print_diagnostics(path, source, diagnostics),
        PackageAnalysisError::InterfaceNotFound { package_name, path } => {
            eprintln!(
                "error: referenced package `{package_name}` is missing interface artifact `{}`",
                path.display()
            );
        }
        PackageAnalysisError::InterfaceParse { path, message } => {
            eprintln!("error: invalid interface `{}`: {message}", path.display());
        }
    }
}
