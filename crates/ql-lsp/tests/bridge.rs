use std::collections::HashMap;

use ql_analysis::analyze_source;
use ql_diagnostics::{Diagnostic as CompilerDiagnostic, Label};
use ql_lsp::bridge::{
    completion_for_analysis, definition_for_analysis, diagnostics_to_lsp, hover_for_analysis,
    position_to_offset, prepare_rename_for_analysis, references_for_analysis, rename_for_analysis,
    semantic_tokens_for_analysis, semantic_tokens_legend, span_to_range,
};
use ql_span::Span;
use tower_lsp::lsp_types::{
    CompletionItemKind, CompletionResponse, GotoDefinitionResponse, HoverContents, Location,
    Position, PrepareRenameResponse, SemanticTokenType, SemanticTokensResult, TextEdit, Url,
    WorkspaceEdit,
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

fn decode_semantic_tokens(
    tokens: &[tower_lsp::lsp_types::SemanticToken],
) -> Vec<(u32, u32, u32, u32)> {
    let mut line = 0u32;
    let mut start = 0u32;
    let mut decoded = Vec::new();

    for token in tokens {
        line += token.delta_line;
        if token.delta_line == 0 {
            start += token.delta_start;
        } else {
            start = token.delta_start;
        }
        decoded.push((line, start, token.length, token.token_type));
    }

    decoded
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
fn hover_definition_and_references_bridge_follow_lexical_semantic_symbols() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
fn id[T](param: T) -> T {
    let local_value = param
    return local_value
}

struct Counter {
    value: String,
}

impl Counter {
    fn read(self, input: String) -> String {
        let alias = input
        return self.value
    }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let generic_position = span_to_range(source, nth_span(source, "T", 3)).start;
    let parameter_position = span_to_range(source, nth_span(source, "param", 2)).start;
    let local_position = span_to_range(source, nth_span(source, "local_value", 2)).start;
    let self_position = span_to_range(source, nth_span(source, "self", 2)).start;
    let builtin_position = span_to_range(source, nth_span(source, "String", 2)).start;

    let generic_hover = hover_for_analysis(source, &analysis, generic_position)
        .expect("generic hover should exist");
    let HoverContents::Markup(markup) = generic_hover.contents else {
        panic!("hover should use markdown content");
    };
    assert!(markup.value.contains("**generic** `T`"));
    assert!(markup.value.contains("generic T"));

    let parameter_hover = hover_for_analysis(source, &analysis, parameter_position)
        .expect("parameter hover should exist");
    let HoverContents::Markup(markup) = parameter_hover.contents else {
        panic!("hover should use markdown content");
    };
    assert!(markup.value.contains("**parameter** `param`"));
    assert!(markup.value.contains("param param: T"));
    assert!(markup.value.contains("Type: `T`"));

    let local_hover =
        hover_for_analysis(source, &analysis, local_position).expect("local hover should exist");
    let HoverContents::Markup(markup) = local_hover.contents else {
        panic!("hover should use markdown content");
    };
    assert!(markup.value.contains("**local** `local_value`"));
    assert!(markup.value.contains("local local_value: T"));
    assert!(markup.value.contains("Type: `T`"));

    let self_hover =
        hover_for_analysis(source, &analysis, self_position).expect("receiver hover should exist");
    let HoverContents::Markup(markup) = self_hover.contents else {
        panic!("hover should use markdown content");
    };
    assert!(markup.value.contains("**receiver** `self`"));
    assert!(markup.value.contains("receiver self: Counter"));
    assert!(markup.value.contains("Type: `Counter`"));

    let builtin_hover = hover_for_analysis(source, &analysis, builtin_position)
        .expect("builtin type hover should exist");
    let HoverContents::Markup(markup) = builtin_hover.contents else {
        panic!("hover should use markdown content");
    };
    assert!(markup.value.contains("**builtin type** `String`"));
    assert!(markup.value.contains("builtin type String"));

    let definition = definition_for_analysis(&uri, source, &analysis, generic_position)
        .expect("generic definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };
    assert_eq!(location.uri, uri);
    assert_eq!(
        location.range,
        span_to_range(source, nth_span(source, "T", 1))
    );

    let definition = definition_for_analysis(&uri, source, &analysis, parameter_position)
        .expect("parameter definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };
    assert_eq!(location.uri, uri);
    assert_eq!(
        location.range,
        span_to_range(source, nth_span(source, "param", 1))
    );

    let definition = definition_for_analysis(&uri, source, &analysis, local_position)
        .expect("local definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };
    assert_eq!(location.uri, uri);
    assert_eq!(
        location.range,
        span_to_range(source, nth_span(source, "local_value", 1))
    );

    let definition = definition_for_analysis(&uri, source, &analysis, self_position)
        .expect("receiver definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };
    assert_eq!(location.uri, uri);
    assert_eq!(
        location.range,
        span_to_range(source, nth_span(source, "self", 1))
    );

    assert_eq!(
        definition_for_analysis(&uri, source, &analysis, builtin_position),
        None
    );

    assert_eq!(
        references_for_analysis(&uri, source, &analysis, generic_position, true)
            .expect("generic references should exist"),
        vec![
            Location::new(uri.clone(), span_to_range(source, nth_span(source, "T", 1))),
            Location::new(uri.clone(), span_to_range(source, nth_span(source, "T", 2))),
            Location::new(uri.clone(), span_to_range(source, nth_span(source, "T", 3))),
        ]
    );
    assert_eq!(
        references_for_analysis(&uri, source, &analysis, generic_position, false)
            .expect("generic references should exist"),
        vec![
            Location::new(uri.clone(), span_to_range(source, nth_span(source, "T", 2))),
            Location::new(uri.clone(), span_to_range(source, nth_span(source, "T", 3))),
        ]
    );

    assert_eq!(
        references_for_analysis(&uri, source, &analysis, parameter_position, true)
            .expect("parameter references should exist"),
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "param", 1))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "param", 2))
            ),
        ]
    );
    assert_eq!(
        references_for_analysis(&uri, source, &analysis, parameter_position, false)
            .expect("parameter references should exist"),
        vec![Location::new(
            uri.clone(),
            span_to_range(source, nth_span(source, "param", 2)),
        )]
    );

    assert_eq!(
        references_for_analysis(&uri, source, &analysis, local_position, true)
            .expect("local references should exist"),
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "local_value", 1)),
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "local_value", 2)),
            ),
        ]
    );
    assert_eq!(
        references_for_analysis(&uri, source, &analysis, local_position, false)
            .expect("local references should exist"),
        vec![Location::new(
            uri.clone(),
            span_to_range(source, nth_span(source, "local_value", 2)),
        )]
    );

    assert_eq!(
        references_for_analysis(&uri, source, &analysis, self_position, true)
            .expect("receiver references should exist"),
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "self", 1))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "self", 2))
            ),
        ]
    );
    assert_eq!(
        references_for_analysis(&uri, source, &analysis, self_position, false)
            .expect("receiver references should exist"),
        vec![Location::new(
            uri.clone(),
            span_to_range(source, nth_span(source, "self", 2)),
        )]
    );

    assert_eq!(
        references_for_analysis(&uri, source, &analysis, builtin_position, true)
            .expect("builtin type references should exist"),
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "String", 1))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "String", 2))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "String", 3))
            ),
        ]
    );
    assert_eq!(
        references_for_analysis(&uri, source, &analysis, builtin_position, false)
            .expect("builtin type references should exist"),
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "String", 1))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "String", 2))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "String", 3))
            ),
        ]
    );
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
fn hover_definition_and_references_bridge_follow_direct_member_symbols() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn get(self) -> Int {
        return self.value
    }

    fn read(self) -> Int {
        return self.get()
    }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let field_position = span_to_range(source, nth_span(source, "value", 2)).start;
    let method_position = span_to_range(source, nth_span(source, "get", 2)).start;

    let field_hover =
        hover_for_analysis(source, &analysis, field_position).expect("field hover should exist");
    let HoverContents::Markup(field_markup) = field_hover.contents else {
        panic!("hover should use markdown content");
    };
    assert!(field_markup.value.contains("**field** `value`"));
    assert!(field_markup.value.contains("field value: Int"));
    assert!(field_markup.value.contains("Type: `Int`"));

    let definition = definition_for_analysis(&uri, source, &analysis, field_position)
        .expect("field definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };
    assert_eq!(location.uri, uri.clone());
    assert_eq!(
        location.range,
        span_to_range(source, nth_span(source, "value", 1))
    );

    let field_with_declaration =
        references_for_analysis(&uri, source, &analysis, field_position, true)
            .expect("field references should exist");
    let field_without_declaration =
        references_for_analysis(&uri, source, &analysis, field_position, false)
            .expect("field references should exist");
    assert_eq!(
        field_with_declaration,
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
        field_without_declaration,
        vec![Location::new(
            uri.clone(),
            span_to_range(source, nth_span(source, "value", 2))
        )]
    );

    let method_hover =
        hover_for_analysis(source, &analysis, method_position).expect("method hover should exist");
    let HoverContents::Markup(method_markup) = method_hover.contents else {
        panic!("hover should use markdown content");
    };
    assert!(method_markup.value.contains("**method** `get`"));
    assert!(method_markup.value.contains("fn get(self) -> Int"));

    let definition = definition_for_analysis(&uri, source, &analysis, method_position)
        .expect("method definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };
    assert_eq!(location.uri, uri.clone());
    assert_eq!(
        location.range,
        span_to_range(source, nth_span(source, "get", 1))
    );

    let method_with_declaration =
        references_for_analysis(&uri, source, &analysis, method_position, true)
            .expect("method references should exist");
    let method_without_declaration =
        references_for_analysis(&uri, source, &analysis, method_position, false)
            .expect("method references should exist");
    assert_eq!(
        method_with_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "get", 1))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "get", 2))
            ),
        ]
    );
    assert_eq!(
        method_without_declaration,
        vec![Location::new(
            uri,
            span_to_range(source, nth_span(source, "get", 2))
        )]
    );
}

#[test]
fn hover_definition_and_references_bridge_follow_same_file_direct_member_surface() {
    struct MemberCase<'a> {
        name: &'a str,
        use_occurrence: usize,
        hover_label: &'a str,
        detail: &'a str,
        type_note: Option<&'a str>,
        reference_occurrences: &'a [usize],
    }

    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn get(self) -> Int {
        return self.value
    }

    fn read(self) -> Int {
        return self.get()
    }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let cases = [
        MemberCase {
            name: "value",
            use_occurrence: 2,
            hover_label: "**field** `value`",
            detail: "field value: Int",
            type_note: Some("Type: `Int`"),
            reference_occurrences: &[1, 2],
        },
        MemberCase {
            name: "get",
            use_occurrence: 2,
            hover_label: "**method** `get`",
            detail: "fn get(self) -> Int",
            type_note: None,
            reference_occurrences: &[1, 2],
        },
    ];

    for case in cases {
        let position =
            span_to_range(source, nth_span(source, case.name, case.use_occurrence)).start;

        let hover = hover_for_analysis(source, &analysis, position)
            .expect("direct member hover should exist");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("hover should use markdown content");
        };
        assert!(markup.value.contains(case.hover_label), "{}", case.name);
        assert!(markup.value.contains(case.detail), "{}", case.name);
        if let Some(type_note) = case.type_note {
            assert!(markup.value.contains(type_note), "{}", case.name);
        }

        let definition = definition_for_analysis(&uri, source, &analysis, position)
            .expect("direct member definition should exist");
        let GotoDefinitionResponse::Scalar(location) = definition else {
            panic!("expected scalar definition location");
        };
        assert_eq!(location.uri, uri, "{}", case.name);
        assert_eq!(
            location.range,
            span_to_range(source, nth_span(source, case.name, 1)),
            "{}",
            case.name
        );

        assert_eq!(
            references_for_analysis(&uri, source, &analysis, position, true)
                .expect("direct member references should exist"),
            case.reference_occurrences
                .iter()
                .map(|occurrence| {
                    Location::new(
                        uri.clone(),
                        span_to_range(source, nth_span(source, case.name, *occurrence)),
                    )
                })
                .collect::<Vec<_>>(),
            "{}",
            case.name
        );
        assert_eq!(
            references_for_analysis(&uri, source, &analysis, position, false)
                .expect("direct member references should exist"),
            case.reference_occurrences[1..]
                .iter()
                .map(|occurrence| {
                    Location::new(
                        uri.clone(),
                        span_to_range(source, nth_span(source, case.name, *occurrence)),
                    )
                })
                .collect::<Vec<_>>(),
            "{}",
            case.name
        );
    }
}

#[test]
fn hover_and_definition_bridge_prefer_impl_methods_over_extend_methods() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn read(self, delta: Int) -> Int {
        return self.value + delta
    }
}

extend Counter {
    fn read(self) -> Int {
        return self.value
    }
}

fn main() -> Int {
    let counter = Counter { value: 1 }
    return counter.read(1)
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let method_position = span_to_range(source, nth_span(source, "read", 3)).start;

    let hover =
        hover_for_analysis(source, &analysis, method_position).expect("method hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown content");
    };
    assert!(markup.value.contains("**method** `read`"));
    assert!(markup.value.contains("fn read(self, delta: Int) -> Int"));

    let definition = definition_for_analysis(&uri, source, &analysis, method_position)
        .expect("method definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };
    assert_eq!(location.uri, uri);
    assert_eq!(
        location.range,
        span_to_range(source, nth_span(source, "read", 1))
    );

    let with_declaration = references_for_analysis(&uri, source, &analysis, method_position, true)
        .expect("method references should exist");
    let without_declaration =
        references_for_analysis(&uri, source, &analysis, method_position, false)
            .expect("method references should exist");
    assert_eq!(
        with_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "read", 1))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "read", 3))
            ),
        ]
    );
    assert_eq!(
        without_declaration,
        vec![Location::new(
            uri,
            span_to_range(source, nth_span(source, "read", 3))
        )]
    );
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
fn hover_bridge_renders_markdown_for_import_alias_variant_symbols() {
    let source = r#"
use Command as Cmd

enum Command {
    Retry(Int),
}

fn build() -> Command {
    return Cmd.Retry(1)
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let variant_position = span_to_range(source, nth_span(source, "Retry", 2)).start;
    let hover = hover_for_analysis(source, &analysis, variant_position)
        .expect("variant hover through import alias should exist");

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
fn hover_and_definition_bridge_keep_shorthand_struct_field_tokens_on_local_symbols() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
struct Point {
    x: Int,
}

fn read(value: Int) -> Int {
    let x = value
    let built = Point { x }
    return x
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let shorthand_position = span_to_range(source, nth_span(source, "x", 3)).start;

    let hover = hover_for_analysis(source, &analysis, shorthand_position)
        .expect("shorthand token hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown content");
    };
    assert!(markup.value.contains("**local** `x`"));
    assert!(markup.value.contains("local x: Int"));
    assert!(markup.value.contains("Type: `Int`"));

    let definition = definition_for_analysis(&uri, source, &analysis, shorthand_position)
        .expect("shorthand token definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };
    assert_eq!(location.uri, uri);
    assert_eq!(
        location.range,
        span_to_range(source, nth_span(source, "x", 2))
    );
}

#[test]
fn hover_and_definition_bridge_follow_type_namespace_item_symbols() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
type IdAlias = Int

struct Account {
    id: IdAlias,
}

enum Mode {
    Ready,
}

trait Taggable {
    fn mode(self) -> Mode
}

impl Taggable for Account {
    fn mode(self) -> Mode {
        return Mode.Ready
    }
}

fn build(account: Account, value: IdAlias) -> Mode {
    let copy = value
    return account.mode()
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let type_alias_position = span_to_range(source, nth_span(source, "IdAlias", 2)).start;
    let struct_position = span_to_range(source, nth_span(source, "Account", 2)).start;
    let enum_position = span_to_range(source, nth_span(source, "Mode", 5)).start;
    let trait_position = span_to_range(source, nth_span(source, "Taggable", 2)).start;

    let type_alias_hover = hover_for_analysis(source, &analysis, type_alias_position)
        .expect("type alias hover should exist");
    let HoverContents::Markup(markup) = type_alias_hover.contents else {
        panic!("hover should use markdown content");
    };
    assert!(markup.value.contains("**type alias** `IdAlias`"));
    assert!(markup.value.contains("type IdAlias = Int"));

    let struct_hover =
        hover_for_analysis(source, &analysis, struct_position).expect("struct hover should exist");
    let HoverContents::Markup(markup) = struct_hover.contents else {
        panic!("hover should use markdown content");
    };
    assert!(markup.value.contains("**struct** `Account`"));
    assert!(markup.value.contains("struct Account"));

    let enum_hover =
        hover_for_analysis(source, &analysis, enum_position).expect("enum hover should exist");
    let HoverContents::Markup(markup) = enum_hover.contents else {
        panic!("hover should use markdown content");
    };
    assert!(markup.value.contains("**enum** `Mode`"));
    assert!(markup.value.contains("enum Mode"));

    let trait_hover =
        hover_for_analysis(source, &analysis, trait_position).expect("trait hover should exist");
    let HoverContents::Markup(markup) = trait_hover.contents else {
        panic!("hover should use markdown content");
    };
    assert!(markup.value.contains("**trait** `Taggable`"));
    assert!(markup.value.contains("trait Taggable"));

    let definition = definition_for_analysis(&uri, source, &analysis, type_alias_position)
        .expect("type alias definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };
    assert_eq!(location.uri, uri);
    assert_eq!(
        location.range,
        span_to_range(source, nth_span(source, "IdAlias", 1))
    );

    let definition = definition_for_analysis(
        &Url::parse("file:///sample.ql").expect("URI should parse"),
        source,
        &analysis,
        struct_position,
    )
    .expect("struct definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };
    assert_eq!(
        location.range,
        span_to_range(source, nth_span(source, "Account", 1))
    );

    let definition = definition_for_analysis(
        &Url::parse("file:///sample.ql").expect("URI should parse"),
        source,
        &analysis,
        enum_position,
    )
    .expect("enum definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };
    assert_eq!(
        location.range,
        span_to_range(source, nth_span(source, "Mode", 1))
    );

    let definition = definition_for_analysis(
        &Url::parse("file:///sample.ql").expect("URI should parse"),
        source,
        &analysis,
        trait_position,
    )
    .expect("trait definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };
    assert_eq!(
        location.range,
        span_to_range(source, nth_span(source, "Taggable", 1))
    );
}

#[test]
fn hover_definition_and_references_bridge_follow_opaque_type_symbols() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
opaque type UserId = Int

struct Account {
    id: UserId,
}

fn build(value: UserId) -> UserId {
    return value
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let opaque_position = span_to_range(source, nth_span(source, "UserId", 2)).start;

    let hover = hover_for_analysis(source, &analysis, opaque_position)
        .expect("opaque type hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown content");
    };
    assert!(markup.value.contains("**type alias** `UserId`"));
    assert!(markup.value.contains("opaque type UserId = Int"));

    let definition = definition_for_analysis(&uri, source, &analysis, opaque_position)
        .expect("opaque type definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };
    assert_eq!(location.uri, uri);
    assert_eq!(
        location.range,
        span_to_range(source, nth_span(source, "UserId", 1))
    );

    assert_eq!(
        references_for_analysis(&uri, source, &analysis, opaque_position, true)
            .expect("opaque type references should exist"),
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "UserId", 1))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "UserId", 2))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "UserId", 3))
            ),
            Location::new(uri, span_to_range(source, nth_span(source, "UserId", 4))),
        ]
    );
}

#[test]
fn hover_definition_and_references_bridge_follow_same_file_type_namespace_item_surface() {
    struct ItemCase<'a> {
        name: &'a str,
        use_occurrence: usize,
        hover_label: &'a str,
        detail: &'a str,
        reference_occurrences: &'a [usize],
    }

    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
type IdAlias = Int

opaque type UserId = Int

struct Account {
    id: UserId,
    alias: IdAlias,
}

enum Mode {
    Ready,
}

trait Taggable {
    fn mode(self) -> Mode
}

