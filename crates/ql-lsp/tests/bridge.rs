use ql_analysis::analyze_source;
use ql_diagnostics::{Diagnostic as CompilerDiagnostic, Label};
use ql_lsp::bridge::{
    definition_for_analysis, diagnostics_to_lsp, hover_for_analysis, position_to_offset,
    references_for_analysis, span_to_range,
};
use ql_span::Span;
use tower_lsp::lsp_types::{GotoDefinitionResponse, HoverContents, Location, Position, Url};

fn nth_span(source: &str, needle: &str, occurrence: usize) -> Span {
    source
        .match_indices(needle)
        .nth(occurrence.saturating_sub(1))
        .map(|(start, matched)| Span::new(start, start + matched.len()))
        .expect("needle occurrence should exist")
}

#[test]
fn position_to_offset_handles_utf16_columns() {
    let source = "😀value\n";

    assert_eq!(position_to_offset(source, Position::new(0, 0)), Some(0));
    assert_eq!(
        position_to_offset(source, Position::new(0, 2)),
        Some("😀".len())
    );
    assert_eq!(
        position_to_offset(source, Position::new(0, 7)),
        Some("😀value".len())
    );
}

#[test]
fn diagnostics_conversion_uses_primary_span_and_related_information() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = "let left = right\n";
    let diagnostics = diagnostics_to_lsp(
        &uri,
        source,
        &[CompilerDiagnostic::error("duplicate binding")
            .with_label(
                Label::new(Span::new(4, 8))
                    .secondary()
                    .with_message("first declared here"),
            )
            .with_label(Label::new(Span::new(11, 16)).with_message("duplicate here"))
            .with_note("rename one side")],
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].range,
        span_to_range(source, Span::new(11, 16))
    );
    assert!(diagnostics[0].message.contains("rename one side"));
    assert_eq!(
        diagnostics[0]
            .related_information
            .as_ref()
            .expect("secondary labels should be preserved")[0]
            .location
            .uri,
        uri
    );
}

#[test]
fn hover_bridge_renders_markdown_for_semantic_symbols() {
    let source = r#"
fn id[T](value: T) -> T {
    value
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let hover =
        hover_for_analysis(source, &analysis, Position::new(2, 4)).expect("hover should exist");

    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown content");
    };

    assert!(markup.value.contains("**parameter** `value`"));
    assert!(markup.value.contains("param value: T"));
    assert!(markup.value.contains("Type: `T`"));
}

#[test]
fn hover_bridge_renders_markdown_for_member_symbols() {
    let source = r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn get(self) -> Int {
        return self.value
    }
    }
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let field_position = span_to_range(source, nth_span(source, "value", 2)).start;
    let hover =
        hover_for_analysis(source, &analysis, field_position).expect("field hover should exist");

    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown content");
    };

    assert!(markup.value.contains("**field** `value`"));
    assert!(markup.value.contains("field value: Int"));
    assert!(markup.value.contains("Type: `Int`"));
}

#[test]
fn definition_bridge_returns_same_file_locations() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
fn id[T](value: T) -> T {
    value
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let definition = definition_for_analysis(&uri, source, &analysis, Position::new(2, 4))
        .expect("definition should exist");

    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };

    assert_eq!(location.uri, uri);
    assert_eq!(
        location.range,
        span_to_range(source, nth_span(source, "value", 1))
    );
}

#[test]
fn references_bridge_respects_include_declaration() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
fn id[T](value: T) -> T {
    value
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");

    let with_declaration =
        references_for_analysis(&uri, source, &analysis, Position::new(2, 4), true)
            .expect("references should exist");
    let without_declaration =
        references_for_analysis(&uri, source, &analysis, Position::new(2, 4), false)
            .expect("references should exist");

    assert_eq!(
        with_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "value", 1))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "value", 2))
            ),
        ]
    );
    assert_eq!(
        without_declaration,
        vec![Location::new(
            uri,
            span_to_range(source, nth_span(source, "value", 2))
        )]
    );
}
