#![allow(dead_code)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_lsp::Backend;
use serde_json::json;
use tower::{Service, ServiceExt};
use tower_lsp::LspService;
use tower_lsp::jsonrpc::{Id, Request};
use tower_lsp::lsp_types::request::{
    GotoDeclarationParams, GotoDeclarationResponse, GotoImplementationParams,
    GotoImplementationResponse,
};
use tower_lsp::lsp_types::{
    CodeActionOrCommand, CompletionParams, CompletionResponse, Diagnostic,
    DidOpenTextDocumentParams, DocumentHighlight, GotoDefinitionParams, GotoDefinitionResponse,
    Hover, HoverParams, InitializeParams, Location, Position, Range, ReferenceContext,
    ReferenceParams, SymbolInformation, TextDocumentIdentifier, TextDocumentItem,
    TextDocumentPositionParams, Url, WorkspaceFolder,
};

static NEXT_REQUEST_ID: AtomicI64 = AtomicI64::new(2);

pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(prefix: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!("{prefix}-{unique}"));
        fs::create_dir_all(&path).expect("create temporary test directory");
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directory for temp file");
        }
        fs::write(&path, contents).expect("write temp file");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub fn nth_offset(source: &str, needle: &str, occurrence: usize) -> usize {
    source
        .match_indices(needle)
        .nth(occurrence.saturating_sub(1))
        .map(|(start, _)| start)
        .expect("needle occurrence should exist")
}

pub fn nth_offset_in_context(
    source: &str,
    needle: &str,
    context: &str,
    occurrence: usize,
) -> usize {
    let context_start = nth_offset(source, context, occurrence);
    let relative = context
        .match_indices(needle)
        .last()
        .map(|(start, _)| start)
        .expect("needle should exist inside context");
    context_start + relative
}

pub fn offset_to_position(source: &str, offset: usize) -> Position {
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let line_start = prefix.rfind('\n').map(|index| index + 1).unwrap_or(0);
    Position::new(line, prefix[line_start..].chars().count() as u32)
}

async fn initialize_service_with_params(
    service: &mut LspService<Backend>,
    params: InitializeParams,
) {
    let request = Request::build("initialize")
        .params(json!(params))
        .id(1)
        .finish();
    let response = service
        .ready()
        .await
        .expect("service should become ready for initialize")
        .call(request)
        .await
        .expect("initialize request should succeed");
    let response = response.expect("initialize should return a response");
    assert_eq!(response.id(), &Id::Number(1));
    assert!(response.is_ok(), "initialize should succeed: {response:?}");
}

pub async fn initialize_service(service: &mut LspService<Backend>) {
    initialize_service_with_params(service, InitializeParams::default()).await;
}

pub async fn initialize_service_with_workspace_roots(
    service: &mut LspService<Backend>,
    workspace_roots: Vec<Url>,
) {
    let root_uri = workspace_roots.first().cloned();
    let workspace_folders = if workspace_roots.is_empty() {
        None
    } else {
        Some(
            workspace_roots
                .into_iter()
                .enumerate()
                .map(|(index, uri)| WorkspaceFolder {
                    uri,
                    name: format!("workspace-{index}"),
                })
                .collect(),
        )
    };
    initialize_service_with_params(
        service,
        InitializeParams {
            root_uri,
            workspace_folders,
            ..InitializeParams::default()
        },
    )
    .await;
}

pub async fn did_open_via_request(service: &mut LspService<Backend>, uri: Url, text: String) {
    let request = Request::build("textDocument/didOpen")
        .params(json!(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id: "ql".to_owned(),
                version: 1,
                text,
            },
        }))
        .finish();
    let response = service
        .ready()
        .await
        .expect("service should become ready for didOpen")
        .call(request)
        .await
        .expect("didOpen notification should succeed");
    assert_eq!(response, None);
}

pub async fn initialized_service_with_open_documents(
    documents: Vec<(Url, String)>,
) -> LspService<Backend> {
    let (mut service, _) = LspService::new(Backend::new);
    initialize_service(&mut service).await;
    for (uri, text) in documents {
        did_open_via_request(&mut service, uri, text).await;
    }
    service
}