impl Taggable for Account {
    fn mode(self) -> Mode {
        return Mode.Ready
    }
}

fn build(account: Account, user_id: UserId, alias: IdAlias) -> Mode {
    let copied_id = user_id
    let copied_alias = alias
    return account.mode()
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let cases = [
        ItemCase {
            name: "IdAlias",
            use_occurrence: 3,
            hover_label: "**type alias** `IdAlias`",
            detail: "type IdAlias = Int",
            reference_occurrences: &[1, 2, 3],
        },
        ItemCase {
            name: "UserId",
            use_occurrence: 3,
            hover_label: "**type alias** `UserId`",
            detail: "opaque type UserId = Int",
            reference_occurrences: &[1, 2, 3],
        },
        ItemCase {
            name: "Account",
            use_occurrence: 3,
            hover_label: "**struct** `Account`",
            detail: "struct Account",
            reference_occurrences: &[1, 2, 3],
        },
        ItemCase {
            name: "Mode",
            use_occurrence: 5,
            hover_label: "**enum** `Mode`",
            detail: "enum Mode",
            reference_occurrences: &[1, 2, 3, 4, 5],
        },
        ItemCase {
            name: "Taggable",
            use_occurrence: 2,
            hover_label: "**trait** `Taggable`",
            detail: "trait Taggable",
            reference_occurrences: &[1, 2],
        },
    ];

    for case in cases {
        let position =
            span_to_range(source, nth_span(source, case.name, case.use_occurrence)).start;

        let hover = hover_for_analysis(source, &analysis, position)
            .expect("type-namespace item hover should exist");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("hover should use markdown content");
        };
        assert!(markup.value.contains(case.hover_label), "{}", case.name);
        assert!(markup.value.contains(case.detail), "{}", case.name);

        let definition = definition_for_analysis(&uri, source, &analysis, position)
            .expect("type-namespace item definition should exist");
        let GotoDefinitionResponse::Scalar(location) = definition else {
            panic!("expected scalar definition location");
        };
        assert_eq!(location.uri, uri, "{}", case.name);
        assert_eq!(
            location.range,
            span_to_range(source, nth_span(source, case.name, 1)),
            "{}",
            case.name
        );

        assert_eq!(
            references_for_analysis(&uri, source, &analysis, position, true)
                .expect("type-namespace item references should exist"),
            case.reference_occurrences
                .iter()
                .map(|occurrence| {
                    Location::new(
                        uri.clone(),
                        span_to_range(source, nth_span(source, case.name, *occurrence)),
                    )
                })
                .collect::<Vec<_>>(),
            "{}",
            case.name
        );
        assert_eq!(
            references_for_analysis(&uri, source, &analysis, position, false)
                .expect("type-namespace item references should exist"),
            case.reference_occurrences[1..]
                .iter()
                .map(|occurrence| {
                    Location::new(
                        uri.clone(),
                        span_to_range(source, nth_span(source, case.name, *occurrence)),
                    )
                })
                .collect::<Vec<_>>(),
            "{}",
            case.name
        );
    }
}

#[test]
fn hover_definition_and_references_bridge_follow_global_value_item_symbols() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
const LIMIT: Int = 10

static CURRENT: Int = LIMIT

fn read() -> Int {
    let snapshot = CURRENT
    return LIMIT
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let const_position = span_to_range(source, nth_span(source, "LIMIT", 3)).start;
    let static_position = span_to_range(source, nth_span(source, "CURRENT", 2)).start;

    let const_hover =
        hover_for_analysis(source, &analysis, const_position).expect("const hover should exist");
    let HoverContents::Markup(markup) = const_hover.contents else {
        panic!("hover should use markdown content");
    };
    assert!(markup.value.contains("**const** `LIMIT`"));
    assert!(markup.value.contains("const LIMIT: Int"));
    assert!(markup.value.contains("Type: `Int`"));

    let static_hover =
        hover_for_analysis(source, &analysis, static_position).expect("static hover should exist");
    let HoverContents::Markup(markup) = static_hover.contents else {
        panic!("hover should use markdown content");
    };
    assert!(markup.value.contains("**static** `CURRENT`"));
    assert!(markup.value.contains("static CURRENT: Int"));
    assert!(markup.value.contains("Type: `Int`"));

    let definition = definition_for_analysis(&uri, source, &analysis, const_position)
        .expect("const definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };
    assert_eq!(location.uri, uri);
    assert_eq!(
        location.range,
        span_to_range(source, nth_span(source, "LIMIT", 1))
    );

    let definition = definition_for_analysis(&uri, source, &analysis, static_position)
        .expect("static definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };
    assert_eq!(location.uri, uri);
    assert_eq!(
        location.range,
        span_to_range(source, nth_span(source, "CURRENT", 1))
    );

    assert_eq!(
        references_for_analysis(&uri, source, &analysis, const_position, true)
            .expect("const references should exist"),
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "LIMIT", 1))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "LIMIT", 2))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "LIMIT", 3))
            ),
        ]
    );
    assert_eq!(
        references_for_analysis(&uri, source, &analysis, const_position, false)
            .expect("const references should exist"),
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "LIMIT", 2))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "LIMIT", 3))
            ),
        ]
    );

    assert_eq!(
        references_for_analysis(&uri, source, &analysis, static_position, true)
            .expect("static references should exist"),
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "CURRENT", 1)),
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "CURRENT", 2)),
            ),
        ]
    );
    assert_eq!(
        references_for_analysis(&uri, source, &analysis, static_position, false)
            .expect("static references should exist"),
        vec![Location::new(
            uri,
            span_to_range(source, nth_span(source, "CURRENT", 2)),
        )]
    );
}

#[test]
fn hover_definition_and_references_bridge_follow_import_alias_symbols() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
use std.collections.HashMap as Map

fn build(cache: Map[String, Int]) -> Map[String, Int] {
    return cache
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let first_use = source
        .find("Map[String, Int]")
        .expect("first import alias use should exist");
    let second_use = source
        .rfind("Map[String, Int]")
        .expect("second import alias use should exist");
    let import_position =
        span_to_range(source, Span::new(first_use, first_use + "Map".len())).start;

    let hover = hover_for_analysis(source, &analysis, import_position)
        .expect("import alias hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown content");
    };
    assert!(markup.value.contains("**import** `Map`"));
    assert!(markup.value.contains("import std.collections.HashMap"));

    let definition = definition_for_analysis(&uri, source, &analysis, import_position)
        .expect("import alias definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };
    assert_eq!(location.uri, uri);
    assert_eq!(
        location.range,
        span_to_range(source, alias_span(source, "Map"))
    );

    assert_eq!(
        references_for_analysis(&uri, source, &analysis, import_position, true)
            .expect("import alias references should exist"),
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, alias_span(source, "Map"))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, Span::new(first_use, first_use + "Map".len()))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, Span::new(second_use, second_use + "Map".len()))
            ),
        ]
    );
    assert_eq!(
        references_for_analysis(&uri, source, &analysis, import_position, false)
            .expect("import alias references should exist"),
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, Span::new(first_use, first_use + "Map".len()))
            ),
            Location::new(
                uri,
                span_to_range(source, Span::new(second_use, second_use + "Map".len()))
            ),
        ]
    );
}

#[test]
fn hover_definition_and_references_bridge_follow_extern_block_function_symbols() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
extern "c" {
    fn q_add(left: Int, right: Int) -> Int
}

fn main() -> Int {
    return q_add(1, 2)
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let extern_position = span_to_range(source, nth_span(source, "q_add", 2)).start;

    let hover = hover_for_analysis(source, &analysis, extern_position)
        .expect("extern function hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown content");
    };
    assert!(markup.value.contains("**function** `q_add`"));
    assert!(
        markup
            .value
            .contains("extern \"c\" fn q_add(left: Int, right: Int) -> Int")
    );

    let definition = definition_for_analysis(&uri, source, &analysis, extern_position)
        .expect("extern function definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };
    assert_eq!(location.uri, uri);
    assert_eq!(
        location.range,
        span_to_range(source, nth_span(source, "q_add", 1))
    );

    assert_eq!(
        references_for_analysis(&uri, source, &analysis, extern_position, true)
            .expect("extern function references should exist"),
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "q_add", 1))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "q_add", 2))
            ),
        ]
    );
    assert_eq!(
        references_for_analysis(&uri, source, &analysis, extern_position, false)
            .expect("extern function references should exist"),
        vec![Location::new(
            uri,
            span_to_range(source, nth_span(source, "q_add", 2)),
        )]
    );
}

#[test]
fn hover_definition_and_references_bridge_follow_top_level_extern_function_symbols() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
extern "c" fn q_add(left: Int, right: Int) -> Int

fn main() -> Int {
    return q_add(1, 2)
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let extern_position = span_to_range(source, nth_span(source, "q_add", 2)).start;

    let hover = hover_for_analysis(source, &analysis, extern_position)
        .expect("extern function hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown content");
    };
    assert!(markup.value.contains("**function** `q_add`"));
    assert!(
        markup
            .value
            .contains("extern \"c\" fn q_add(left: Int, right: Int) -> Int")
    );

    let definition = definition_for_analysis(&uri, source, &analysis, extern_position)
        .expect("extern function definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };
    assert_eq!(location.uri, uri);
    assert_eq!(
        location.range,
        span_to_range(source, nth_span(source, "q_add", 1))
    );

    assert_eq!(
        references_for_analysis(&uri, source, &analysis, extern_position, true)
            .expect("extern function references should exist"),
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "q_add", 1))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "q_add", 2))
            ),
        ]
    );
    assert_eq!(
        references_for_analysis(&uri, source, &analysis, extern_position, false)
            .expect("extern function references should exist"),
        vec![Location::new(
            uri,
            span_to_range(source, nth_span(source, "q_add", 2)),
        )]
    );
}

#[test]
fn hover_definition_and_references_bridge_follow_top_level_extern_function_definition_symbols() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
extern "c" fn q_add(left: Int, right: Int) -> Int {
    return left + right
}

fn main() -> Int {
    return q_add(1, 2)
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let extern_position = span_to_range(source, nth_span(source, "q_add", 2)).start;

    let hover = hover_for_analysis(source, &analysis, extern_position)
        .expect("extern definition hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown content");
    };
    assert!(markup.value.contains("**function** `q_add`"));
    assert!(
        markup
            .value
            .contains("extern \"c\" fn q_add(left: Int, right: Int) -> Int")
    );

    let definition = definition_for_analysis(&uri, source, &analysis, extern_position)
        .expect("extern definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };
    assert_eq!(location.uri, uri);
    assert_eq!(
        location.range,
        span_to_range(source, nth_span(source, "q_add", 1))
    );

    assert_eq!(
        references_for_analysis(&uri, source, &analysis, extern_position, true)
            .expect("extern definition references should exist"),
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "q_add", 1))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "q_add", 2))
            ),
        ]
    );
    assert_eq!(
        references_for_analysis(&uri, source, &analysis, extern_position, false)
            .expect("extern definition references should exist"),
        vec![Location::new(
            uri,
            span_to_range(source, nth_span(source, "q_add", 2)),
        )]
    );
}

#[test]
fn hover_and_definition_bridge_follow_struct_field_labels_through_import_alias_paths() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
use Point as P

struct Point {
    x: Int,
}

fn read(value: Int) -> Point {
    return P { x: value }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let field_position = span_to_range(source, nth_span(source, "x", 2)).start;
    let hover = hover_for_analysis(source, &analysis, field_position)
        .expect("field hover through import alias should exist");

    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown content");
    };

    assert!(markup.value.contains("**field** `x`"));
    assert!(markup.value.contains("field x: Int"));
    assert!(markup.value.contains("Type: `Int`"));

    let definition = definition_for_analysis(&uri, source, &analysis, field_position)
        .expect("field definition through import alias should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };

    assert_eq!(location.uri, uri);
    assert_eq!(
        location.range,
        span_to_range(source, nth_span(source, "x", 1))
    );
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
fn hover_definition_and_references_bridge_follow_free_function_symbols() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
fn helper(value: Int) -> Int {
    return value
}

fn compute() -> Int {
    return helper(1) + helper(2)
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let function_position = span_to_range(source, nth_span(source, "helper", 2)).start;

    let hover = hover_for_analysis(source, &analysis, function_position)
        .expect("free function hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown content");
    };
    assert!(markup.value.contains("**function** `helper`"));
    assert!(markup.value.contains("fn helper(value: Int) -> Int"));

    let definition = definition_for_analysis(&uri, source, &analysis, function_position)
        .expect("free function definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };
    assert_eq!(location.uri, uri.clone());
    assert_eq!(
        location.range,
        span_to_range(source, nth_span(source, "helper", 1))
    );

    let with_declaration =
        references_for_analysis(&uri, source, &analysis, function_position, true)
            .expect("free function references should exist");
    let without_declaration =
        references_for_analysis(&uri, source, &analysis, function_position, false)
            .expect("free function references should exist");
    assert_eq!(
        with_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "helper", 1))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "helper", 2))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "helper", 3))
            ),
        ]
    );
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "helper", 2))
            ),
            Location::new(uri, span_to_range(source, nth_span(source, "helper", 3))),
        ]
    );
}

#[test]
fn hover_definition_and_references_bridge_follow_same_file_callable_surface() {
    struct CallableCase<'a> {
        name: &'a str,
        use_occurrence: usize,
        detail: &'a str,
        reference_occurrences: &'a [usize],
    }

    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
extern "c" {
    fn q_add(left: Int, right: Int) -> Int
}

extern "c" fn q_sub(left: Int, right: Int) -> Int

extern "c" fn q_mul(left: Int, right: Int) -> Int {
    return left * right
}

fn helper(value: Int) -> Int {
    return value
}

fn compute() -> Int {
    return q_add(1, 2) + q_sub(1, 2) + q_mul(1, 2) + helper(1) + helper(2)
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let cases = [
        CallableCase {
            name: "q_add",
            use_occurrence: 2,
            detail: "extern \"c\" fn q_add(left: Int, right: Int) -> Int",
            reference_occurrences: &[1, 2],
        },
        CallableCase {
            name: "q_sub",
            use_occurrence: 2,
            detail: "extern \"c\" fn q_sub(left: Int, right: Int) -> Int",
            reference_occurrences: &[1, 2],
        },
        CallableCase {
            name: "q_mul",
            use_occurrence: 2,
            detail: "extern \"c\" fn q_mul(left: Int, right: Int) -> Int",
            reference_occurrences: &[1, 2],
        },
        CallableCase {
            name: "helper",
            use_occurrence: 2,
            detail: "fn helper(value: Int) -> Int",
            reference_occurrences: &[1, 2, 3],
        },
    ];

    for case in cases {
        let position =
            span_to_range(source, nth_span(source, case.name, case.use_occurrence)).start;

        let hover =
            hover_for_analysis(source, &analysis, position).expect("callable hover should exist");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("hover should use markdown content");
        };
        assert!(markup.value.contains("**function**"), "{}", case.name);
        assert!(markup.value.contains(case.detail), "{}", case.name);

        let definition = definition_for_analysis(&uri, source, &analysis, position)
            .expect("callable definition should exist");
        let GotoDefinitionResponse::Scalar(location) = definition else {
            panic!("expected scalar definition location");
        };
        assert_eq!(location.uri, uri, "{}", case.name);
        assert_eq!(
            location.range,
            span_to_range(source, nth_span(source, case.name, 1)),
            "{}",
            case.name
        );

        assert_eq!(
            references_for_analysis(&uri, source, &analysis, position, true)
                .expect("callable references should exist"),
            case.reference_occurrences
                .iter()
                .map(|occurrence| {
                    Location::new(
                        uri.clone(),
                        span_to_range(source, nth_span(source, case.name, *occurrence)),
                    )
                })
                .collect::<Vec<_>>(),
            "{}",
            case.name
        );
        assert_eq!(
            references_for_analysis(&uri, source, &analysis, position, false)
                .expect("callable references should exist"),
            case.reference_occurrences[1..]
                .iter()
                .map(|occurrence| {
                    Location::new(
                        uri.clone(),
                        span_to_range(source, nth_span(source, case.name, *occurrence)),
                    )
                })
                .collect::<Vec<_>>(),
            "{}",
            case.name
        );
    }
}

#[test]
fn definition_and_references_bridge_follow_variant_symbols() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
enum Command {
    Retry(Int),
    Config { retries: Int },
}

fn build(flag: Bool) -> Command {
    if flag {
        return Command.Retry(1)
    }
    return Command.Config { retries: 2 }
}

fn read(command: Command) -> Int {
    match command {
        Command.Retry(times) => times,
        Command.Config { retries } => retries,
    }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let retry_position = span_to_range(source, nth_span(source, "Retry", 2)).start;
    let config_literal_position = span_to_range(source, nth_span(source, "Config", 2)).start;
    let config_pattern_position = span_to_range(source, nth_span(source, "Config", 3)).start;

    let definition = definition_for_analysis(&uri, source, &analysis, retry_position)
        .expect("tuple variant definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };
    assert_eq!(location.uri, uri.clone());
    assert_eq!(
        location.range,
        span_to_range(source, nth_span(source, "Retry", 1))
    );

    let definition = definition_for_analysis(&uri, source, &analysis, config_pattern_position)
        .expect("struct variant definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };
    assert_eq!(location.uri, uri.clone());
    assert_eq!(
        location.range,
        span_to_range(source, nth_span(source, "Config", 1))
    );

    let retry_with_declaration =
        references_for_analysis(&uri, source, &analysis, retry_position, true)
            .expect("tuple variant references should exist");
    let retry_without_declaration =
        references_for_analysis(&uri, source, &analysis, retry_position, false)
            .expect("tuple variant references should exist");
    assert_eq!(
        retry_with_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "Retry", 1))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "Retry", 2))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "Retry", 3))
            ),
        ]
    );
    assert_eq!(
        retry_without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "Retry", 2))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "Retry", 3))
            ),
        ]
    );

    let config_with_declaration =
        references_for_analysis(&uri, source, &analysis, config_literal_position, true)
            .expect("struct variant references should exist");
    let config_without_declaration =
        references_for_analysis(&uri, source, &analysis, config_literal_position, false)
            .expect("struct variant references should exist");
    assert_eq!(
        config_with_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "Config", 1))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "Config", 2))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "Config", 3))
            ),
        ]
    );
    assert_eq!(
        config_without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "Config", 2))
            ),
            Location::new(uri, span_to_range(source, nth_span(source, "Config", 3))),
        ]
    );
}

#[test]
fn definition_bridge_returns_variant_locations_through_import_alias_paths() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
use Command as Cmd

enum Command {
    Retry(Int),
}

fn build() -> Command {
    return Cmd.Retry(1)
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let variant_position = span_to_range(source, nth_span(source, "Retry", 2)).start;
    let definition = definition_for_analysis(&uri, source, &analysis, variant_position)
        .expect("definition should exist");

    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };

    assert_eq!(location.uri, uri);
    assert_eq!(
        location.range,
        span_to_range(source, nth_span(source, "Retry", 1))
    );
}

#[test]
fn definition_bridge_stays_empty_on_deeper_variant_like_member_paths() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
use Command as Cmd

enum Command {
    Retry(Int),
    Stop,
}

fn main() -> Int {
    let alias = Cmd.Retry.Stop
    return 0
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let bogus_position = span_to_range(source, nth_span(source, "Stop", 2)).start;

    assert_eq!(
        definition_for_analysis(&uri, source, &analysis, bogus_position),
        None
    );
}

#[test]
fn definition_bridge_stays_empty_on_deeper_struct_literal_and_pattern_variant_paths() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
use Command as Cmd

enum Command {
    Retry(Int),
    Config { value: Int },
    Stop,
}

fn build() -> Int {
    let direct = Command.Scope.Config { value: 1 }
    let alias = Cmd.Scope.Config { value: 2 }
    return 0
}

