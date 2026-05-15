mod common;

use std::fs;
use std::path::Path;

use common::request::{
    TempDir, completion_via_request, did_open_via_request, document_highlight_via_request,
    folding_range_via_request, formatting_via_request, goto_definition_via_request,
    goto_type_definition_via_request, hover_via_request, initialize_service_with_workspace_roots,
    inlay_hint_via_request, nth_offset, offset_to_position, prepare_rename_via_request,
    references_via_request, rename_via_request, selection_range_via_request,
    semantic_tokens_full_via_request, signature_help_via_request,
};
use common::stdlib_real::{real_stdlib_source_path, write_real_stdlib_workspace};
use ql_lsp::Backend;
use ql_lsp::bridge::{semantic_tokens_legend, span_to_range};
use tower_lsp::LspService;
use tower_lsp::lsp_types::request::GotoTypeDefinitionResponse;
use tower_lsp::lsp_types::{
    CompletionResponse, DocumentHighlight, FoldingRange, GotoDefinitionResponse, HoverContents,
    InlayHint, InlayHintKind, InlayHintLabel, Location, PrepareRenameResponse, Range,
    SelectionRange, SemanticToken, SemanticTokenType, SemanticTokensResult, TextEdit, Url,
};

async fn open_real_stdlib_workspace(
    temp: &TempDir,
    app_source: &str,
) -> (LspService<Backend>, Url, std::path::PathBuf) {
    let workspace = write_real_stdlib_workspace(temp, app_source);
    let workspace_root_uri = Url::from_file_path(temp.path().join("workspace"))
        .expect("workspace root path should convert to URI");
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service_with_workspace_roots(&mut service, vec![workspace_root_uri]).await;
    did_open_via_request(
        &mut service,
        workspace.app_uri.clone(),
        app_source.to_owned(),
    )
    .await;
    (service, workspace.app_uri, workspace.stdlib_root)
}

#[tokio::test(flavor = "current_thread")]
async fn completion_request_uses_current_real_stdlib_surface() {
    let temp = TempDir::new("ql-lsp-real-stdlib-completion-request");
    let app_source = r#"
package demo.app

use std.core.
use std.option.
use std.result.
use std.array.
use std.test.

pub fn main() -> Int {
    return 0
}
"#;
    let (mut service, app_uri, _) = open_real_stdlib_workspace(&temp, app_source).await;

    let core = completion_labels(
        completion_at(&mut service, app_uri.clone(), app_source, "std.core.").await,
    );
    assert_contains_all(&core, &["max_int", "is_even_int", "not_bool"]);

    let option = completion_labels(
        completion_at(&mut service, app_uri.clone(), app_source, "std.option.").await,
    );
    assert_contains_all(&option, &["Option", "some", "none_option"]);
    assert_not_contains_any(&option, &["IntOption", "some_int"]);

    let result = completion_labels(
        completion_at(&mut service, app_uri.clone(), app_source, "std.result.").await,
    );
    assert_contains_all(&result, &["Result", "ok", "ok_or"]);
    assert_not_contains_any(&result, &["IntResult", "ok_int"]);

    let array = completion_labels(
        completion_at(&mut service, app_uri.clone(), app_source, "std.array.").await,
    );
    assert_contains_all(&array, &["repeat_array", "sum_int_array", "reverse_array"]);

    let test =
        completion_labels(completion_at(&mut service, app_uri, app_source, "std.test.").await);
    assert_contains_all(
        &test,
        &["expect_eq", "expect_option_some", "expect_result_ok"],
    );
}