async fn request_value(
    service: &mut LspService<Backend>,
    method: &'static str,
    params: serde_json::Value,
) -> serde_json::Value {
    let request_id = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
    let request = Request::build(method)
        .params(params)
        .id(request_id)
        .finish();
    let response = service
        .ready()
        .await
        .unwrap_or_else(|_| panic!("service should become ready for {method}"))
        .call(request)
        .await
        .unwrap_or_else(|_| panic!("{method} request should succeed"))
        .unwrap_or_else(|| panic!("{method} should return a response"));
    assert_eq!(response.id(), &Id::Number(request_id));
    response
        .result()
        .cloned()
        .unwrap_or_else(|| panic!("{method} should succeed"))
}

fn text_document_position(uri: Url, position: Position) -> TextDocumentPositionParams {
    TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri },
        position,
    }
}

pub async fn hover_via_request(
    service: &mut LspService<Backend>,
    uri: Url,
    position: Position,
) -> Option<Hover> {
    let value = request_value(
        service,
        "textDocument/hover",
        json!(HoverParams {
            text_document_position_params: text_document_position(uri, position),
            work_done_progress_params: Default::default(),
        }),
    )
    .await;
    serde_json::from_value(value).expect("textDocument/hover result should deserialize")
}

pub async fn goto_definition_via_request(
    service: &mut LspService<Backend>,
    uri: Url,
    position: Position,
) -> Option<GotoDefinitionResponse> {
    let value = request_value(
        service,
        "textDocument/definition",
        json!(GotoDefinitionParams {
            text_document_position_params: text_document_position(uri, position),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        }),
    )
    .await;
    serde_json::from_value(value).expect("textDocument/definition result should deserialize")
}

pub async fn goto_declaration_via_request(
    service: &mut LspService<Backend>,
    uri: Url,
    position: Position,
) -> Option<GotoDeclarationResponse> {
    let value = request_value(
        service,
        "textDocument/declaration",
        json!(GotoDeclarationParams {
            text_document_position_params: text_document_position(uri, position),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        }),
    )
    .await;
    serde_json::from_value(value).expect("textDocument/declaration result should deserialize")
}

pub async fn goto_implementation_via_request(
    service: &mut LspService<Backend>,
    uri: Url,
    position: Position,
) -> Option<GotoImplementationResponse> {
    let value = request_value(
        service,
        "textDocument/implementation",
        json!(GotoImplementationParams {
            text_document_position_params: text_document_position(uri, position),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        }),
    )
    .await;
    serde_json::from_value(value).expect("textDocument/implementation result should deserialize")
}

pub async fn completion_via_request(
    service: &mut LspService<Backend>,
    uri: Url,
    position: Position,
) -> Option<CompletionResponse> {
    let value = request_value(
        service,
        "textDocument/completion",
        json!(CompletionParams {
            text_document_position: text_document_position(uri, position),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        }),
    )
    .await;
    serde_json::from_value(value).expect("textDocument/completion result should deserialize")
}

pub async fn code_action_via_request(
    service: &mut LspService<Backend>,
    uri: Url,
    range: Range,
    diagnostics: Vec<Diagnostic>,
) -> Option<Vec<CodeActionOrCommand>> {
    let value = request_value(
        service,
        "textDocument/codeAction",
        json!({
            "textDocument": {
                "uri": uri,
            },
            "range": range,
            "context": {
                "diagnostics": diagnostics,
            },
        }),
    )
    .await;
    serde_json::from_value(value).expect("textDocument/codeAction result should deserialize")
}

pub async fn references_via_request(
    service: &mut LspService<Backend>,
    uri: Url,
    position: Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let value = request_value(
        service,
        "textDocument/references",
        json!(ReferenceParams {
            text_document_position: text_document_position(uri, position),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: ReferenceContext {
                include_declaration,
            },
        }),
    )
    .await;
    serde_json::from_value(value).expect("textDocument/references result should deserialize")
}

pub async fn document_highlight_via_request(
    service: &mut LspService<Backend>,
    uri: Url,
    position: Position,
) -> Option<Vec<DocumentHighlight>> {
    let value = request_value(
        service,
        "textDocument/documentHighlight",
        json!({
            "textDocument": {
                "uri": uri,
            },
            "position": position,
        }),
    )
    .await;
    serde_json::from_value(value).expect("textDocument/documentHighlight result should deserialize")
}

pub async fn workspace_symbol_via_request(
    service: &mut LspService<Backend>,
    query: &str,
) -> Vec<SymbolInformation> {
    let value = request_value(
        service,
        "workspace/symbol",
        json!({
            "query": query,
        }),
    )
    .await;
    serde_json::from_value(value).expect("workspace/symbol result should deserialize")
}