fn read(command: Command) -> Int {
    return match command {
        Command.Scope.Retry(value) => value,
        Cmd.Scope.Retry(value) => value,
        _ => 0,
    }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");

    for position in [
        span_to_range(source, nth_span(source, "Config", 2)).start,
        span_to_range(source, nth_span(source, "Config", 3)).start,
        span_to_range(source, nth_span(source, "Retry", 2)).start,
        span_to_range(source, nth_span(source, "Retry", 3)).start,
    ] {
        assert_eq!(
            definition_for_analysis(&uri, source, &analysis, position),
            None
        );
    }
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
fn definition_and_references_bridge_follow_explicit_struct_field_labels() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
struct Point {
    x: Int,
    y: Int,
}

fn read(point: Point, value: Int) -> Int {
    let built = Point { x: value, y: 1 }
    match point {
        Point { x: alias, y: 2 } => alias,
    }
    return point.x
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let literal_field_position = span_to_range(source, nth_span(source, "x", 2)).start;
    let pattern_field_position = span_to_range(source, nth_span(source, "x", 3)).start;

    let definition = definition_for_analysis(&uri, source, &analysis, pattern_field_position)
        .expect("field definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("expected scalar definition location");
    };
    assert_eq!(location.uri, uri.clone());
    assert_eq!(
        location.range,
        span_to_range(source, nth_span(source, "x", 1))
    );

    let with_declaration =
        references_for_analysis(&uri, source, &analysis, literal_field_position, true)
            .expect("field references should exist");
    let without_declaration =
        references_for_analysis(&uri, source, &analysis, literal_field_position, false)
            .expect("field references should exist");
    assert_eq!(
        with_declaration,
        vec![
            Location::new(uri.clone(), span_to_range(source, nth_span(source, "x", 1))),
            Location::new(uri.clone(), span_to_range(source, nth_span(source, "x", 2))),
            Location::new(uri.clone(), span_to_range(source, nth_span(source, "x", 3))),
            Location::new(uri.clone(), span_to_range(source, nth_span(source, "x", 4))),
        ]
    );
    assert_eq!(
        without_declaration,
        vec![
            Location::new(uri.clone(), span_to_range(source, nth_span(source, "x", 2))),
            Location::new(uri.clone(), span_to_range(source, nth_span(source, "x", 3))),
            Location::new(uri, span_to_range(source, nth_span(source, "x", 4))),
        ]
    );
}

#[test]
fn hover_definition_and_references_bridge_follow_same_file_direct_symbol_surface() {
    struct DirectCase<'a> {
        name: &'a str,
        use_occurrence: usize,
        hover_label: &'a str,
        detail: &'a str,
        type_note: &'a str,
        reference_occurrences: &'a [usize],
    }

    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
enum Command {
    Retry(Int),
    Config { retries: Int },
}

struct Point {
    x: Int,
    y: Int,
}

fn build(flag: Bool) -> Command {
    if flag {
        return Command.Retry(1)
    }
    return Command.Config { retries: 2 }
}

fn project(point: Point, value: Int) -> Int {
    let built = Point { x: value, y: 1 }
    match point {
        Point { x: alias, y: 2 } => alias,
    }
    return 0
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let cases = [
        DirectCase {
            name: "Retry",
            use_occurrence: 2,
            hover_label: "**variant** `Retry`",
            detail: "variant Command.Retry(Int)",
            type_note: "Type: `Command`",
            reference_occurrences: &[1, 2],
        },
        DirectCase {
            name: "Config",
            use_occurrence: 2,
            hover_label: "**variant** `Config`",
            detail: "variant Command.Config { retries: Int }",
            type_note: "Type: `Command`",
            reference_occurrences: &[1, 2],
        },
        DirectCase {
            name: "x",
            use_occurrence: 2,
            hover_label: "**field** `x`",
            detail: "field x: Int",
            type_note: "Type: `Int`",
            reference_occurrences: &[1, 2, 3],
        },
    ];

    for case in cases {
        let position =
            span_to_range(source, nth_span(source, case.name, case.use_occurrence)).start;

        let hover = hover_for_analysis(source, &analysis, position)
            .expect("direct symbol hover should exist");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("hover should use markdown content");
        };
        assert!(markup.value.contains(case.hover_label), "{}", case.name);
        assert!(markup.value.contains(case.detail), "{}", case.name);
        assert!(markup.value.contains(case.type_note), "{}", case.name);

        let definition = definition_for_analysis(&uri, source, &analysis, position)
            .expect("direct symbol definition should exist");
        let GotoDefinitionResponse::Scalar(location) = definition else {
            panic!("expected scalar definition location");
        };
        assert_eq!(location.uri, uri, "{}", case.name);
        assert_eq!(
            location.range,
            span_to_range(source, nth_span(source, case.name, 1)),
            "{}",
            case.name
        );

        assert_eq!(
            references_for_analysis(&uri, source, &analysis, position, true)
                .expect("direct symbol references should exist"),
            case.reference_occurrences
                .iter()
                .map(|occurrence| {
                    Location::new(
                        uri.clone(),
                        span_to_range(source, nth_span(source, case.name, *occurrence)),
                    )
                })
                .collect::<Vec<_>>(),
            "{}",
            case.name
        );
        assert_eq!(
            references_for_analysis(&uri, source, &analysis, position, false)
                .expect("direct symbol references should exist"),
            case.reference_occurrences[1..]
                .iter()
                .map(|occurrence| {
                    Location::new(
                        uri.clone(),
                        span_to_range(source, nth_span(source, case.name, *occurrence)),
                    )
                })
                .collect::<Vec<_>>(),
            "{}",
            case.name
        );
    }
}

#[test]
fn references_bridge_follow_variant_symbols_through_import_alias_paths() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
use Command as Cmd

enum Command {
    Retry(Int),
    Stop,
}

fn build(flag: Bool) -> Command {
    if flag {
        return Cmd.Retry(1)
    }
    return Cmd.Stop
}

fn read(command: Command) -> Int {
    match command {
        Cmd.Retry(times) => times,
        Cmd.Stop => 0,
    }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let variant_position = span_to_range(source, nth_span(source, "Retry", 2)).start;

    let with_declaration = references_for_analysis(&uri, source, &analysis, variant_position, true)
        .expect("references should exist");
    let without_declaration =
        references_for_analysis(&uri, source, &analysis, variant_position, false)
            .expect("references should exist");

    assert_eq!(
        with_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "Retry", 1))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "Retry", 2))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "Retry", 3))
            ),
        ]
    );
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "Retry", 2))
            ),
            Location::new(uri, span_to_range(source, nth_span(source, "Retry", 3))),
        ]
    );
}

#[test]
fn references_bridge_follow_struct_field_labels_through_import_alias_paths() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
use Point as P

struct Point {
    x: Int,
    y: Int,
}

fn read(point: Point, value: Int) -> Int {
    let built = P { x: value, y: 1 }
    match point {
        P { x: alias, y: 2 } => alias,
    }
    return point.x
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let field_position = span_to_range(source, nth_span(source, "x", 2)).start;

    let with_declaration = references_for_analysis(&uri, source, &analysis, field_position, true)
        .expect("references should exist");
    let without_declaration =
        references_for_analysis(&uri, source, &analysis, field_position, false)
            .expect("references should exist");

    assert_eq!(
        with_declaration,
        vec![
            Location::new(uri.clone(), span_to_range(source, nth_span(source, "x", 1))),
            Location::new(uri.clone(), span_to_range(source, nth_span(source, "x", 2))),
            Location::new(uri.clone(), span_to_range(source, nth_span(source, "x", 3))),
            Location::new(uri.clone(), span_to_range(source, nth_span(source, "x", 4))),
        ]
    );
    assert_eq!(
        without_declaration,
        vec![
            Location::new(uri.clone(), span_to_range(source, nth_span(source, "x", 2))),
            Location::new(uri.clone(), span_to_range(source, nth_span(source, "x", 3))),
            Location::new(uri, span_to_range(source, nth_span(source, "x", 4))),
        ]
    );
}

#[test]
fn references_bridge_follow_type_namespace_item_symbols() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
type IdAlias = Int

struct Account {
    id: IdAlias,
}

enum Mode {
    Ready,
}

trait Taggable {
    fn mode(self) -> Mode
}

impl Taggable for Account {
    fn mode(self) -> Mode {
        return Mode.Ready
    }
}

fn build(account: Account, value: IdAlias) -> Mode {
    let copy = value
    return account.mode()
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let type_alias_position = span_to_range(source, nth_span(source, "IdAlias", 2)).start;
    let struct_position = span_to_range(source, nth_span(source, "Account", 2)).start;
    let enum_position = span_to_range(source, nth_span(source, "Mode", 4)).start;
    let trait_position = span_to_range(source, nth_span(source, "Taggable", 2)).start;

    assert_eq!(
        references_for_analysis(&uri, source, &analysis, type_alias_position, true)
            .expect("type alias references should exist"),
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "IdAlias", 1))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "IdAlias", 2))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "IdAlias", 3))
            ),
        ]
    );
    assert_eq!(
        references_for_analysis(&uri, source, &analysis, struct_position, true)
            .expect("struct references should exist"),
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "Account", 1))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "Account", 2))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "Account", 3))
            ),
        ]
    );
    assert_eq!(
        references_for_analysis(&uri, source, &analysis, enum_position, true)
            .expect("enum references should exist"),
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "Mode", 1))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "Mode", 2))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "Mode", 3))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "Mode", 4))
            ),
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "Mode", 5))
            ),
        ]
    );
    assert_eq!(
        references_for_analysis(&uri, source, &analysis, trait_position, true)
            .expect("trait references should exist"),
        vec![
            Location::new(
                uri.clone(),
                span_to_range(source, nth_span(source, "Taggable", 1))
            ),
            Location::new(uri, span_to_range(source, nth_span(source, "Taggable", 2))),
        ]
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
fn completion_bridge_filters_value_scope_items_by_prefix() {
    let source = r#"
use std.collections.HashMap as Map

fn build[T](input: T) -> T {
    let output = input
    if true {
        let input = output
        return input
    }
    return output
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let input_range = span_to_range(source, nth_span(source, "input", 4));
    let position = Position::new(input_range.start.line, input_range.start.character + 2);

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "input");
    assert_eq!(items[0].kind, Some(CompletionItemKind::VARIABLE));
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(input_range, "input".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_surfaces_visible_value_bindings_and_shadowing() {
    let source = r#"
use std.collections.HashMap as Map

fn build[T](input: T) -> T {
    let output = input
    if true {
        let input = output
        return input
    }
    return output
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let input_range = span_to_range(source, nth_span(source, "input", 4));
    let position = Position::new(input_range.start.line, input_range.start.character);

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(
        items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Map", "build", "input", "output"]
    );
    assert_eq!(items[0].kind, Some(CompletionItemKind::MODULE));
    assert_eq!(items[1].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(items[2].kind, Some(CompletionItemKind::VARIABLE));
    assert_eq!(items[3].kind, Some(CompletionItemKind::VARIABLE));
    assert_eq!(
        items[0].detail.as_deref(),
        Some("import std.collections.HashMap")
    );
    assert_eq!(
        items[1].detail.as_deref(),
        Some("fn build[T](input: T) -> T")
    );
    assert_eq!(items[2].detail.as_deref(), Some("local input: T"));
    assert_eq!(items[3].detail.as_deref(), Some("local output: T"));
    assert!(!items.iter().any(|item| item.label == "T"));
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(input_range, "Map".to_owned(),)
        ))
    );
    assert_eq!(
        items[1].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(input_range, "build".to_owned(),)
        ))
    );
    assert_eq!(
        items[2].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(input_range, "input".to_owned(),)
        ))
    );
    assert_eq!(
        items[3].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(input_range, "output".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_surfaces_value_context_candidate_lists() {
    let source = r#"
use std.collections.HashMap as amap

const bconst: Int = 10
static cstatic: Int = 20

extern "c" {
    fn d_block(left: Int, right: Int) -> Int
}

extern "c" fn e_decl(left: Int, right: Int) -> Int

extern "c" fn f_def(left: Int, right: Int) -> Int {
    return left * right
}

fn g_helper() -> Int {
    return bconst + cstatic
}

fn run(param_value: Int) -> Int {
    let local_value = param_value
    return hole
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let hole_offset = source
        .find("hole\n")
        .expect("return expression should contain the placeholder");
    let hole_range = span_to_range(source, Span::new(hole_offset, hole_offset + "hole".len()));
    let position = Position::new(hole_range.start.line, hole_range.start.character);

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(
        items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec![
            "amap",
            "bconst",
            "cstatic",
            "d_block",
            "e_decl",
            "f_def",
            "g_helper",
            "local_value",
            "param_value",
            "run",
        ]
    );
    assert_eq!(items[0].kind, Some(CompletionItemKind::MODULE));
    assert_eq!(
        items[0].detail.as_deref(),
        Some("import std.collections.HashMap")
    );
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(hole_range, "amap".to_owned(),)
        ))
    );
    assert_eq!(items[1].kind, Some(CompletionItemKind::CONSTANT));
    assert_eq!(items[1].detail.as_deref(), Some("const bconst: Int"));
    assert_eq!(
        items[1].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(hole_range, "bconst".to_owned(),)
        ))
    );
    assert_eq!(items[2].kind, Some(CompletionItemKind::CONSTANT));
    assert_eq!(items[2].detail.as_deref(), Some("static cstatic: Int"));
    assert_eq!(
        items[2].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(hole_range, "cstatic".to_owned(),)
        ))
    );
    assert_eq!(items[3].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(
        items[3].detail.as_deref(),
        Some("extern \"c\" fn d_block(left: Int, right: Int) -> Int")
    );
    assert_eq!(
        items[3].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(hole_range, "d_block".to_owned(),)
        ))
    );
    assert_eq!(items[4].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(
        items[4].detail.as_deref(),
        Some("extern \"c\" fn e_decl(left: Int, right: Int) -> Int")
    );
    assert_eq!(
        items[4].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(hole_range, "e_decl".to_owned(),)
        ))
    );
    assert_eq!(items[5].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(
        items[5].detail.as_deref(),
        Some("extern \"c\" fn f_def(left: Int, right: Int) -> Int")
    );
    assert_eq!(
        items[5].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(hole_range, "f_def".to_owned(),)
        ))
    );
    assert_eq!(items[6].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(items[6].detail.as_deref(), Some("fn g_helper() -> Int"));
    assert_eq!(
        items[6].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(hole_range, "g_helper".to_owned(),)
        ))
    );
    assert_eq!(items[7].kind, Some(CompletionItemKind::VARIABLE));
    assert_eq!(items[7].detail.as_deref(), Some("local local_value: Int"));
    assert_eq!(
        items[7].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(hole_range, "local_value".to_owned(),)
        ))
    );
    assert_eq!(items[8].kind, Some(CompletionItemKind::VARIABLE));
    assert_eq!(items[8].detail.as_deref(), Some("param param_value: Int"));
    assert_eq!(
        items[8].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(hole_range, "param_value".to_owned(),)
        ))
    );
    assert_eq!(items[9].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(
        items[9].detail.as_deref(),
        Some("fn run(param_value: Int) -> Int")
    );
    assert_eq!(
        items[9].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(hole_range, "run".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_maps_free_function_value_candidates() {
    let source = r#"
fn build(value: Int) -> Int {
    return value
}

fn choose() -> (Int) -> Int {
    return build
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let build_range = span_to_range(source, nth_span(source, "build", 2));
    let position = Position::new(build_range.start.line, build_range.start.character + 2);

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "build");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(
        items[0].detail.as_deref(),
        Some("fn build(value: Int) -> Int")
    );
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(build_range, "build".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_maps_extern_callable_value_candidates() {
    let source = r#"
extern "c" {
    fn q_add(left: Int, right: Int) -> Int
}

extern "c" fn q_sub(left: Int, right: Int) -> Int

extern "c" fn q_mul(left: Int, right: Int) -> Int {
    return left * right
}

fn choose_decl() -> (Int, Int) -> Int {
    return q_ad
}

fn choose_top_level() -> (Int, Int) -> Int {
    return q_su
}

fn choose_definition() -> (Int, Int) -> Int {
    return q_mu
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");

    let extern_block_offset = source
        .find("q_ad\n")
        .expect("extern block completion site should exist");
    let extern_block_range = span_to_range(
        source,
        Span::new(extern_block_offset, extern_block_offset + "q_ad".len()),
    );
    let extern_block_position = Position::new(
        extern_block_range.start.line,
        extern_block_range.start.character + 2,
    );
    let Some(CompletionResponse::Array(extern_block_items)) =
        completion_for_analysis(source, &analysis, extern_block_position)
    else {
        panic!("expected array completion response");
    };
    assert!(extern_block_items.iter().any(|item| {
        item.label == "q_add"
            && item.kind == Some(CompletionItemKind::FUNCTION)
            && item.detail.as_deref() == Some("extern \"c\" fn q_add(left: Int, right: Int) -> Int")
            && item.text_edit
                == Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
                    TextEdit::new(extern_block_range, "q_add".to_owned()),
                ))
    }));

    let top_level_decl_offset = source
        .find("q_su\n")
        .expect("top-level extern declaration completion site should exist");
    let top_level_decl_range = span_to_range(
        source,
        Span::new(top_level_decl_offset, top_level_decl_offset + "q_su".len()),
    );
    let top_level_decl_position = Position::new(
        top_level_decl_range.start.line,
        top_level_decl_range.start.character + 2,
    );
    let Some(CompletionResponse::Array(top_level_decl_items)) =
        completion_for_analysis(source, &analysis, top_level_decl_position)
    else {
        panic!("expected array completion response");
    };
    assert!(top_level_decl_items.iter().any(|item| {
        item.label == "q_sub"
            && item.kind == Some(CompletionItemKind::FUNCTION)
            && item.detail.as_deref() == Some("extern \"c\" fn q_sub(left: Int, right: Int) -> Int")
            && item.text_edit
                == Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
                    TextEdit::new(top_level_decl_range, "q_sub".to_owned()),
                ))
    }));

    let top_level_def_offset = source
        .find("q_mu\n")
        .expect("top-level extern definition completion site should exist");
    let top_level_def_range = span_to_range(
        source,
        Span::new(top_level_def_offset, top_level_def_offset + "q_mu".len()),
    );
    let top_level_def_position = Position::new(
        top_level_def_range.start.line,
        top_level_def_range.start.character + 2,
    );
    let Some(CompletionResponse::Array(top_level_def_items)) =
        completion_for_analysis(source, &analysis, top_level_def_position)
    else {
        panic!("expected array completion response");
    };
    assert!(top_level_def_items.iter().any(|item| {
        item.label == "q_mul"
            && item.kind == Some(CompletionItemKind::FUNCTION)
            && item.detail.as_deref() == Some("extern \"c\" fn q_mul(left: Int, right: Int) -> Int")
            && item.text_edit
                == Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
                    TextEdit::new(top_level_def_range, "q_mul".to_owned()),
                ))
    }));
}

