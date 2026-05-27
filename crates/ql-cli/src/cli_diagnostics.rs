use std::path::Path;

use ql_diagnostics::{Diagnostic, render_diagnostics};

use crate::cli_utils::normalize_path;

pub(crate) fn print_diagnostics(path: &Path, source: &str, diagnostics: &[Diagnostic]) {
    eprint!("{}", render_cli_diagnostics(path, source, diagnostics));
}

pub(crate) fn render_cli_diagnostics(
    path: &Path,
    source: &str,
    diagnostics: &[Diagnostic],
) -> String {
    let normalized_path = normalize_path(path);
    render_diagnostics(Path::new(&normalized_path), source, diagnostics)
}

#[cfg(test)]
#[path = "cli_diagnostics_tests.rs"]
mod tests;
