use std::collections::HashMap;

use ql_analysis::analyze_source;
use ql_diagnostics::{Diagnostic as CompilerDiagnostic, Label};
use ql_lsp::bridge::{
    definition_for_analysis, diagnostics_to_lsp, hover_for_analysis, position_to_offset,
    prepare_rename_for_analysis, references_for_analysis, rename_for_analysis, span_to_range,
};
use ql_span::Span;
use tower_lsp::lsp_types::{
    GotoDefinitionResponse, HoverContents, Location, Position, PrepareRenameResponse, TextEdit,
    Url, WorkspaceEdit,
};

fn nth_span(source: &str, needle: &str, occurrence: usize) -> Span {
    source
        .match_indices(needle)
        .nth(occurrence.saturating_sub(1))
        .map(|(start, matched)| Span::new(start, start + matched.len()))
        .expect("needle occurrence should exist")
}

fn alias_span(source: &str, alias: &str) -> Span {
    source
        .find(&format!("as {alias}"))
        .map(|offset| Span::new(offset + 3, offset + 3 + alias.len()))
        .expect("import alias definition should exist")
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
fn hover_bridge_renders_markdown_for_variant_symbols() {
    let source = r#"
enum Command {
    Retry(Int),
}

fn build() -> Command {
    return Command.Retry(1)
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let variant_position = span_to_range(source, nth_span(source, "Retry", 2)).start;
    let hover = hover_for_analysis(source, &analysis, variant_position)
        .expect("variant hover should exist");

    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown content");
    };

    assert!(markup.value.contains("**variant** `Retry`"));
    assert!(markup.value.contains("variant Command.Retry(Int)"));
    assert!(markup.value.contains("Type: `Command`"));
}

#[test]
fn hover_bridge_renders_markdown_for_explicit_struct_field_labels() {
    let source = r#"
struct Point {
    x: Int,
}

fn read(value: Int) -> Point {
    return Point { x: value }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let field_position = span_to_range(source, nth_span(source, "x", 2)).start;
    let hover =
        hover_for_analysis(source, &analysis, field_position).expect("field hover should exist");

    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown content");
    };

    assert!(markup.value.contains("**field** `x`"));
    assert!(markup.value.contains("field x: Int"));
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

#[test]
fn prepare_rename_bridge_returns_range_and_placeholder() {
    let source = r#"
fn id(value: Int) -> Int {
    return value
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");

    let response = prepare_rename_for_analysis(source, &analysis, Position::new(2, 11))
        .expect("prepare rename should exist");

    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = response else {
        panic!("expected range plus placeholder");
    };

    assert_eq!(range, span_to_range(source, nth_span(source, "value", 2)));
    assert_eq!(placeholder, "value");
}

#[test]
fn rename_bridge_returns_same_file_workspace_edits() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
fn id(value: Int) -> Int {
    return value
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");

    let edit = rename_for_analysis(&uri, source, &analysis, Position::new(2, 11), "input")
        .expect("rename should validate")
        .expect("rename should produce edits");

    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri,
        vec![
            TextEdit::new(
                span_to_range(source, nth_span(source, "value", 1)),
                "input".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "value", 2)),
                "input".to_owned(),
            ),
        ],
    );

    assert_eq!(edit, WorkspaceEdit::new(expected_changes));
}

#[test]
fn rename_bridge_surfaces_invalid_names() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
fn id(value: Int) -> Int {
    return value
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");

    assert_eq!(
        rename_for_analysis(&uri, source, &analysis, Position::new(2, 11), "match"),
        Err(ql_analysis::RenameError::Keyword("match".to_owned()))
    );
}

#[test]
fn rename_bridge_supports_import_aliases() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
use std.collections.HashMap as Map

fn build(cache: Map[String, Int]) -> Map[String, Int] {
    return cache
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let first_use_start = source
        .find("Map[String, Int]")
        .expect("first import alias use should exist");
    let second_use_start = source
        .rfind("Map[String, Int]")
        .expect("second import alias use should exist");
    let first_use_span = Span::new(first_use_start, first_use_start + "Map".len());
    let second_use_span = Span::new(second_use_start, second_use_start + "Map".len());
    let import_use = span_to_range(source, first_use_span).start;

    let prepare = prepare_rename_for_analysis(source, &analysis, import_use)
        .expect("import alias prepare rename should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(range, span_to_range(source, first_use_span));
    assert_eq!(placeholder, "Map");

    let edit = rename_for_analysis(&uri, source, &analysis, import_use, "CacheMap")
        .expect("rename should validate")
        .expect("rename should produce edits");

    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri,
        vec![
            TextEdit::new(
                span_to_range(source, alias_span(source, "Map")),
                "CacheMap".to_owned(),
            ),
            TextEdit::new(span_to_range(source, first_use_span), "CacheMap".to_owned()),
            TextEdit::new(
                span_to_range(source, second_use_span),
                "CacheMap".to_owned(),
            ),
        ],
    );

    assert_eq!(edit, WorkspaceEdit::new(expected_changes));
}