#[test]
fn completion_bridge_maps_plain_import_alias_value_candidates() {
    let source = r#"
use std.collections.HashMap as Map

fn build() -> Int {
    let value = Ma
    return 0
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let partial_range = span_to_range(
        source,
        ql_span::Span::new(
            source
                .find("Ma\n")
                .expect("partial import alias should exist"),
            source
                .find("Ma\n")
                .expect("partial import alias should exist")
                + "Ma".len(),
        ),
    );
    let position = Position::new(partial_range.start.line, partial_range.start.character + 2);

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Map");
    assert_eq!(items[0].kind, Some(CompletionItemKind::MODULE));
    assert_eq!(
        items[0].detail.as_deref(),
        Some("import std.collections.HashMap")
    );
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(partial_range, "Map".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_maps_const_and_static_value_candidates() {
    let source = r#"
const LIMIT: Int = 10
static CURRENT: Int = 20

fn build() -> Int {
    return LIM + CURR
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");

    let const_range = span_to_range(
        source,
        ql_span::Span::new(
            source
                .find("LIM +")
                .expect("partial const name should exist"),
            source
                .find("LIM +")
                .expect("partial const name should exist")
                + "LIM".len(),
        ),
    );
    let const_position = Position::new(
        const_range.start.line,
        const_range.start.character + "LIM".len() as u32,
    );

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, const_position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "LIMIT");
    assert_eq!(items[0].kind, Some(CompletionItemKind::CONSTANT));
    assert_eq!(items[0].detail.as_deref(), Some("const LIMIT: Int"));
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(const_range, "LIMIT".to_owned(),)
        ))
    );

    let static_range = span_to_range(
        source,
        ql_span::Span::new(
            source
                .rfind("CURR")
                .expect("partial static name should exist"),
            source
                .rfind("CURR")
                .expect("partial static name should exist")
                + "CURR".len(),
        ),
    );
    let static_position = Position::new(
        static_range.start.line,
        static_range.start.character + "CURR".len() as u32,
    );

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, static_position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "CURRENT");
    assert_eq!(items[0].kind, Some(CompletionItemKind::CONSTANT));
    assert_eq!(items[0].detail.as_deref(), Some("static CURRENT: Int"));
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(static_range, "CURRENT".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_maps_local_value_candidates() {
    let source = r#"
fn build(seed: Int) -> Int {
    let local_value = seed
    return local_v
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let local_range = span_to_range(
        source,
        ql_span::Span::new(
            source
                .find("local_v\n")
                .expect("partial local name should exist"),
            source
                .find("local_v\n")
                .expect("partial local name should exist")
                + "local_v".len(),
        ),
    );
    let position = Position::new(
        local_range.start.line,
        local_range.start.character + "local_v".len() as u32,
    );

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "local_value");
    assert_eq!(items[0].kind, Some(CompletionItemKind::VARIABLE));
    assert_eq!(items[0].detail.as_deref(), Some("local local_value: Int"));
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(local_range, "local_value".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_maps_parameter_value_candidates() {
    let source = r#"
fn build(value: Int) -> Int {
    return val
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let param_range = span_to_range(
        source,
        ql_span::Span::new(
            source
                .find("val\n")
                .expect("partial parameter name should exist"),
            source
                .find("val\n")
                .expect("partial parameter name should exist")
                + "val".len(),
        ),
    );
    let position = Position::new(
        param_range.start.line,
        param_range.start.character + "val".len() as u32,
    );

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "value");
    assert_eq!(items[0].kind, Some(CompletionItemKind::VARIABLE));
    assert_eq!(items[0].detail.as_deref(), Some("param value: Int"));
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(param_range, "value".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_uses_escaped_insert_text_for_keyword_bindings() {
    let source = r#"
fn keyword_passthrough(`type`: String) -> String {
    return `type`
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let escaped_range = span_to_range(source, nth_span(source, "`type`", 2));
    let position = Position::new(escaped_range.start.line, escaped_range.start.character + 3);

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "type");
    assert_eq!(items[0].kind, Some(CompletionItemKind::VARIABLE));
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(escaped_range, "`type`".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_uses_type_namespace_candidates() {
    let source = r#"
use std.collections.HashMap as Map

struct User {}

fn build[T](value: Map[String, T]) -> User {
    return User {}
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let generic_range = span_to_range(source, nth_span(source, "T", 2));
    let position = Position::new(generic_range.start.line, generic_range.start.character + 1);

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "T");
    assert_eq!(items[0].kind, Some(CompletionItemKind::TYPE_PARAMETER));
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(generic_range, "T".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_maps_plain_import_alias_type_candidates() {
    let source = r#"
use std.collections.HashMap as Map

fn build(value: Ma) -> Map[String, Int] {
    return value
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let partial_range = span_to_range(
        source,
        ql_span::Span::new(
            source
                .find("Ma)")
                .expect("partial import alias should exist"),
            source
                .find("Ma)")
                .expect("partial import alias should exist")
                + "Ma".len(),
        ),
    );
    let position = Position::new(partial_range.start.line, partial_range.start.character + 2);

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Map");
    assert_eq!(items[0].kind, Some(CompletionItemKind::MODULE));
    assert_eq!(
        items[0].detail.as_deref(),
        Some("import std.collections.HashMap")
    );
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(partial_range, "Map".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_surfaces_type_context_candidates() {
    let source = r#"
use std.collections.HashMap as ZMap

type ZAlias = Int
opaque type ZOpaque = Int

enum ZMode {
    Idle,
}

trait ZReader {}

struct ZUser {}

fn build[ZT](value: ZMap[String, ZT]) -> ZUser {
    return ZUser {}
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let type_offset = source
        .find("ZMap[String, ZT]")
        .expect("function parameter should contain the import alias");
    let type_range = span_to_range(source, Span::new(type_offset, type_offset + "ZMap".len()));
    let position = Position::new(type_range.start.line, type_range.start.character);

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(
        items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec![
            "Bool", "Bytes", "Char", "F32", "F64", "I16", "I32", "I64", "I8", "ISize", "Int",
            "Never", "String", "U16", "U32", "U64", "U8", "UInt", "USize", "Void", "ZAlias",
            "ZMap", "ZMode", "ZOpaque", "ZReader", "ZT", "ZUser"
        ]
    );
    assert_eq!(items[0].kind, Some(CompletionItemKind::CLASS));
    assert_eq!(items[20].kind, Some(CompletionItemKind::CLASS));
    assert_eq!(items[20].detail.as_deref(), Some("type ZAlias = Int"));
    assert_eq!(items[21].kind, Some(CompletionItemKind::MODULE));
    assert_eq!(
        items[21].detail.as_deref(),
        Some("import std.collections.HashMap")
    );
    assert_eq!(items[22].kind, Some(CompletionItemKind::ENUM));
    assert_eq!(items[22].detail.as_deref(), Some("enum ZMode"));
    assert_eq!(items[23].kind, Some(CompletionItemKind::CLASS));
    assert_eq!(
        items[23].detail.as_deref(),
        Some("opaque type ZOpaque = Int")
    );
    assert_eq!(items[24].kind, Some(CompletionItemKind::INTERFACE));
    assert_eq!(items[24].detail.as_deref(), Some("trait ZReader"));
    assert_eq!(items[25].kind, Some(CompletionItemKind::TYPE_PARAMETER));
    assert_eq!(items[25].detail.as_deref(), Some("generic ZT"));
    assert_eq!(items[26].kind, Some(CompletionItemKind::STRUCT));
    assert_eq!(items[26].detail.as_deref(), Some("struct ZUser"));
    assert!(!items.iter().any(|item| item.label == "value"));
    assert!(!items.iter().any(|item| item.label == "build"));
    assert_eq!(
        items[21].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(type_range, "ZMap".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_maps_builtin_and_struct_type_candidates() {
    let source = r#"
struct User {}

fn build(value: Str) -> Us {
    return User {}
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");

    let builtin_range = span_to_range(
        source,
        ql_span::Span::new(
            source
                .find("Str)")
                .expect("partial builtin type should exist"),
            source
                .find("Str)")
                .expect("partial builtin type should exist")
                + "Str".len(),
        ),
    );
    let builtin_position = Position::new(
        builtin_range.start.line,
        builtin_range.start.character + "Str".len() as u32,
    );

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, builtin_position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "String");
    assert_eq!(items[0].kind, Some(CompletionItemKind::CLASS));
    assert_eq!(items[0].detail.as_deref(), Some("builtin type String"));
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(builtin_range, "String".to_owned(),)
        ))
    );

    let struct_range = span_to_range(
        source,
        ql_span::Span::new(
            source
                .find("Us {")
                .expect("partial struct type should exist"),
            source
                .find("Us {")
                .expect("partial struct type should exist")
                + "Us".len(),
        ),
    );
    let struct_position = Position::new(
        struct_range.start.line,
        struct_range.start.character + "Us".len() as u32,
    );

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, struct_position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "User");
    assert_eq!(items[0].kind, Some(CompletionItemKind::STRUCT));
    assert_eq!(items[0].detail.as_deref(), Some("struct User"));
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(struct_range, "User".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_maps_type_alias_type_candidates() {
    let source = r#"
type IdAlias = Int

fn build(value: IdA) -> IdAlias {
    return value
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let partial_range = span_to_range(
        source,
        ql_span::Span::new(
            source
                .find("IdA)")
                .expect("partial type alias should exist"),
            source
                .find("IdA)")
                .expect("partial type alias should exist")
                + "IdA".len(),
        ),
    );
    let position = Position::new(
        partial_range.start.line,
        partial_range.start.character + "IdA".len() as u32,
    );

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "IdAlias");
    assert_eq!(items[0].kind, Some(CompletionItemKind::CLASS));
    assert_eq!(items[0].detail.as_deref(), Some("type IdAlias = Int"));
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(partial_range, "IdAlias".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_maps_opaque_type_candidates_by_prefix() {
    let source = r#"
opaque type UserId = Int

fn build(value: Us) -> UserId {
    return value
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let partial_range = span_to_range(
        source,
        ql_span::Span::new(
            source
                .find("Us)")
                .expect("partial opaque type should exist"),
            source
                .find("Us)")
                .expect("partial opaque type should exist")
                + "Us".len(),
        ),
    );
    let position = Position::new(
        partial_range.start.line,
        partial_range.start.character + "Us".len() as u32,
    );

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "UserId");
    assert_eq!(items[0].kind, Some(CompletionItemKind::CLASS));
    assert_eq!(items[0].detail.as_deref(), Some("opaque type UserId = Int"));
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(partial_range, "UserId".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_maps_generic_type_candidates_by_prefix() {
    let source = r#"
fn build[ResultType](value: Res) -> ResultType {
    return value
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let partial_range = span_to_range(
        source,
        ql_span::Span::new(
            source
                .find("Res)")
                .expect("partial generic type should exist"),
            source
                .find("Res)")
                .expect("partial generic type should exist")
                + "Res".len(),
        ),
    );
    let position = Position::new(
        partial_range.start.line,
        partial_range.start.character + "Res".len() as u32,
    );

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "ResultType");
    assert_eq!(items[0].kind, Some(CompletionItemKind::TYPE_PARAMETER));
    assert_eq!(items[0].detail.as_deref(), Some("generic ResultType"));
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(partial_range, "ResultType".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_maps_enum_type_candidates() {
    let source = r#"
enum Mode {
    Idle,
}

fn build(value: Mo) -> Mode {
    return value
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");

    let enum_range = span_to_range(
        source,
        ql_span::Span::new(
            source.find("Mo)").expect("partial enum type should exist"),
            source.find("Mo)").expect("partial enum type should exist") + "Mo".len(),
        ),
    );
    let enum_position = Position::new(
        enum_range.start.line,
        enum_range.start.character + "Mo".len() as u32,
    );

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, enum_position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Mode");
    assert_eq!(items[0].kind, Some(CompletionItemKind::ENUM));
    assert_eq!(items[0].detail.as_deref(), Some("enum Mode"));
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(enum_range, "Mode".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_maps_trait_type_candidates() {
    let source = r#"
trait Reader {}

fn build(value: Re) -> Reader {
    return value
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");

    let trait_range = span_to_range(
        source,
        ql_span::Span::new(
            source.find("Re)").expect("partial trait type should exist"),
            source.find("Re)").expect("partial trait type should exist") + "Re".len(),
        ),
    );
    let trait_position = Position::new(
        trait_range.start.line,
        trait_range.start.character + "Re".len() as u32,
    );

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, trait_position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Reader");
    assert_eq!(items[0].kind, Some(CompletionItemKind::INTERFACE));
    assert_eq!(items[0].detail.as_deref(), Some("trait Reader"));
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(trait_range, "Reader".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_filters_member_candidates_by_prefix() {
    let source = r#"
struct Counter {
    total: Int,
    value: Int,
}

impl Counter {
    fn get(self) -> Int {
        return self.value
    }

    fn read(self) -> Int {
        return self.total
    }
}

fn main(counter: Counter) -> Int {
    return counter.get()
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let member_range = span_to_range(source, nth_span(source, "get", 2));
    let position = Position::new(member_range.start.line, member_range.start.character + 1);

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "get");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(member_range, "get".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_surfaces_member_candidates_on_stable_receiver_types() {
    let source = r#"
struct Counter {
    total: Int,
    value: Int,
}

impl Counter {
    fn get(self) -> Int {
        return self.value
    }

    fn read(self) -> Int {
        return self.total
    }
}

extend Counter {
    fn extra(self) -> Int {
        return self.value
    }
}

fn main(counter: Counter) -> Int {
    return counter.read()
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let member_offset = source
        .rfind(".read")
        .map(|offset| offset + 1)
        .expect("member use should exist");
    let member_range = span_to_range(source, Span::new(member_offset, member_offset + 4));
    let position = Position::new(member_range.start.line, member_range.start.character);

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(
        items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["extra", "get", "read", "total", "value"]
    );
    assert_eq!(items[0].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(items[1].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(items[2].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(items[3].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[4].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("fn extra(self) -> Int"));
    assert_eq!(items[1].detail.as_deref(), Some("fn get(self) -> Int"));
    assert_eq!(items[2].detail.as_deref(), Some("fn read(self) -> Int"));
    assert_eq!(items[3].detail.as_deref(), Some("field total: Int"));
    assert_eq!(items[4].detail.as_deref(), Some("field value: Int"));
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(member_range, "extra".to_owned(),)
        ))
    );
    assert_eq!(
        items[1].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(member_range, "get".to_owned(),)
        ))
    );
    assert_eq!(
        items[2].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(member_range, "read".to_owned(),)
        ))
    );
    assert_eq!(
        items[3].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(member_range, "total".to_owned(),)
        ))
    );
    assert_eq!(
        items[4].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(member_range, "value".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_maps_field_candidates_on_stable_receiver_types() {
    let source = r#"
struct Counter {
    value: Int,
    total: Int,
}

fn main(counter: Counter) -> Int {
    return counter.va
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let member_offset = source
        .rfind(".va")
        .map(|offset| offset + 1)
        .expect("member use should exist");
    let member_range = span_to_range(source, Span::new(member_offset, member_offset + 2));
    let position = Position::new(member_range.start.line, member_range.start.character + 1);

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "value");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field value: Int"));
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(member_range, "value".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_maps_unique_method_candidates_on_stable_receiver_types() {
    let source = r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn read(self, delta: Int) -> Int {
        return self.value + delta
    }
}

fn main(counter: Counter) -> Int {
    return counter.re
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let member_offset = source
        .rfind(".re")
        .map(|offset| offset + 1)
        .expect("member use should exist");
    let member_range = span_to_range(source, Span::new(member_offset, member_offset + 2));
    let position = Position::new(member_range.start.line, member_range.start.character + 2);

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "read");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(
        items[0].detail.as_deref(),
        Some("fn read(self, delta: Int) -> Int")
    );
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(member_range, "read".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_prefers_impl_methods_and_skips_ambiguous_extend_candidates() {
    let source = r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn read(self, delta: Int) -> Int {
        return self.value + delta
    }
}

extend Counter {
    fn read(self) -> Int {
        return self.value
    }

    fn ping(self) -> Int {
        return self.value
    }
}

extend Counter {
    fn ping(self, delta: Int) -> Int {
        return self.value + delta
    }
}

fn main(counter: Counter) -> Int {
    return counter.read(1)
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let member_offset = source
        .rfind(".read")
        .map(|offset| offset + 1)
        .expect("member use should exist");
    let member_range = span_to_range(source, Span::new(member_offset, member_offset + 4));
    let position = Position::new(member_range.start.line, member_range.start.character);

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 2);
    let read_item = &items[0];
    assert_eq!(read_item.label, "read");
    assert_eq!(read_item.kind, Some(CompletionItemKind::FUNCTION));
    assert_eq!(
        read_item.detail.as_deref(),
        Some("fn read(self, delta: Int) -> Int")
    );
    let field_item = &items[1];
    assert_eq!(field_item.label, "value");
    assert_eq!(field_item.kind, Some(CompletionItemKind::FIELD));
    assert_eq!(field_item.detail.as_deref(), Some("field value: Int"));
    assert!(!items.iter().any(|item| item.label == "ping"));
    assert_eq!(
        read_item.text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(member_range, "read".to_owned(),)
        ))
    );
    assert_eq!(
        field_item.text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(member_range, "value".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_skips_deferred_multi_segment_member_targets_on_concrete_receivers() {
    let source = r#"
struct Counter {
    value: Int,
}

impl Counter.Scope.Config {
    fn read(self) -> Int {
        return 1
    }
}

extend Counter.Scope.Config {
    fn extra(self) -> Int {
        return 1
    }
}

fn main(counter: Counter) -> Int {
    let read_result = counter.re
    let extra_result = counter.ex
    return counter.value
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");

    let read_offset = source
        .find(".re")
        .map(|offset| offset + 1)
        .expect("read member use should exist");
    let read_range = span_to_range(source, Span::new(read_offset, read_offset + 2));
    let read_position = Position::new(read_range.start.line, read_range.start.character + 2);

    let Some(CompletionResponse::Array(read_items)) =
        completion_for_analysis(source, &analysis, read_position)
    else {
        panic!("expected array completion response");
    };

    assert!(
        read_items.is_empty(),
        "expected no fake deferred impl candidates on concrete receiver, got {read_items:?}"
    );

    let extra_offset = source
        .find(".ex")
        .map(|offset| offset + 1)
        .expect("extra member use should exist");
    let extra_range = span_to_range(source, Span::new(extra_offset, extra_offset + 2));
    let extra_position = Position::new(extra_range.start.line, extra_range.start.character + 2);

    let Some(CompletionResponse::Array(extra_items)) =
        completion_for_analysis(source, &analysis, extra_position)
    else {
        panic!("expected array completion response");
    };

    assert!(
        extra_items.is_empty(),
        "expected no fake deferred extend candidates on concrete receiver, got {extra_items:?}"
    );
}

#[test]
fn completion_bridge_filters_variant_candidates_by_prefix() {
    let source = r#"
enum Command {
    Retry(Int),
    Stop,
}

fn main() -> Command {
    return Command.Re()
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let variant_offset = source
        .find(".Re")
        .map(|offset| offset + 1)
        .expect("variant path should exist");
    let variant_range = span_to_range(source, Span::new(variant_offset, variant_offset + 2));
    let position = Position::new(variant_range.start.line, variant_range.start.character + 1);

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Retry");
    assert_eq!(items[0].kind, Some(CompletionItemKind::ENUM_MEMBER));
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(variant_range, "Retry".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_maps_variant_candidates_on_enum_item_roots() {
    let source = r#"
enum Command {
    Retry(Int),
    Stop,
}

fn main() -> Command {
    return Command.Re()
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let variant_offset = source
        .find(".Re")
        .map(|offset| offset + 1)
        .expect("variant path should exist");
    let variant_range = span_to_range(source, Span::new(variant_offset, variant_offset + 2));
    let position = Position::new(variant_range.start.line, variant_range.start.character + 1);

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Retry");
    assert_eq!(items[0].kind, Some(CompletionItemKind::ENUM_MEMBER));
    assert_eq!(
        items[0].detail.as_deref(),
        Some("variant Command.Retry(Int)")
    );
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(variant_range, "Retry".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_filters_struct_variant_candidates_by_prefix() {
    let source = r#"
enum Command {
    Config { value: Int },
    Stop,
}

fn main() -> Command {
    return Command.Con { value: 1 }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let variant_offset = source
        .find(".Con")
        .map(|offset| offset + 1)
        .expect("variant struct-literal path should exist");
    let variant_range = span_to_range(source, Span::new(variant_offset, variant_offset + 3));
    let position = Position::new(variant_range.start.line, variant_range.start.character + 1);

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Config");
    assert_eq!(items[0].kind, Some(CompletionItemKind::ENUM_MEMBER));
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(variant_range, "Config".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_maps_struct_variant_candidates_by_prefix() {
    let source = r#"
enum Command {
    Config { value: Int },
    Stop,
}

fn main() -> Command {
    return Command.Con { value: 1 }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let variant_offset = source
        .find(".Con")
        .map(|offset| offset + 1)
        .expect("variant struct-literal path should exist");
    let variant_range = span_to_range(source, Span::new(variant_offset, variant_offset + 3));
    let position = Position::new(variant_range.start.line, variant_range.start.character + 1);

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Config");
    assert_eq!(items[0].kind, Some(CompletionItemKind::ENUM_MEMBER));
    assert_eq!(
        items[0].detail.as_deref(),
        Some("variant Command.Config { value: Int }")
    );
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(variant_range, "Config".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_filters_import_alias_variant_candidates_by_prefix() {
    let source = r#"
use Command as Cmd

enum Command {
    Retry(Int),
    Stop,
}

fn main() -> Command {
    return Cmd.Re()
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let variant_offset = source
        .find(".Re")
        .map(|offset| offset + 1)
        .expect("variant path through import alias should exist");
    let variant_range = span_to_range(source, Span::new(variant_offset, variant_offset + 2));
    let position = Position::new(variant_range.start.line, variant_range.start.character + 1);

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Retry");
    assert_eq!(items[0].kind, Some(CompletionItemKind::ENUM_MEMBER));
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(variant_range, "Retry".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_maps_variant_candidates_in_pattern_paths() {
    let source = r#"
enum Command {
    Retry(Int),
    Stop,
}

fn read(command: Command) -> Int {
    return match command {
        Command.Re(value) => value,
        Command.Stop => 0,
    }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let variant_offset = source
        .find(".Re(")
        .map(|offset| offset + 1)
        .expect("variant pattern path should exist");
    let variant_range = span_to_range(source, Span::new(variant_offset, variant_offset + 2));
    let position = Position::new(variant_range.start.line, variant_range.start.character + 1);

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Retry");
    assert_eq!(items[0].kind, Some(CompletionItemKind::ENUM_MEMBER));
    assert_eq!(
        items[0].detail.as_deref(),
        Some("variant Command.Retry(Int)")
    );
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(variant_range, "Retry".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_maps_import_alias_variant_candidates() {
    let source = r#"
use Command as Cmd

enum Command {
    Retry(Int),
    Stop,
}

fn main() -> Command {
    return Cmd.Re()
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let variant_offset = source
        .find(".Re")
        .map(|offset| offset + 1)
        .expect("variant path through import alias should exist");
    let variant_range = span_to_range(source, Span::new(variant_offset, variant_offset + 2));
    let position = Position::new(variant_range.start.line, variant_range.start.character + 1);

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Retry");
    assert_eq!(items[0].kind, Some(CompletionItemKind::ENUM_MEMBER));
    assert_eq!(
        items[0].detail.as_deref(),
        Some("variant Command.Retry(Int)")
    );
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(variant_range, "Retry".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_does_not_offer_variant_candidates_on_deeper_struct_literal_and_pattern_paths()
{
    let source = r#"
use Command as Cmd

enum Command {
    Retry(Int),
    Config { value: Int },
    Stop,
    Reset,
}

fn build() -> Int {
    let direct = Command.Scope.Con { value: 1 }
    let alias = Cmd.Scope.Con { value: 2 }
    return 0
}

fn read(command: Command) -> Int {
    return match command {
        Command.Scope.Re(value) => value,
        Cmd.Scope.Re(value) => value,
        _ => 0,
    }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");

    for (offset, width) in [
        (
            source
                .find("Command.Scope.Con {")
                .map(|value| value + "Command.Scope.".len())
                .expect("direct deeper struct-literal path should exist"),
            3usize,
        ),
        (
            source
                .find("Cmd.Scope.Con {")
                .map(|value| value + "Cmd.Scope.".len())
                .expect("alias deeper struct-literal path should exist"),
            3usize,
        ),
        (
            source
                .find("Command.Scope.Re(")
                .map(|value| value + "Command.Scope.".len())
                .expect("direct deeper pattern path should exist"),
            2usize,
        ),
        (
            source
                .find("Cmd.Scope.Re(")
                .map(|value| value + "Cmd.Scope.".len())
                .expect("alias deeper pattern path should exist"),
            2usize,
        ),
    ] {
        let range = span_to_range(source, Span::new(offset, offset + width));
        let position = Position::new(range.start.line, range.start.character + 1);
        let Some(CompletionResponse::Array(items)) =
            completion_for_analysis(source, &analysis, position)
        else {
            panic!("expected array completion response");
        };
        assert!(
            items
                .iter()
                .all(|item| item.kind != Some(CompletionItemKind::ENUM_MEMBER))
        );
    }
}

#[test]
fn completion_bridge_stays_empty_on_deeper_variant_like_member_paths() {
    let source = r#"
use Command as Cmd

enum Command {
    Retry(Int),
    Stop,
    Reset,
}

fn main() -> Int {
    let alias = Cmd.Retry.St
    return 0
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let variant_offset = source
        .find("Cmd.Retry.St")
        .map(|offset| offset + "Cmd.Retry.".len())
        .expect("alias deeper member path should exist");
    let variant_range = span_to_range(source, Span::new(variant_offset, variant_offset + 2));
    let position = Position::new(variant_range.start.line, variant_range.start.character + 1);

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert!(items.is_empty());
}

#[test]
fn completion_bridge_filters_import_alias_struct_variant_candidates_by_prefix() {
    let source = r#"
use Command as Cmd

enum Command {
    Config { value: Int },
    Stop,
}

fn main() -> Command {
    return Cmd.Con { value: 1 }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let variant_offset = source
        .find(".Con")
        .map(|offset| offset + 1)
        .expect("variant struct-literal path through import alias should exist");
    let variant_range = span_to_range(source, Span::new(variant_offset, variant_offset + 3));
    let position = Position::new(variant_range.start.line, variant_range.start.character + 1);

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Config");
    assert_eq!(items[0].kind, Some(CompletionItemKind::ENUM_MEMBER));
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(variant_range, "Config".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_maps_import_alias_struct_variant_candidates() {
    let source = r#"
use Command as Cmd

enum Command {
    Config { value: Int },
    Stop,
}

fn main() -> Command {
    return Cmd.Con { value: 1 }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let variant_offset = source
        .find(".Con")
        .map(|offset| offset + 1)
        .expect("variant struct-literal path through import alias should exist");
    let variant_range = span_to_range(source, Span::new(variant_offset, variant_offset + 3));
    let position = Position::new(variant_range.start.line, variant_range.start.character + 1);

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Config");
    assert_eq!(items[0].kind, Some(CompletionItemKind::ENUM_MEMBER));
    assert_eq!(
        items[0].detail.as_deref(),
        Some("variant Command.Config { value: Int }")
    );
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(variant_range, "Config".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_maps_import_alias_variant_candidates_in_pattern_paths() {
    let source = r#"
use Command as Cmd

enum Command {
    Retry(Int),
    Stop,
}

fn read(command: Command) -> Int {
    return match command {
        Cmd.Re(value) => value,
        Cmd.Stop => 0,
    }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let variant_offset = source
        .find(".Re(")
        .map(|offset| offset + 1)
        .expect("variant pattern path through import alias should exist");
    let variant_range = span_to_range(source, Span::new(variant_offset, variant_offset + 2));
    let position = Position::new(variant_range.start.line, variant_range.start.character + 1);

    let Some(CompletionResponse::Array(items)) =
        completion_for_analysis(source, &analysis, position)
    else {
        panic!("expected array completion response");
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Retry");
    assert_eq!(items[0].kind, Some(CompletionItemKind::ENUM_MEMBER));
    assert_eq!(
        items[0].detail.as_deref(),
        Some("variant Command.Retry(Int)")
    );
    assert_eq!(
        items[0].text_edit,
        Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
            TextEdit::new(variant_range, "Retry".to_owned(),)
        ))
    );
}

#[test]
fn completion_bridge_surfaces_variant_candidate_lists_across_supported_paths() {
    #[derive(Clone, Copy)]
    struct ExpectedCompletion<'a> {
        label: &'a str,
        detail: &'a str,
        replacement: &'a str,
    }

    struct VariantCompletionCase<'a> {
        name: &'a str,
        source: &'a str,
        marker: &'a str,
        replace_len: usize,
        expected: &'a [ExpectedCompletion<'a>],
    }

    let cases = [
        VariantCompletionCase {
            name: "enum item root",
            source: r#"
enum Command {
    Retry(Int),
    Stop,
}

fn main() -> Command {
    return Command.Re()
}
"#,
            marker: ".Re",
            replace_len: "Re".len(),
            expected: &[
                ExpectedCompletion {
                    label: "Retry",
                    detail: "variant Command.Retry(Int)",
                    replacement: "Retry",
                },
                ExpectedCompletion {
                    label: "Stop",
                    detail: "variant Command.Stop",
                    replacement: "Stop",
                },
            ],
        },
        VariantCompletionCase {
            name: "struct literal path",
            source: r#"
enum Command {
    Config { value: Int },
    Stop,
}

fn build() -> Command {
    return Command.Con { value: 1 }
}
"#,
            marker: ".Con",
            replace_len: "Con".len(),
            expected: &[
                ExpectedCompletion {
                    label: "Config",
                    detail: "variant Command.Config { value: Int }",
                    replacement: "Config",
                },
                ExpectedCompletion {
                    label: "Stop",
                    detail: "variant Command.Stop",
                    replacement: "Stop",
                },
            ],
        },
        VariantCompletionCase {
            name: "pattern path",
            source: r#"
enum Command {
    Retry(Int),
    Stop,
}

fn read(command: Command) -> Int {
    return match command {
        Command.Re(value) => value,
        Command.Stop => 0,
    }
}
"#,
            marker: ".Re(",
            replace_len: "Re".len(),
            expected: &[
                ExpectedCompletion {
                    label: "Retry",
                    detail: "variant Command.Retry(Int)",
                    replacement: "Retry",
                },
                ExpectedCompletion {
                    label: "Stop",
                    detail: "variant Command.Stop",
                    replacement: "Stop",
                },
            ],
        },
        VariantCompletionCase {
            name: "import alias root",
            source: r#"
use Command as Cmd

enum Command {
    Retry(Int),
    Stop,
}

fn main() -> Command {
    return Cmd.Re()
}
"#,
            marker: ".Re",
            replace_len: "Re".len(),
            expected: &[
                ExpectedCompletion {
                    label: "Retry",
                    detail: "variant Command.Retry(Int)",
                    replacement: "Retry",
                },
                ExpectedCompletion {
                    label: "Stop",
                    detail: "variant Command.Stop",
                    replacement: "Stop",
                },
            ],
        },
        VariantCompletionCase {
            name: "import alias struct literal path",
            source: r#"
use Command as Cmd

enum Command {
    Config { value: Int },
    Stop,
}

fn build() -> Command {
    return Cmd.Con { value: 1 }
}
"#,
            marker: ".Con",
            replace_len: "Con".len(),
            expected: &[
                ExpectedCompletion {
                    label: "Config",
                    detail: "variant Command.Config { value: Int }",
                    replacement: "Config",
                },
                ExpectedCompletion {
                    label: "Stop",
                    detail: "variant Command.Stop",
                    replacement: "Stop",
                },
            ],
        },
        VariantCompletionCase {
            name: "import alias pattern path",
            source: r#"
use Command as Cmd

enum Command {
    Retry(Int),
    Stop,
}

fn read(command: Command) -> Int {
    return match command {
        Cmd.Re(value) => value,
        Cmd.Stop => 0,
    }
}
"#,
            marker: ".Re(",
            replace_len: "Re".len(),
            expected: &[
                ExpectedCompletion {
                    label: "Retry",
                    detail: "variant Command.Retry(Int)",
                    replacement: "Retry",
                },
                ExpectedCompletion {
                    label: "Stop",
                    detail: "variant Command.Stop",
                    replacement: "Stop",
                },
            ],
        },
    ];

    for case in cases {
        let analysis = analyze_source(case.source).expect("source should analyze");
        let variant_offset = case
            .source
            .find(case.marker)
            .map(|offset| offset + 1)
            .expect("variant path should exist");
        let variant_range = span_to_range(
            case.source,
            Span::new(variant_offset, variant_offset + case.replace_len),
        );
        let position = Position::new(variant_range.start.line, variant_range.start.character);

        let Some(CompletionResponse::Array(items)) =
            completion_for_analysis(case.source, &analysis, position)
        else {
            panic!("expected array completion response");
        };

        let projected = items
            .iter()
            .map(|item| {
                (
                    item.label.clone(),
                    item.kind,
                    item.detail.clone(),
                    item.text_edit.clone(),
                )
            })
            .collect::<Vec<_>>();
        let expected = case
            .expected
            .iter()
            .map(|item| {
                (
                    item.label.to_owned(),
                    Some(CompletionItemKind::ENUM_MEMBER),
                    Some(item.detail.to_owned()),
                    Some(tower_lsp::lsp_types::CompletionTextEdit::Edit(
                        TextEdit::new(variant_range, item.replacement.to_owned()),
                    )),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(projected, expected, "{}", case.name);
    }
}

#[test]
fn semantic_tokens_bridge_maps_current_semantic_surface() {
    let source = r#"
use std.collections.HashMap as Map

struct Counter {
    value: Int,
}

impl Counter {
    fn get(self, cache: Map[String, Int]) -> Int {
        return self.value + self.get()
    }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let SemanticTokensResult::Tokens(tokens) = semantic_tokens_for_analysis(source, &analysis)
    else {
        panic!("expected full semantic tokens");
    };
    let decoded = decode_semantic_tokens(&tokens.data);
    let legend = semantic_tokens_legend();
    let namespace_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::NAMESPACE)
        .expect("namespace legend entry should exist") as u32;
    let property_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::PROPERTY)
        .expect("property legend entry should exist") as u32;
    let method_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::METHOD)
        .expect("method legend entry should exist") as u32;

    let import_range = span_to_range(source, alias_span(source, "Map"));
    let field_use = source
        .find(".value")
        .map(|offset| offset + 1)
        .expect("field use should exist");
    let field_range = span_to_range(source, Span::new(field_use, field_use + "value".len()));
    let method_use = source
        .find(".get")
        .map(|offset| offset + 1)
        .expect("method use should exist");
    let method_range = span_to_range(source, Span::new(method_use, method_use + "get".len()));

    assert!(decoded.contains(&(
        import_range.start.line,
        import_range.start.character,
        "Map".len() as u32,
        namespace_type,
    )));
    assert!(decoded.contains(&(
        field_range.start.line,
        field_range.start.character,
        "value".len() as u32,
        property_type,
    )));
    assert!(decoded.contains(&(
        method_range.start.line,
        method_range.start.character,
        "get".len() as u32,
        method_type,
    )));
}

#[test]
fn semantic_tokens_bridge_maps_lexical_semantic_symbol_surface() {
    let source = r#"
fn id[T](param: T) -> T {
    let local_value = param
    return local_value
}

struct Counter {
    value: String,
}

impl Counter {
    fn read(self, input: String) -> String {
        let alias = input
        return self.value
    }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let SemanticTokensResult::Tokens(tokens) = semantic_tokens_for_analysis(source, &analysis)
    else {
        panic!("expected full semantic tokens");
    };
    let decoded = decode_semantic_tokens(&tokens.data);
    let legend = semantic_tokens_legend();
    let type_parameter_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::TYPE_PARAMETER)
        .expect("type parameter legend entry should exist") as u32;
    let parameter_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::PARAMETER)
        .expect("parameter legend entry should exist") as u32;
    let variable_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::VARIABLE)
        .expect("variable legend entry should exist") as u32;
    let builtin_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::TYPE)
        .expect("type legend entry should exist") as u32;

    let generic_def = span_to_range(source, nth_span(source, "T", 1));
    let generic_param_use = span_to_range(source, nth_span(source, "T", 2));
    let generic_return_use = span_to_range(source, nth_span(source, "T", 3));
    let parameter_def = span_to_range(source, nth_span(source, "param", 1));
    let parameter_use = span_to_range(source, nth_span(source, "param", 2));
    let local_def = span_to_range(source, nth_span(source, "local_value", 1));
    let local_use = span_to_range(source, nth_span(source, "local_value", 2));
    let self_def = span_to_range(source, nth_span(source, "self", 1));
    let self_use = span_to_range(source, nth_span(source, "self", 2));
    let builtin_string = span_to_range(source, nth_span(source, "String", 2));

    assert!(decoded.contains(&(
        generic_def.start.line,
        generic_def.start.character,
        "T".len() as u32,
        type_parameter_type
    )));
    assert!(decoded.contains(&(
        generic_param_use.start.line,
        generic_param_use.start.character,
        "T".len() as u32,
        type_parameter_type
    )));
    assert!(decoded.contains(&(
        generic_return_use.start.line,
        generic_return_use.start.character,
        "T".len() as u32,
        type_parameter_type
    )));
    assert!(decoded.contains(&(
        parameter_def.start.line,
        parameter_def.start.character,
        "param".len() as u32,
        parameter_type
    )));
    assert!(decoded.contains(&(
        parameter_use.start.line,
        parameter_use.start.character,
        "param".len() as u32,
        parameter_type
    )));
    assert!(decoded.contains(&(
        local_def.start.line,
        local_def.start.character,
        "local_value".len() as u32,
        variable_type
    )));
    assert!(decoded.contains(&(
        local_use.start.line,
        local_use.start.character,
        "local_value".len() as u32,
        variable_type
    )));
    assert!(decoded.contains(&(
        self_def.start.line,
        self_def.start.character,
        "self".len() as u32,
        variable_type
    )));
    assert!(decoded.contains(&(
        self_use.start.line,
        self_use.start.character,
        "self".len() as u32,
        variable_type
    )));
    assert!(decoded.contains(&(
        builtin_string.start.line,
        builtin_string.start.character,
        "String".len() as u32,
        builtin_type
    )));
}

#[test]
fn semantic_tokens_bridge_maps_import_alias_surface() {
    let source = r#"
use std.collections.HashMap as Map

fn build(cache: Map[String, Int]) -> Map[String, Int] {
    return cache
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let SemanticTokensResult::Tokens(tokens) = semantic_tokens_for_analysis(source, &analysis)
    else {
        panic!("expected full semantic tokens");
    };
    let decoded = decode_semantic_tokens(&tokens.data);
    let legend = semantic_tokens_legend();
    let namespace_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::NAMESPACE)
        .expect("namespace legend entry should exist") as u32;

    let import_def = span_to_range(source, alias_span(source, "Map"));
    let first_use = source
        .find("Map[String, Int]")
        .expect("first import alias use should exist");
    let second_use = source
        .rfind("Map[String, Int]")
        .expect("second import alias use should exist");
    let first_use_range = span_to_range(source, Span::new(first_use, first_use + "Map".len()));
    let second_use_range = span_to_range(source, Span::new(second_use, second_use + "Map".len()));

    assert!(decoded.contains(&(
        import_def.start.line,
        import_def.start.character,
        "Map".len() as u32,
        namespace_type,
    )));
    assert!(decoded.contains(&(
        first_use_range.start.line,
        first_use_range.start.character,
        "Map".len() as u32,
        namespace_type,
    )));
    assert!(decoded.contains(&(
        second_use_range.start.line,
        second_use_range.start.character,
        "Map".len() as u32,
        namespace_type,
    )));
}

#[test]
fn semantic_tokens_bridge_maps_import_alias_variant_surface() {
    let source = r#"
use Command as Cmd

enum Command {
    Retry(Int),
    Config { retries: Int },
}

fn build(flag: Bool) -> Command {
    if flag {
        return Cmd.Retry(1)
    }
    return Cmd.Config { retries: 2 }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let SemanticTokensResult::Tokens(tokens) = semantic_tokens_for_analysis(source, &analysis)
    else {
        panic!("expected full semantic tokens");
    };
    let decoded = decode_semantic_tokens(&tokens.data);
    let legend = semantic_tokens_legend();
    let namespace_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::NAMESPACE)
        .expect("namespace legend entry should exist") as u32;
    let enum_member_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::ENUM_MEMBER)
        .expect("enum member legend entry should exist") as u32;

    let import_range = span_to_range(source, alias_span(source, "Cmd"));
    let retry_range = span_to_range(source, nth_span(source, "Retry", 2));
    let config_range = span_to_range(source, nth_span(source, "Config", 2));

    assert!(decoded.contains(&(
        import_range.start.line,
        import_range.start.character,
        "Cmd".len() as u32,
        namespace_type,
    )));
    assert!(decoded.contains(&(
        retry_range.start.line,
        retry_range.start.character,
        "Retry".len() as u32,
        enum_member_type,
    )));
    assert!(decoded.contains(&(
        config_range.start.line,
        config_range.start.character,
        "Config".len() as u32,
        enum_member_type,
    )));
}

#[test]
fn semantic_tokens_bridge_maps_direct_variant_surface() {
    let source = r#"
enum Command {
    Retry(Int),
    Config { retries: Int },
}

fn build(flag: Bool) -> Command {
    if flag {
        return Command.Retry(1)
    }
    return Command.Config { retries: 2 }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let SemanticTokensResult::Tokens(tokens) = semantic_tokens_for_analysis(source, &analysis)
    else {
        panic!("expected full semantic tokens");
    };
    let decoded = decode_semantic_tokens(&tokens.data);
    let legend = semantic_tokens_legend();
    let enum_member_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::ENUM_MEMBER)
        .expect("enum member legend entry should exist") as u32;

    let retry_def_range = span_to_range(source, nth_span(source, "Retry", 1));
    let retry_use_range = span_to_range(source, nth_span(source, "Retry", 2));
    let config_def_range = span_to_range(source, nth_span(source, "Config", 1));
    let config_use_range = span_to_range(source, nth_span(source, "Config", 2));

    assert!(decoded.contains(&(
        retry_def_range.start.line,
        retry_def_range.start.character,
        "Retry".len() as u32,
        enum_member_type,
    )));
    assert!(decoded.contains(&(
        retry_use_range.start.line,
        retry_use_range.start.character,
        "Retry".len() as u32,
        enum_member_type,
    )));
    assert!(decoded.contains(&(
        config_def_range.start.line,
        config_def_range.start.character,
        "Config".len() as u32,
        enum_member_type,
    )));
    assert!(decoded.contains(&(
        config_use_range.start.line,
        config_use_range.start.character,
        "Config".len() as u32,
        enum_member_type,
    )));
}

#[test]
fn semantic_tokens_bridge_keeps_deeper_struct_literal_and_pattern_variant_paths_closed() {
    let source = r#"
use Command as Cmd

enum Command {
    Retry(Int),
    Config { value: Int },
    Stop,
}

fn build() -> Int {
    let direct = Command.Scope.Config { value: 1 }
    let alias = Cmd.Scope.Config { value: 2 }
    return 0
}

fn read(command: Command) -> Int {
    return match command {
        Command.Scope.Retry(value) => value,
        Cmd.Scope.Retry(value) => value,
        _ => 0,
    }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let SemanticTokensResult::Tokens(tokens) = semantic_tokens_for_analysis(source, &analysis)
    else {
        panic!("expected full semantic tokens");
    };
    let decoded = decode_semantic_tokens(&tokens.data);
    let legend = semantic_tokens_legend();
    let enum_member_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::ENUM_MEMBER)
        .expect("enum member legend entry should exist") as u32;

    for range in [
        span_to_range(source, nth_span(source, "Config", 2)),
        span_to_range(source, nth_span(source, "Config", 3)),
        span_to_range(source, nth_span(source, "Retry", 2)),
        span_to_range(source, nth_span(source, "Retry", 3)),
    ] {
        assert!(!decoded.contains(&(
            range.start.line,
            range.start.character,
            range.end.character - range.start.character,
            enum_member_type,
        )));
    }
}

#[test]
fn semantic_tokens_bridge_keeps_deeper_variant_like_member_paths_closed() {
    let source = r#"
use Command as Cmd

enum Command {
    Retry(Int),
    Stop,
}

fn main() -> Int {
    let direct = Command.Retry.Stop
    let alias = Cmd.Retry.Stop
    return 0
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let SemanticTokensResult::Tokens(tokens) = semantic_tokens_for_analysis(source, &analysis)
    else {
        panic!("expected full semantic tokens");
    };
    let decoded = decode_semantic_tokens(&tokens.data);
    let legend = semantic_tokens_legend();
    let enum_member_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::ENUM_MEMBER)
        .expect("enum member legend entry should exist") as u32;

    let direct_use_range = span_to_range(source, nth_span(source, "Stop", 2));
    let alias_use_range = span_to_range(source, nth_span(source, "Stop", 3));

    assert!(!decoded.contains(&(
        direct_use_range.start.line,
        direct_use_range.start.character,
        "Stop".len() as u32,
        enum_member_type,
    )));
    assert!(!decoded.contains(&(
        alias_use_range.start.line,
        alias_use_range.start.character,
        "Stop".len() as u32,
        enum_member_type,
    )));
}

#[test]
fn semantic_tokens_bridge_maps_import_alias_struct_field_surface() {
    let source = r#"
use Point as P

struct Point {
    x: Int,
    y: Int,
}

fn read(point: Point, value: Int) -> Int {
    let built = P { x: value, y: 1 }
    match point {
        P { x: alias, y: 2 } => alias,
    }
    return point.x
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let SemanticTokensResult::Tokens(tokens) = semantic_tokens_for_analysis(source, &analysis)
    else {
        panic!("expected full semantic tokens");
    };
    let decoded = decode_semantic_tokens(&tokens.data);
    let legend = semantic_tokens_legend();
    let namespace_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::NAMESPACE)
        .expect("namespace legend entry should exist") as u32;
    let property_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::PROPERTY)
        .expect("property legend entry should exist") as u32;

    let import_range = span_to_range(source, alias_span(source, "P"));
    let literal_field_range = span_to_range(source, nth_span(source, "x", 2));
    let pattern_field_range = span_to_range(source, nth_span(source, "x", 3));
    let member_field_range = span_to_range(source, nth_span(source, "x", 4));

    assert!(decoded.contains(&(
        import_range.start.line,
        import_range.start.character,
        "P".len() as u32,
        namespace_type,
    )));
    assert!(decoded.contains(&(
        literal_field_range.start.line,
        literal_field_range.start.character,
        "x".len() as u32,
        property_type,
    )));
    assert!(decoded.contains(&(
        pattern_field_range.start.line,
        pattern_field_range.start.character,
        "x".len() as u32,
        property_type,
    )));
    assert!(decoded.contains(&(
        member_field_range.start.line,
        member_field_range.start.character,
        "x".len() as u32,
        property_type,
    )));
}

#[test]
fn semantic_tokens_bridge_maps_direct_struct_field_surface() {
    let source = r#"
struct Point {
    x: Int,
    y: Int,
}

fn read(point: Point, value: Int) -> Int {
    let built = Point { x: value, y: 1 }
    match point {
        Point { x: alias, y: 2 } => alias,
    }
    return point.x
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let SemanticTokensResult::Tokens(tokens) = semantic_tokens_for_analysis(source, &analysis)
    else {
        panic!("expected full semantic tokens");
    };
    let decoded = decode_semantic_tokens(&tokens.data);
    let legend = semantic_tokens_legend();
    let property_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::PROPERTY)
        .expect("property legend entry should exist") as u32;

    let field_def_range = span_to_range(source, nth_span(source, "x", 1));
    let literal_field_range = span_to_range(source, nth_span(source, "x", 2));
    let pattern_field_range = span_to_range(source, nth_span(source, "x", 3));
    let member_field_range = span_to_range(source, nth_span(source, "x", 4));

    assert!(decoded.contains(&(
        field_def_range.start.line,
        field_def_range.start.character,
        "x".len() as u32,
        property_type,
    )));
    assert!(decoded.contains(&(
        literal_field_range.start.line,
        literal_field_range.start.character,
        "x".len() as u32,
        property_type,
    )));
    assert!(decoded.contains(&(
        pattern_field_range.start.line,
        pattern_field_range.start.character,
        "x".len() as u32,
        property_type,
    )));
    assert!(decoded.contains(&(
        member_field_range.start.line,
        member_field_range.start.character,
        "x".len() as u32,
        property_type,
    )));
}

#[test]
fn semantic_tokens_bridge_maps_same_file_direct_symbol_surface() {
    let source = r#"
enum Command {
    Retry(Int),
    Config { retries: Int },
}

struct Point {
    x: Int,
    y: Int,
}

fn build(flag: Bool) -> Command {
    if flag {
        return Command.Retry(1)
    }
    return Command.Config { retries: 2 }
}

fn project(point: Point, value: Int) -> Int {
    let built = Point { x: value, y: 1 }
    match point {
        Point { x: alias, y: 2 } => alias,
    }
    return 0
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let SemanticTokensResult::Tokens(tokens) = semantic_tokens_for_analysis(source, &analysis)
    else {
        panic!("expected full semantic tokens");
    };
    let decoded = decode_semantic_tokens(&tokens.data);
    let legend = semantic_tokens_legend();
    let enum_member_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::ENUM_MEMBER)
        .expect("enum member legend entry should exist") as u32;
    let property_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::PROPERTY)
        .expect("property legend entry should exist") as u32;
    let cases = [
        ("Retry", enum_member_type, vec![1, 2]),
        ("Config", enum_member_type, vec![1, 2]),
        ("x", property_type, vec![1, 2, 3]),
    ];

    for (name, token_type, occurrences) in cases {
        for occurrence in occurrences {
            let range = span_to_range(source, nth_span(source, name, occurrence));
            assert!(
                decoded.contains(&(
                    range.start.line,
                    range.start.character,
                    name.len() as u32,
                    token_type
                )),
                "{} occurrence {}",
                name,
                occurrence
            );
        }
    }
}

#[test]
fn semantic_tokens_bridge_maps_direct_member_surface() {
    let source = r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn get(self) -> Int {
        return self.value
    }

    fn read(self) -> Int {
        return self.get()
    }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let SemanticTokensResult::Tokens(tokens) = semantic_tokens_for_analysis(source, &analysis)
    else {
        panic!("expected full semantic tokens");
    };
    let decoded = decode_semantic_tokens(&tokens.data);
    let legend = semantic_tokens_legend();
    let property_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::PROPERTY)
        .expect("property legend entry should exist") as u32;
    let method_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::METHOD)
        .expect("method legend entry should exist") as u32;

    let field_def_range = span_to_range(source, nth_span(source, "value", 1));
    let field_use_range = span_to_range(source, nth_span(source, "value", 2));
    let method_def_range = span_to_range(source, nth_span(source, "get", 1));
    let method_use_range = span_to_range(source, nth_span(source, "get", 2));

    assert!(decoded.contains(&(
        field_def_range.start.line,
        field_def_range.start.character,
        "value".len() as u32,
        property_type,
    )));
    assert!(decoded.contains(&(
        field_use_range.start.line,
        field_use_range.start.character,
        "value".len() as u32,
        property_type,
    )));
    assert!(decoded.contains(&(
        method_def_range.start.line,
        method_def_range.start.character,
        "get".len() as u32,
        method_type,
    )));
    assert!(decoded.contains(&(
        method_use_range.start.line,
        method_use_range.start.character,
        "get".len() as u32,
        method_type,
    )));
}

#[test]
fn semantic_tokens_bridge_maps_same_file_direct_member_surface() {
    let source = r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn get(self) -> Int {
        return self.value
    }

    fn read(self) -> Int {
        return self.get()
    }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let SemanticTokensResult::Tokens(tokens) = semantic_tokens_for_analysis(source, &analysis)
    else {
        panic!("expected full semantic tokens");
    };
    let decoded = decode_semantic_tokens(&tokens.data);
    let legend = semantic_tokens_legend();
    let property_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::PROPERTY)
        .expect("property legend entry should exist") as u32;
    let method_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::METHOD)
        .expect("method legend entry should exist") as u32;
    let cases = [
        ("value", property_type, vec![1, 2]),
        ("get", method_type, vec![1, 2]),
    ];

    for (name, token_type, occurrences) in cases {
        for occurrence in occurrences {
            let range = span_to_range(source, nth_span(source, name, occurrence));
            assert!(
                decoded.contains(&(
                    range.start.line,
                    range.start.character,
                    name.len() as u32,
                    token_type
                )),
                "{} occurrence {}",
                name,
                occurrence
            );
        }
    }
}

#[test]
fn semantic_tokens_bridge_maps_type_namespace_item_surface() {
    let source = r#"
type IdAlias = Int

struct Account {
    id: IdAlias,
}

enum Mode {
    Ready,
}

trait Taggable {
    fn mode(self) -> Mode
}

impl Taggable for Account {
    fn mode(self) -> Mode {
        return Mode.Ready
    }
}

fn build(account: Account, value: IdAlias) -> Mode {
    let copy = value
    return account.mode()
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let SemanticTokensResult::Tokens(tokens) = semantic_tokens_for_analysis(source, &analysis)
    else {
        panic!("expected full semantic tokens");
    };
    let decoded = decode_semantic_tokens(&tokens.data);
    let legend = semantic_tokens_legend();
    let class_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::CLASS)
        .expect("class legend entry should exist") as u32;
    let type_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::TYPE)
        .expect("type legend entry should exist") as u32;
    let enum_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::ENUM)
        .expect("enum legend entry should exist") as u32;
    let interface_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::INTERFACE)
        .expect("interface legend entry should exist") as u32;

    let type_alias_def = span_to_range(source, nth_span(source, "IdAlias", 1));
    let type_alias_use = span_to_range(source, nth_span(source, "IdAlias", 2));
    let struct_def = span_to_range(source, nth_span(source, "Account", 1));
    let struct_use = span_to_range(source, nth_span(source, "Account", 2));
    let enum_def = span_to_range(source, nth_span(source, "Mode", 1));
    let enum_use = span_to_range(source, nth_span(source, "Mode", 2));
    let trait_def = span_to_range(source, nth_span(source, "Taggable", 1));
    let trait_use = span_to_range(source, nth_span(source, "Taggable", 2));

    assert!(decoded.contains(&(
        type_alias_def.start.line,
        type_alias_def.start.character,
        "IdAlias".len() as u32,
        type_type
    )));
    assert!(decoded.contains(&(
        type_alias_use.start.line,
        type_alias_use.start.character,
        "IdAlias".len() as u32,
        type_type
    )));
    assert!(decoded.contains(&(
        struct_def.start.line,
        struct_def.start.character,
        "Account".len() as u32,
        class_type
    )));
    assert!(decoded.contains(&(
        struct_use.start.line,
        struct_use.start.character,
        "Account".len() as u32,
        class_type
    )));
    assert!(decoded.contains(&(
        enum_def.start.line,
        enum_def.start.character,
        "Mode".len() as u32,
        enum_type
    )));
    assert!(decoded.contains(&(
        enum_use.start.line,
        enum_use.start.character,
        "Mode".len() as u32,
        enum_type
    )));
    assert!(decoded.contains(&(
        trait_def.start.line,
        trait_def.start.character,
        "Taggable".len() as u32,
        interface_type
    )));
    assert!(decoded.contains(&(
        trait_use.start.line,
        trait_use.start.character,
        "Taggable".len() as u32,
        interface_type
    )));
}

#[test]
fn semantic_tokens_bridge_maps_opaque_type_surface() {
    let source = r#"
opaque type UserId = Int

struct Account {
    id: UserId,
}

fn build(value: UserId) -> UserId {
    return value
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let SemanticTokensResult::Tokens(tokens) = semantic_tokens_for_analysis(source, &analysis)
    else {
        panic!("expected full semantic tokens");
    };
    let decoded = decode_semantic_tokens(&tokens.data);
    let legend = semantic_tokens_legend();
    let type_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::TYPE)
        .expect("type legend entry should exist") as u32;

    for occurrence in 1..=4 {
        let range = span_to_range(source, nth_span(source, "UserId", occurrence));
        assert!(decoded.contains(&(
            range.start.line,
            range.start.character,
            "UserId".len() as u32,
            type_type
        )));
    }
}

#[test]
fn semantic_tokens_bridge_maps_same_file_type_namespace_item_surface() {
    let source = r#"
type IdAlias = Int

opaque type UserId = Int

struct Account {
    id: UserId,
    alias: IdAlias,
}

enum Mode {
    Ready,
}

trait Taggable {
    fn mode(self) -> Mode
}

impl Taggable for Account {
    fn mode(self) -> Mode {
        return Mode.Ready
    }
}

fn build(account: Account, user_id: UserId, alias: IdAlias) -> Mode {
    let copied_id = user_id
    let copied_alias = alias
    return account.mode()
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let SemanticTokensResult::Tokens(tokens) = semantic_tokens_for_analysis(source, &analysis)
    else {
        panic!("expected full semantic tokens");
    };
    let decoded = decode_semantic_tokens(&tokens.data);
    let legend = semantic_tokens_legend();
    let class_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::CLASS)
        .expect("class legend entry should exist") as u32;
    let type_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::TYPE)
        .expect("type legend entry should exist") as u32;
    let enum_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::ENUM)
        .expect("enum legend entry should exist") as u32;
    let interface_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::INTERFACE)
        .expect("interface legend entry should exist") as u32;
    let cases = [
        ("IdAlias", type_type, vec![1, 2, 3]),
        ("UserId", type_type, vec![1, 2, 3]),
        ("Account", class_type, vec![1, 2, 3]),
        ("Mode", enum_type, vec![1, 2, 3, 4, 5]),
        ("Taggable", interface_type, vec![1, 2]),
    ];

    for (name, token_type, occurrences) in cases {
        for occurrence in occurrences {
            let range = span_to_range(source, nth_span(source, name, occurrence));
            assert!(
                decoded.contains(&(
                    range.start.line,
                    range.start.character,
                    name.len() as u32,
                    token_type
                )),
                "{} occurrence {}",
                name,
                occurrence
            );
        }
    }
}

#[test]
fn semantic_tokens_bridge_maps_global_value_item_surface() {
    let source = r#"
const LIMIT: Int = 10

static CURRENT: Int = LIMIT

fn read() -> Int {
    let snapshot = CURRENT
    return LIMIT
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let SemanticTokensResult::Tokens(tokens) = semantic_tokens_for_analysis(source, &analysis)
    else {
        panic!("expected full semantic tokens");
    };
    let decoded = decode_semantic_tokens(&tokens.data);
    let legend = semantic_tokens_legend();
    let function_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::FUNCTION)
        .expect("function legend entry should exist") as u32;

    let const_def = span_to_range(source, nth_span(source, "LIMIT", 1));
    let const_static_use = span_to_range(source, nth_span(source, "LIMIT", 2));
    let const_return_use = span_to_range(source, nth_span(source, "LIMIT", 3));
    let static_def = span_to_range(source, nth_span(source, "CURRENT", 1));
    let static_use = span_to_range(source, nth_span(source, "CURRENT", 2));

    assert!(decoded.contains(&(
        const_def.start.line,
        const_def.start.character,
        "LIMIT".len() as u32,
        function_type
    )));
    assert!(decoded.contains(&(
        const_static_use.start.line,
        const_static_use.start.character,
        "LIMIT".len() as u32,
        function_type
    )));
    assert!(decoded.contains(&(
        const_return_use.start.line,
        const_return_use.start.character,
        "LIMIT".len() as u32,
        function_type
    )));
    assert!(decoded.contains(&(
        static_def.start.line,
        static_def.start.character,
        "CURRENT".len() as u32,
        function_type
    )));
    assert!(decoded.contains(&(
        static_use.start.line,
        static_use.start.character,
        "CURRENT".len() as u32,
        function_type
    )));
}

#[test]
fn semantic_tokens_bridge_maps_extern_block_function_surface() {
    let source = r#"
extern "c" {
    fn q_add(left: Int, right: Int) -> Int
}

fn main() -> Int {
    return q_add(1, 2)
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let SemanticTokensResult::Tokens(tokens) = semantic_tokens_for_analysis(source, &analysis)
    else {
        panic!("expected full semantic tokens");
    };
    let decoded = decode_semantic_tokens(&tokens.data);
    let legend = semantic_tokens_legend();
    let function_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::FUNCTION)
        .expect("function legend entry should exist") as u32;

    let extern_def = span_to_range(source, nth_span(source, "q_add", 1));
    let extern_use = span_to_range(source, nth_span(source, "q_add", 2));

    assert!(decoded.contains(&(
        extern_def.start.line,
        extern_def.start.character,
        "q_add".len() as u32,
        function_type
    )));
    assert!(decoded.contains(&(
        extern_use.start.line,
        extern_use.start.character,
        "q_add".len() as u32,
        function_type
    )));
}

#[test]
fn semantic_tokens_bridge_maps_top_level_extern_function_surface() {
    let source = r#"
extern "c" fn q_add(left: Int, right: Int) -> Int

fn main() -> Int {
    return q_add(1, 2)
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");

    let SemanticTokensResult::Tokens(tokens) = semantic_tokens_for_analysis(source, &analysis)
    else {
        panic!("expected full semantic tokens");
    };
    let decoded = decode_semantic_tokens(&tokens.data);
    let legend = semantic_tokens_legend();
    let function_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::FUNCTION)
        .expect("function legend entry should exist") as u32;

    let extern_def = span_to_range(source, nth_span(source, "q_add", 1));
    let extern_use = span_to_range(source, nth_span(source, "q_add", 2));

    assert!(decoded.contains(&(
        extern_def.start.line,
        extern_def.start.character,
        "q_add".len() as u32,
        function_type
    )));
    assert!(decoded.contains(&(
        extern_use.start.line,
        extern_use.start.character,
        "q_add".len() as u32,
        function_type
    )));
}

#[test]
fn semantic_tokens_bridge_maps_top_level_extern_function_definition_surface() {
    let source = r#"
extern "c" fn q_add(left: Int, right: Int) -> Int {
    return left + right
}

fn main() -> Int {
    return q_add(1, 2)
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");

    let SemanticTokensResult::Tokens(tokens) = semantic_tokens_for_analysis(source, &analysis)
    else {
        panic!("expected full semantic tokens");
    };
    let decoded = decode_semantic_tokens(&tokens.data);
    let legend = semantic_tokens_legend();
    let function_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::FUNCTION)
        .expect("function legend entry should exist") as u32;

    let extern_def = span_to_range(source, nth_span(source, "q_add", 1));
    let extern_use = span_to_range(source, nth_span(source, "q_add", 2));

    assert!(decoded.contains(&(
        extern_def.start.line,
        extern_def.start.character,
        "q_add".len() as u32,
        function_type
    )));
    assert!(decoded.contains(&(
        extern_use.start.line,
        extern_use.start.character,
        "q_add".len() as u32,
        function_type
    )));
}

#[test]
fn semantic_tokens_bridge_maps_free_function_surface() {
    let source = r#"
fn helper(value: Int) -> Int {
    return value
}

fn compute() -> Int {
    return helper(1) + helper(2)
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let SemanticTokensResult::Tokens(tokens) = semantic_tokens_for_analysis(source, &analysis)
    else {
        panic!("expected full semantic tokens");
    };
    let decoded = decode_semantic_tokens(&tokens.data);
    let legend = semantic_tokens_legend();
    let function_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::FUNCTION)
        .expect("function legend entry should exist") as u32;

    let function_def = span_to_range(source, nth_span(source, "helper", 1));
    let first_use = span_to_range(source, nth_span(source, "helper", 2));
    let second_use = span_to_range(source, nth_span(source, "helper", 3));

    assert!(decoded.contains(&(
        function_def.start.line,
        function_def.start.character,
        "helper".len() as u32,
        function_type
    )));
    assert!(decoded.contains(&(
        first_use.start.line,
        first_use.start.character,
        "helper".len() as u32,
        function_type
    )));
    assert!(decoded.contains(&(
        second_use.start.line,
        second_use.start.character,
        "helper".len() as u32,
        function_type
    )));
}

#[test]
fn semantic_tokens_bridge_maps_same_file_callable_surface() {
    let source = r#"
extern "c" {
    fn q_add(left: Int, right: Int) -> Int
}

extern "c" fn q_sub(left: Int, right: Int) -> Int

extern "c" fn q_mul(left: Int, right: Int) -> Int {
    return left * right
}

fn helper(value: Int) -> Int {
    return value
}

fn compute() -> Int {
    return q_add(1, 2) + q_sub(1, 2) + q_mul(1, 2) + helper(1) + helper(2)
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let SemanticTokensResult::Tokens(tokens) = semantic_tokens_for_analysis(source, &analysis)
    else {
        panic!("expected full semantic tokens");
    };
    let decoded = decode_semantic_tokens(&tokens.data);
    let legend = semantic_tokens_legend();
    let function_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::FUNCTION)
        .expect("function legend entry should exist") as u32;
    let cases = [
        ("q_add", vec![1, 2]),
        ("q_sub", vec![1, 2]),
        ("q_mul", vec![1, 2]),
        ("helper", vec![1, 2, 3]),
    ];

    for (name, occurrences) in cases {
        for occurrence in occurrences {
            let range = span_to_range(source, nth_span(source, name, occurrence));
            assert!(
                decoded.contains(&(
                    range.start.line,
                    range.start.character,
                    name.len() as u32,
                    function_type
                )),
                "{} occurrence {}",
                name,
                occurrence
            );
        }
    }
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
fn rename_bridge_supports_function_const_and_static_symbols() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
const LIMIT: Int = 10

static CURRENT: Int = LIMIT

fn compute(value: Int) -> Int {
    return value + LIMIT
}

fn read() -> Int {
    return compute(CURRENT) + LIMIT
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let function_position = span_to_range(source, nth_span(source, "compute", 2)).start;
    let const_position = span_to_range(source, nth_span(source, "LIMIT", 4)).start;
    let static_position = span_to_range(source, nth_span(source, "CURRENT", 2)).start;

    let prepare = prepare_rename_for_analysis(source, &analysis, function_position)
        .expect("function prepare rename should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(range, span_to_range(source, nth_span(source, "compute", 2)));
    assert_eq!(placeholder, "compute");

    let edit = rename_for_analysis(&uri, source, &analysis, function_position, "measure")
        .expect("rename should validate")
        .expect("rename should produce edits");
    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri.clone(),
        vec![
            TextEdit::new(
                span_to_range(source, nth_span(source, "compute", 1)),
                "measure".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "compute", 2)),
                "measure".to_owned(),
            ),
        ],
    );
    assert_eq!(edit, WorkspaceEdit::new(expected_changes));

    let prepare = prepare_rename_for_analysis(source, &analysis, const_position)
        .expect("const prepare rename should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(range, span_to_range(source, nth_span(source, "LIMIT", 4)));
    assert_eq!(placeholder, "LIMIT");

    let edit = rename_for_analysis(&uri, source, &analysis, const_position, "MAX_LIMIT")
        .expect("rename should validate")
        .expect("rename should produce edits");
    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri.clone(),
        vec![
            TextEdit::new(
                span_to_range(source, nth_span(source, "LIMIT", 1)),
                "MAX_LIMIT".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "LIMIT", 2)),
                "MAX_LIMIT".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "LIMIT", 3)),
                "MAX_LIMIT".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "LIMIT", 4)),
                "MAX_LIMIT".to_owned(),
            ),
        ],
    );
    assert_eq!(edit, WorkspaceEdit::new(expected_changes));

    let prepare = prepare_rename_for_analysis(source, &analysis, static_position)
        .expect("static prepare rename should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(range, span_to_range(source, nth_span(source, "CURRENT", 2)));
    assert_eq!(placeholder, "CURRENT");

    let edit = rename_for_analysis(&uri, source, &analysis, static_position, "LATEST")
        .expect("rename should validate")
        .expect("rename should produce edits");
    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri,
        vec![
            TextEdit::new(
                span_to_range(source, nth_span(source, "CURRENT", 1)),
                "LATEST".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "CURRENT", 2)),
                "LATEST".to_owned(),
            ),
        ],
    );
    assert_eq!(edit, WorkspaceEdit::new(expected_changes));
}

#[test]
fn rename_bridge_supports_extern_block_function_symbols() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
extern "c" {
    fn q_add(left: Int, right: Int) -> Int
}

fn main() -> Int {
    return q_add(1, 2)
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let extern_position = span_to_range(source, nth_span(source, "q_add", 2)).start;

    let prepare = prepare_rename_for_analysis(source, &analysis, extern_position)
        .expect("extern function prepare rename should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(range, span_to_range(source, nth_span(source, "q_add", 2)));
    assert_eq!(placeholder, "q_add");

    let edit = rename_for_analysis(&uri, source, &analysis, extern_position, "q_sum")
        .expect("rename should validate")
        .expect("rename should produce edits");

    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri,
        vec![
            TextEdit::new(
                span_to_range(source, nth_span(source, "q_add", 1)),
                "q_sum".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "q_add", 2)),
                "q_sum".to_owned(),
            ),
        ],
    );

    assert_eq!(edit, WorkspaceEdit::new(expected_changes));
}

#[test]
fn rename_bridge_supports_top_level_extern_function_symbols() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
extern "c" fn q_add(left: Int, right: Int) -> Int

fn main() -> Int {
    return q_add(1, 2)
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let extern_position = span_to_range(source, nth_span(source, "q_add", 2)).start;

    let prepare = prepare_rename_for_analysis(source, &analysis, extern_position)
        .expect("extern function prepare rename should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(range, span_to_range(source, nth_span(source, "q_add", 2)));
    assert_eq!(placeholder, "q_add");

    let edit = rename_for_analysis(&uri, source, &analysis, extern_position, "q_sum")
        .expect("rename should validate")
        .expect("rename should produce edits");

    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri,
        vec![
            TextEdit::new(
                span_to_range(source, nth_span(source, "q_add", 1)),
                "q_sum".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "q_add", 2)),
                "q_sum".to_owned(),
            ),
        ],
    );

    assert_eq!(edit, WorkspaceEdit::new(expected_changes));
}

#[test]
fn rename_bridge_supports_top_level_extern_function_definition_symbols() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
extern "c" fn q_add(left: Int, right: Int) -> Int {
    return left + right
}

fn main() -> Int {
    return q_add(1, 2)
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let extern_position = span_to_range(source, nth_span(source, "q_add", 2)).start;

    let prepare = prepare_rename_for_analysis(source, &analysis, extern_position)
        .expect("extern definition prepare rename should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(range, span_to_range(source, nth_span(source, "q_add", 2)));
    assert_eq!(placeholder, "q_add");

    let edit = rename_for_analysis(&uri, source, &analysis, extern_position, "q_sum")
        .expect("rename should validate")
        .expect("rename should produce edits");

    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri,
        vec![
            TextEdit::new(
                span_to_range(source, nth_span(source, "q_add", 1)),
                "q_sum".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "q_add", 2)),
                "q_sum".to_owned(),
            ),
        ],
    );

    assert_eq!(edit, WorkspaceEdit::new(expected_changes));
}

#[test]
fn rename_bridge_supports_lexical_semantic_symbols_and_keeps_closed_surfaces_closed() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
fn id[T](param: T) -> T {
    let local_value = param
    return local_value
}

struct Counter {
    value: String,
}

impl Counter {
    fn read(self, input: String) -> String {
        let alias = input
        return self.value
    }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let generic_position = span_to_range(source, nth_span(source, "T", 3)).start;
    let parameter_position = span_to_range(source, nth_span(source, "param", 2)).start;
    let local_position = span_to_range(source, nth_span(source, "local_value", 2)).start;
    let self_position = span_to_range(source, nth_span(source, "self", 2)).start;
    let builtin_position = span_to_range(source, nth_span(source, "String", 2)).start;

    let prepare = prepare_rename_for_analysis(source, &analysis, generic_position)
        .expect("generic prepare rename should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(range, span_to_range(source, nth_span(source, "T", 3)));
    assert_eq!(placeholder, "T");

    let edit = rename_for_analysis(&uri, source, &analysis, generic_position, "Value")
        .expect("rename should validate")
        .expect("rename should produce edits");
    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri.clone(),
        vec![
            TextEdit::new(
                span_to_range(source, nth_span(source, "T", 1)),
                "Value".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "T", 2)),
                "Value".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "T", 3)),
                "Value".to_owned(),
            ),
        ],
    );
    assert_eq!(edit, WorkspaceEdit::new(expected_changes));

    let prepare = prepare_rename_for_analysis(source, &analysis, parameter_position)
        .expect("parameter prepare rename should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(range, span_to_range(source, nth_span(source, "param", 2)));
    assert_eq!(placeholder, "param");

    let edit = rename_for_analysis(&uri, source, &analysis, parameter_position, "input_value")
        .expect("rename should validate")
        .expect("rename should produce edits");
    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri.clone(),
        vec![
            TextEdit::new(
                span_to_range(source, nth_span(source, "param", 1)),
                "input_value".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "param", 2)),
                "input_value".to_owned(),
            ),
        ],
    );
    assert_eq!(edit, WorkspaceEdit::new(expected_changes));

    let prepare = prepare_rename_for_analysis(source, &analysis, local_position)
        .expect("local prepare rename should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(
        range,
        span_to_range(source, nth_span(source, "local_value", 2))
    );
    assert_eq!(placeholder, "local_value");

    let edit = rename_for_analysis(&uri, source, &analysis, local_position, "result_value")
        .expect("rename should validate")
        .expect("rename should produce edits");
    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri.clone(),
        vec![
            TextEdit::new(
                span_to_range(source, nth_span(source, "local_value", 1)),
                "result_value".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "local_value", 2)),
                "result_value".to_owned(),
            ),
        ],
    );
    assert_eq!(edit, WorkspaceEdit::new(expected_changes));

    assert_eq!(
        prepare_rename_for_analysis(source, &analysis, self_position),
        None
    );
    assert_eq!(
        rename_for_analysis(&uri, source, &analysis, self_position, "owner")
            .expect("rename should validate"),
        None
    );

    assert_eq!(
        prepare_rename_for_analysis(source, &analysis, builtin_position),
        None
    );
    assert_eq!(
        rename_for_analysis(&uri, source, &analysis, builtin_position, "Text")
            .expect("rename should validate"),
        None
    );
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

#[test]
fn rename_bridge_supports_type_namespace_item_symbols() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
type UserId = Int

struct User {
    id: UserId,
}

enum Status {
    Active,
}

trait Named {
    fn id(self) -> UserId
}

impl Named for User {
    fn id(self) -> UserId {
        return self.id
    }
}

fn active(user: User, current: UserId) -> Status {
    let next = current
    return Status.Active
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");

    let type_alias_use = span_to_range(source, nth_span(source, "UserId", 2)).start;
    let prepare = prepare_rename_for_analysis(source, &analysis, type_alias_use)
        .expect("type alias prepare rename should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(range, span_to_range(source, nth_span(source, "UserId", 2)));
    assert_eq!(placeholder, "UserId");

    let type_alias_edit = rename_for_analysis(&uri, source, &analysis, type_alias_use, "AccountId")
        .expect("rename should validate")
        .expect("rename should produce edits");
    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri.clone(),
        vec![
            TextEdit::new(
                span_to_range(source, nth_span(source, "UserId", 1)),
                "AccountId".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "UserId", 2)),
                "AccountId".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "UserId", 3)),
                "AccountId".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "UserId", 4)),
                "AccountId".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "UserId", 5)),
                "AccountId".to_owned(),
            ),
        ],
    );
    assert_eq!(type_alias_edit, WorkspaceEdit::new(expected_changes));

    let struct_def = source
        .find("struct User")
        .map(|offset| offset + "struct ".len())
        .expect("struct definition should exist");
    let struct_use = source
        .find("for User")
        .map(|offset| offset + "for ".len())
        .expect("struct use in impl header should exist");
    let struct_param_use = source
        .find("user: User")
        .map(|offset| offset + "user: ".len())
        .expect("struct use in function parameter should exist");
    let struct_position =
        span_to_range(source, Span::new(struct_use, struct_use + "User".len())).start;
    let prepare = prepare_rename_for_analysis(source, &analysis, struct_position)
        .expect("struct prepare rename should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(
        range,
        span_to_range(source, Span::new(struct_use, struct_use + "User".len()))
    );
    assert_eq!(placeholder, "User");

    let struct_edit = rename_for_analysis(&uri, source, &analysis, struct_position, "Member")
        .expect("rename should validate")
        .expect("rename should produce edits");
    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri.clone(),
        vec![
            TextEdit::new(
                span_to_range(source, Span::new(struct_def, struct_def + "User".len())),
                "Member".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, Span::new(struct_use, struct_use + "User".len())),
                "Member".to_owned(),
            ),
            TextEdit::new(
                span_to_range(
                    source,
                    Span::new(struct_param_use, struct_param_use + "User".len()),
                ),
                "Member".to_owned(),
            ),
        ],
    );
    assert_eq!(struct_edit, WorkspaceEdit::new(expected_changes));

    let enum_use = source
        .find("-> Status")
        .map(|offset| offset + 3)
        .expect("enum use in return type should exist");
    let enum_position = span_to_range(source, Span::new(enum_use, enum_use + "Status".len())).start;
    let prepare = prepare_rename_for_analysis(source, &analysis, enum_position)
        .expect("enum prepare rename should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(
        range,
        span_to_range(source, Span::new(enum_use, enum_use + "Status".len()))
    );
    assert_eq!(placeholder, "Status");

    let enum_edit = rename_for_analysis(&uri, source, &analysis, enum_position, "Phase")
        .expect("rename should validate")
        .expect("rename should produce edits");
    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri.clone(),
        vec![
            TextEdit::new(
                span_to_range(source, nth_span(source, "Status", 1)),
                "Phase".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "Status", 2)),
                "Phase".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "Status", 3)),
                "Phase".to_owned(),
            ),
        ],
    );
    assert_eq!(enum_edit, WorkspaceEdit::new(expected_changes));

    let trait_use = source
        .find("impl Named")
        .map(|offset| offset + "impl ".len())
        .expect("trait use in impl header should exist");
    let trait_position =
        span_to_range(source, Span::new(trait_use, trait_use + "Named".len())).start;
    let prepare = prepare_rename_for_analysis(source, &analysis, trait_position)
        .expect("trait prepare rename should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(
        range,
        span_to_range(source, Span::new(trait_use, trait_use + "Named".len()))
    );
    assert_eq!(placeholder, "Named");

    let trait_edit = rename_for_analysis(&uri, source, &analysis, trait_position, "Identified")
        .expect("rename should validate")
        .expect("rename should produce edits");
    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri,
        vec![
            TextEdit::new(
                span_to_range(source, nth_span(source, "Named", 1)),
                "Identified".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "Named", 2)),
                "Identified".to_owned(),
            ),
        ],
    );
    assert_eq!(trait_edit, WorkspaceEdit::new(expected_changes));
}

