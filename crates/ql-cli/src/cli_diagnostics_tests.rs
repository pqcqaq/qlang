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