#[tokio::test(flavor = "current_thread")]
async fn rich_requests_use_current_real_stdlib_interfaces() {
    let temp = TempDir::new("ql-lsp-real-stdlib-rich-requests");
    let app_source = r#"
package demo.app

use std.option.Option as Option
use std.option.some as option_some
use std.result.Result as Result
use std.result.ok_or as result_ok_or
use std.array.repeat_array as repeat_array
use std.core.clamp_int as clamp_int
use std.test.expect_eq as expect_eq

pub fn main() -> Int {
    let option_value: Option[Int] = option_some(42)
    let result_value: Result[Int, Int] = result_ok_or(option_value, 5)
    let values: [Int; 3] = repeat_array(2)
    let check = expect_eq(clamp_int(42, 0, 100), 42)
    return check
}
"#;
    let (mut service, app_uri, stdlib_root) = open_real_stdlib_workspace(&temp, app_source).await;

    let option_hover = hover_markup(
        hover_via_request(
            &mut service,
            app_uri.clone(),
            offset_to_position(
                app_source,
                nth_offset(app_source, ": Option", 1) + ": ".len(),
            ),
        )
        .await
        .expect("real std.option hover request should return markup"),
    );
    assert!(option_hover.contains("**enum** `Option`"));
    assert!(option_hover.contains("enum Option[T]"));
    assert!(!option_hover.contains("Compatibility API"));

    assert_definition_targets_snippet(
        goto_definition_via_request(
            &mut service,
            app_uri.clone(),
            offset_to_position(app_source, nth_offset(app_source, "option_some", 2)),
        )
        .await
        .expect("real std.option function definition should exist"),
        &real_stdlib_source_path(&stdlib_root, "option"),
        "some",
    );
    let option_references = references_via_request(
        &mut service,
        app_uri.clone(),
        offset_to_position(app_source, nth_offset(app_source, "option_some", 2)),
        true,
    )
    .await
    .expect("real std.option references should exist");
    assert_reference_targets_snippet(
        &option_references,
        &real_stdlib_source_path(&stdlib_root, "option"),
        "some",
    );
    assert_reference_targets_source(
        &option_references,
        &app_uri,
        app_source,
        "option_some",
        nth_offset(app_source, "option_some", 1),
    );
    assert_reference_targets_source(
        &option_references,
        &app_uri,
        app_source,
        "option_some",
        nth_offset(app_source, "option_some", 2),
    );

    let option_highlights = document_highlight_via_request(
        &mut service,
        app_uri.clone(),
        offset_to_position(app_source, nth_offset(app_source, "option_some", 2)),
    )
    .await
    .expect("real stdlib documentHighlight should return current-file highlights");
    assert_document_highlight_source(
        &option_highlights,
        app_source,
        "option_some",
        nth_offset(app_source, "option_some", 1),
    );
    assert_document_highlight_source(
        &option_highlights,
        app_source,
        "option_some",
        nth_offset(app_source, "option_some", 2),
    );

    assert_type_definition_targets_snippet(
        goto_type_definition_via_request(
            &mut service,
            app_uri.clone(),
            offset_to_position(
                app_source,
                nth_offset(app_source, ": Option", 1) + ": ".len(),
            ),
        )
        .await
        .expect("real std.option type definition should exist"),
        &real_stdlib_source_path(&stdlib_root, "option"),
        "Option",
    );

    let signature = signature_help_via_request(
        &mut service,
        app_uri.clone(),
        offset_to_position(
            app_source,
            nth_offset(app_source, "result_ok_or(option_value, ", 1)
                + "result_ok_or(option_value, ".len(),
        ),
    )
    .await
    .expect("real std.result signatureHelp should return a signature");
    assert_eq!(signature.active_parameter, Some(1));
    assert_eq!(
        signature.signatures[0].label,
        "fn ok_or[T, E](value: Option[T], error: E) -> Result[T, E]"
    );

    let hints =
        inlay_hint_via_request(&mut service, app_uri.clone(), full_source_range(app_source))
            .await
            .expect("real stdlib inlayHint should return parameter hints");
    assert_parameter_hint(&hints, "value:");
    assert_parameter_hint(&hints, "error:");
    assert_parameter_hint(&hints, "low:");
    assert_parameter_hint(&hints, "high:");
    assert_parameter_hint(&hints, "actual:");
    assert_parameter_hint(&hints, "expected:");

    let SemanticTokensResult::Tokens(tokens) =
        semantic_tokens_full_via_request(&mut service, app_uri)
            .await
            .expect("real stdlib semantic tokens request should return tokens")
    else {
        panic!("semantic tokens should use full token payload")
    };
    assert_semantic_token(
        app_source,
        &tokens.data,
        nth_offset(app_source, ": Option", 1) + ": ".len(),
        "Option".len(),
        SemanticTokenType::ENUM,
    );
    assert_semantic_token(
        app_source,
        &tokens.data,
        nth_offset(app_source, "result_ok_or", 2),
        "result_ok_or".len(),
        SemanticTokenType::FUNCTION,
    );
    assert_semantic_token(
        app_source,
        &tokens.data,
        nth_offset(app_source, "clamp_int", 2),
        "clamp_int".len(),
        SemanticTokenType::FUNCTION,
    );
}