#[test]
fn rename_bridge_supports_opaque_type_symbols() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
opaque type UserId = Int

struct Account {
    id: UserId,
}

fn build(value: UserId) -> UserId {
    return value
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let opaque_position = span_to_range(source, nth_span(source, "UserId", 2)).start;

    let prepare = prepare_rename_for_analysis(source, &analysis, opaque_position)
        .expect("opaque type prepare rename should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(range, span_to_range(source, nth_span(source, "UserId", 2)));
    assert_eq!(placeholder, "UserId");

    let edit = rename_for_analysis(&uri, source, &analysis, opaque_position, "AccountId")
        .expect("rename should validate")
        .expect("rename should produce edits");

    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri,
        vec![
            TextEdit::new(
                span_to_range(source, nth_span(source, "UserId", 1)),
                "AccountId".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "UserId", 2)),
                "AccountId".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "UserId", 3)),
                "AccountId".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "UserId", 4)),
                "AccountId".to_owned(),
            ),
        ],
    );

    assert_eq!(edit, WorkspaceEdit::new(expected_changes));
}

#[test]
fn rename_bridge_supports_variants_through_import_alias_paths() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
use Command as Cmd

enum Command {
    Retry(Int),
    Stop,
}

fn build(flag: Bool) -> Command {
    if flag {
        return Cmd.Retry(1)
    }
    return Cmd.Stop
}

