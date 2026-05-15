#![allow(dead_code)]

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::common::request::{
    TempDir, completion_via_request, did_open_via_request, initialize_service_with_workspace_roots,
    nth_offset, offset_to_position,
};
use crate::common::stdlib_real::write_real_stdlib_workspace;
use ql_lsp::Backend;
use ql_lsp::bridge::{semantic_tokens_legend, span_to_range};
use ql_span::Span;
use tower_lsp::LspService;
use tower_lsp::lsp_types::request::{GotoDeclarationResponse, GotoTypeDefinitionResponse};
use tower_lsp::lsp_types::{
    CallHierarchyOutgoingCall, CodeActionOrCommand, CompletionItem as LspCompletionItem,
    CompletionResponse, Diagnostic, DocumentHighlight, DocumentSymbolResponse, Documentation,
    FoldingRange, GotoDefinitionResponse, Hover, HoverContents, InlayHint, InlayHintKind,
    InlayHintLabel, Location, NumberOrString, Range, SelectionRange, SemanticToken,
    SemanticTokenType, SymbolKind, TextEdit, Url,
};

pub async fn open_real_stdlib_workspace(
    temp: &TempDir,
    app_source: &str,
) -> (LspService<Backend>, Url, PathBuf) {
    open_real_stdlib_workspace_with_open_source(temp, app_source, app_source).await
}

