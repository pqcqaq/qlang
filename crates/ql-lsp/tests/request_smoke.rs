mod common;

use common::request::{
    TempDir, completion_via_request, goto_declaration_via_request, goto_definition_via_request,
    goto_implementation_via_request, hover_via_request, initialized_service_with_open_documents,
    nth_offset, offset_to_position,
};
use tower_lsp::lsp_types::request::{GotoDeclarationResponse, GotoImplementationResponse};
use tower_lsp::lsp_types::{CompletionResponse, GotoDefinitionResponse, HoverContents, Location, Url};

#[tokio::test(flavor = "current_thread")]
async fn request_smoke_covers_core_editor_requests() {
    let temp = TempDir::new("ql-lsp-request-smoke");
    let source_path = temp.write(
        "sample.ql",
        r#"
struct Config {
    value: Int,
}

impl Config {
    fn get(self) -> Int {
        return self.value
    }
}

fn build(config: Config) -> Int {
    return config.get()
}

fn complete(config: Config) -> Int {
    return config.va
}
"#,
    );
    let source = std::fs::read_to_string(&source_path).expect("source should read");
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let mut service = initialized_service_with_open_documents(vec![(uri.clone(), source.clone())])
        .await;

    let hover = hover_via_request(
        &mut service,
        uri.clone(),
        offset_to_position(&source, nth_offset(&source, "Config", 3)),
    )
    .await
    .expect("hover request should return source-backed info");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover request should return markup contents")
    };
    assert!(
        markup.value.contains("Config"),
        "hover markup should mention Config: {}",
        markup.value,
    );

    let definition = goto_definition_via_request(
        &mut service,
        uri.clone(),
        offset_to_position(&source, nth_offset(&source, "Config", 3)),
    )
    .await
    .expect("definition request should return a location");
    let GotoDefinitionResponse::Scalar(Location { uri: def_uri, range }) = definition else {
        panic!("definition request should return a scalar location")
    };
    assert_eq!(def_uri, uri);
    assert_eq!(
        range.start,
        offset_to_position(&source, nth_offset(&source, "Config", 1)),
    );

    let declaration = goto_declaration_via_request(
        &mut service,
        uri.clone(),
        offset_to_position(&source, nth_offset(&source, "Config", 3)),
    )
    .await
    .expect("declaration request should return a location");
    let GotoDeclarationResponse::Scalar(Location { uri: decl_uri, range }) = declaration else {
        panic!("declaration request should return a scalar location")
    };
    assert_eq!(decl_uri, uri);
    assert_eq!(
        range.start,
        offset_to_position(&source, nth_offset(&source, "Config", 1)),
    );

    let completion = completion_via_request(
        &mut service,
        uri.clone(),
        offset_to_position(&source, nth_offset(&source, "config.va", 1) + "config.va".len()),
    )
    .await
    .expect("completion request should return member candidates");
    let items = match completion {
        CompletionResponse::Array(items) => items,
        CompletionResponse::List(list) => list.items,
    };
    assert!(
        items.iter().any(|item| item.label == "value"),
        "completion request should include Config.value",
    );

    let implementation = goto_implementation_via_request(
        &mut service,
        uri.clone(),
        offset_to_position(&source, nth_offset(&source, "get()", 1)),
    )
    .await
    .expect("implementation request should return a method definition");
    let GotoImplementationResponse::Scalar(Location { uri: impl_uri, range }) = implementation else {
        panic!("implementation request should return a scalar location")
    };
    assert_eq!(impl_uri, uri);
    assert_eq!(
        range.start,
        offset_to_position(&source, nth_offset(&source, "get(self)", 1)),
    );
}