fn read(command: Command) -> Int {
    match command {
        Cmd.Retry(times) => times,
        Cmd.Stop => 0,
    }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let retry_use = span_to_range(source, nth_span(source, "Retry", 2)).start;

    let prepare = prepare_rename_for_analysis(source, &analysis, retry_use)
        .expect("variant prepare rename through import alias should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(range, span_to_range(source, nth_span(source, "Retry", 2)));
    assert_eq!(placeholder, "Retry");

    let edit = rename_for_analysis(&uri, source, &analysis, retry_use, "Repeat")
        .expect("rename should validate")
        .expect("rename should produce edits");

    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri,
        vec![
            TextEdit::new(
                span_to_range(source, nth_span(source, "Retry", 1)),
                "Repeat".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "Retry", 2)),
                "Repeat".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "Retry", 3)),
                "Repeat".to_owned(),
            ),
        ],
    );

    assert_eq!(edit, WorkspaceEdit::new(expected_changes));
}

#[test]
fn rename_bridge_keeps_deeper_variant_like_member_paths_closed() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
use Command as Cmd

enum Command {
    Retry(Int),
    Stop,
}

fn main() -> Int {
    let direct = Command.Retry.Stop
    let alias = Cmd.Retry.Stop
    return 0
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let direct_use = span_to_range(source, nth_span(source, "Stop", 2)).start;
    let alias_use = span_to_range(source, nth_span(source, "Stop", 3)).start;

    assert_eq!(
        prepare_rename_for_analysis(source, &analysis, direct_use),
        None
    );
    assert_eq!(
        rename_for_analysis(&uri, source, &analysis, direct_use, "Halt"),
        Ok(None)
    );

    assert_eq!(
        prepare_rename_for_analysis(source, &analysis, alias_use),
        None
    );
    assert_eq!(
        rename_for_analysis(&uri, source, &analysis, alias_use, "Halt"),
        Ok(None)
    );
}

