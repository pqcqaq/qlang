mod common;

use common::request::{
    TempDir, completion_resolve_via_request, completion_via_request,
    initialized_service_with_open_documents, nth_offset, offset_to_position,
};
use tower_lsp::lsp_types::{
    CompletionItem as LspCompletionItem, CompletionItemKind, CompletionResponse, Documentation,
    InsertTextFormat, Url,
};

fn completion_documentation(item: &LspCompletionItem) -> String {
    match item
        .documentation
        .as_ref()
        .expect("completion item should include documentation")
    {
        Documentation::String(value) => value.clone(),
        Documentation::MarkupContent(markup) => markup.value.clone(),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn completion_request_offers_keyword_snippets_when_semantic_completion_is_empty() {
    let temp = TempDir::new("ql-lsp-keyword-completion");
    let source_path = temp.write(
        "completion.ql",
        r#"
f
"#,
    );
    let source = std::fs::read_to_string(&source_path).expect("source should read");
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.clone())]).await;

    let completion = completion_via_request(
        &mut service,
        uri,
        offset_to_position(&source, nth_offset(&source, "f", 1) + 1),
    )
    .await
    .expect("keyword completion should be available in broken source");
    let items = match completion {
        CompletionResponse::Array(items) => items,
        CompletionResponse::List(list) => list.items,
    };
    let fn_item = items
        .iter()
        .find(|item| item.label == "fn" && item.kind == Some(CompletionItemKind::SNIPPET))
        .expect("fn snippet should be offered");
    assert_eq!(fn_item.insert_text_format, Some(InsertTextFormat::SNIPPET));
    assert!(
        fn_item.documentation.is_some(),
        "keyword snippets should carry inline docs",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn completion_resolve_enriches_items_without_inline_docs() {
    let mut service = initialized_service_with_open_documents(Vec::new()).await;

    let resolved_keyword = completion_resolve_via_request(
        &mut service,
        LspCompletionItem {
            label: "fn".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            ..LspCompletionItem::default()
        },
    )
    .await;
    assert_eq!(
        resolved_keyword.detail.as_deref(),
        Some("declaration keyword")
    );
    assert!(
        completion_documentation(&resolved_keyword).contains("Declares a function or method."),
        "keyword resolve should add keyword documentation: {resolved_keyword:#?}",
    );

    let resolved_symbol = completion_resolve_via_request(
        &mut service,
        LspCompletionItem {
            label: "helper".to_owned(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some("fn helper() -> Int".to_owned()),
            ..LspCompletionItem::default()
        },
    )
    .await;
    assert!(
        completion_documentation(&resolved_symbol).contains("fn helper() -> Int"),
        "symbol resolve should turn detail into markdown documentation: {resolved_symbol:#?}",
    );
}
