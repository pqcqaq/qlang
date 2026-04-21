use std::path::Path;

use ql_span::{Span, locate, slice_for_line};

pub const UNRESOLVED_VALUE_CODE: &str = "unresolved-value";
pub const UNRESOLVED_TYPE_CODE: &str = "unresolved-type";

/// Severity level attached to a rendered compiler diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Note,
}

impl DiagnosticSeverity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Note => "note",
        }
    }
}

/// A span highlight attached to a diagnostic message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub span: Span,
    pub message: Option<String>,
    pub is_primary: bool,
}

impl Label {
    pub const fn new(span: Span) -> Self {
        Self {
            span,
            message: None,
            is_primary: true,
        }
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub fn secondary(mut self) -> Self {
        self.is_primary = false;
        self
    }
}

/// Structured compiler diagnostic with optional labels and notes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub code: Option<&'static str>,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            code: None,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Warning,
            code: None,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
        }
    }

    pub fn note(message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Note,
            code: None,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
        }
    }

    pub fn with_code(mut self, code: &'static str) -> Self {
        self.code = Some(code);
        self
    }

    pub fn with_label(mut self, label: Label) -> Self {
        self.labels.push(label);
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}

/// Render diagnostics into a stable text format for CLI output and tests.
pub fn render_diagnostics(path: &Path, source: &str, diagnostics: &[Diagnostic]) -> String {
    let mut output = String::new();

    for diagnostic in diagnostics {
        render_diagnostic(&mut output, path, source, diagnostic);
    }

    output
}

fn render_diagnostic(output: &mut String, path: &Path, source: &str, diagnostic: &Diagnostic) {
    let header_span = diagnostic
        .labels
        .first()
        .map(|label| {
            diagnostic
                .labels
                .iter()
                .find(|candidate| candidate.is_primary)
                .unwrap_or(label)
                .span
        })
        .unwrap_or_default();
    let header_location = locate(source, header_span);

    output.push_str(&format!(
        "{}: {}:{}:{}: {}\n",
        diagnostic.severity.as_str(),
        path.display(),
        header_location.start.line,
        header_location.start.column,
        diagnostic.message
    ));

    if !diagnostic.labels.is_empty() {
        for label in &diagnostic.labels {
            let location = locate(source, label.span);
            if let Some(line) = slice_for_line(source, location.start.line) {
                output.push_str(&format!("  {line}\n"));

                let indent = " ".repeat(location.start.column.saturating_sub(1));
                let marker = "^".repeat(label.span.len().clamp(1, 8));
                output.push_str("  ");
                output.push_str(&indent);
                output.push_str(&marker);
                if let Some(message) = &label.message {
                    output.push(' ');
                    output.push_str(message);
                }
                output.push('\n');
            }
        }
    }

    for note in &diagnostic.notes {
        output.push_str(&format!("  note: {note}\n"));
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use ql_span::Span;

    use super::{Diagnostic, Label, render_diagnostics};

    #[test]
    fn render_diagnostics_marks_primary_span_and_note() {
        let diagnostic = Diagnostic::error("duplicate binding")
            .with_label(Label::new(Span::new(4, 5)).with_message("duplicate here"))
            .with_note("rename one side of the pattern");

        let rendered =
            render_diagnostics(Path::new("sample.ql"), "let a = value;\n", &[diagnostic]);

        assert!(rendered.contains("error: sample.ql:1:5: duplicate binding"));
        assert!(rendered.contains("^ duplicate here"));
        assert!(rendered.contains("note: rename one side of the pattern"));
    }

    #[test]
    fn render_diagnostics_preserves_notes_without_labels() {
        let diagnostic = Diagnostic::warning("semantic warning").with_note("details survive");
        let rendered = render_diagnostics(Path::new("sample.ql"), "fn main() {}\n", &[diagnostic]);

        assert!(rendered.contains("warning: sample.ql:1:1: semantic warning"));
        assert!(rendered.contains("note: details survive"));
    }

    #[test]
    fn render_diagnostics_uses_primary_label_for_header_location() {
        let diagnostic = Diagnostic::error("duplicate item")
            .with_label(
                Label::new(Span::new(0, 2))
                    .secondary()
                    .with_message("first seen here"),
            )
            .with_label(Label::new(Span::new(10, 12)).with_message("duplicate here"));
        let rendered =
            render_diagnostics(Path::new("sample.ql"), "aa\nbbbbbbbb\ncc\n", &[diagnostic]);

        assert!(rendered.contains("error: sample.ql:2:8: duplicate item"));
    }
}
