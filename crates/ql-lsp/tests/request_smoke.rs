mod common;

use common::request::{
    TempDir, code_lens_via_request, completion_via_request, did_open_via_request,
    document_symbol_via_request, goto_declaration_via_request, goto_definition_via_request,
    goto_implementation_via_request, goto_type_definition_via_request, hover_via_request,
    initialize_service_with_workspace_roots, initialized_service_with_open_documents,
    inlay_hint_via_request, nth_offset, offset_to_position, semantic_tokens_full_via_request,
    semantic_tokens_range_via_request, signature_help_via_request,
};
use ql_lsp::Backend;
use tower_lsp::LspService;
use tower_lsp::lsp_types::request::{
    GotoDeclarationResponse, GotoImplementationResponse, GotoTypeDefinitionResponse,
};
use tower_lsp::lsp_types::{
    CompletionResponse, DocumentSymbolResponse, GotoDefinitionResponse, HoverContents, Location,
    Range, SemanticToken, SemanticTokenType, SemanticTokensRangeResult, SemanticTokensResult,
    SymbolKind as LspSymbolKind, Url,
};

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
    let mut service =
        initialized_service_with_open_documents(vec![(uri.clone(), source.clone())]).await;

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
    let GotoDefinitionResponse::Scalar(Location {
        uri: def_uri,
        range,
    }) = definition
    else {
        panic!("definition request should return a scalar location")
    };
    assert_eq!(def_uri, uri);
    assert_eq!(
        range.start,
        offset_to_position(&source, nth_offset(&source, "Config", 1)),
    );

    let code_lenses = code_lens_via_request(&mut service, uri.clone())
        .await
        .expect("codeLens request should return lenses");
    assert!(
        code_lenses.iter().any(|lens| {
            lens.command
                .as_ref()
                .is_some_and(|command| command.title.contains("reference"))
        }),
        "codeLens should expose reference lenses: {code_lenses:#?}",
    );

    let declaration = goto_declaration_via_request(
        &mut service,
        uri.clone(),
        offset_to_position(&source, nth_offset(&source, "Config", 3)),
    )
    .await
    .expect("declaration request should return a location");
    let GotoDeclarationResponse::Scalar(Location {
        uri: decl_uri,
        range,
    }) = declaration
    else {
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
        offset_to_position(
            &source,
            nth_offset(&source, "config.va", 1) + "config.va".len(),
        ),
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
    let GotoImplementationResponse::Scalar(Location {
        uri: impl_uri,
        range,
    }) = implementation
    else {
        panic!("implementation request should return a scalar location")
    };
    assert_eq!(impl_uri, uri);
    assert_eq!(
        range.start,
        offset_to_position(&source, nth_offset(&source, "get(self)", 1)),
    );

    let type_definition = goto_type_definition_via_request(
        &mut service,
        uri.clone(),
        offset_to_position(&source, nth_offset(&source, "Config", 3)),
    )
    .await
    .expect("typeDefinition request should return a source-backed type location");
    let GotoTypeDefinitionResponse::Scalar(Location {
        uri: type_uri,
        range,
    }) = type_definition
    else {
        panic!("typeDefinition request should return a scalar location")
    };
    assert_eq!(type_uri, uri);
    assert_eq!(
        range.start,
        offset_to_position(&source, nth_offset(&source, "Config", 1)),
    );

    let document_symbols = document_symbol_via_request(&mut service, uri.clone())
        .await
        .expect("documentSymbol request should return nested symbols");
    let DocumentSymbolResponse::Nested(symbols) = document_symbols else {
        panic!("documentSymbol request should return nested symbols")
    };
    assert!(
        symbols
            .iter()
            .any(|symbol| symbol.name == "Config" && symbol.kind == LspSymbolKind::STRUCT),
        "documentSymbol request should include Config struct: {symbols:#?}",
    );
    assert!(
        symbols
            .iter()
            .any(|symbol| symbol.name == "build" && symbol.kind == LspSymbolKind::FUNCTION),
        "documentSymbol request should include build function: {symbols:#?}",
    );

    let semantic_tokens = semantic_tokens_full_via_request(&mut service, uri)
        .await
        .expect("semanticTokens/full request should return tokens");
    let SemanticTokensResult::Tokens(tokens) = semantic_tokens else {
        panic!("semanticTokens/full request should return full token data")
    };
    assert!(
        !tokens.data.is_empty(),
        "semanticTokens/full request should return at least one token",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn workspace_requests_prefer_open_dependency_source_for_navigation_and_hover() {
    let fixture = setup_open_dependency_source_fixture(
        "ql-lsp-request-open-dependency-source",
        r#"
package demo.app

use demo.dep.Config
use demo.dep.make as make

pub fn main(value: Config) -> Config {
    let probe = make()
    return value
}
"#,
    )
    .await;
    let OpenDependencySourceFixture {
        _temp,
        mut service,
        app_source,
        app_uri,
        dep_uri,
        open_dep_source,
    } = fixture;
    let function_position = offset_to_position(&app_source, nth_offset(&app_source, "make", 2));
    let type_position = offset_to_position(&app_source, nth_offset(&app_source, "Config", 2));

    let hover = hover_via_request(&mut service, app_uri.clone(), function_position)
        .await
        .expect("hover request should return open dependency source info");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover request should return markup contents")
    };
    assert!(markup.value.contains("**function** `make`"));
    assert!(markup.value.contains("fn make(input: String) -> Bool"));
    assert!(
        !markup.value.contains("fn make() -> Int"),
        "hover should use open dependency source, not disk source: {}",
        markup.value,
    );

    assert_definition_targets_open_dependency(
        goto_definition_via_request(&mut service, app_uri.clone(), function_position)
            .await
            .expect("definition request should return open dependency source location"),
        &dep_uri,
        &open_dep_source,
        "make",
    );
    assert_declaration_targets_open_dependency(
        goto_declaration_via_request(&mut service, app_uri.clone(), function_position)
            .await
            .expect("declaration request should return open dependency source location"),
        &dep_uri,
        &open_dep_source,
        "make",
    );
    assert_type_definition_targets_open_dependency(
        goto_type_definition_via_request(&mut service, app_uri, type_position)
            .await
            .expect("typeDefinition request should return open dependency source location"),
        &dep_uri,
        &open_dep_source,
        "Config",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn broken_workspace_import_requests_prefer_open_dependency_source() {
    let fixture = setup_open_dependency_source_fixture(
        "ql-lsp-request-broken-open-dependency-source",
        r#"
package demo.app

use demo.dep.Config
use demo.dep.make as make

pub fn main(value: Config) -> Int {
    let probe = make()
    let broken =
    return 0
}
"#,
    )
    .await;
    let OpenDependencySourceFixture {
        _temp,
        mut service,
        app_source,
        app_uri,
        dep_uri,
        open_dep_source,
    } = fixture;
    let function_position = offset_to_position(&app_source, nth_offset(&app_source, "make", 2));
    let type_position = offset_to_position(&app_source, nth_offset(&app_source, "Config", 2));

    let hover = hover_via_request(&mut service, app_uri.clone(), function_position)
        .await
        .expect("broken-source hover request should return open dependency source info");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover request should return markup contents")
    };
    assert!(markup.value.contains("fn make(input: String) -> Bool"));
    assert!(
        !markup.value.contains("fn make() -> Int"),
        "broken-source hover should use open dependency source, not disk source: {}",
        markup.value,
    );

    assert_definition_targets_open_dependency(
        goto_definition_via_request(&mut service, app_uri.clone(), function_position)
            .await
            .expect(
                "broken-source definition request should return open dependency source location",
            ),
        &dep_uri,
        &open_dep_source,
        "make",
    );
    assert_declaration_targets_open_dependency(
        goto_declaration_via_request(&mut service, app_uri.clone(), function_position)
            .await
            .expect(
                "broken-source declaration request should return open dependency source location",
            ),
        &dep_uri,
        &open_dep_source,
        "make",
    );
    assert_type_definition_targets_open_dependency(
        goto_type_definition_via_request(&mut service, app_uri, type_position)
            .await
            .expect(
                "broken-source typeDefinition request should return open dependency source location",
            ),
        &dep_uri,
        &open_dep_source,
        "Config",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn rich_workspace_requests_prefer_open_dependency_source() {
    let fixture = setup_open_dependency_source_fixture(
        "ql-lsp-request-rich-open-dependency-source",
        r#"
package demo.app

use demo.dep.Config
use demo.dep.make as make

pub fn main(value: Config) -> Int {
    let probe = make("current")
    let picked = value.open_value
    return 0
}
"#,
    )
    .await;
    let OpenDependencySourceFixture {
        _temp,
        mut service,
        app_source,
        app_uri,
        dep_uri: _,
        open_dep_source: _,
    } = fixture;

    let completion = completion_via_request(
        &mut service,
        app_uri.clone(),
        offset_to_position(
            &app_source,
            nth_offset(&app_source, "value.o", 1) + "value.o".len(),
        ),
    )
    .await
    .expect("completion request should use open dependency source members");
    let labels = completion_labels(completion);
    assert!(
        labels.iter().any(|label| label == "open_value"),
        "completion should include open dependency member: {labels:#?}",
    );
    assert!(
        labels.iter().all(|label| label != "disk_value"),
        "completion should not use stale disk/interface members: {labels:#?}",
    );

    let signature = signature_help_via_request(
        &mut service,
        app_uri.clone(),
        offset_to_position(
            &app_source,
            nth_offset(&app_source, "make(\"current", 1) + "make(\"current".len(),
        ),
    )
    .await
    .expect("signatureHelp should use open dependency source signature");
    assert_eq!(
        signature.signatures[0].label,
        "fn make(input: String) -> Bool"
    );
    assert_eq!(signature.active_parameter, Some(0));

    let hints = inlay_hint_via_request(
        &mut service,
        app_uri.clone(),
        full_source_range(&app_source),
    )
    .await
    .expect("inlayHint should use open dependency source parameters");
    assert!(
        hints
            .iter()
            .any(|hint| format!("{:?}", hint.label).contains("input:")),
        "inlay hints should include open dependency parameter name: {hints:#?}",
    );

    let legend = ql_lsp::bridge::semantic_tokens_legend();
    let property_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::PROPERTY)
        .expect("property token type should exist") as u32;
    let member_position = offset_to_position(&app_source, nth_offset(&app_source, "open_value", 1));
    let member_entry = (
        member_position.line,
        member_position.character,
        "open_value".len() as u32,
        property_type,
    );

    let SemanticTokensResult::Tokens(tokens) =
        semantic_tokens_full_via_request(&mut service, app_uri.clone())
            .await
            .expect("semanticTokens/full should use open dependency source")
    else {
        panic!("semanticTokens/full should return token data")
    };
    assert!(
        decode_semantic_tokens(&tokens.data).contains(&member_entry),
        "full semantic tokens should classify open dependency member"
    );

    let SemanticTokensRangeResult::Tokens(range_tokens) = semantic_tokens_range_via_request(
        &mut service,
        app_uri,
        Range::new(
            offset_to_position(&app_source, nth_offset(&app_source, "let picked", 1)),
            offset_to_position(&app_source, nth_offset(&app_source, "return 0", 1)),
        ),
    )
    .await
    .expect("semanticTokens/range should use open dependency source") else {
        panic!("semanticTokens/range should return token data")
    };
    assert!(
        decode_semantic_tokens(&range_tokens.data).contains(&member_entry),
        "range semantic tokens should classify open dependency member"
    );
}

struct OpenDependencySourceFixture {
    _temp: TempDir,
    service: LspService<Backend>,
    app_source: String,
    app_uri: Url,
    dep_uri: Url,
    open_dep_source: String,
}

async fn setup_open_dependency_source_fixture(
    prefix: &str,
    app_source: &str,
) -> OpenDependencySourceFixture {
    let temp = TempDir::new(prefix);
    let workspace_root = temp.path().join("workspace");
    let app_path = temp.write("workspace/packages/app/src/main.ql", app_source);
    let dep_path = temp.write(
        "workspace/packages/dep/src/lib.ql",
        r#"
package demo.dep

pub fn make() -> Int {
    return 1
}

pub struct Config {
    disk_value: Int,
}
"#,
    );
    let open_dep_source = r#"
package demo.dep

pub fn make(input: String) -> Bool {
    return true
}

pub struct Config {
    open_value: Int,
}
"#
    .to_owned();

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/dep"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write(
        "workspace/packages/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/packages/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn make() -> Int

pub struct Config {
    disk_value: Int,
}
"#,
    );

    let workspace_root_uri =
        Url::from_file_path(&workspace_root).expect("workspace root path should convert to URI");
    let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
    let dep_uri = Url::from_file_path(&dep_path).expect("dependency path should convert to URI");
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service_with_workspace_roots(&mut service, vec![workspace_root_uri]).await;
    did_open_via_request(&mut service, app_uri.clone(), app_source.to_owned()).await;
    did_open_via_request(&mut service, dep_uri.clone(), open_dep_source.clone()).await;

    OpenDependencySourceFixture {
        _temp: temp,
        service,
        app_source: app_source.to_owned(),
        app_uri,
        dep_uri,
        open_dep_source,
    }
}

fn assert_definition_targets_open_dependency(
    response: GotoDefinitionResponse,
    dep_uri: &Url,
    open_dep_source: &str,
    snippet: &str,
) {
    let GotoDefinitionResponse::Scalar(location) = response else {
        panic!("definition request should return one location")
    };
    assert_location_targets_open_dependency(location, dep_uri, open_dep_source, snippet);
}

fn assert_declaration_targets_open_dependency(
    response: GotoDeclarationResponse,
    dep_uri: &Url,
    open_dep_source: &str,
    snippet: &str,
) {
    let GotoDeclarationResponse::Scalar(location) = response else {
        panic!("declaration request should return one location")
    };
    assert_location_targets_open_dependency(location, dep_uri, open_dep_source, snippet);
}

fn assert_type_definition_targets_open_dependency(
    response: GotoTypeDefinitionResponse,
    dep_uri: &Url,
    open_dep_source: &str,
    snippet: &str,
) {
    let GotoTypeDefinitionResponse::Scalar(location) = response else {
        panic!("typeDefinition request should return one location")
    };
    assert_location_targets_open_dependency(location, dep_uri, open_dep_source, snippet);
}

fn assert_location_targets_open_dependency(
    location: Location,
    dep_uri: &Url,
    open_dep_source: &str,
    snippet: &str,
) {
    let start = nth_offset(open_dep_source, snippet, 1);
    assert_eq!(location.uri, *dep_uri);
    assert_eq!(
        location.range.start,
        offset_to_position(open_dep_source, start),
    );
    assert_eq!(
        location.range.end,
        offset_to_position(open_dep_source, start + snippet.len()),
    );
}

fn completion_labels(completion: CompletionResponse) -> Vec<String> {
    match completion {
        CompletionResponse::Array(items) => items,
        CompletionResponse::List(list) => list.items,
    }
    .into_iter()
    .map(|item| item.label)
    .collect()
}

fn full_source_range(source: &str) -> Range {
    Range::new(
        offset_to_position(source, 0),
        offset_to_position(source, source.len()),
    )
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