#[test]
fn rename_bridge_supports_struct_field_labels_through_import_alias_paths() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
use Point as P

struct Point {
    x: Int,
    y: Int,
}

fn read(point: Point, value: Int) -> Int {
    let built = P { x: value, y: 1 }
    match point {
        P { x: alias, y: 2 } => alias,
    }
    return point.x
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let field_position = span_to_range(source, nth_span(source, "x", 2)).start;

    let prepare = prepare_rename_for_analysis(source, &analysis, field_position)
        .expect("field prepare rename through import alias should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(range, span_to_range(source, nth_span(source, "x", 2)));
    assert_eq!(placeholder, "x");

    let edit = rename_for_analysis(&uri, source, &analysis, field_position, "coord_x")
        .expect("rename should validate")
        .expect("rename should produce edits");

    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri,
        vec![
            TextEdit::new(
                span_to_range(source, nth_span(source, "x", 1)),
                "coord_x".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "x", 2)),
                "coord_x".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "x", 3)),
                "coord_x".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "x", 4)),
                "coord_x".to_owned(),
            ),
        ],
    );

    assert_eq!(edit, WorkspaceEdit::new(expected_changes));
}

#[test]
fn rename_bridge_supports_unique_method_symbols() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
struct Counter {
    total: Int,
}