#[tokio::test(flavor = "current_thread")]
async fn structure_requests_use_current_real_stdlib_sources() {
    let temp = TempDir::new("ql-lsp-real-stdlib-structure-requests");
    let app_source = r#"
package demo.app

use std.option.Option as Option

pub fn main() -> Int {
    return 0
}
"#;
    let (mut service, _, stdlib_root) = open_real_stdlib_workspace(&temp, app_source).await;
    let option_source_path = real_stdlib_source_path(&stdlib_root, "option");
    let option_source = fs::read_to_string(&option_source_path)
        .expect("temp std.option source should exist")
        .replace("\r\n", "\n");
    let option_uri = Url::from_file_path(&option_source_path)
        .expect("temp std.option source path should convert to URI");
    did_open_via_request(&mut service, option_uri.clone(), option_source.clone()).await;

    let folds = folding_range_via_request(&mut service, option_uri.clone())
        .await
        .expect("real stdlib foldingRange should return source folds");
    assert_folding_range_starts_at_source_line(&folds, &option_source, "pub enum Option", 1);
    assert_folding_range_starts_at_source_line(&folds, &option_source, "pub fn unwrap_or", 1);
    assert_folding_range_starts_at_source_line(&folds, &option_source, "return match value", 1);

    let inner_offset =
        nth_offset(&option_source, "Option.Some(inner) => inner", 1) + "Option.Some(".len();
    let selections = selection_range_via_request(
        &mut service,
        option_uri.clone(),
        vec![offset_to_position(&option_source, inner_offset + 1)],
    )
    .await
    .expect("real stdlib selectionRange should return token selection");
    assert_selection_range_source(&selections, &option_source, "inner", inner_offset);

    let edits = formatting_via_request(&mut service, option_uri)
        .await
        .expect("real stdlib formatting should return an edit list for parseable source");
    assert!(
        edits.is_empty(),
        "real stdlib source should already be qfmt-stable: {edits:#?}",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn rename_request_updates_current_real_stdlib_source_roots() {
    let temp = TempDir::new("ql-lsp-real-stdlib-rename-request");
    let app_source = r#"
package demo.app

use std.core.max_int as maximum

pub fn main() -> Int {
    return maximum(1, 2)
}
"#;
    let (mut service, app_uri, stdlib_root) = open_real_stdlib_workspace(&temp, app_source).await;
    let core_source_path = real_stdlib_source_path(&stdlib_root, "core");
    let core_source = fs::read_to_string(&core_source_path)
        .expect("temp std.core source should exist")
        .replace("\r\n", "\n");
    let core_uri = Url::from_file_path(&core_source_path)
        .expect("temp std.core source path should convert to URI");
    let import_position = offset_to_position(app_source, nth_offset(app_source, "max_int", 1));

    let prepare = prepare_rename_via_request(&mut service, app_uri.clone(), import_position)
        .await
        .expect("real stdlib prepareRename should target imported function");
    let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } = prepare else {
        panic!("prepareRename should return range plus placeholder")
    };
    assert_eq!(range, range_for(app_source, "max_int", 1));
    assert_eq!(placeholder, "max_int");

    let edit = rename_via_request(
        &mut service,
        app_uri.clone(),
        import_position,
        "largest_int",
    )
    .await
    .expect("real stdlib rename should return workspace edit");
    let changes = edit
        .changes
        .expect("real stdlib rename should use simple workspace changes");

    assert_edit(
        changes
            .get(&app_uri)
            .expect("rename should edit importing app source"),
        range_for(app_source, "max_int", 1),
        "largest_int",
    );
    assert_edit(
        changes
            .get(&core_uri)
            .expect("rename should edit temp std.core source"),
        range_for(&core_source, "max_int", 1),
        "largest_int",
    );
    assert_edit(
        changes
            .get(&core_uri)
            .expect("rename should edit std.core call sites"),
        range_for_in_context(
            &core_source,
            "max_int",
            "max_int(first_bound, second_bound)",
            1,
        ),
        "largest_int",
    );
}

async fn completion_at(
    service: &mut LspService<Backend>,
    uri: Url,
    source: &str,
    prefix: &str,
) -> CompletionResponse {
    completion_via_request(
        service,
        uri,
        offset_to_position(source, nth_offset(source, prefix, 1) + prefix.len()),
    )
    .await
    .unwrap_or_else(|| panic!("{prefix} completion request should return items"))
}

fn range_for(source: &str, needle: &str, occurrence: usize) -> Range {
    let start = nth_offset(source, needle, occurrence);
    range_at(source, start, needle.len())
}

fn full_source_range(source: &str) -> Range {
    range_at(source, 0, source.len())
}

fn range_for_in_context(source: &str, needle: &str, context: &str, occurrence: usize) -> Range {
    let context_start = nth_offset(source, context, occurrence);
    let needle_start = context
        .find(needle)
        .expect("needle should exist inside context");
    range_at(source, context_start + needle_start, needle.len())
}

fn range_at(source: &str, start: usize, len: usize) -> Range {
    Range::new(
        offset_to_position(source, start),
        offset_to_position(source, start + len),
    )
}

fn assert_edit(edits: &[TextEdit], range: Range, replacement: &str) {
    assert!(
        edits
            .iter()
            .any(|edit| edit.range == range && edit.new_text == replacement),
        "edits should include `{replacement}` at {range:?}: {edits:#?}",
    );
}

fn assert_parameter_hint(hints: &[InlayHint], expected: &str) {
    assert!(
        hints.iter().any(
            |hint| matches!((&hint.kind, &hint.label), (Some(InlayHintKind::PARAMETER), InlayHintLabel::String(label)) if label == expected)
        ),
        "inlay hints should include real stdlib parameter `{expected}`: {hints:#?}",
    );
}

fn completion_labels(completion: CompletionResponse) -> Vec<String> {
    match completion {
        CompletionResponse::Array(items) => items.into_iter().map(|item| item.label).collect(),
        CompletionResponse::List(list) => list.items.into_iter().map(|item| item.label).collect(),
    }
}

fn assert_contains_all(labels: &[String], expected: &[&str]) {
    for label in expected {
        assert!(
            labels.iter().any(|candidate| candidate == label),
            "completion should include `{label}`: {labels:#?}",
        );
    }
}

fn assert_not_contains_any(labels: &[String], unexpected: &[&str]) {
    for label in unexpected {
        assert!(
            labels.iter().all(|candidate| candidate != label),
            "completion should not include legacy `{label}`: {labels:#?}",
        );
    }
}

fn hover_markup(hover: tower_lsp::lsp_types::Hover) -> String {
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    markup.value
}

fn assert_definition_targets_snippet(
    definition: GotoDefinitionResponse,
    interface_path: &Path,
    snippet: &str,
) {
    let GotoDefinitionResponse::Scalar(Location { uri, range }) = definition else {
        panic!("definition should be one location")
    };
    assert_location_targets_snippet(uri, range, interface_path, snippet);
}

fn assert_type_definition_targets_snippet(
    definition: GotoTypeDefinitionResponse,
    interface_path: &Path,
    snippet: &str,
) {
    let GotoTypeDefinitionResponse::Scalar(Location { uri, range }) = definition else {
        panic!("type definition should be one location")
    };
    assert_location_targets_snippet(uri, range, interface_path, snippet);
}

fn assert_location_targets_snippet(
    uri: Url,
    range: tower_lsp::lsp_types::Range,
    interface_path: &Path,
    snippet: &str,
) {
    assert_eq!(
        uri.to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        interface_path
            .canonicalize()
            .expect("stdlib interface path should canonicalize"),
    );
    let artifact = fs::read_to_string(interface_path)
        .expect("stdlib interface should read")
        .replace("\r\n", "\n");
    let start = artifact
        .find(snippet)
        .unwrap_or_else(|| panic!("snippet should exist in stdlib interface: {snippet}"));
    assert_eq!(
        range,
        span_to_range(&artifact, ql_span::Span::new(start, start + snippet.len())),
    );
}

fn assert_reference_targets_snippet(references: &[Location], interface_path: &Path, snippet: &str) {
    let expected_path = interface_path
        .canonicalize()
        .expect("stdlib interface path should canonicalize");
    let artifact = fs::read_to_string(interface_path)
        .expect("stdlib interface should read")
        .replace("\r\n", "\n");
    let start = artifact
        .find(snippet)
        .unwrap_or_else(|| panic!("snippet should exist in stdlib interface: {snippet}"));
    let expected_range = span_to_range(&artifact, ql_span::Span::new(start, start + snippet.len()));
    assert!(
        references.iter().any(|reference| {
            reference.range == expected_range
                && reference
                    .uri
                    .to_file_path()
                    .ok()
                    .and_then(|path| path.canonicalize().ok())
                    .is_some_and(|path| path == expected_path)
        }),
        "references should include stdlib definition `{snippet}` from {}: {references:#?}",
        expected_path.display(),
    );
}

fn assert_reference_targets_source(
    references: &[Location],
    uri: &Url,
    source: &str,
    name: &str,
    offset: usize,
) {
    let expected_range = span_to_range(source, ql_span::Span::new(offset, offset + name.len()));
    assert!(
        references
            .iter()
            .any(|reference| reference.uri == *uri && reference.range == expected_range),
        "references should include source occurrence at {expected_range:?}: {references:#?}",
    );
}

fn assert_document_highlight_source(
    highlights: &[DocumentHighlight],
    source: &str,
    name: &str,
    offset: usize,
) {
    let expected_range = span_to_range(source, ql_span::Span::new(offset, offset + name.len()));
    assert!(
        highlights
            .iter()
            .any(|highlight| highlight.range == expected_range),
        "document highlights should include source occurrence at {expected_range:?}: {highlights:#?}",
    );
}

fn assert_folding_range_starts_at_source_line(
    folds: &[FoldingRange],
    source: &str,
    needle: &str,
    occurrence: usize,
) {
    let expected_line = offset_to_position(source, nth_offset(source, needle, occurrence)).line;
    assert!(
        folds
            .iter()
            .any(|fold| fold.start_line == expected_line && fold.end_line > fold.start_line),
        "folding ranges should include multiline fold starting at `{needle}`: {folds:#?}",
    );
}

fn assert_selection_range_source(
    selections: &[SelectionRange],
    source: &str,
    name: &str,
    offset: usize,
) {
    assert_eq!(selections.len(), 1);
    assert_eq!(selections[0].range, range_at(source, offset, name.len()));
    assert!(
        selections[0].parent.is_some(),
        "selection range should include parent expansion: {selections:#?}",
    );
}

fn assert_semantic_token(
    source: &str,
    tokens: &[SemanticToken],
    offset: usize,
    len: usize,
    expected_type: SemanticTokenType,
) {
    let position = offset_to_position(source, offset);
    let expected_type_index = semantic_tokens_legend()
        .token_types
        .iter()
        .position(|token_type| *token_type == expected_type)
        .unwrap_or_else(|| panic!("semantic token legend should include {expected_type:?}"))
        as u32;
    let decoded = decode_semantic_tokens(tokens);
    assert!(
        decoded.contains(&(
            position.line,
            position.character,
            len as u32,
            expected_type_index,
        )),
        "semantic tokens should include {expected_type:?} at {position:?}: {decoded:#?}",
    );
}

fn decode_semantic_tokens(tokens: &[SemanticToken]) -> Vec<(u32, u32, u32, u32)> {
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
