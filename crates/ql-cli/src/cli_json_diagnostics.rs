use ql_diagnostics::{Diagnostic, Label};
use ql_span::locate;
use serde_json::{Value as JsonValue, json};

pub(crate) fn diagnostics_json(source: &str, diagnostics: &[Diagnostic]) -> Vec<JsonValue> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic_json(source, diagnostic))
        .collect()
}

fn diagnostic_json(source: &str, diagnostic: &Diagnostic) -> JsonValue {
    json!({
        "severity": diagnostic.severity.as_str(),
        "message": diagnostic.message,
        "labels": diagnostic
            .labels
            .iter()
            .map(|label| label_json(source, label))
            .collect::<Vec<_>>(),
        "notes": diagnostic.notes,
    })
}

fn label_json(source: &str, label: &Label) -> JsonValue {
    let location = locate(source, label.span);
    json!({
        "is_primary": label.is_primary,
        "message": label.message,
        "span": {
            "start_offset": label.span.start,
            "end_offset": label.span.end,
            "start": {
                "line": location.start.line,
                "column": location.start.column,
            },
            "end": {
                "line": location.end.line,
                "column": location.end.column,
            },
        },
    })
}