impl Counter {
    fn read(self) -> Int {
        return self.total
    }
}

fn main(counter: Counter) -> Int {
    return counter.read()
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let method_use = span_to_range(source, nth_span(source, "read", 2)).start;

    let prepare = prepare_rename_for_analysis(source, &analysis, method_use)
        .expect("method prepare rename should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(range, span_to_range(source, nth_span(source, "read", 2)));
    assert_eq!(placeholder, "read");

    let edit = rename_for_analysis(&uri, source, &analysis, method_use, "fetch")
        .expect("rename should validate")
        .expect("rename should produce edits");

    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri,
        vec![
            TextEdit::new(
                span_to_range(source, nth_span(source, "read", 1)),
                "fetch".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "read", 2)),
                "fetch".to_owned(),
            ),
        ],
    );

    assert_eq!(edit, WorkspaceEdit::new(expected_changes));
}

#[test]
fn rename_bridge_keeps_ambiguous_method_surfaces_closed() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
struct Counter {
    value: Int,
}

extend Counter {
    fn ping(self) -> Int {
        return self.value
    }
}

extend Counter {
    fn ping(self, delta: Int) -> Int {
        return self.value + delta
    }
}

fn main(counter: Counter) -> Int {
    return counter.ping()
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let method_use = span_to_range(source, nth_span(source, "ping", 3)).start;

    assert_eq!(
        prepare_rename_for_analysis(source, &analysis, method_use),
        None
    );
    assert_eq!(
        rename_for_analysis(&uri, source, &analysis, method_use, "pong"),
        Ok(None)
    );
}

#[test]
fn rename_bridge_expands_shorthand_field_sites() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
struct Point {
    x: Int,
}

fn read(point: Point, value: Int) -> Int {
    let x = value
    let built = Point { x }
    match point {
        Point { x } => x,
    }
    return point.x
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let literal_shorthand = source
        .find("{ x }")
        .map(|offset| offset + 2)
        .expect("shorthand struct literal field should exist");
    let pattern_shorthand = source
        .rfind("{ x }")
        .map(|offset| offset + 2)
        .expect("shorthand struct pattern field should exist");
    let member_use = source
        .rfind(".x")
        .map(|offset| offset + 1)
        .expect("field member use should exist");
    let member_position =
        span_to_range(source, Span::new(member_use, member_use + "x".len())).start;

    let prepare = prepare_rename_for_analysis(source, &analysis, member_position)
        .expect("field prepare rename should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(
        range,
        span_to_range(source, Span::new(member_use, member_use + "x".len()))
    );
    assert_eq!(placeholder, "x");

    let edit = rename_for_analysis(&uri, source, &analysis, member_position, "coord_x")
        .expect("rename should validate")
        .expect("rename should produce edits");

    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri,
        vec![
            TextEdit::new(
                span_to_range(source, nth_span(source, "x", 1)),
                "coord_x".to_owned(),
            ),
            TextEdit::new(
                span_to_range(
                    source,
                    Span::new(literal_shorthand, literal_shorthand + "x".len()),
                ),
                "coord_x: x".to_owned(),
            ),
            TextEdit::new(
                span_to_range(
                    source,
                    Span::new(pattern_shorthand, pattern_shorthand + "x".len()),
                ),
                "coord_x: x".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, Span::new(member_use, member_use + "x".len())),
                "coord_x".to_owned(),
            ),
        ],
    );

    assert_eq!(edit, WorkspaceEdit::new(expected_changes));
}

#[test]
fn rename_bridge_preserves_shorthand_binding_sites() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
struct Point {
    x: Int,
}

fn read(value: Int) -> Int {
    let x = value
    let built = Point { x }
    return x
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let literal_shorthand = source
        .find("{ x }")
        .map(|offset| offset + 2)
        .expect("shorthand struct literal field should exist");
    let literal_position = span_to_range(
        source,
        Span::new(literal_shorthand, literal_shorthand + "x".len()),
    )
    .start;

    let prepare = prepare_rename_for_analysis(source, &analysis, literal_position)
        .expect("local prepare rename through shorthand site should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(
        range,
        span_to_range(
            source,
            Span::new(literal_shorthand, literal_shorthand + "x".len()),
        )
    );
    assert_eq!(placeholder, "x");

    let edit = rename_for_analysis(&uri, source, &analysis, literal_position, "coord_x")
        .expect("rename should validate")
        .expect("rename should produce edits");

    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri,
        vec![
            TextEdit::new(
                span_to_range(source, nth_span(source, "x", 2)),
                "coord_x".to_owned(),
            ),
            TextEdit::new(
                span_to_range(
                    source,
                    Span::new(literal_shorthand, literal_shorthand + "x".len()),
                ),
                "x: coord_x".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, nth_span(source, "x", 4)),
                "coord_x".to_owned(),
            ),
        ],
    );

    assert_eq!(edit, WorkspaceEdit::new(expected_changes));
}

#[test]
fn rename_bridge_preserves_import_alias_shorthand_binding_sites() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
use source_value as source

struct Point {
    source: Int,
}

const source_value: Int = 1

fn read() -> Int {
    let built = Point { source }
    return source
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let literal_shorthand = source
        .find("{ source }")
        .map(|offset| offset + 2)
        .expect("import-alias shorthand struct literal field should exist");
    let return_source = source
        .rfind("return source")
        .map(|offset| offset + "return ".len())
        .expect("return source use should exist");
    let literal_position = span_to_range(
        source,
        Span::new(literal_shorthand, literal_shorthand + "source".len()),
    )
    .start;

    let prepare = prepare_rename_for_analysis(source, &analysis, literal_position)
        .expect("import alias prepare rename through shorthand site should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(
        range,
        span_to_range(
            source,
            Span::new(literal_shorthand, literal_shorthand + "source".len()),
        )
    );
    assert_eq!(placeholder, "source");

    let edit = rename_for_analysis(&uri, source, &analysis, literal_position, "feed")
        .expect("rename should validate")
        .expect("rename should produce edits");

    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri,
        vec![
            TextEdit::new(
                span_to_range(source, alias_span(source, "source")),
                "feed".to_owned(),
            ),
            TextEdit::new(
                span_to_range(
                    source,
                    Span::new(literal_shorthand, literal_shorthand + "source".len()),
                ),
                "source: feed".to_owned(),
            ),
            TextEdit::new(
                span_to_range(
                    source,
                    Span::new(return_source, return_source + "source".len()),
                ),
                "feed".to_owned(),
            ),
        ],
    );

    assert_eq!(edit, WorkspaceEdit::new(expected_changes));
}

#[test]
fn rename_bridge_preserves_function_shorthand_binding_sites() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
struct Ops {
    add_one: (Int) -> Int,
}

fn add_one(value: Int) -> Int {
    return value + 1
}

fn read() -> Int {
    let built = Ops { add_one }
    return add_one(1)
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let literal_shorthand = source
        .find("{ add_one }")
        .map(|offset| offset + 2)
        .expect("function shorthand struct literal field should exist");
    let function_name = source
        .find("fn add_one")
        .map(|offset| offset + "fn ".len())
        .expect("function definition should exist");
    let call_site = source
        .rfind("add_one(1)")
        .expect("function call should exist");
    let literal_position = span_to_range(
        source,
        Span::new(literal_shorthand, literal_shorthand + "add_one".len()),
    )
    .start;

    let prepare = prepare_rename_for_analysis(source, &analysis, literal_position)
        .expect("function prepare rename through shorthand site should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(
        range,
        span_to_range(
            source,
            Span::new(literal_shorthand, literal_shorthand + "add_one".len()),
        )
    );
    assert_eq!(placeholder, "add_one");

    let edit = rename_for_analysis(&uri, source, &analysis, literal_position, "inc")
        .expect("rename should validate")
        .expect("rename should produce edits");

    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri,
        vec![
            TextEdit::new(
                span_to_range(
                    source,
                    Span::new(function_name, function_name + "add_one".len()),
                ),
                "inc".to_owned(),
            ),
            TextEdit::new(
                span_to_range(
                    source,
                    Span::new(literal_shorthand, literal_shorthand + "add_one".len()),
                ),
                "add_one: inc".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, Span::new(call_site, call_site + "add_one".len())),
                "inc".to_owned(),
            ),
        ],
    );

    assert_eq!(edit, WorkspaceEdit::new(expected_changes));
}

#[test]
fn rename_bridge_preserves_const_shorthand_binding_sites() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
struct Limits {
    max: Int,
}

const max: Int = 10

fn read() -> Int {
    let built = Limits { max }
    return max
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let literal_shorthand = source
        .find("{ max }")
        .map(|offset| offset + 2)
        .expect("const shorthand struct literal field should exist");
    let const_name = source
        .find("const max")
        .map(|offset| offset + "const ".len())
        .expect("const definition should exist");
    let return_use = source
        .rfind("return max")
        .map(|offset| offset + "return ".len())
        .expect("const return use should exist");
    let literal_position = span_to_range(
        source,
        Span::new(literal_shorthand, literal_shorthand + "max".len()),
    )
    .start;

    let prepare = prepare_rename_for_analysis(source, &analysis, literal_position)
        .expect("const prepare rename through shorthand site should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(
        range,
        span_to_range(
            source,
            Span::new(literal_shorthand, literal_shorthand + "max".len()),
        )
    );
    assert_eq!(placeholder, "max");

    let edit = rename_for_analysis(&uri, source, &analysis, literal_position, "upper_bound")
        .expect("rename should validate")
        .expect("rename should produce edits");

    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri,
        vec![
            TextEdit::new(
                span_to_range(source, Span::new(const_name, const_name + "max".len())),
                "upper_bound".to_owned(),
            ),
            TextEdit::new(
                span_to_range(
                    source,
                    Span::new(literal_shorthand, literal_shorthand + "max".len()),
                ),
                "max: upper_bound".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, Span::new(return_use, return_use + "max".len())),
                "upper_bound".to_owned(),
            ),
        ],
    );

    assert_eq!(edit, WorkspaceEdit::new(expected_changes));
}

#[test]
fn rename_bridge_preserves_static_shorthand_binding_sites() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
struct Limits {
    current: Int,
}

static current: Int = 10

fn read() -> Int {
    let built = Limits { current }
    return current
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let literal_shorthand = source
        .find("{ current }")
        .map(|offset| offset + 2)
        .expect("static shorthand struct literal field should exist");
    let static_name = source
        .find("static current")
        .map(|offset| offset + "static ".len())
        .expect("static definition should exist");
    let return_use = source
        .rfind("return current")
        .map(|offset| offset + "return ".len())
        .expect("static return use should exist");
    let literal_position = span_to_range(
        source,
        Span::new(literal_shorthand, literal_shorthand + "current".len()),
    )
    .start;

    let prepare = prepare_rename_for_analysis(source, &analysis, literal_position)
        .expect("static prepare rename through shorthand site should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(
        range,
        span_to_range(
            source,
            Span::new(literal_shorthand, literal_shorthand + "current".len()),
        )
    );
    assert_eq!(placeholder, "current");

    let edit = rename_for_analysis(&uri, source, &analysis, literal_position, "current_value")
        .expect("rename should validate")
        .expect("rename should produce edits");

    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri,
        vec![
            TextEdit::new(
                span_to_range(
                    source,
                    Span::new(static_name, static_name + "current".len()),
                ),
                "current_value".to_owned(),
            ),
            TextEdit::new(
                span_to_range(
                    source,
                    Span::new(literal_shorthand, literal_shorthand + "current".len()),
                ),
                "current: current_value".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, Span::new(return_use, return_use + "current".len())),
                "current_value".to_owned(),
            ),
        ],
    );

    assert_eq!(edit, WorkspaceEdit::new(expected_changes));
}

#[test]
fn rename_bridge_expands_shorthand_field_sites_through_import_alias_paths() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
use Point as P

struct Point {
    x: Int,
}

fn read(point: Point, value: Int) -> Int {
    let x = value
    let built = P { x }
    match point {
        P { x } => x,
    }
    return point.x
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let literal_shorthand = source
        .find("{ x }")
        .map(|offset| offset + 2)
        .expect("shorthand struct literal field through import alias should exist");
    let pattern_shorthand = source
        .rfind("{ x }")
        .map(|offset| offset + 2)
        .expect("shorthand struct pattern field through import alias should exist");
    let member_use = source
        .rfind(".x")
        .map(|offset| offset + 1)
        .expect("field member use should exist");
    let member_position =
        span_to_range(source, Span::new(member_use, member_use + "x".len())).start;

    let prepare = prepare_rename_for_analysis(source, &analysis, member_position)
        .expect("field prepare rename through import alias should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(
        range,
        span_to_range(source, Span::new(member_use, member_use + "x".len()))
    );
    assert_eq!(placeholder, "x");

    let edit = rename_for_analysis(&uri, source, &analysis, member_position, "coord_x")
        .expect("rename should validate")
        .expect("rename should produce edits");

    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri,
        vec![
            TextEdit::new(
                span_to_range(source, nth_span(source, "x", 1)),
                "coord_x".to_owned(),
            ),
            TextEdit::new(
                span_to_range(
                    source,
                    Span::new(literal_shorthand, literal_shorthand + "x".len()),
                ),
                "coord_x: x".to_owned(),
            ),
            TextEdit::new(
                span_to_range(
                    source,
                    Span::new(pattern_shorthand, pattern_shorthand + "x".len()),
                ),
                "coord_x: x".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, Span::new(member_use, member_use + "x".len())),
                "coord_x".to_owned(),
            ),
        ],
    );

    assert_eq!(edit, WorkspaceEdit::new(expected_changes));
}

#[test]
fn rename_bridge_preserves_shorthand_binding_sites_through_import_alias_paths() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
use Point as P

struct Point {
    x: Int,
}

fn read(value: Int) -> Int {
    let x = value
    let built = P { x }
    return x
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let literal_shorthand = source
        .find("{ x }")
        .map(|offset| offset + 2)
        .expect("shorthand struct literal field through import alias should exist");
    let return_x = source
        .rfind("return x")
        .map(|offset| offset + "return ".len())
        .expect("return x should exist");
    let literal_position = span_to_range(
        source,
        Span::new(literal_shorthand, literal_shorthand + "x".len()),
    )
    .start;

    let prepare = prepare_rename_for_analysis(source, &analysis, literal_position)
        .expect("local prepare rename through import-alias shorthand site should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(
        range,
        span_to_range(
            source,
            Span::new(literal_shorthand, literal_shorthand + "x".len())
        )
    );
    assert_eq!(placeholder, "x");

    let edit = rename_for_analysis(&uri, source, &analysis, literal_position, "coord_x")
        .expect("rename should validate")
        .expect("rename should produce edits");

    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri,
        vec![
            TextEdit::new(
                span_to_range(source, nth_span(source, "x", 2)),
                "coord_x".to_owned(),
            ),
            TextEdit::new(
                span_to_range(
                    source,
                    Span::new(literal_shorthand, literal_shorthand + "x".len()),
                ),
                "x: coord_x".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, Span::new(return_x, return_x + "x".len())),
                "coord_x".to_owned(),
            ),
        ],
    );

    assert_eq!(edit, WorkspaceEdit::new(expected_changes));
}

#[test]
fn rename_bridge_preserves_shorthand_pattern_binding_sites_through_import_alias_paths() {
    let uri = Url::parse("file:///sample.ql").expect("URI should parse");
    let source = r#"
use Point as P

struct Point {
    x: Int,
}

fn read(point: Point) -> Int {
    return match point {
        P { x } => x,
    }
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");
    let pattern_shorthand = source
        .find("{ x }")
        .map(|offset| offset + 2)
        .expect("shorthand struct pattern field through import alias should exist");
    let arm_use = source
        .rfind("=> x")
        .map(|offset| offset + "=> ".len())
        .expect("match arm use should exist");
    let pattern_position = span_to_range(
        source,
        Span::new(pattern_shorthand, pattern_shorthand + "x".len()),
    )
    .start;

    let prepare = prepare_rename_for_analysis(source, &analysis, pattern_position)
        .expect("local prepare rename through import-alias pattern shorthand site should exist");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("expected range plus placeholder");
    };
    assert_eq!(
        range,
        span_to_range(
            source,
            Span::new(pattern_shorthand, pattern_shorthand + "x".len())
        )
    );
    assert_eq!(placeholder, "x");

    let edit = rename_for_analysis(&uri, source, &analysis, pattern_position, "coord_x")
        .expect("rename should validate")
        .expect("rename should produce edits");

    let mut expected_changes = HashMap::new();
    expected_changes.insert(
        uri,
        vec![
            TextEdit::new(
                span_to_range(
                    source,
                    Span::new(pattern_shorthand, pattern_shorthand + "x".len()),
                ),
                "x: coord_x".to_owned(),
            ),
            TextEdit::new(
                span_to_range(source, Span::new(arm_use, arm_use + "x".len())),
                "coord_x".to_owned(),
            ),
        ],
    );

    assert_eq!(edit, WorkspaceEdit::new(expected_changes));
}
