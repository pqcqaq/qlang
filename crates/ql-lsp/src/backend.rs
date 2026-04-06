use ql_analysis::{
    Analysis, PackageAnalysisError, analyze_package, analyze_package_dependencies, analyze_source,
};
use tower_lsp::jsonrpc::{Error, Result};
use tower_lsp::lsp_types::{
    CompletionOptions, CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverParams, HoverProviderCapability, InitializeParams,
    InitializeResult, InitializedParams, Location, MessageType, OneOf, PrepareRenameResponse,
    ReferenceParams, RenameOptions, RenameParams, SemanticTokensFullOptions, SemanticTokensOptions,
    SemanticTokensParams, SemanticTokensResult, SemanticTokensServerCapabilities,
    ServerCapabilities, ServerInfo, TextDocumentPositionParams, TextDocumentSyncCapability,
    TextDocumentSyncKind, TextDocumentSyncOptions, Url, WorkspaceEdit,
};
use tower_lsp::{Client, LanguageServer};

use crate::bridge::{
    completion_for_analysis, completion_for_dependency_imports,
    completion_for_dependency_struct_fields, completion_for_dependency_variants,
    completion_for_package_analysis, definition_for_dependency_variants,
    definition_for_package_analysis, diagnostics_to_lsp, hover_for_dependency_variants,
    hover_for_package_analysis, prepare_rename_for_analysis, references_for_analysis,
    references_for_package_analysis, rename_for_analysis, semantic_tokens_for_analysis,
    semantic_tokens_legend,
};
use crate::store::DocumentStore;

#[derive(Debug)]
pub struct Backend {
    client: Client,
    documents: DocumentStore,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: DocumentStore::default(),
        }
    }

    async fn publish_document_diagnostics(&self, uri: &tower_lsp::lsp_types::Url, source: &str) {
        let diagnostics = match analyze_source(source) {
            Ok(analysis) => diagnostics_to_lsp(uri, source, analysis.diagnostics()),
            Err(diagnostics) => diagnostics_to_lsp(uri, source, &diagnostics),
        };

        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
    }

    async fn analyzed_document(&self, uri: &Url) -> Option<(String, Analysis)> {
        let source = self.documents.get(uri).await?;
        let analysis = analyze_source(&source).ok()?;
        Some((source, analysis))
    }

    fn package_analysis_for_uri(&self, uri: &Url) -> Option<ql_analysis::PackageAnalysis> {
        let path = uri.to_file_path().ok()?;
        match analyze_package(&path) {
            Ok(package) => Some(package),
            Err(PackageAnalysisError::SourceDiagnostics { .. }) => {
                analyze_package_dependencies(&path).ok()
            }
            Err(_) => None,
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "qlsp".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        ..Default::default()
                    },
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions::default()),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: semantic_tokens_legend(),
                            range: None,
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            ..Default::default()
                        },
                    ),
                ),
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: Default::default(),
                })),
                ..Default::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "qlsp initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let source = params.text_document.text;
        self.documents.insert(uri.clone(), source.clone()).await;
        self.publish_document_diagnostics(&uri, &source).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let Some(change) = params.content_changes.into_iter().last() else {
            return;
        };

        self.documents
            .insert(uri.clone(), change.text.clone())
            .await;
        self.publish_document_diagnostics(&uri, &change.text).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents.remove(&params.text_document.uri).await;
        self.client
            .publish_diagnostics(params.text_document.uri, Vec::new(), None)
            .await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let Some(source) = self.documents.get(&uri).await else {
            return Ok(None);
        };

        if let Some(package) = self.package_analysis_for_uri(&uri) {
            if let Some(hover) = hover_for_dependency_variants(&source, &package, position) {
                return Ok(Some(hover));
            }
            let Ok(analysis) = analyze_source(&source) else {
                return Ok(None);
            };
            return Ok(hover_for_package_analysis(
                &source, &analysis, &package, position,
            ));
        }

        let Ok(analysis) = analyze_source(&source) else {
            return Ok(None);
        };
        Ok(crate::bridge::hover_for_analysis(
            &source, &analysis, position,
        ))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let Some(source) = self.documents.get(&uri).await else {
            return Ok(None);
        };

        if let Some(package) = self.package_analysis_for_uri(&uri) {
            if let Some(definition) =
                definition_for_dependency_variants(&source, &package, position)
            {
                return Ok(Some(definition));
            }
            let Ok(analysis) = analyze_source(&source) else {
                return Ok(None);
            };
            return Ok(definition_for_package_analysis(
                &uri, &source, &analysis, &package, position,
            ));
        }

        let Ok(analysis) = analyze_source(&source) else {
            return Ok(None);
        };
        Ok(crate::bridge::definition_for_analysis(
            &uri, &source, &analysis, position,
        ))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let Some((source, analysis)) = self.analyzed_document(&uri).await else {
            return Ok(None);
        };

        if let Some(package) = self.package_analysis_for_uri(&uri) {
            return Ok(references_for_package_analysis(
                &uri,
                &source,
                &analysis,
                &package,
                position,
                params.context.include_declaration,
            ));
        }

        Ok(references_for_analysis(
            &uri,
            &source,
            &analysis,
            position,
            params.context.include_declaration,
        ))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let Some(source) = self.documents.get(&uri).await else {
            return Ok(None);
        };
        let package = self.package_analysis_for_uri(&uri);

        if let Some(package) = package.as_ref() {
            if let Some(completion) = completion_for_dependency_imports(&source, package, position)
            {
                return Ok(Some(completion));
            }
            if let Some(completion) =
                completion_for_dependency_struct_fields(&source, package, position)
            {
                return Ok(Some(completion));
            }
            if let Some(completion) = completion_for_dependency_variants(&source, package, position)
            {
                return Ok(Some(completion));
            }
        }

        let Ok(analysis) = analyze_source(&source) else {
            return Ok(None);
        };

        if let Some(package) = package.as_ref() {
            return Ok(completion_for_package_analysis(
                &source, &analysis, package, position,
            ));
        }

        Ok(completion_for_analysis(&source, &analysis, position))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let Some((source, analysis)) = self.analyzed_document(&uri).await else {
            return Ok(None);
        };

        Ok(Some(semantic_tokens_for_analysis(&source, &analysis)))
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri;
        let position = params.position;
        let Some((source, analysis)) = self.analyzed_document(&uri).await else {
            return Ok(None);
        };

        Ok(prepare_rename_for_analysis(&source, &analysis, position))
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let Some((source, analysis)) = self.analyzed_document(&uri).await else {
            return Ok(None);
        };

        rename_for_analysis(&uri, &source, &analysis, position, &params.new_name)
            .map_err(|error| Error::invalid_params(error.to_string()))
    }
}
