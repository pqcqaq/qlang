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
mod tests {
    use std::path::PathBuf;

    use ql_diagnostics::{Diagnostic, Label};
    use ql_span::Span;

    use super::*;

    #[test]
    fn render_cli_diagnostics_normalizes_display_path() {
        let diagnostic = Diagnostic::error("bad binding")
            .with_label(Label::new(Span::new(4, 5)).with_message("here"));
        let rendered = render_cli_diagnostics(
            &PathBuf::from("workspace/./src/../src/main.ql"),
            "let value = 1\n",
            &[diagnostic],
        );

        assert!(rendered.contains("workspace/src/main.ql:1:5: bad binding"));
        assert!(rendered.contains("^ here"));
    }
}