pub async fn open_real_stdlib_workspace_with_open_source(
    temp: &TempDir,
    disk_app_source: &str,
    open_app_source: &str,
) -> (LspService<Backend>, Url, PathBuf) {
    let workspace = write_real_stdlib_workspace(temp, disk_app_source);
    let workspace_root_uri = Url::from_file_path(temp.path().join("workspace"))
        .expect("workspace root path should convert to URI");
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service_with_workspace_roots(&mut service, vec![workspace_root_uri]).await;
    did_open_via_request(
        &mut service,
        workspace.app_uri.clone(),
        open_app_source.to_owned(),
    )
    .await;
    (service, workspace.app_uri, workspace.stdlib_root)
}
pub async fn completion_at(
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

pub fn range_at(source: &str, start: usize, len: usize) -> Range {
    Range::new(
        offset_to_position(source, start),
        offset_to_position(source, start + len),
    )
}

pub fn range_for(source: &str, needle: &str, occurrence: usize) -> Range {
    let start = nth_offset(source, needle, occurrence);
    range_at(source, start, needle.len())
}

pub fn full_source_range(source: &str) -> Range {
    range_at(source, 0, source.len())
}

pub fn range_for_in_context(source: &str, needle: &str, context: &str, occurrence: usize) -> Range {
    let context_start = nth_offset(source, context, occurrence);
    let needle_start = context
        .find(needle)
        .expect("needle should exist inside context");
    range_at(source, context_start + needle_start, needle.len())
}

pub fn assert_edit(edits: &[TextEdit], range: Range, replacement: &str) {
    assert!(
        edits
            .iter()
            .any(|edit| edit.range == range && edit.new_text == replacement),
        "edits should include `{replacement}` at {range:?}: {edits:#?}",
    );
}

pub fn completion_items(completion: CompletionResponse) -> Vec<LspCompletionItem> {
    match completion {
        CompletionResponse::Array(items) => items,
        CompletionResponse::List(list) => list.items,
    }
}

pub fn completion_labels(completion: CompletionResponse) -> Vec<String> {
    completion_items(completion)
        .into_iter()
        .map(|item| item.label)
        .collect()
}

pub fn completion_documentation(item: &LspCompletionItem) -> String {
    match item
        .documentation
        .as_ref()
        .expect("completion item should include documentation")
    {
        Documentation::String(value) => value.clone(),
        Documentation::MarkupContent(markup) => markup.value.clone(),
    }
}

pub fn hover_markup(hover: Hover) -> String {
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    markup.value
}

pub fn assert_contains_all(labels: &[String], expected: &[&str]) {
    for label in expected {
        assert!(
            labels.iter().any(|candidate| candidate == label),
            "completion should include `{label}`: {labels:#?}",
        );
    }
}

pub fn assert_not_contains_any(labels: &[String], unexpected: &[&str]) {
    for label in unexpected {
        assert!(
            labels.iter().all(|candidate| candidate != label),
            "completion should not include legacy `{label}`: {labels:#?}",
        );
    }
}

pub fn assert_document_symbol(
    response: &DocumentSymbolResponse,
    name: &str,
    expected_kind: SymbolKind,
) {
    let DocumentSymbolResponse::Nested(symbols) = response else {
        panic!("documentSymbol should return nested symbols")
    };
    assert!(
        symbols
            .iter()
            .any(|symbol| symbol.name == name && symbol.kind == expected_kind),
        "document symbols should include {expected_kind:?} `{name}`: {symbols:#?}",
    );
}

pub fn assert_call_hierarchy_targets(calls: &[CallHierarchyOutgoingCall], names: &[&str]) {
    for name in names {
        assert!(
            calls.iter().any(|call| call.to.name == *name),
            "call hierarchy should include outgoing target `{name}`: {calls:#?}",
        );
    }
}

pub fn assert_parameter_hint(hints: &[InlayHint], expected: &str) {
    assert!(
        hints.iter().any(|hint| matches!(
            (&hint.kind, &hint.label),
            (Some(InlayHintKind::PARAMETER), InlayHintLabel::String(label)) if label == expected
        )),
        "inlay hints should include real stdlib parameter `{expected}`: {hints:#?}",
    );
}

pub fn assert_folding_range_starts_at_source_line(
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

pub fn assert_selection_range_source(
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

pub fn assert_definition_targets_snippet(
    definition: GotoDefinitionResponse,
    interface_path: &Path,
    snippet: &str,
) {
    let GotoDefinitionResponse::Scalar(Location { uri, range }) = definition else {
        panic!("definition should be one location")
    };
    assert_location_targets_snippet(uri, range, interface_path, snippet);
}

pub fn assert_declaration_targets_snippet(
    declaration: GotoDeclarationResponse,
    interface_path: &Path,
    snippet: &str,
) {
    let GotoDeclarationResponse::Scalar(Location { uri, range }) = declaration else {
        panic!("declaration should be one location")
    };
    assert_location_targets_snippet(uri, range, interface_path, snippet);
}

pub fn assert_type_definition_targets_snippet(
    definition: GotoTypeDefinitionResponse,
    interface_path: &Path,
    snippet: &str,
) {
    let GotoTypeDefinitionResponse::Scalar(Location { uri, range }) = definition else {
        panic!("type definition should be one location")
    };
    assert_location_targets_snippet(uri, range, interface_path, snippet);
}

pub fn assert_location_targets_snippet(
    uri: Url,
    range: Range,
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
        span_to_range(&artifact, Span::new(start, start + snippet.len())),
    );
}

pub fn assert_reference_targets_snippet(
    references: &[Location],
    interface_path: &Path,
    snippet: &str,
) {
    let expected_path = interface_path
        .canonicalize()
        .expect("stdlib interface path should canonicalize");
    let artifact = fs::read_to_string(interface_path)
        .expect("stdlib interface should read")
        .replace("\r\n", "\n");
    let start = artifact
        .find(snippet)
        .unwrap_or_else(|| panic!("snippet should exist in stdlib interface: {snippet}"));
    let expected_range = span_to_range(&artifact, Span::new(start, start + snippet.len()));
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

pub fn assert_reference_targets_source(
    references: &[Location],
    uri: &Url,
    source: &str,
    name: &str,
    offset: usize,
) {
    let expected_range = span_to_range(source, Span::new(offset, offset + name.len()));
    assert!(
        references
            .iter()
            .any(|reference| reference.uri == *uri && reference.range == expected_range),
        "references should include source occurrence at {expected_range:?}: {references:#?}",
    );
}

pub fn assert_document_highlight_source(
    highlights: &[DocumentHighlight],
    source: &str,
    name: &str,
    offset: usize,
) {
    let expected_range = span_to_range(source, Span::new(offset, offset + name.len()));
    assert!(
        highlights
            .iter()
            .any(|highlight| highlight.range == expected_range),
        "document highlights should include source occurrence at {expected_range:?}: {highlights:#?}",
    );
}

pub fn assert_semantic_token(
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

pub fn assert_code_action<'a>(
    actions: &'a [CodeActionOrCommand],
    title: &str,
) -> &'a HashMap<Url, Vec<TextEdit>> {
    let action = actions
        .iter()
        .find_map(|action| match action {
            CodeActionOrCommand::CodeAction(action) if action.title == title => Some(action),
            _ => None,
        })
        .unwrap_or_else(|| panic!("code actions should include `{title}`: {actions:#?}"));
    action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .unwrap_or_else(|| panic!("code action `{title}` should contain direct changes"))
}

pub fn unresolved_type_diagnostic(source: &str, name: &str) -> Diagnostic {
    let start = nth_offset(source, name, 1);
    Diagnostic {
        range: range_at(source, start, name.len()),
        severity: None,
        code: Some(NumberOrString::String(
            ql_diagnostics::UNRESOLVED_TYPE_CODE.to_owned(),
        )),
        code_description: None,
        source: None,
        message: format!("unresolved type `{name}`"),
        related_information: None,
        tags: None,
        data: None,
    }
}
