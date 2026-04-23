use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use ql_analysis::{
    Analysis, DependencyDefinitionTarget, DependencyInterface, PackageAnalysisError, RenameError,
    RenameTarget, analyze_available_package_dependencies, analyze_package,
    analyze_package_with_available_dependencies, analyze_source,
};
use ql_ast::{ItemKind as AstItemKind, TypeExpr, TypeExprKind};
use ql_diagnostics::{UNRESOLVED_TYPE_CODE, UNRESOLVED_VALUE_CODE};
use ql_fmt::format_source;
use ql_lexer::{Token, TokenKind, is_keyword, is_valid_identifier, lex};
use ql_project::{
    collect_package_sources, load_project_manifest, package_name as manifest_package_name,
    render_manifest_with_added_local_dependency,
};
use ql_span::Span;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::{Error, Result};
use tower_lsp::lsp_types::request::{
    GotoDeclarationParams, GotoDeclarationResponse, GotoImplementationParams,
    GotoImplementationResponse, GotoTypeDefinitionParams, GotoTypeDefinitionResponse,
};
use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams,
    CodeActionProviderCapability, CompletionOptions, CompletionParams, CompletionResponse,
    DeclarationCapability, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DocumentFormattingParams, DocumentHighlight,
    DocumentHighlightParams, DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverParams, HoverProviderCapability,
    ImplementationProviderCapability, InitializeParams, InitializeResult, InitializedParams,
    Location, MessageType, NumberOrString, OneOf, PrepareRenameResponse, ReferenceParams,
    RenameOptions, RenameParams, SemanticTokensFullOptions, SemanticTokensOptions,
    SemanticTokensParams, SemanticTokensResult, SemanticTokensServerCapabilities,
    ServerCapabilities, ServerInfo, SymbolInformation, SymbolKind as LspSymbolKind,
    TextDocumentPositionParams, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextDocumentSyncOptions, TextEdit, TypeDefinitionProviderCapability, Url, WorkspaceEdit,
    WorkspaceSymbolParams,
};
use tower_lsp::{Client, LanguageServer};

use crate::bridge::{
    completion_for_analysis, completion_for_dependency_imports,
    completion_for_dependency_member_fields, completion_for_dependency_methods,
    completion_for_dependency_struct_fields, completion_for_dependency_variants,
    completion_for_package_analysis, completion_response, declaration_for_dependency_imports,
    declaration_for_dependency_methods, declaration_for_dependency_struct_fields,
    declaration_for_dependency_values, declaration_for_dependency_variants,
    declaration_for_package_analysis, definition_for_dependency_imports,
    definition_for_dependency_methods, definition_for_dependency_struct_fields,
    definition_for_dependency_values, definition_for_dependency_variants,
    definition_for_package_analysis, diagnostics_to_lsp, document_symbol_kind,
    document_symbols_for_analysis, hover_for_dependency_imports, hover_for_dependency_methods,
    hover_for_dependency_struct_fields, hover_for_dependency_values, hover_for_dependency_variants,
    hover_for_package_analysis, implementation_for_analysis, position_to_offset,
    prepare_rename_for_analysis, prepare_rename_for_dependency_imports, references_for_analysis,
    references_for_dependency_imports, references_for_dependency_methods,
    references_for_dependency_struct_fields, references_for_dependency_values,
    references_for_dependency_variants, references_for_package_analysis, rename_for_analysis,
    rename_for_dependency_imports, semantic_tokens_for_analysis, semantic_tokens_legend,
    semantic_tokens_result_from_occurrences, span_to_range, type_definition_for_analysis,
    type_definition_for_dependency_imports, type_definition_for_dependency_method_types,
    type_definition_for_dependency_struct_field_types, type_definition_for_dependency_values,
    type_definition_for_dependency_variants, type_definition_for_package_analysis,
    workspace_symbols_for_analysis,
};
use crate::store::DocumentStore;

#[derive(Debug)]
pub struct Backend {
    client: Client,
    documents: DocumentStore,
    workspace_roots: RwLock<Vec<PathBuf>>,
}

type OpenDocuments = HashMap<PathBuf, (Url, String)>;

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: DocumentStore::default(),
            workspace_roots: RwLock::default(),
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
        package_analysis_for_path(&path)
    }

    async fn open_file_documents(&self) -> OpenDocuments {
        file_open_documents(self.documents.entries().await)
    }
}

fn file_open_documents(documents: Vec<(Url, String)>) -> OpenDocuments {
    let mut open_docs = HashMap::new();
    for (uri, source) in documents {
        let Ok(path) = uri.to_file_path() else {
            continue;
        };
        open_docs.insert(canonicalize_or_clone(&path), (uri, source));
    }
    open_docs
}

fn document_formatting_edits(source: &str) -> std::result::Result<Vec<TextEdit>, String> {
    let formatted = format_source(source).map_err(|errors| {
        let Some(error) = errors.first() else {
            return "qlang: document formatting skipped because the document has parse errors"
                .to_owned();
        };
        let range = span_to_range(source, error.span);
        format!(
            "qlang: document formatting skipped because the document has parse errors at {}:{}: {}",
            range.start.line + 1,
            range.start.character + 1,
            error.message
        )
    })?;

    if formatted == source {
        return Ok(Vec::new());
    }

    Ok(vec![TextEdit::new(
        span_to_range(source, Span::new(0, source.len())),
        formatted,
    )])
}

fn configure_workspace_roots(params: &InitializeParams) -> Vec<PathBuf> {
    let mut roots = params
        .workspace_folders
        .as_ref()
        .into_iter()
        .flatten()
        .filter_map(|folder| folder.uri.to_file_path().ok())
        .map(|path| canonicalize_or_clone(&path))
        .collect::<Vec<_>>();

    if roots.is_empty()
        && let Some(root_uri) = params.root_uri.as_ref()
        && let Ok(root_path) = root_uri.to_file_path()
    {
        roots.push(canonicalize_or_clone(&root_path));
    }

    roots.sort();
    roots.dedup();
    roots
}

fn package_analysis_for_path(path: &Path) -> Option<ql_analysis::PackageAnalysis> {
    match analyze_package(path) {
        Ok(package) => Some(package),
        Err(PackageAnalysisError::SourceDiagnostics { .. }) => {
            analyze_package_with_available_dependencies(path).ok()
        }
        Err(PackageAnalysisError::Project(_)) => {
            analyze_package_with_available_dependencies(path).ok()
        }
        Err(error) if is_interface_artifact_failure(&error) => {
            analyze_package_with_available_dependencies(path).ok()
        }
        Err(_) => None,
    }
}

fn canonicalize_or_clone(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn workspace_member_manifest_paths_for_package(package_manifest_path: &Path) -> Vec<PathBuf> {
    let package_manifest_canonical = canonicalize_or_clone(package_manifest_path);
    let mut candidate_manifests = vec![package_manifest_path.to_path_buf()];
    let mut current = package_manifest_path.parent().and_then(Path::parent);
    while let Some(dir) = current {
        let candidate = dir.join("qlang.toml");
        if candidate.is_file() {
            candidate_manifests.push(candidate);
        }
        current = dir.parent();
    }

    for candidate in candidate_manifests {
        let Ok(manifest) = load_project_manifest(&candidate) else {
            continue;
        };
        let Some(workspace) = manifest.workspace.as_ref() else {
            continue;
        };

        let workspace_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
        let mut contains_current =
            canonicalize_or_clone(&manifest.manifest_path) == package_manifest_canonical;
        let mut member_manifests = Vec::new();

        for member in &workspace.members {
            let Ok(member_manifest) = load_project_manifest(&workspace_dir.join(member)) else {
                continue;
            };
            if member_manifest.package.is_none() {
                continue;
            }

            let member_manifest_path = member_manifest.manifest_path;
            if canonicalize_or_clone(&member_manifest_path) == package_manifest_canonical {
                contains_current = true;
            } else {
                member_manifests.push(member_manifest_path);
            }
        }

        if contains_current {
            member_manifests.sort();
            member_manifests.dedup();
            return member_manifests;
        }
    }

    Vec::new()
}

fn local_dependency_manifest_paths_for_package(package_manifest_path: &Path) -> Vec<PathBuf> {
    let Ok(manifest) = load_project_manifest(package_manifest_path) else {
        return Vec::new();
    };
    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let current_manifest_canonical = canonicalize_or_clone(&manifest.manifest_path);
    let mut dependency_manifests = manifest
        .references
        .packages
        .iter()
        .filter_map(|reference| load_project_manifest(&manifest_dir.join(reference)).ok())
        .filter(|dependency_manifest| dependency_manifest.package.is_some())
        .map(|dependency_manifest| dependency_manifest.manifest_path)
        .filter(|dependency_manifest_path| {
            canonicalize_or_clone(dependency_manifest_path) != current_manifest_canonical
        })
        .collect::<Vec<_>>();
    dependency_manifests.sort();
    dependency_manifests.dedup();
    dependency_manifests
}

fn source_preferred_manifest_paths_for_package(package_manifest_path: &Path) -> Vec<PathBuf> {
    let current_manifest_canonical = canonicalize_or_clone(package_manifest_path);
    let mut manifests = workspace_member_manifest_paths_for_package(package_manifest_path);
    manifests.extend(local_dependency_manifest_paths_for_package(
        package_manifest_path,
    ));
    manifests
        .retain(|manifest_path| canonicalize_or_clone(manifest_path) != current_manifest_canonical);
    manifests.sort_by_key(|manifest_path| {
        canonicalize_or_clone(manifest_path)
            .to_string_lossy()
            .into_owned()
    });
    manifests.dedup_by(|left, right| canonicalize_or_clone(left) == canonicalize_or_clone(right));
    manifests
}

fn append_manifest_and_workspace_symbols(
    manifest: &ql_project::ProjectManifest,
    open_docs: &HashMap<PathBuf, (Url, String)>,
    searched_packages: &mut HashSet<PathBuf>,
    covered_files: &mut HashSet<PathBuf>,
    symbols: &mut Vec<SymbolInformation>,
    query: &str,
) {
    if manifest.package.is_some() {
        let manifest_path = manifest.manifest_path.clone();
        if searched_packages.insert(manifest_path.clone()) {
            append_manifest_source_workspace_symbols(
                manifest,
                open_docs,
                covered_files,
                symbols,
                query,
            );
            let preferred_local_dependency_manifest_paths =
                append_local_dependency_workspace_symbols(
                    manifest_path.as_path(),
                    open_docs,
                    searched_packages,
                    covered_files,
                    symbols,
                    query,
                );
            append_dependency_workspace_symbols_excluding(
                &manifest_path,
                &preferred_local_dependency_manifest_paths,
                symbols,
                query,
            );
        }
    }

    let Some(workspace) = manifest.workspace.as_ref() else {
        return;
    };

    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let mut member_manifests = workspace
        .members
        .iter()
        .filter_map(|member| load_project_manifest(&manifest_dir.join(member)).ok())
        .map(|member_manifest| member_manifest.manifest_path)
        .collect::<Vec<_>>();
    member_manifests.sort();
    member_manifests.dedup();

    for member_manifest_path in member_manifests {
        if !searched_packages.insert(member_manifest_path.clone()) {
            continue;
        }
        append_workspace_member_symbols(
            &member_manifest_path,
            open_docs,
            searched_packages,
            covered_files,
            symbols,
            query,
        );
    }
}

#[allow(deprecated)]
fn append_package_workspace_symbols(
    package: &ql_analysis::PackageAnalysis,
    open_docs: &HashMap<PathBuf, (Url, String)>,
    covered_files: &mut HashSet<PathBuf>,
    symbols: &mut Vec<SymbolInformation>,
    query: &str,
    include_dependencies: bool,
) {
    for module in package.modules() {
        let module_path = module.path().to_path_buf();
        covered_files.insert(module_path.clone());

        if let Some((open_uri, open_source)) = open_docs.get(&module_path)
            && let Ok(analysis) = analyze_source(open_source)
        {
            symbols.extend(workspace_symbols_for_analysis(
                open_uri,
                open_source,
                &analysis,
                query,
            ));
            continue;
        }

        let module_location_path = fs::canonicalize(&module_path).unwrap_or(module_path.clone());
        let Ok(module_uri) = Url::from_file_path(&module_location_path) else {
            continue;
        };
        let Ok(module_source) = fs::read_to_string(&module_path) else {
            continue;
        };
        symbols.extend(workspace_symbols_for_analysis(
            &module_uri,
            &module_source,
            module.analysis(),
            query,
        ));
    }

    if include_dependencies {
        symbols.extend(workspace_symbols_for_dependencies(
            package.dependencies(),
            query,
        ));
    }
}

#[allow(deprecated)]
fn append_manifest_source_workspace_symbols(
    manifest: &ql_project::ProjectManifest,
    open_docs: &HashMap<PathBuf, (Url, String)>,
    covered_files: &mut HashSet<PathBuf>,
    symbols: &mut Vec<SymbolInformation>,
    query: &str,
) {
    let Ok(source_paths) = collect_package_sources(manifest) else {
        return;
    };

    for source_path in source_paths {
        covered_files.insert(source_path.clone());

        if let Some((open_uri, open_source)) = open_docs.get(&source_path) {
            if let Ok(analysis) = analyze_source(open_source) {
                symbols.extend(workspace_symbols_for_analysis(
                    open_uri,
                    open_source,
                    &analysis,
                    query,
                ));
            }
            continue;
        }

        let source_location_path = fs::canonicalize(&source_path).unwrap_or(source_path.clone());
        let Ok(source_uri) = Url::from_file_path(&source_location_path) else {
            continue;
        };
        let Ok(source) = fs::read_to_string(&source_path) else {
            continue;
        };
        let Ok(analysis) = analyze_source(&source) else {
            continue;
        };
        symbols.extend(workspace_symbols_for_analysis(
            &source_uri,
            &source,
            &analysis,
            query,
        ));
    }
}

fn append_dependency_workspace_symbols_excluding(
    package_path: &Path,
    excluded_manifest_paths: &HashSet<PathBuf>,
    symbols: &mut Vec<SymbolInformation>,
    query: &str,
) {
    if let Ok(dependencies) = analyze_available_package_dependencies(package_path) {
        let filtered_dependencies = dependencies
            .into_iter()
            .filter(|dependency| {
                !excluded_manifest_paths
                    .contains(&canonicalize_or_clone(&dependency.manifest().manifest_path))
            })
            .collect::<Vec<_>>();
        symbols.extend(workspace_symbols_for_dependencies(
            &filtered_dependencies,
            query,
        ));
    }
}

fn manifest_has_workspace_symbol_source(
    manifest_path: &Path,
    open_docs: &HashMap<PathBuf, (Url, String)>,
) -> bool {
    let Ok(manifest) = load_project_manifest(manifest_path) else {
        return false;
    };
    let Ok(source_paths) = collect_package_sources(&manifest) else {
        return false;
    };

    source_paths.into_iter().any(|source_path| {
        if let Some((_, open_source)) = open_docs.get(&source_path) {
            return analyze_source(open_source).is_ok();
        }

        let Ok(source) = fs::read_to_string(&source_path) else {
            return false;
        };
        analyze_source(&source).is_ok()
    })
}

fn append_local_dependency_workspace_symbols(
    package_manifest_path: &Path,
    open_docs: &HashMap<PathBuf, (Url, String)>,
    searched_packages: &mut HashSet<PathBuf>,
    covered_files: &mut HashSet<PathBuf>,
    symbols: &mut Vec<SymbolInformation>,
    query: &str,
) -> HashSet<PathBuf> {
    let mut preferred_manifest_paths = HashSet::new();

    for local_dependency_manifest_path in
        local_dependency_manifest_paths_for_package(package_manifest_path)
    {
        if !manifest_has_workspace_symbol_source(&local_dependency_manifest_path, open_docs) {
            continue;
        }

        preferred_manifest_paths.insert(canonicalize_or_clone(&local_dependency_manifest_path));

        if !searched_packages.insert(local_dependency_manifest_path.clone()) {
            continue;
        }

        append_workspace_member_symbols(
            &local_dependency_manifest_path,
            open_docs,
            searched_packages,
            covered_files,
            symbols,
            query,
        );
    }

    preferred_manifest_paths
}

#[allow(deprecated)]
fn append_workspace_member_symbols(
    member_manifest_path: &Path,
    open_docs: &HashMap<PathBuf, (Url, String)>,
    searched_packages: &mut HashSet<PathBuf>,
    covered_files: &mut HashSet<PathBuf>,
    symbols: &mut Vec<SymbolInformation>,
    query: &str,
) {
    match analyze_package(member_manifest_path) {
        Ok(member_package) => {
            append_package_workspace_symbols(
                &member_package,
                open_docs,
                covered_files,
                symbols,
                query,
                false,
            );
            let preferred_local_dependency_manifest_paths =
                append_local_dependency_workspace_symbols(
                    member_manifest_path,
                    open_docs,
                    searched_packages,
                    covered_files,
                    symbols,
                    query,
                );
            append_dependency_workspace_symbols_excluding(
                member_manifest_path,
                &preferred_local_dependency_manifest_paths,
                symbols,
                query,
            );
        }
        Err(PackageAnalysisError::SourceDiagnostics { .. }) => {
            let Ok(member_manifest) = load_project_manifest(member_manifest_path) else {
                return;
            };
            append_manifest_source_workspace_symbols(
                &member_manifest,
                open_docs,
                covered_files,
                symbols,
                query,
            );
            let preferred_local_dependency_manifest_paths =
                append_local_dependency_workspace_symbols(
                    member_manifest_path,
                    open_docs,
                    searched_packages,
                    covered_files,
                    symbols,
                    query,
                );
            append_dependency_workspace_symbols_excluding(
                member_manifest_path,
                &preferred_local_dependency_manifest_paths,
                symbols,
                query,
            );
        }
        Err(PackageAnalysisError::Project(_)) => {
            let Ok(member_manifest) = load_project_manifest(member_manifest_path) else {
                return;
            };
            append_manifest_source_workspace_symbols(
                &member_manifest,
                open_docs,
                covered_files,
                symbols,
                query,
            );
            let preferred_local_dependency_manifest_paths =
                append_local_dependency_workspace_symbols(
                    member_manifest_path,
                    open_docs,
                    searched_packages,
                    covered_files,
                    symbols,
                    query,
                );
            append_dependency_workspace_symbols_excluding(
                member_manifest_path,
                &preferred_local_dependency_manifest_paths,
                symbols,
                query,
            );
        }
        Err(error) if is_interface_artifact_failure(&error) => {
            let Ok(member_manifest) = load_project_manifest(member_manifest_path) else {
                return;
            };
            append_manifest_source_workspace_symbols(
                &member_manifest,
                open_docs,
                covered_files,
                symbols,
                query,
            );
            let preferred_local_dependency_manifest_paths =
                append_local_dependency_workspace_symbols(
                    member_manifest_path,
                    open_docs,
                    searched_packages,
                    covered_files,
                    symbols,
                    query,
                );
            append_dependency_workspace_symbols_excluding(
                member_manifest_path,
                &preferred_local_dependency_manifest_paths,
                symbols,
                query,
            );
        }
        Err(_) => {}
    }
}

fn is_interface_artifact_failure(error: &PackageAnalysisError) -> bool {
    match error {
        PackageAnalysisError::InterfaceNotFound { .. }
        | PackageAnalysisError::InterfaceParse { .. } => true,
        PackageAnalysisError::Read { path, .. } => {
            path.extension().is_some_and(|extension| extension == "qi")
        }
        _ => false,
    }
}

#[cfg(test)]
fn workspace_symbols_for_documents(
    documents: Vec<(Url, String)>,
    query: &str,
) -> Vec<SymbolInformation> {
    workspace_symbols_for_documents_and_roots(documents, &[], query)
}

fn workspace_symbols_for_documents_and_roots(
    documents: Vec<(Url, String)>,
    workspace_roots: &[PathBuf],
    query: &str,
) -> Vec<SymbolInformation> {
    let normalized_query = query.trim().to_ascii_lowercase();
    let mut open_docs = HashMap::<PathBuf, (Url, String)>::new();
    let mut non_file_docs = Vec::<(Url, String)>::new();
    for (uri, source) in documents {
        if let Ok(path) = uri.to_file_path() {
            open_docs.insert(path, (uri, source));
        } else {
            non_file_docs.push((uri, source));
        }
    }

    let mut file_paths = open_docs.keys().cloned().collect::<Vec<_>>();
    file_paths.sort();

    let mut searched_packages = HashSet::<PathBuf>::new();
    let mut covered_files = HashSet::<PathBuf>::new();
    let mut symbols = Vec::<SymbolInformation>::new();

    for path in file_paths {
        if covered_files.contains(&path) {
            continue;
        }

        let Some((uri, source)) = open_docs.get(&path) else {
            continue;
        };

        match analyze_package(&path) {
            Ok(package) => {
                let manifest_path = package.manifest().manifest_path.clone();
                if !searched_packages.insert(manifest_path) {
                    continue;
                }
                append_package_workspace_symbols(
                    &package,
                    &open_docs,
                    &mut covered_files,
                    &mut symbols,
                    &normalized_query,
                    false,
                );
                let preferred_local_dependency_manifest_paths =
                    append_local_dependency_workspace_symbols(
                        package.manifest().manifest_path.as_path(),
                        &open_docs,
                        &mut searched_packages,
                        &mut covered_files,
                        &mut symbols,
                        &normalized_query,
                    );
                append_dependency_workspace_symbols_excluding(
                    package.manifest().manifest_path.as_path(),
                    &preferred_local_dependency_manifest_paths,
                    &mut symbols,
                    &normalized_query,
                );

                for member_manifest_path in workspace_member_manifest_paths_for_package(
                    package.manifest().manifest_path.as_path(),
                ) {
                    if !searched_packages.insert(member_manifest_path.clone()) {
                        continue;
                    }
                    append_workspace_member_symbols(
                        &member_manifest_path,
                        &open_docs,
                        &mut searched_packages,
                        &mut covered_files,
                        &mut symbols,
                        &normalized_query,
                    );
                }
            }
            Err(PackageAnalysisError::SourceDiagnostics { .. }) => {
                let Ok(manifest) = load_project_manifest(&path) else {
                    covered_files.insert(path.clone());
                    if let Ok(analysis) = analyze_source(source) {
                        symbols.extend(workspace_symbols_for_analysis(
                            uri,
                            source,
                            &analysis,
                            &normalized_query,
                        ));
                    }
                    continue;
                };

                let manifest_path = manifest.manifest_path.clone();
                if !searched_packages.insert(manifest_path) {
                    continue;
                }

                append_manifest_source_workspace_symbols(
                    &manifest,
                    &open_docs,
                    &mut covered_files,
                    &mut symbols,
                    &normalized_query,
                );
                let preferred_local_dependency_manifest_paths =
                    append_local_dependency_workspace_symbols(
                        manifest.manifest_path.as_path(),
                        &open_docs,
                        &mut searched_packages,
                        &mut covered_files,
                        &mut symbols,
                        &normalized_query,
                    );

                let workspace_member_manifests =
                    workspace_member_manifest_paths_for_package(manifest.manifest_path.as_path());

                append_dependency_workspace_symbols_excluding(
                    manifest.manifest_path.as_path(),
                    &preferred_local_dependency_manifest_paths,
                    &mut symbols,
                    &normalized_query,
                );

                for member_manifest_path in workspace_member_manifests {
                    if !searched_packages.insert(member_manifest_path.clone()) {
                        continue;
                    }
                    append_workspace_member_symbols(
                        &member_manifest_path,
                        &open_docs,
                        &mut searched_packages,
                        &mut covered_files,
                        &mut symbols,
                        &normalized_query,
                    );
                }
            }
            Err(PackageAnalysisError::Project(_)) => {
                let Ok(manifest) = load_project_manifest(&path) else {
                    covered_files.insert(path.clone());
                    if let Ok(analysis) = analyze_source(source) {
                        symbols.extend(workspace_symbols_for_analysis(
                            uri,
                            source,
                            &analysis,
                            &normalized_query,
                        ));
                    }
                    continue;
                };

                let manifest_path = manifest.manifest_path.clone();
                if !searched_packages.insert(manifest_path) {
                    continue;
                }

                append_manifest_source_workspace_symbols(
                    &manifest,
                    &open_docs,
                    &mut covered_files,
                    &mut symbols,
                    &normalized_query,
                );
                let preferred_local_dependency_manifest_paths =
                    append_local_dependency_workspace_symbols(
                        manifest.manifest_path.as_path(),
                        &open_docs,
                        &mut searched_packages,
                        &mut covered_files,
                        &mut symbols,
                        &normalized_query,
                    );

                let workspace_member_manifests =
                    workspace_member_manifest_paths_for_package(manifest.manifest_path.as_path());

                append_dependency_workspace_symbols_excluding(
                    manifest.manifest_path.as_path(),
                    &preferred_local_dependency_manifest_paths,
                    &mut symbols,
                    &normalized_query,
                );

                for member_manifest_path in workspace_member_manifests {
                    if !searched_packages.insert(member_manifest_path.clone()) {
                        continue;
                    }
                    append_workspace_member_symbols(
                        &member_manifest_path,
                        &open_docs,
                        &mut searched_packages,
                        &mut covered_files,
                        &mut symbols,
                        &normalized_query,
                    );
                }
            }
            Err(error) if is_interface_artifact_failure(&error) => {
                let Ok(manifest) = load_project_manifest(&path) else {
                    covered_files.insert(path.clone());
                    if let Ok(analysis) = analyze_source(source) {
                        symbols.extend(workspace_symbols_for_analysis(
                            uri,
                            source,
                            &analysis,
                            &normalized_query,
                        ));
                    }
                    continue;
                };

                let manifest_path = manifest.manifest_path.clone();
                if !searched_packages.insert(manifest_path) {
                    continue;
                }

                append_manifest_source_workspace_symbols(
                    &manifest,
                    &open_docs,
                    &mut covered_files,
                    &mut symbols,
                    &normalized_query,
                );
                let preferred_local_dependency_manifest_paths =
                    append_local_dependency_workspace_symbols(
                        manifest.manifest_path.as_path(),
                        &open_docs,
                        &mut searched_packages,
                        &mut covered_files,
                        &mut symbols,
                        &normalized_query,
                    );
                append_dependency_workspace_symbols_excluding(
                    manifest.manifest_path.as_path(),
                    &preferred_local_dependency_manifest_paths,
                    &mut symbols,
                    &normalized_query,
                );

                for member_manifest_path in
                    workspace_member_manifest_paths_for_package(manifest.manifest_path.as_path())
                {
                    if !searched_packages.insert(member_manifest_path.clone()) {
                        continue;
                    }
                    append_workspace_member_symbols(
                        &member_manifest_path,
                        &open_docs,
                        &mut searched_packages,
                        &mut covered_files,
                        &mut symbols,
                        &normalized_query,
                    );
                }
            }
            Err(_) => {
                covered_files.insert(path.clone());
                if let Ok(analysis) = analyze_source(source) {
                    symbols.extend(workspace_symbols_for_analysis(
                        uri,
                        source,
                        &analysis,
                        &normalized_query,
                    ));
                }
            }
        }
    }

    let mut sorted_workspace_roots = workspace_roots
        .iter()
        .map(|path| canonicalize_or_clone(path))
        .collect::<Vec<_>>();
    sorted_workspace_roots.sort();
    sorted_workspace_roots.dedup();

    for workspace_root in sorted_workspace_roots {
        let Ok(manifest) = load_project_manifest(&workspace_root) else {
            continue;
        };
        append_manifest_and_workspace_symbols(
            &manifest,
            &open_docs,
            &mut searched_packages,
            &mut covered_files,
            &mut symbols,
            &normalized_query,
        );
    }

    for (uri, source) in non_file_docs {
        if let Ok(analysis) = analyze_source(&source) {
            symbols.extend(workspace_symbols_for_analysis(
                &uri,
                &source,
                &analysis,
                &normalized_query,
            ));
        }
    }

    symbols.sort_by_key(|symbol| {
        (
            symbol.name.to_ascii_lowercase(),
            symbol.location.uri.to_string(),
            symbol.location.range.start.line,
            symbol.location.range.start.character,
        )
    });
    symbols.dedup();
    symbols
}

#[allow(deprecated)]
fn workspace_symbols_for_dependencies(
    dependencies: &[DependencyInterface],
    query: &str,
) -> Vec<SymbolInformation> {
    let mut symbols = Vec::new();

    for dependency in dependencies {
        let interface_path = fs::canonicalize(dependency.interface_path())
            .unwrap_or_else(|_| dependency.interface_path().to_path_buf());
        let Ok(uri) = Url::from_file_path(&interface_path) else {
            continue;
        };
        let Ok(source) = fs::read_to_string(&interface_path) else {
            continue;
        };
        let source = source.replace("\r\n", "\n");

        for symbol in dependency.workspace_symbols() {
            if !query.is_empty() && !symbol.name.to_ascii_lowercase().contains(query) {
                continue;
            }
            let Some(span) = dependency.definition_span_for_symbol(&symbol) else {
                continue;
            };

            symbols.push(SymbolInformation {
                name: symbol.name.clone(),
                kind: document_symbol_kind(symbol.kind),
                tags: None,
                deprecated: None,
                location: Location::new(uri.clone(), span_to_range(&source, span)),
                container_name: Some(symbol.package_name.clone()),
            });
        }
    }

    symbols
}

fn package_path_segments(source: &str) -> Option<Vec<&str>> {
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        let rest = trimmed.strip_prefix("package ")?;
        let segments = rest
            .split('.')
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();
        return (!segments.is_empty()).then_some(segments);
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AutoImportKind {
    Value,
    Type,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AutoImportMissingDependencyEdit {
    dependency_name: String,
    manifest_uri: Url,
    edit: TextEdit,
}

fn auto_import_code_actions_for_source(
    uri: &Url,
    source: &str,
    diagnostics: &[tower_lsp::lsp_types::Diagnostic],
    documents: Vec<(Url, String)>,
    workspace_roots: &[PathBuf],
) -> Vec<CodeActionOrCommand> {
    let mut actions = Vec::new();
    let mut seen_paths = HashSet::<Vec<String>>::new();
    let document_sources = documents.iter().cloned().collect::<HashMap<Url, String>>();

    for diagnostic in diagnostics {
        let Some((kind, name)) = unresolved_auto_import_request(diagnostic) else {
            continue;
        };

        for symbol in
            workspace_symbols_for_documents_and_roots(documents.clone(), workspace_roots, &name)
        {
            if symbol.name != name
                || symbol.location.uri == *uri
                || !supports_auto_import_symbol_kind(symbol.kind, kind)
            {
                continue;
            }
            let Some(import_path) = auto_import_path_for_symbol(&symbol, &document_sources) else {
                continue;
            };
            if source_already_imports_path(source, &import_path)
                || !seen_paths.insert(import_path.clone())
            {
                continue;
            }

            let edit = auto_import_insertion_edit(source, &import_path);
            let missing_dependency_edit = auto_import_missing_workspace_dependency_edit(
                uri,
                &symbol.location.uri,
                &document_sources,
            );
            let mut changes = HashMap::new();
            changes.insert(uri.clone(), vec![edit]);
            let title = if let Some(missing_dependency_edit) = &missing_dependency_edit {
                changes.insert(
                    missing_dependency_edit.manifest_uri.clone(),
                    vec![missing_dependency_edit.edit.clone()],
                );
                format!(
                    "Import `{}` and add dependency `{}`",
                    import_path.join("."),
                    missing_dependency_edit.dependency_name
                )
            } else {
                format!("Import `{}`", import_path.join("."))
            };

            actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                title,
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(WorkspaceEdit::new(changes)),
                command: None,
                is_preferred: None,
                disabled: None,
                data: None,
            }));
        }
    }

    actions.sort_by_key(|action| match action {
        CodeActionOrCommand::CodeAction(action) => action.title.clone(),
        CodeActionOrCommand::Command(command) => command.title.clone(),
    });
    actions
}

fn import_missing_dependency_code_actions_for_position(
    uri: &Url,
    source: &str,
    position: tower_lsp::lsp_types::Position,
    documents: Vec<(Url, String)>,
    workspace_roots: &[PathBuf],
) -> Vec<CodeActionOrCommand> {
    let document_sources = documents.iter().cloned().collect::<HashMap<Url, String>>();
    let Some(import_path) = import_path_segments_at_position(source, position) else {
        return Vec::new();
    };
    let Some(symbol) = workspace_symbol_for_import_path(
        uri,
        &import_path,
        documents,
        workspace_roots,
        &document_sources,
    ) else {
        return Vec::new();
    };
    let Some(missing_dependency_edit) =
        auto_import_missing_workspace_dependency_edit(uri, &symbol.location.uri, &document_sources)
    else {
        return Vec::new();
    };

    let mut changes = HashMap::new();
    changes.insert(
        missing_dependency_edit.manifest_uri.clone(),
        vec![missing_dependency_edit.edit],
    );

    vec![CodeActionOrCommand::CodeAction(CodeAction {
        title: format!(
            "Add dependency `{}` for `{}`",
            missing_dependency_edit.dependency_name,
            import_path.join(".")
        ),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: None,
        edit: Some(WorkspaceEdit::new(changes)),
        command: None,
        is_preferred: None,
        disabled: None,
        data: None,
    })]
}

fn import_path_segments_at_position(
    source: &str,
    position: tower_lsp::lsp_types::Position,
) -> Option<Vec<String>> {
    let offset = position_to_offset(source, position)?;
    let (tokens, _) = lex(source);
    if let Some(path_segments) = import_path_segments_in_tokens_at_offset(&tokens, offset) {
        return Some(path_segments);
    }
    if let Ok(analysis) = analyze_source(source)
        && let Some(occurrence) = analyzed_import_binding_at(source, &analysis, offset)
    {
        return Some(occurrence.path_segments);
    }

    let binding = broken_source_import_binding_at(source, position)?;
    let mut path_segments = binding.import_prefix;
    path_segments.push(binding.imported_name);
    Some(path_segments)
}

fn import_path_segments_in_tokens_at_offset(
    tokens: &[Token],
    offset: usize,
) -> Option<Vec<String>> {
    let mut index = 0usize;

    if tokens.get(index).map(|token| token.kind) == Some(TokenKind::Package)
        && let Some((_, next_index)) = top_level_import_path_in_tokens(tokens, index + 1)
    {
        index = next_index;
    }

    while tokens.get(index).map(|token| token.kind) == Some(TokenKind::Use) {
        let Some((next_index, use_paths)) = top_level_import_paths_after_use(tokens, index + 1)
        else {
            break;
        };
        for path in use_paths {
            if path.iter().any(|(_, span)| span.contains(offset)) {
                return Some(path.into_iter().map(|(segment, _)| segment).collect());
            }
        }
        index = next_index;
    }

    None
}

fn workspace_symbol_for_import_path(
    current_uri: &Url,
    import_path: &[String],
    documents: Vec<(Url, String)>,
    workspace_roots: &[PathBuf],
    document_sources: &HashMap<Url, String>,
) -> Option<SymbolInformation> {
    let imported_name = import_path.last()?;
    let mut matches =
        workspace_symbols_for_documents_and_roots(documents, workspace_roots, imported_name)
            .into_iter()
            .filter(|symbol| symbol.location.uri != *current_uri)
            .filter(|symbol| {
                auto_import_path_for_symbol(symbol, document_sources)
                    .is_some_and(|candidate| candidate == import_path)
            })
            .collect::<Vec<_>>();
    if matches.len() == 1 {
        return matches.pop();
    }
    None
}

fn unresolved_auto_import_request(
    diagnostic: &tower_lsp::lsp_types::Diagnostic,
) -> Option<(AutoImportKind, String)> {
    let kind = match diagnostic.code.as_ref()? {
        NumberOrString::String(code) if code == UNRESOLVED_VALUE_CODE => AutoImportKind::Value,
        NumberOrString::String(code) if code == UNRESOLVED_TYPE_CODE => AutoImportKind::Type,
        _ => return None,
    };
    let start = diagnostic.message.find('`')?;
    let rest = &diagnostic.message[start + 1..];
    let end = rest.find('`')?;
    Some((kind, rest[..end].to_owned()))
}

fn supports_auto_import_symbol_kind(kind: LspSymbolKind, unresolved_kind: AutoImportKind) -> bool {
    match unresolved_kind {
        AutoImportKind::Value => matches!(
            kind,
            LspSymbolKind::FUNCTION | LspSymbolKind::CONSTANT | LspSymbolKind::VARIABLE
        ),
        AutoImportKind::Type => matches!(
            kind,
            LspSymbolKind::STRUCT
                | LspSymbolKind::ENUM
                | LspSymbolKind::INTERFACE
                | LspSymbolKind::CLASS
        ),
    }
}

fn auto_import_path_for_symbol(
    symbol: &SymbolInformation,
    document_sources: &HashMap<Url, String>,
) -> Option<Vec<String>> {
    let mut segments = package_path_segments_for_uri(&symbol.location.uri, document_sources)
        .or_else(|| {
            symbol.container_name.as_ref().map(|container_name| {
                container_name
                    .split('.')
                    .map(str::trim)
                    .filter(|segment| !segment.is_empty())
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
        })?;
    if segments.is_empty() {
        return None;
    }
    segments.push(symbol.name.clone());
    Some(segments)
}

fn package_path_segments_for_uri(
    uri: &Url,
    document_sources: &HashMap<Url, String>,
) -> Option<Vec<String>> {
    let source = source_for_uri(uri, document_sources)?;
    package_path_segments(&source).map(|segments| {
        segments
            .into_iter()
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>()
    })
}

fn auto_import_missing_workspace_dependency_edit(
    current_uri: &Url,
    symbol_uri: &Url,
    document_sources: &HashMap<Url, String>,
) -> Option<AutoImportMissingDependencyEdit> {
    let current_manifest = package_manifest_for_uri(current_uri)?;
    let symbol_manifest = package_manifest_for_uri(symbol_uri)?;
    let current_manifest_canonical = canonicalize_or_clone(&current_manifest.manifest_path);
    let symbol_manifest_canonical = canonicalize_or_clone(&symbol_manifest.manifest_path);

    if current_manifest_canonical == symbol_manifest_canonical {
        return None;
    }

    if !workspace_member_manifest_paths_for_package(&current_manifest.manifest_path)
        .into_iter()
        .any(|manifest_path| canonicalize_or_clone(&manifest_path) == symbol_manifest_canonical)
    {
        return None;
    }

    if package_manifest_references_target(&current_manifest, &symbol_manifest_canonical) {
        return None;
    }

    let dependency_name = manifest_package_name(&symbol_manifest).ok()?.to_owned();
    let current_manifest_dir = current_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    let symbol_manifest_dir = symbol_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    let dependency_path = relative_path_from(current_manifest_dir, symbol_manifest_dir);
    let manifest_uri = Url::from_file_path(&current_manifest.manifest_path).ok()?;
    let manifest_source = source_for_uri(&manifest_uri, document_sources)?;
    let updated_manifest = render_manifest_with_added_local_dependency(
        &manifest_source,
        &dependency_name,
        &dependency_path,
    )
    .ok()?;

    if updated_manifest == manifest_source {
        return None;
    }

    Some(AutoImportMissingDependencyEdit {
        dependency_name,
        manifest_uri,
        edit: TextEdit::new(
            span_to_range(&manifest_source, Span::new(0, manifest_source.len())),
            updated_manifest,
        ),
    })
}

fn source_for_uri(uri: &Url, document_sources: &HashMap<Url, String>) -> Option<String> {
    if let Some(source) = document_sources.get(uri) {
        return Some(source.clone());
    }

    let path = uri.to_file_path().ok()?;
    fs::read_to_string(path).ok()
}

fn package_manifest_for_uri(uri: &Url) -> Option<ql_project::ProjectManifest> {
    let path = uri.to_file_path().ok()?;
    let manifest = load_project_manifest(&path).ok()?;
    manifest.package.as_ref()?;
    Some(manifest)
}

fn package_manifest_references_target(
    manifest: &ql_project::ProjectManifest,
    target_manifest_canonical: &Path,
) -> bool {
    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    manifest.references.packages.iter().any(|reference| {
        load_project_manifest(&manifest_dir.join(reference))
            .ok()
            .is_some_and(|dependency_manifest| {
                canonicalize_or_clone(&dependency_manifest.manifest_path)
                    == target_manifest_canonical
            })
    })
}

fn relative_path_from(from: &Path, to: &Path) -> String {
    let from = normalize_path(from);
    let to = normalize_path(to);
    let from_parts = from
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>();
    let to_parts = to
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>();

    let mut common = 0usize;
    while common < from_parts.len()
        && common < to_parts.len()
        && from_parts[common] == to_parts[common]
    {
        common += 1;
    }

    if common == 0 && !from_parts.is_empty() && !to_parts.is_empty() && from_parts[0] != to_parts[0]
    {
        return to;
    }

    let mut relative = Vec::new();
    relative.extend(std::iter::repeat_n(
        "..",
        from_parts.len().saturating_sub(common),
    ));
    relative.extend_from_slice(&to_parts[common..]);
    if relative.is_empty() {
        ".".to_owned()
    } else {
        relative.join("/")
    }
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn source_already_imports_path(source: &str, import_path: &[String]) -> bool {
    let (tokens, _) = lex(source);
    top_level_import_paths_in_tokens(&tokens)
        .into_iter()
        .any(|candidate| candidate == import_path)
}

fn auto_import_insertion_edit(source: &str, import_path: &[String]) -> TextEdit {
    let offset = auto_import_insert_offset(source);
    let import_stmt = format!("use {}", import_path.join("."));
    TextEdit::new(
        span_to_range(source, Span::new(offset, offset)),
        auto_import_insert_text(source, offset, &import_stmt),
    )
}

fn auto_import_insert_offset(source: &str) -> usize {
    let (tokens, _) = lex(source);
    let Some(anchor_end) = top_level_import_anchor_end_in_tokens(&tokens) else {
        return leading_comment_or_blank_insert_offset(source);
    };
    line_break_end_offset(source, anchor_end)
}

fn top_level_import_anchor_end_in_tokens(tokens: &[Token]) -> Option<usize> {
    let mut index = 0usize;
    let mut anchor_end = None;

    if tokens.get(index).map(|token| token.kind) == Some(TokenKind::Package) {
        let (package_path, next_index) = top_level_import_path_in_tokens(tokens, index + 1)?;
        anchor_end = package_path.last().map(|segment| segment.1.end);
        index = next_index;
    }

    while tokens.get(index).map(|token| token.kind) == Some(TokenKind::Use) {
        let (next_index, import_paths) = top_level_import_paths_after_use(tokens, index + 1)?;
        anchor_end = import_paths
            .last()
            .and_then(|path| path.last().map(|segment| segment.1.end))
            .or(anchor_end);
        index = next_index;
    }

    anchor_end
}

fn leading_comment_or_blank_insert_offset(source: &str) -> usize {
    let mut offset = 0usize;
    for chunk in source.split_inclusive('\n') {
        let trimmed = chunk.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            offset += chunk.len();
            continue;
        }
        break;
    }
    offset
}

fn line_break_end_offset(source: &str, offset: usize) -> usize {
    source[offset..]
        .find('\n')
        .map(|relative| offset + relative + 1)
        .unwrap_or(source.len())
}

fn auto_import_insert_text(source: &str, offset: usize, import_stmt: &str) -> String {
    if offset == 0 {
        return if source.is_empty() {
            format!("{import_stmt}\n")
        } else {
            format!("{import_stmt}\n\n")
        };
    }

    let suffix = &source[offset..];
    if suffix.starts_with('\n') {
        format!("{import_stmt}\n")
    } else if suffix.is_empty() {
        format!("{import_stmt}\n")
    } else {
        format!("{import_stmt}\n\n")
    }
}

fn top_level_import_paths_in_tokens(tokens: &[Token]) -> Vec<Vec<String>> {
    let mut paths = Vec::new();
    let mut index = 0usize;

    if tokens.get(index).map(|token| token.kind) == Some(TokenKind::Package)
        && let Some((_, next_index)) = top_level_import_path_in_tokens(tokens, index + 1)
    {
        index = next_index;
    }

    while tokens.get(index).map(|token| token.kind) == Some(TokenKind::Use) {
        let Some((next_index, use_paths)) = top_level_import_paths_after_use(tokens, index + 1)
        else {
            break;
        };
        paths.extend(
            use_paths
                .into_iter()
                .map(|path| path.into_iter().map(|segment| segment.0).collect()),
        );
        index = next_index;
    }

    paths
}

fn top_level_import_paths_after_use(
    tokens: &[Token],
    index: usize,
) -> Option<(usize, Vec<Vec<(String, Span)>>)> {
    let (prefix, mut index) = top_level_import_path_in_tokens(tokens, index)?;
    if tokens.get(index).map(|token| token.kind) == Some(TokenKind::Dot)
        && tokens.get(index + 1).map(|token| token.kind) == Some(TokenKind::LBrace)
    {
        index += 2;
        let mut bindings = Vec::new();
        loop {
            if tokens.get(index).map(|token| token.kind) == Some(TokenKind::RBrace) {
                return Some((index + 1, bindings));
            }

            let item = top_level_import_ident_token(tokens, index)?;
            let mut path = prefix.clone();
            path.push((item.text.clone(), item.span));
            index += 1;
            index = top_level_import_alias_in_tokens(tokens, index)?;
            bindings.push(path);

            match tokens.get(index).map(|token| token.kind) {
                Some(TokenKind::Comma) => index += 1,
                Some(TokenKind::RBrace) => return Some((index + 1, bindings)),
                _ => return None,
            }
        }
    }

    let index = top_level_import_alias_in_tokens(tokens, index)?;
    Some((index, vec![prefix]))
}

fn top_level_import_path_in_tokens(
    tokens: &[Token],
    index: usize,
) -> Option<(Vec<(String, Span)>, usize)> {
    let mut index = index;
    let first = top_level_import_ident_token(tokens, index)?;
    let mut segments = vec![(first.text.clone(), first.span)];
    index += 1;

    while tokens.get(index).map(|token| token.kind) == Some(TokenKind::Dot)
        && tokens.get(index + 1).map(|token| token.kind) == Some(TokenKind::Ident)
    {
        let segment = tokens.get(index + 1)?;
        segments.push((segment.text.clone(), segment.span));
        index += 2;
    }

    Some((segments, index))
}

fn top_level_import_alias_in_tokens(tokens: &[Token], index: usize) -> Option<usize> {
    if tokens.get(index).map(|token| token.kind) != Some(TokenKind::As) {
        return Some(index);
    }

    top_level_import_ident_token(tokens, index + 1)?;
    Some(index + 2)
}

fn top_level_import_ident_token(tokens: &[Token], index: usize) -> Option<&Token> {
    let token = tokens.get(index)?;
    (token.kind == TokenKind::Ident).then_some(token)
}

fn supports_workspace_import_definition(kind: ql_analysis::SymbolKind) -> bool {
    matches!(
        kind,
        ql_analysis::SymbolKind::Function
            | ql_analysis::SymbolKind::Const
            | ql_analysis::SymbolKind::Static
            | ql_analysis::SymbolKind::Struct
            | ql_analysis::SymbolKind::Enum
            | ql_analysis::SymbolKind::Variant
            | ql_analysis::SymbolKind::Trait
            | ql_analysis::SymbolKind::TypeAlias
    )
}

fn supports_workspace_source_root_definition_references(kind: ql_analysis::SymbolKind) -> bool {
    matches!(
        kind,
        ql_analysis::SymbolKind::Function
            | ql_analysis::SymbolKind::Const
            | ql_analysis::SymbolKind::Static
            | ql_analysis::SymbolKind::Struct
            | ql_analysis::SymbolKind::Enum
            | ql_analysis::SymbolKind::Variant
            | ql_analysis::SymbolKind::Trait
            | ql_analysis::SymbolKind::TypeAlias
            | ql_analysis::SymbolKind::Field
            | ql_analysis::SymbolKind::Method
    )
}

fn supports_workspace_source_root_definition_rename(kind: ql_analysis::SymbolKind) -> bool {
    matches!(
        kind,
        ql_analysis::SymbolKind::Function
            | ql_analysis::SymbolKind::Const
            | ql_analysis::SymbolKind::Static
            | ql_analysis::SymbolKind::Struct
            | ql_analysis::SymbolKind::Enum
            | ql_analysis::SymbolKind::Trait
            | ql_analysis::SymbolKind::TypeAlias
    )
}

fn supports_workspace_source_root_member_rename(kind: ql_analysis::SymbolKind) -> bool {
    matches!(
        kind,
        ql_analysis::SymbolKind::Variant
            | ql_analysis::SymbolKind::Field
            | ql_analysis::SymbolKind::Method
    )
}

fn supports_workspace_import_type_definition(kind: ql_analysis::SymbolKind) -> bool {
    matches!(
        kind,
        ql_analysis::SymbolKind::Struct
            | ql_analysis::SymbolKind::Enum
            | ql_analysis::SymbolKind::Trait
            | ql_analysis::SymbolKind::TypeAlias
    )
}

fn supports_workspace_dependency_definition(kind: ql_analysis::SymbolKind) -> bool {
    matches!(
        kind,
        ql_analysis::SymbolKind::Function
            | ql_analysis::SymbolKind::Const
            | ql_analysis::SymbolKind::Static
            | ql_analysis::SymbolKind::Struct
            | ql_analysis::SymbolKind::Enum
            | ql_analysis::SymbolKind::Variant
            | ql_analysis::SymbolKind::Trait
            | ql_analysis::SymbolKind::TypeAlias
            | ql_analysis::SymbolKind::Field
            | ql_analysis::SymbolKind::Method
    )
}

fn normalized_relative_source_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn normalized_dependency_source_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_owned()
}

fn package_source_path_for_module(
    package: &ql_analysis::PackageAnalysis,
    module_path: &Path,
) -> Option<String> {
    let package_root = package.manifest().manifest_path.parent()?;
    let relative_path = module_path.strip_prefix(package_root).ok()?;
    Some(normalized_relative_source_path(relative_path))
}

fn package_module_matches_dependency_source_path(
    package: &ql_analysis::PackageAnalysis,
    module_path: &Path,
    dependency_source_path: &str,
) -> bool {
    let Some(package_root) = package.manifest().manifest_path.parent() else {
        return false;
    };
    let Ok(relative_path) = module_path.strip_prefix(package_root) else {
        return false;
    };
    normalized_relative_source_path(relative_path)
        == normalized_dependency_source_path(dependency_source_path)
}

fn open_document_snapshot(
    open_docs: &OpenDocuments,
    path: &Path,
) -> Option<(Url, String, Analysis)> {
    let canonical_path = canonicalize_or_clone(path);
    let (uri, source) = open_docs.get(&canonical_path)?;
    let analysis = analyze_source(source).ok()?;
    Some((uri.clone(), source.clone(), analysis))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct WorkspaceSourceSymbolMatch {
    location: Location,
    kind: ql_analysis::SymbolKind,
}

fn extend_workspace_import_symbol_matches_with_open_docs(
    package: &ql_analysis::PackageAnalysis,
    current_path: Option<&Path>,
    current_source: Option<&str>,
    current_analysis: Option<&Analysis>,
    open_docs: &OpenDocuments,
    import_prefix: &[String],
    imported_name: &str,
    supports_kind: fn(ql_analysis::SymbolKind) -> bool,
    matches: &mut Vec<WorkspaceSourceSymbolMatch>,
) {
    for module in package.modules() {
        let module_path = module.path();
        if current_path
            .is_some_and(|path| canonicalize_or_clone(path) == canonicalize_or_clone(module_path))
        {
            let Some(module_source) = current_source else {
                continue;
            };
            let module_analysis = current_analysis.unwrap_or(module.analysis());
            let Some(package_segments) = package_path_segments(module_source) else {
                continue;
            };
            if package_segments.len() != import_prefix.len()
                || !package_segments
                    .iter()
                    .zip(import_prefix)
                    .all(|(left, right)| *left == right)
            {
                continue;
            }
            let Ok(module_uri) = Url::from_file_path(module_path) else {
                continue;
            };
            for symbol in module_analysis.document_symbols() {
                if symbol.name != imported_name || !supports_kind(symbol.kind) {
                    continue;
                }
                matches.push(WorkspaceSourceSymbolMatch {
                    location: Location::new(
                        module_uri.clone(),
                        span_to_range(module_source, symbol.span),
                    ),
                    kind: symbol.kind,
                });
            }
            continue;
        }

        if let Some((module_uri, module_source, module_analysis)) =
            open_document_snapshot(open_docs, module_path)
        {
            let Some(package_segments) = package_path_segments(&module_source) else {
                continue;
            };
            if package_segments.len() != import_prefix.len()
                || !package_segments
                    .iter()
                    .zip(import_prefix)
                    .all(|(left, right)| *left == right)
            {
                continue;
            }
            for symbol in module_analysis.document_symbols() {
                if symbol.name != imported_name || !supports_kind(symbol.kind) {
                    continue;
                }
                matches.push(WorkspaceSourceSymbolMatch {
                    location: Location::new(
                        module_uri.clone(),
                        span_to_range(&module_source, symbol.span),
                    ),
                    kind: symbol.kind,
                });
            }
            continue;
        }

        let Ok(source) = fs::read_to_string(module_path) else {
            continue;
        };
        let module_source = source.replace("\r\n", "\n");
        let Some(package_segments) = package_path_segments(&module_source) else {
            continue;
        };
        if package_segments.len() != import_prefix.len()
            || !package_segments
                .iter()
                .zip(import_prefix)
                .all(|(left, right)| *left == right)
        {
            continue;
        }

        let Ok(module_uri) = Url::from_file_path(module_path) else {
            continue;
        };
        for symbol in module.analysis().document_symbols() {
            if symbol.name != imported_name || !supports_kind(symbol.kind) {
                continue;
            }
            matches.push(WorkspaceSourceSymbolMatch {
                location: Location::new(
                    module_uri.clone(),
                    span_to_range(&module_source, symbol.span),
                ),
                kind: symbol.kind,
            });
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BrokenSourceImportBinding {
    imported_name: String,
    import_prefix: Vec<String>,
    local_name: String,
    imported_span: Span,
    definition_span: Span,
}

fn workspace_source_symbol_matches_for_import_binding_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    import_prefix: &[String],
    imported_name: &str,
    supports_kind: fn(ql_analysis::SymbolKind) -> bool,
) -> Vec<WorkspaceSourceSymbolMatch> {
    let current_path = uri.to_file_path().ok();
    let mut matches = Vec::new();

    extend_workspace_import_symbol_matches_with_open_docs(
        package,
        current_path.as_deref(),
        Some(source),
        analysis,
        open_docs,
        import_prefix,
        imported_name,
        supports_kind,
        &mut matches,
    );

    for candidate_manifest_path in
        source_preferred_manifest_paths_for_package(package.manifest().manifest_path.as_path())
    {
        let Some(member_package) = package_analysis_for_path(&candidate_manifest_path) else {
            continue;
        };
        extend_workspace_import_symbol_matches_with_open_docs(
            &member_package,
            None,
            None,
            None,
            open_docs,
            import_prefix,
            imported_name,
            supports_kind,
            &mut matches,
        );
    }

    matches.sort_by_key(|symbol| {
        (
            symbol.location.uri.to_string(),
            symbol.location.range.start.line,
            symbol.location.range.start.character,
        )
    });
    matches.dedup_by(|left, right| left.location == right.location && left.kind == right.kind);
    matches
}

fn workspace_source_locations_for_import_binding_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    import_prefix: &[String],
    imported_name: &str,
    supports_kind: fn(ql_analysis::SymbolKind) -> bool,
) -> Vec<Location> {
    workspace_source_symbol_matches_for_import_binding_with_open_docs(
        uri,
        source,
        analysis,
        package,
        open_docs,
        import_prefix,
        imported_name,
        supports_kind,
    )
    .into_iter()
    .map(|symbol| symbol.location)
    .collect()
}

fn workspace_source_location_for_import_binding(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    import_prefix: &[String],
    imported_name: &str,
) -> Option<Location> {
    let open_docs = OpenDocuments::new();
    workspace_source_location_for_import_binding_with_open_docs(
        uri,
        source,
        analysis,
        package,
        &open_docs,
        import_prefix,
        imported_name,
    )
}

fn workspace_source_location_for_import_binding_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    import_prefix: &[String],
    imported_name: &str,
) -> Option<Location> {
    let matches = workspace_source_locations_for_import_binding_with_open_docs(
        uri,
        source,
        analysis,
        package,
        open_docs,
        import_prefix,
        imported_name,
        supports_workspace_import_definition,
    );
    (matches.len() == 1).then(|| matches[0].clone())
}

fn workspace_source_type_definition_location_for_import_binding_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    import_prefix: &[String],
    imported_name: &str,
) -> Option<Location> {
    let matches = workspace_source_locations_for_import_binding_with_open_docs(
        uri,
        source,
        analysis,
        package,
        open_docs,
        import_prefix,
        imported_name,
        supports_workspace_import_type_definition,
    );
    (matches.len() == 1).then(|| matches[0].clone())
}

fn workspace_source_kind_for_import_binding_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    import_prefix: &[String],
    imported_name: &str,
    supports_kind: fn(ql_analysis::SymbolKind) -> bool,
) -> Option<ql_analysis::SymbolKind> {
    let matches = workspace_source_symbol_matches_for_import_binding_with_open_docs(
        uri,
        source,
        analysis,
        package,
        open_docs,
        import_prefix,
        imported_name,
        supports_kind,
    );
    (matches.len() == 1).then(|| matches[0].kind)
}

#[derive(Clone, Debug)]
struct AnalyzedImportOccurrence {
    path_segments: Vec<String>,
    imported_span: Span,
    definition_span: Span,
    occurrence_span: Span,
}

fn analyzed_import_binding_at(
    source: &str,
    analysis: &Analysis,
    offset: usize,
) -> Option<AnalyzedImportOccurrence> {
    if let Some((binding, span)) = analysis.import_binding_at(offset) {
        let imported_span = binding
            .path
            .last_segment_span()
            .unwrap_or(binding.definition_span);
        return Some(AnalyzedImportOccurrence {
            path_segments: binding.path.segments,
            imported_span,
            definition_span: binding.definition_span,
            occurrence_span: span,
        });
    }

    let binding = analysis.type_import_binding_at(offset)?;
    let token = lex(source).0.into_iter().find(|token| {
        token.kind == TokenKind::Ident
            && token.text == binding.local_name
            && token.span.contains(offset)
    })?;
    let imported_span = binding
        .path
        .last_segment_span()
        .unwrap_or(binding.definition_span);
    Some(AnalyzedImportOccurrence {
        path_segments: binding.path.segments,
        imported_span,
        definition_span: binding.definition_span,
        occurrence_span: token.span,
    })
}

fn extend_workspace_import_reference_locations_with_open_docs(
    package: &ql_analysis::PackageAnalysis,
    current_path: Option<&Path>,
    open_docs: &OpenDocuments,
    import_path: &[String],
    include_declaration: bool,
    locations: &mut Vec<Location>,
) {
    for module in package.modules() {
        if current_path
            .is_some_and(|path| canonicalize_or_clone(path) == canonicalize_or_clone(module.path()))
        {
            continue;
        }

        let (uri, source, analysis) = if let Some((open_uri, open_source, open_analysis)) =
            open_document_snapshot(open_docs, module.path())
        {
            (open_uri, open_source, open_analysis)
        } else {
            let Ok(source) = fs::read_to_string(module.path()) else {
                continue;
            };
            let Ok(uri) = Url::from_file_path(module.path()) else {
                continue;
            };
            (uri, source.replace("\r\n", "\n"), module.analysis().clone())
        };

        locations.extend(workspace_import_reference_locations_in_source(
            &uri,
            &source,
            &analysis,
            import_path,
            include_declaration,
        ));
    }
}

fn workspace_import_reference_locations_in_source(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    import_path: &[String],
    include_declaration: bool,
) -> Vec<Location> {
    let (tokens, _) = lex(source);
    let mut locations = tokens
        .iter()
        .filter(|token| token.kind == TokenKind::Ident)
        .filter_map(|token| {
            let occurrence = analyzed_import_binding_at(source, analysis, token.span.start)?;
            (occurrence.path_segments.as_slice() == import_path
                && occurrence.occurrence_span == token.span
                && (include_declaration
                    || occurrence.occurrence_span != occurrence.definition_span))
                .then(|| {
                    Location::new(
                        uri.clone(),
                        span_to_range(source, occurrence.occurrence_span),
                    )
                })
        })
        .collect::<Vec<_>>();
    locations.sort_by_key(|location| {
        (
            location.range.start.line,
            location.range.start.character,
            location.range.end.line,
            location.range.end.character,
        )
    });
    locations.dedup_by(|left, right| same_location_anchor(left, right));
    locations
}

fn workspace_import_reference_locations_with_open_docs(
    package: &ql_analysis::PackageAnalysis,
    current_path: Option<&Path>,
    open_docs: &OpenDocuments,
    import_path: &[String],
    include_declaration: bool,
) -> Vec<Location> {
    let mut locations = Vec::new();
    extend_workspace_import_reference_locations_with_open_docs(
        package,
        current_path,
        open_docs,
        import_path,
        include_declaration,
        &mut locations,
    );
    for candidate_manifest_path in
        source_preferred_manifest_paths_for_package(package.manifest().manifest_path.as_path())
    {
        let Some(member_package) = package_analysis_for_path(&candidate_manifest_path) else {
            continue;
        };
        extend_workspace_import_reference_locations_with_open_docs(
            &member_package,
            None,
            open_docs,
            import_path,
            include_declaration,
            &mut locations,
        );
    }
    locations.sort_by_key(|location| {
        (
            location.uri.to_string(),
            location.range.start.line,
            location.range.start.character,
            location.range.end.line,
            location.range.end.character,
        )
    });
    locations.dedup_by(|left, right| same_location_anchor(left, right));
    locations
}

fn workspace_source_definition_for_import(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<GotoDefinitionResponse> {
    let open_docs = OpenDocuments::new();
    workspace_source_definition_for_import_with_open_docs(
        uri, source, analysis, package, &open_docs, position,
    )
}

fn workspace_source_definition_for_import_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<GotoDefinitionResponse> {
    let offset = position_to_offset(source, position)?;
    let (binding, _) = analysis.import_binding_at(offset)?;
    let (imported_name, import_prefix) = binding.path.segments.split_last()?;
    workspace_source_location_for_import_binding_with_open_docs(
        uri,
        source,
        Some(analysis),
        package,
        open_docs,
        import_prefix,
        imported_name,
    )
    .map(GotoDefinitionResponse::Scalar)
}

fn workspace_source_type_definition_for_import(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<GotoTypeDefinitionResponse> {
    let open_docs = OpenDocuments::new();
    workspace_source_type_definition_for_import_with_open_docs(
        uri, source, analysis, package, &open_docs, position,
    )
}

fn workspace_source_type_definition_for_import_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<GotoTypeDefinitionResponse> {
    let offset = position_to_offset(source, position)?;
    let binding = analysis.type_import_binding_at(offset)?;
    let (imported_name, import_prefix) = binding.path.segments.split_last()?;
    workspace_source_type_definition_location_for_import_binding_with_open_docs(
        uri,
        source,
        Some(analysis),
        package,
        open_docs,
        import_prefix,
        imported_name,
    )
    .map(GotoTypeDefinitionResponse::Scalar)
}

fn hover_from_workspace_source_location_with_open_docs(
    current_source: &str,
    current_span: Span,
    source_location: Location,
    open_docs: &OpenDocuments,
) -> Option<Hover> {
    let source_path = source_location.uri.to_file_path().ok()?;
    let (source, analysis) = if let Some((_, open_source, open_analysis)) =
        open_document_snapshot(open_docs, &source_path)
    {
        (open_source, open_analysis)
    } else {
        let source = fs::read_to_string(source_path).ok()?.replace("\r\n", "\n");
        let analysis = analyze_source(&source).ok()?;
        (source, analysis)
    };
    let hover = crate::bridge::hover_for_analysis(&source, &analysis, source_location.range.start)?;

    Some(Hover {
        contents: hover.contents,
        range: Some(span_to_range(current_source, current_span)),
    })
}

fn workspace_source_hover_for_import(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<Hover> {
    let open_docs = OpenDocuments::new();
    workspace_source_hover_for_import_with_open_docs(
        uri, source, analysis, package, &open_docs, position,
    )
}

fn workspace_source_hover_for_import_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<Hover> {
    let offset = position_to_offset(source, position)?;
    let (binding, occurrence_span) = analysis.import_binding_at(offset)?;
    let (imported_name, import_prefix) = binding.path.segments.split_last()?;
    let source_location = workspace_source_location_for_import_binding_with_open_docs(
        uri,
        source,
        Some(analysis),
        package,
        open_docs,
        import_prefix,
        imported_name,
    )?;

    hover_from_workspace_source_location_with_open_docs(
        source,
        occurrence_span,
        source_location,
        open_docs,
    )
}

fn workspace_source_definition_for_import_in_broken_source(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<GotoDefinitionResponse> {
    let open_docs = OpenDocuments::new();
    workspace_source_definition_for_import_in_broken_source_with_open_docs(
        uri, source, package, &open_docs, position,
    )
}

fn workspace_source_definition_for_import_in_broken_source_with_open_docs(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<GotoDefinitionResponse> {
    let binding = broken_source_import_binding_at(source, position)?;
    workspace_source_location_for_import_binding_with_open_docs(
        uri,
        source,
        None,
        package,
        open_docs,
        &binding.import_prefix,
        binding.imported_name.as_str(),
    )
    .map(GotoDefinitionResponse::Scalar)
}

fn workspace_source_type_definition_for_import_in_broken_source(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<GotoTypeDefinitionResponse> {
    let open_docs = OpenDocuments::new();
    workspace_source_type_definition_for_import_in_broken_source_with_open_docs(
        uri, source, package, &open_docs, position,
    )
}

fn workspace_source_type_definition_for_import_in_broken_source_with_open_docs(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<GotoTypeDefinitionResponse> {
    let binding = broken_source_import_binding_at(source, position)?;
    workspace_source_type_definition_location_for_import_binding_with_open_docs(
        uri,
        source,
        None,
        package,
        open_docs,
        &binding.import_prefix,
        binding.imported_name.as_str(),
    )
    .map(GotoTypeDefinitionResponse::Scalar)
}

fn broken_source_import_occurrence_span_at(
    source: &str,
    position: tower_lsp::lsp_types::Position,
    local_name: &str,
) -> Option<Span> {
    let offset = position_to_offset(source, position)?;
    let (tokens, _) = lex(source);
    tokens
        .iter()
        .find(|token| {
            token.kind == TokenKind::Ident
                && token.text == local_name
                && token.span.contains(offset)
        })
        .map(|token| token.span)
}

fn workspace_source_hover_for_import_in_broken_source(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<Hover> {
    let open_docs = OpenDocuments::new();
    workspace_source_hover_for_import_in_broken_source_with_open_docs(
        uri, source, package, &open_docs, position,
    )
}

fn workspace_source_hover_for_import_in_broken_source_with_open_docs(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<Hover> {
    let binding = broken_source_import_binding_at(source, position)?;
    let occurrence_span =
        broken_source_import_occurrence_span_at(source, position, binding.local_name.as_str())?;
    let source_location = workspace_source_location_for_import_binding_with_open_docs(
        uri,
        source,
        None,
        package,
        open_docs,
        &binding.import_prefix,
        binding.imported_name.as_str(),
    )?;

    hover_from_workspace_source_location_with_open_docs(
        source,
        occurrence_span,
        source_location,
        open_docs,
    )
}

fn workspace_import_semantic_tokens_in_analysis_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
) -> Vec<ql_analysis::SemanticTokenOccurrence> {
    let (tokens, _) = lex(source);
    let bindings = broken_source_import_bindings_in_tokens(&tokens);
    let mut local_name_counts = HashMap::<String, usize>::new();
    let mut local_name_kinds = HashMap::<String, ql_analysis::SymbolKind>::new();

    for binding in bindings {
        let Some(kind) = workspace_source_kind_for_import_binding_with_open_docs(
            uri,
            source,
            Some(analysis),
            package,
            open_docs,
            &binding.import_prefix,
            binding.imported_name.as_str(),
            supports_workspace_import_definition,
        ) else {
            continue;
        };
        *local_name_counts
            .entry(binding.local_name.clone())
            .or_insert(0usize) += 1;
        local_name_kinds.insert(binding.local_name, kind);
    }

    let mut occurrences = Vec::new();
    for (local_name, kind) in local_name_kinds {
        if local_name_counts.get(local_name.as_str()) != Some(&1usize) {
            continue;
        }
        for (start, _) in source.match_indices(local_name.as_str()) {
            let Some((binding, span)) = analysis.import_binding_at(start) else {
                continue;
            };
            if binding.local_name == local_name && span.start == start {
                occurrences.push(ql_analysis::SemanticTokenOccurrence { span, kind });
            }
        }
    }
    occurrences
}

fn workspace_import_semantic_tokens_in_broken_source_with_open_docs(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
) -> Vec<ql_analysis::SemanticTokenOccurrence> {
    let (tokens, _) = lex(source);
    let mut unique_bindings = HashMap::<String, (Span, ql_analysis::SymbolKind)>::new();
    let mut local_name_counts = HashMap::<String, usize>::new();

    for binding in broken_source_import_bindings_in_tokens(&tokens) {
        let Some(kind) = workspace_source_kind_for_import_binding_with_open_docs(
            uri,
            source,
            None,
            package,
            open_docs,
            &binding.import_prefix,
            binding.imported_name.as_str(),
            supports_workspace_import_definition,
        ) else {
            continue;
        };
        *local_name_counts
            .entry(binding.local_name.clone())
            .or_insert(0usize) += 1;
        unique_bindings.insert(binding.local_name, (binding.definition_span, kind));
    }

    tokens
        .iter()
        .enumerate()
        .filter(|(_, token)| token.kind == TokenKind::Ident)
        .filter_map(|(index, token)| {
            let (definition_span, kind) = unique_bindings.get(token.text.as_str())?;
            (local_name_counts.get(token.text.as_str()) == Some(&1usize)
                && (token.span == *definition_span
                    || broken_source_import_token_matches_reference_context(&tokens, index)))
            .then_some(ql_analysis::SemanticTokenOccurrence {
                span: token.span,
                kind: *kind,
            })
        })
        .collect()
}

fn semantic_token_sort_index(kind: ql_analysis::SymbolKind) -> u32 {
    match kind {
        ql_analysis::SymbolKind::Import => 0,
        ql_analysis::SymbolKind::BuiltinType | ql_analysis::SymbolKind::TypeAlias => 1,
        ql_analysis::SymbolKind::Struct => 2,
        ql_analysis::SymbolKind::Enum => 3,
        ql_analysis::SymbolKind::Variant => 4,
        ql_analysis::SymbolKind::Trait => 5,
        ql_analysis::SymbolKind::Generic => 6,
        ql_analysis::SymbolKind::Parameter => 7,
        ql_analysis::SymbolKind::Local | ql_analysis::SymbolKind::SelfParameter => 8,
        ql_analysis::SymbolKind::Field => 9,
        ql_analysis::SymbolKind::Function
        | ql_analysis::SymbolKind::Const
        | ql_analysis::SymbolKind::Static => 10,
        ql_analysis::SymbolKind::Method => 11,
    }
}

fn sort_and_dedup_semantic_tokens(tokens: &mut Vec<ql_analysis::SemanticTokenOccurrence>) {
    tokens.sort_by_key(|token| {
        (
            token.span.start,
            token.span.end,
            semantic_token_sort_index(token.kind),
        )
    });
    tokens.dedup_by(|left, right| left.span == right.span && left.kind == right.kind);
}

fn dependency_member_semantic_tokens_with_open_docs(
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
) -> Vec<ql_analysis::SemanticTokenOccurrence> {
    let mut tokens = lex(source)
        .0
        .iter()
        .filter(|token| token.kind == TokenKind::Ident)
        .filter_map(|token| {
            let position = span_to_range(source, token.span).start;
            let open_occurrence_span =
                dependency_occurrence_span_with_open_docs_at(source, package, open_docs, position)?;
            if open_occurrence_span != token.span {
                return None;
            }
            let open_target = dependency_definition_target_with_open_docs_at(
                source, analysis, package, open_docs, position,
            )?;
            let kind = match open_target.kind {
                ql_analysis::SymbolKind::Field => ql_analysis::SymbolKind::Field,
                ql_analysis::SymbolKind::Method => ql_analysis::SymbolKind::Method,
                _ => return None,
            };

            let disk_occurrence_span = dependency_occurrence_span_at(source, package, position);
            let disk_target = dependency_definition_target_at(source, analysis, package, position);
            let changed = disk_occurrence_span != Some(token.span)
                || match disk_target.as_ref() {
                    Some(target) => !same_dependency_definition_target(target, &open_target),
                    None => true,
                };
            changed.then_some(ql_analysis::SemanticTokenOccurrence {
                span: token.span,
                kind,
            })
        })
        .collect::<Vec<_>>();
    sort_and_dedup_semantic_tokens(&mut tokens);
    tokens
}

fn semantic_tokens_for_workspace_package_analysis(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
) -> SemanticTokensResult {
    let open_docs = OpenDocuments::new();
    semantic_tokens_for_workspace_package_analysis_with_open_docs(
        uri, source, analysis, package, &open_docs,
    )
}

fn semantic_tokens_for_workspace_package_analysis_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
) -> SemanticTokensResult {
    let mut tokens = analysis.semantic_tokens();
    let dependency_import_root_tokens =
        package.dependency_import_root_semantic_tokens_in_source(source);
    let dependency_member_tokens = dependency_member_semantic_tokens_with_open_docs(
        source,
        Some(analysis),
        package,
        open_docs,
    );
    let workspace_import_root_tokens = workspace_import_semantic_tokens_in_analysis_with_open_docs(
        uri, source, analysis, package, open_docs,
    );
    let overridden_import_spans = dependency_import_root_tokens
        .iter()
        .chain(workspace_import_root_tokens.iter())
        .map(|token| (token.span.start, token.span.end))
        .collect::<HashSet<_>>();
    let overridden_dependency_member_spans = dependency_member_tokens
        .iter()
        .map(|token| (token.span.start, token.span.end))
        .collect::<HashSet<_>>();

    tokens.retain(|token| {
        let span = (token.span.start, token.span.end);
        (token.kind != ql_analysis::SymbolKind::Import || !overridden_import_spans.contains(&span))
            && !overridden_dependency_member_spans.contains(&span)
    });
    tokens.extend(package.dependency_semantic_tokens_in_source(source));
    tokens.retain(|token| {
        !overridden_dependency_member_spans.contains(&(token.span.start, token.span.end))
    });
    tokens.extend(dependency_member_tokens);
    tokens.extend(dependency_import_root_tokens);
    tokens.extend(workspace_import_root_tokens);
    sort_and_dedup_semantic_tokens(&mut tokens);
    semantic_tokens_result_from_occurrences(source, tokens)
}

fn semantic_tokens_for_workspace_dependency_fallback(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
) -> SemanticTokensResult {
    let open_docs = OpenDocuments::new();
    semantic_tokens_for_workspace_dependency_fallback_with_open_docs(
        uri, source, package, &open_docs,
    )
}

fn semantic_tokens_for_workspace_dependency_fallback_with_open_docs(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
) -> SemanticTokensResult {
    let mut tokens = package.dependency_fallback_semantic_tokens_in_source(source);
    let dependency_member_tokens =
        dependency_member_semantic_tokens_with_open_docs(source, None, package, open_docs);
    let overridden_dependency_member_spans = dependency_member_tokens
        .iter()
        .map(|token| (token.span.start, token.span.end))
        .collect::<HashSet<_>>();
    tokens.retain(|token| {
        !overridden_dependency_member_spans.contains(&(token.span.start, token.span.end))
    });
    tokens.extend(dependency_member_tokens);
    tokens.extend(
        workspace_import_semantic_tokens_in_broken_source_with_open_docs(
            uri, source, package, open_docs,
        ),
    );
    sort_and_dedup_semantic_tokens(&mut tokens);
    semantic_tokens_result_from_occurrences(source, tokens)
}

fn broken_source_import_binding_at(
    source: &str,
    position: tower_lsp::lsp_types::Position,
) -> Option<BrokenSourceImportBinding> {
    let offset = position_to_offset(source, position)?;
    let (tokens, _) = lex(source);
    let bindings = broken_source_import_bindings_in_tokens(&tokens);
    let mut local_name_counts = HashMap::<String, usize>::new();
    for binding in &bindings {
        *local_name_counts
            .entry(binding.local_name.clone())
            .or_insert(0usize) += 1;
    }
    let (index, token) = tokens
        .iter()
        .enumerate()
        .find(|(_, token)| token.kind == TokenKind::Ident && token.span.contains(offset))?;
    if local_name_counts.get(token.text.as_str()) != Some(&1usize) {
        return None;
    }
    let binding = bindings
        .into_iter()
        .find(|binding| binding.local_name == token.text)?;
    if token.span == binding.definition_span
        || broken_source_import_token_matches_reference_context(&tokens, index)
    {
        return Some(binding);
    }
    None
}

fn broken_source_import_bindings_in_tokens(tokens: &[Token]) -> Vec<BrokenSourceImportBinding> {
    let mut bindings = Vec::new();
    let mut index = 0usize;
    while index < tokens.len() {
        if tokens[index].kind != TokenKind::Use {
            index += 1;
            continue;
        }

        let Some((next_index, use_bindings)) =
            broken_source_import_bindings_after_use(tokens, index + 1)
        else {
            index += 1;
            continue;
        };
        bindings.extend(use_bindings);
        index = next_index.max(index + 1);
    }
    bindings
}

fn broken_source_import_bindings_after_use(
    tokens: &[Token],
    index: usize,
) -> Option<(usize, Vec<BrokenSourceImportBinding>)> {
    let (prefix, mut index) = broken_source_import_path_in_tokens(tokens, index)?;
    if tokens.get(index).map(|token| token.kind) == Some(TokenKind::Dot)
        && tokens.get(index + 1).map(|token| token.kind) == Some(TokenKind::LBrace)
    {
        index += 2;
        let mut bindings = Vec::new();
        loop {
            if tokens.get(index).map(|token| token.kind) == Some(TokenKind::RBrace) {
                return Some((index + 1, bindings));
            }

            let item = broken_source_import_ident_token(tokens, index)?;
            let item_name = item.text.clone();
            let item_span = item.span;
            index += 1;

            let (alias, alias_span, next_index) =
                broken_source_import_alias_in_tokens(tokens, index)?;
            index = next_index;

            bindings.push(BrokenSourceImportBinding {
                imported_name: item_name.clone(),
                import_prefix: prefix.iter().map(|(segment, _)| segment.clone()).collect(),
                local_name: alias.unwrap_or(item_name),
                imported_span: item_span,
                definition_span: alias_span.unwrap_or(item_span),
            });

            match tokens.get(index).map(|token| token.kind) {
                Some(TokenKind::Comma) => index += 1,
                Some(TokenKind::RBrace) => return Some((index + 1, bindings)),
                _ => return None,
            }
        }
    }

    let (imported_name, definition_span) = prefix.last()?.clone();
    let (alias, alias_span, index) = broken_source_import_alias_in_tokens(tokens, index)?;
    Some((
        index,
        vec![BrokenSourceImportBinding {
            imported_name: imported_name.clone(),
            import_prefix: prefix[..prefix.len().saturating_sub(1)]
                .iter()
                .map(|(segment, _)| segment.clone())
                .collect(),
            local_name: alias.unwrap_or(imported_name),
            imported_span: definition_span,
            definition_span: alias_span.unwrap_or(definition_span),
        }],
    ))
}

fn broken_source_import_path_in_tokens(
    tokens: &[Token],
    index: usize,
) -> Option<(Vec<(String, Span)>, usize)> {
    let mut index = index;
    let first = broken_source_import_ident_token(tokens, index)?;
    let mut segments = vec![(first.text.clone(), first.span)];
    index += 1;

    while tokens.get(index).map(|token| token.kind) == Some(TokenKind::Dot)
        && tokens.get(index + 1).map(|token| token.kind) == Some(TokenKind::Ident)
    {
        let segment = tokens.get(index + 1)?;
        segments.push((segment.text.clone(), segment.span));
        index += 2;
    }

    Some((segments, index))
}

fn broken_source_import_alias_in_tokens(
    tokens: &[Token],
    index: usize,
) -> Option<(Option<String>, Option<Span>, usize)> {
    if tokens.get(index).map(|token| token.kind) != Some(TokenKind::As) {
        return Some((None, None, index));
    }
    let alias = broken_source_import_ident_token(tokens, index + 1)?;
    Some((Some(alias.text.clone()), Some(alias.span), index + 2))
}

fn broken_source_import_ident_token(tokens: &[Token], index: usize) -> Option<&Token> {
    let token = tokens.get(index)?;
    (token.kind == TokenKind::Ident).then_some(token)
}

fn broken_source_import_token_matches_reference_context(tokens: &[Token], index: usize) -> bool {
    let prev_kind = index
        .checked_sub(1)
        .and_then(|index| tokens.get(index))
        .map(|token| token.kind);
    let next_kind = tokens.get(index + 1).map(|token| token.kind);

    if matches!(prev_kind, Some(TokenKind::Dot | TokenKind::As)) {
        return false;
    }

    matches!(
        next_kind,
        Some(
            TokenKind::LParen
                | TokenKind::LBracket
                | TokenKind::LBrace
                | TokenKind::Dot
                | TokenKind::Question
                | TokenKind::For
        )
    ) || matches!(prev_kind, Some(TokenKind::Colon | TokenKind::Arrow))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BrokenSourceImplBlockSite {
    location: Location,
    trait_name: Option<String>,
    target_name: String,
    method_spans: Vec<(String, Span)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BrokenSourceRootDefinitionSite {
    kind: ql_analysis::SymbolKind,
    name: String,
    span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BrokenSourceTraitDeclSite {
    trait_name: String,
    trait_span: Span,
    method_spans: Vec<(String, Span)>,
}

fn broken_source_visible_local_names_for_target(
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    target: &DependencyDefinitionTarget,
) -> HashSet<String> {
    let mut names = HashSet::new();
    if canonicalize_or_clone(package.manifest().manifest_path.as_path())
        == canonicalize_or_clone(&target.manifest_path)
    {
        names.insert(target.name.clone());
    }

    let (tokens, _) = lex(source);
    for binding in broken_source_import_bindings_in_tokens(&tokens) {
        if binding.imported_name != target.name {
            continue;
        }
        let Some(resolved_target) =
            package.dependency_type_definition_in_source_at(source, binding.definition_span.start)
        else {
            continue;
        };
        if same_dependency_definition_target(&resolved_target, target)
            || same_dependency_definition_source_identity(&resolved_target, target)
        {
            names.insert(binding.local_name);
        }
    }

    names
}

fn token_index_after_balanced_braces_in_tokens(
    tokens: &[Token],
    open_index: usize,
) -> Option<usize> {
    if tokens.get(open_index).map(|token| token.kind) != Some(TokenKind::LBrace) {
        return None;
    }

    let mut depth = 1usize;
    let mut index = open_index + 1;
    while index < tokens.len() {
        match tokens[index].kind {
            TokenKind::LBrace => depth += 1,
            TokenKind::RBrace => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            _ => {}
        }
        index += 1;
    }

    None
}

fn broken_source_path_last_ident_token(tokens: &[Token], index: usize) -> Option<(&Token, usize)> {
    let mut index = index;
    let mut last = tokens.get(index)?;
    if last.kind != TokenKind::Ident {
        return None;
    }
    index += 1;

    while tokens.get(index).map(|token| token.kind) == Some(TokenKind::Dot)
        && tokens.get(index + 1).map(|token| token.kind) == Some(TokenKind::Ident)
    {
        last = tokens.get(index + 1)?;
        index += 2;
    }

    Some((last, index))
}

fn broken_source_impl_method_name_spans_in_tokens(
    tokens: &[Token],
    start_index: usize,
    end_index: usize,
) -> Vec<(String, Span)> {
    let mut method_spans = Vec::new();
    let mut brace_depth = 0usize;
    let mut index = start_index;

    while index < end_index {
        match tokens[index].kind {
            TokenKind::LBrace => brace_depth += 1,
            TokenKind::RBrace => brace_depth = brace_depth.saturating_sub(1),
            TokenKind::Fn if brace_depth == 0 => {
                if let Some(method_token) = tokens.get(index + 1)
                    && method_token.kind == TokenKind::Ident
                {
                    method_spans.push((method_token.text.clone(), method_token.span));
                }
            }
            _ => {}
        }
        index += 1;
    }

    method_spans
}

fn broken_source_root_definition_site_in_tokens(
    tokens: &[Token],
    index: usize,
) -> Option<(BrokenSourceRootDefinitionSite, usize)> {
    let mut index = index;
    if tokens.get(index).map(|token| token.kind) == Some(TokenKind::Pub) {
        index += 1;
    }

    let kind = match tokens.get(index).map(|token| token.kind) {
        Some(TokenKind::Struct) => ql_analysis::SymbolKind::Struct,
        Some(TokenKind::Enum) => ql_analysis::SymbolKind::Enum,
        Some(TokenKind::Trait) => ql_analysis::SymbolKind::Trait,
        _ => return None,
    };
    let name = broken_source_import_ident_token(tokens, index + 1)?;

    Some((
        BrokenSourceRootDefinitionSite {
            kind,
            name: name.text.clone(),
            span: name.span,
        },
        index + 2,
    ))
}

fn broken_source_root_definition_sites_in_source(
    source: &str,
) -> Vec<BrokenSourceRootDefinitionSite> {
    let (tokens, _) = lex(source);
    let mut sites = Vec::new();
    let mut brace_depth = 0usize;
    let mut index = 0usize;

    while index < tokens.len() {
        match tokens[index].kind {
            TokenKind::LBrace => brace_depth += 1,
            TokenKind::RBrace => brace_depth = brace_depth.saturating_sub(1),
            TokenKind::Pub | TokenKind::Struct | TokenKind::Enum | TokenKind::Trait
                if brace_depth == 0 =>
            {
                if let Some((site, next_index)) =
                    broken_source_root_definition_site_in_tokens(&tokens, index)
                {
                    sites.push(site);
                    index = next_index;
                    continue;
                }
            }
            _ => {}
        }
        index += 1;
    }

    sites
}

fn broken_source_trait_decl_site_in_tokens(
    tokens: &[Token],
    index: usize,
) -> Option<(BrokenSourceTraitDeclSite, usize)> {
    let mut index = index;
    if tokens.get(index).map(|token| token.kind) == Some(TokenKind::Pub) {
        index += 1;
    }
    if tokens.get(index).map(|token| token.kind) != Some(TokenKind::Trait) {
        return None;
    }

    let trait_name = broken_source_import_ident_token(tokens, index + 1)?;
    let open_index = ((index + 2)..tokens.len()).find(|candidate| {
        tokens.get(*candidate).map(|token| token.kind) == Some(TokenKind::LBrace)
    })?;
    let close_index = token_index_after_balanced_braces_in_tokens(tokens, open_index);
    let method_end_index = close_index
        .map(|close_index| close_index.saturating_sub(1))
        .unwrap_or(tokens.len());

    Some((
        BrokenSourceTraitDeclSite {
            trait_name: trait_name.text.clone(),
            trait_span: trait_name.span,
            method_spans: broken_source_impl_method_name_spans_in_tokens(
                tokens,
                open_index + 1,
                method_end_index,
            ),
        },
        close_index.unwrap_or(tokens.len()),
    ))
}

fn broken_source_trait_decl_sites_in_source(source: &str) -> Vec<BrokenSourceTraitDeclSite> {
    let (tokens, _) = lex(source);
    let mut sites = Vec::new();
    let mut brace_depth = 0usize;
    let mut index = 0usize;

    while index < tokens.len() {
        match tokens[index].kind {
            TokenKind::LBrace => brace_depth += 1,
            TokenKind::RBrace => brace_depth = brace_depth.saturating_sub(1),
            TokenKind::Pub | TokenKind::Trait if brace_depth == 0 => {
                if let Some((site, next_index)) =
                    broken_source_trait_decl_site_in_tokens(&tokens, index)
                {
                    sites.push(site);
                    index = next_index;
                    continue;
                }
            }
            _ => {}
        }
        index += 1;
    }

    sites
}

fn broken_source_impl_block_site_in_tokens(
    uri: &Url,
    source: &str,
    tokens: &[Token],
    index: usize,
) -> Option<(BrokenSourceImplBlockSite, usize)> {
    let token = tokens.get(index)?;
    let (trait_name, target_name, open_index) = match token.kind {
        TokenKind::Impl => {
            let (first_name, next_index) = broken_source_path_last_ident_token(tokens, index + 1)?;
            if tokens.get(next_index).map(|token| token.kind) == Some(TokenKind::For) {
                let (target_name, after_target_index) =
                    broken_source_path_last_ident_token(tokens, next_index + 1)?;
                (
                    Some(first_name.text.clone()),
                    target_name.text.clone(),
                    after_target_index,
                )
            } else {
                (None, first_name.text.clone(), next_index)
            }
        }
        TokenKind::Extend => {
            let (target_name, next_index) = broken_source_path_last_ident_token(tokens, index + 1)?;
            (None, target_name.text.clone(), next_index)
        }
        _ => return None,
    };

    if tokens.get(open_index).map(|token| token.kind) != Some(TokenKind::LBrace) {
        return None;
    }

    let close_index = token_index_after_balanced_braces_in_tokens(tokens, open_index);
    let method_end_index = close_index
        .map(|close_index| close_index.saturating_sub(1))
        .unwrap_or(tokens.len());
    let block_end_offset = close_index
        .and_then(|close_index| close_index.checked_sub(1))
        .and_then(|close_token_index| tokens.get(close_token_index))
        .map(|token| token.span.end)
        .unwrap_or(source.len());

    Some((
        BrokenSourceImplBlockSite {
            location: Location::new(
                uri.clone(),
                span_to_range(source, Span::new(token.span.start, block_end_offset)),
            ),
            trait_name,
            target_name,
            method_spans: broken_source_impl_method_name_spans_in_tokens(
                tokens,
                open_index + 1,
                method_end_index,
            ),
        },
        close_index.unwrap_or(tokens.len()),
    ))
}

fn broken_source_impl_block_sites_in_source(
    uri: &Url,
    source: &str,
) -> Vec<BrokenSourceImplBlockSite> {
    let (tokens, _) = lex(source);
    let mut sites = Vec::new();
    let mut brace_depth = 0usize;
    let mut index = 0usize;

    while index < tokens.len() {
        match tokens[index].kind {
            TokenKind::LBrace => brace_depth += 1,
            TokenKind::RBrace => brace_depth = brace_depth.saturating_sub(1),
            TokenKind::Impl | TokenKind::Extend if brace_depth == 0 => {
                if let Some((site, next_index)) =
                    broken_source_impl_block_site_in_tokens(uri, source, &tokens, index)
                {
                    sites.push(site);
                    index = next_index;
                    continue;
                }
            }
            _ => {}
        }
        index += 1;
    }

    sites
}

fn broken_source_implementation_locations_in_source(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    target: &DependencyDefinitionTarget,
) -> Vec<Location> {
    if !matches!(
        target.kind,
        ql_analysis::SymbolKind::Struct
            | ql_analysis::SymbolKind::Enum
            | ql_analysis::SymbolKind::Trait
    ) {
        return Vec::new();
    }

    let local_names = broken_source_visible_local_names_for_target(source, package, target);
    if local_names.is_empty() {
        return Vec::new();
    }

    broken_source_impl_block_sites_in_source(uri, source)
        .into_iter()
        .filter(|site| match target.kind {
            ql_analysis::SymbolKind::Struct | ql_analysis::SymbolKind::Enum => {
                local_names.contains(&site.target_name)
            }
            ql_analysis::SymbolKind::Trait => site
                .trait_name
                .as_ref()
                .is_some_and(|trait_name| local_names.contains(trait_name)),
            _ => false,
        })
        .map(|site| site.location)
        .collect()
}

fn broken_source_trait_method_implementation_sites_in_source(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    target: &DependencyDefinitionTarget,
    method_name: &str,
) -> Vec<WorkspaceMethodDefinitionSite> {
    let local_names = broken_source_visible_local_names_for_target(source, package, target);
    if local_names.is_empty() {
        return Vec::new();
    }

    broken_source_impl_block_sites_in_source(uri, source)
        .into_iter()
        .filter(|site| {
            site.trait_name
                .as_ref()
                .is_some_and(|trait_name| local_names.contains(trait_name))
        })
        .flat_map(|site| {
            site.method_spans
                .into_iter()
                .filter(move |(name, _)| name == method_name)
                .map(move |(name, span)| WorkspaceMethodDefinitionSite {
                    location: Location::new(uri.clone(), span_to_range(source, span)),
                    definition_target: ql_analysis::DefinitionTarget {
                        kind: ql_analysis::SymbolKind::Method,
                        name,
                        span,
                    },
                })
        })
        .collect()
}

fn broken_source_method_definition_locations_in_source(
    uri: &Url,
    source: &str,
    method_name: &str,
) -> Vec<Location> {
    broken_source_impl_block_sites_in_source(uri, source)
        .into_iter()
        .flat_map(|site| {
            site.method_spans
                .into_iter()
                .filter(move |(name, _)| name == method_name)
                .map(move |(_, span)| Location::new(uri.clone(), span_to_range(source, span)))
        })
        .collect()
}

fn broken_source_definition_locations_in_source(
    uri: &Url,
    source: &str,
    target: &DependencyDefinitionTarget,
) -> Vec<Location> {
    match target.kind {
        ql_analysis::SymbolKind::Struct
        | ql_analysis::SymbolKind::Enum
        | ql_analysis::SymbolKind::Trait => broken_source_root_definition_sites_in_source(source)
            .into_iter()
            .filter(|site| site.kind == target.kind && site.name == target.name)
            .map(|site| Location::new(uri.clone(), span_to_range(source, site.span)))
            .collect(),
        _ => Vec::new(),
    }
}

fn extend_workspace_dependency_definition_matches(
    package: &ql_analysis::PackageAnalysis,
    current_path: Option<&Path>,
    current_source: Option<&str>,
    current_analysis: Option<&Analysis>,
    target: &DependencyDefinitionTarget,
    matches: &mut Vec<Location>,
) {
    let open_docs = OpenDocuments::new();
    extend_workspace_dependency_definition_matches_with_open_docs(
        package,
        current_path,
        current_source,
        current_analysis,
        &open_docs,
        target,
        matches,
    );
}

fn extend_workspace_dependency_definition_matches_with_open_docs(
    package: &ql_analysis::PackageAnalysis,
    current_path: Option<&Path>,
    current_source: Option<&str>,
    current_analysis: Option<&Analysis>,
    open_docs: &OpenDocuments,
    target: &DependencyDefinitionTarget,
    matches: &mut Vec<Location>,
) {
    if !supports_workspace_dependency_definition(target.kind) {
        return;
    }

    if canonicalize_or_clone(&package.manifest().manifest_path)
        != canonicalize_or_clone(&target.manifest_path)
    {
        return;
    }

    for module in package.modules() {
        let module_path = module.path();
        if !package_module_matches_dependency_source_path(package, module_path, &target.source_path)
        {
            continue;
        }

        if current_path
            .is_some_and(|path| canonicalize_or_clone(path) == canonicalize_or_clone(module_path))
        {
            let Ok(uri) = Url::from_file_path(module_path) else {
                continue;
            };
            let Some(module_source) = current_source else {
                continue;
            };
            if let Some(module_analysis) = current_analysis {
                for symbol in module_analysis.document_symbols() {
                    if symbol.name != target.name || symbol.kind != target.kind {
                        continue;
                    }
                    matches.push(Location::new(
                        uri.clone(),
                        span_to_range(module_source, symbol.span),
                    ));
                }
            } else {
                let mut module_locations =
                    broken_source_definition_locations_in_source(&uri, module_source, target);
                module_locations.sort_by_key(|location| {
                    (
                        location.range.start.line,
                        location.range.start.character,
                        location.range.end.line,
                        location.range.end.character,
                    )
                });
                module_locations.dedup_by(|left, right| same_location_anchor(left, right));
                matches.extend(module_locations);
            }
            continue;
        }

        if let Some((uri, source, analysis)) = open_document_snapshot(open_docs, module_path) {
            for symbol in analysis.document_symbols() {
                if symbol.name != target.name || symbol.kind != target.kind {
                    continue;
                }
                matches.push(Location::new(
                    uri.clone(),
                    span_to_range(&source, symbol.span),
                ));
            }
            continue;
        }

        let Ok(uri) = Url::from_file_path(module_path) else {
            continue;
        };
        let Ok(source) = fs::read_to_string(module_path) else {
            continue;
        };
        let source = source.replace("\r\n", "\n");
        for symbol in module.analysis().document_symbols() {
            if symbol.name != target.name || symbol.kind != target.kind {
                continue;
            }
            matches.push(Location::new(
                uri.clone(),
                span_to_range(&source, symbol.span),
            ));
        }
    }
}

fn dependency_definition_target_at(
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<DependencyDefinitionTarget> {
    let offset = position_to_offset(source, position)?;
    if let Some(analysis) = analysis {
        return package
            .dependency_method_definition_at(analysis, offset)
            .or_else(|| package.dependency_struct_field_definition_at(analysis, offset))
            .or_else(|| package.dependency_variant_definition_at(analysis, source, offset))
            .or_else(|| package.dependency_value_definition_in_source_at(source, offset))
            .or_else(|| package.dependency_definition_at(analysis, offset));
    }

    package
        .dependency_method_definition_in_source_at(source, offset)
        .or_else(|| package.dependency_struct_field_definition_in_source_at(source, offset))
        .or_else(|| package.dependency_variant_definition_in_source_at(source, offset))
        .or_else(|| package.dependency_value_definition_in_source_at(source, offset))
        .or_else(|| package.dependency_definition_in_source_at(source, offset))
}

fn dependency_identifier_token_at(
    source: &str,
    position: tower_lsp::lsp_types::Position,
) -> Option<Token> {
    let offset = position_to_offset(source, position)?;
    let (tokens, _) = lex(source);
    tokens.into_iter().find(|token| {
        token.kind == TokenKind::Ident && (token.span.contains(offset) || token.span.end == offset)
    })
}

fn workspace_source_dependency_target_from_open_docs<T>(
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    target_manifest_path: &Path,
    target_source_path: &str,
    f: impl Fn(&ql_analysis::PackageAnalysis, &str) -> Option<T>,
) -> Option<T> {
    for candidate_manifest_path in
        source_preferred_manifest_paths_for_package(package.manifest().manifest_path.as_path())
    {
        let Some(candidate_package) = package_analysis_for_path(&candidate_manifest_path) else {
            continue;
        };
        if canonicalize_or_clone(candidate_package.manifest().manifest_path.as_path())
            != canonicalize_or_clone(target_manifest_path)
        {
            continue;
        }
        let open_source = open_docs.iter().find_map(|(path, (_, open_source))| {
            candidate_package.modules().iter().find_map(|module| {
                (canonicalize_or_clone(module.path()) == canonicalize_or_clone(path)
                    && package_module_matches_dependency_source_path(
                        &candidate_package,
                        module.path(),
                        target_source_path,
                    ))
                .then_some(open_source.as_str())
            })
        });
        if let Some(open_source) = open_source
            && let Some(target) = f(&candidate_package, open_source)
        {
            return Some(target);
        }
    }
    None
}

fn dependency_method_definition_target_with_open_docs(
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<DependencyDefinitionTarget> {
    let offset = position_to_offset(source, position)?;
    let token = dependency_identifier_token_at(source, position)?;
    let target = package
        .dependency_method_completion_target_in_source_at(source, offset)
        .or_else(|| {
            offset.checked_sub(1).and_then(|fallback_offset| {
                package.dependency_method_completion_target_in_source_at(source, fallback_offset)
            })
        })?;
    workspace_source_dependency_target_from_open_docs(
        package,
        open_docs,
        target.manifest_path.as_path(),
        &target.source_path,
        |candidate_package, open_source| {
            candidate_package.public_struct_method_definition_in_source(
                &target.source_path,
                open_source,
                &target.struct_name,
                &token.text,
            )
        },
    )
}

fn dependency_struct_field_definition_target_with_open_docs(
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<DependencyDefinitionTarget> {
    let offset = position_to_offset(source, position)?;
    let token = dependency_identifier_token_at(source, position)?;
    let target = package
        .dependency_member_field_completion_target_in_source_at(source, offset)
        .or_else(|| {
            offset.checked_sub(1).and_then(|fallback_offset| {
                package
                    .dependency_member_field_completion_target_in_source_at(source, fallback_offset)
            })
        })?;
    workspace_source_dependency_target_from_open_docs(
        package,
        open_docs,
        target.manifest_path.as_path(),
        &target.source_path,
        |candidate_package, open_source| {
            candidate_package.public_struct_member_field_definition_in_source(
                &target.source_path,
                open_source,
                &target.struct_name,
                &token.text,
            )
        },
    )
}

fn dependency_definition_target_with_open_docs_at(
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<DependencyDefinitionTarget> {
    dependency_definition_target_at(source, analysis, package, position)
        .or_else(|| {
            dependency_method_definition_target_with_open_docs(source, package, open_docs, position)
        })
        .or_else(|| {
            dependency_struct_field_definition_target_with_open_docs(
                source, package, open_docs, position,
            )
        })
}

fn dependency_type_definition_target_at(
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<DependencyDefinitionTarget> {
    let offset = position_to_offset(source, position)?;
    if let Some(analysis) = analysis {
        return package
            .dependency_type_definition_at(analysis, offset)
            .or_else(|| package.dependency_value_type_definition_in_source_at(source, offset))
            .or_else(|| package.dependency_variant_type_definition_at(analysis, source, offset))
            .or_else(|| {
                package.dependency_struct_field_type_definition_in_source_at(source, offset)
            })
            .or_else(|| package.dependency_method_type_definition_in_source_at(source, offset));
    }

    package
        .dependency_type_definition_in_source_at(source, offset)
        .or_else(|| package.dependency_value_type_definition_in_source_at(source, offset))
        .or_else(|| package.dependency_variant_type_definition_in_source_at(source, offset))
        .or_else(|| package.dependency_struct_field_type_definition_in_source_at(source, offset))
        .or_else(|| package.dependency_method_type_definition_in_source_at(source, offset))
}

fn dependency_method_type_definition_target_with_open_docs(
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<DependencyDefinitionTarget> {
    let offset = position_to_offset(source, position)?;
    let token = dependency_identifier_token_at(source, position)?;
    let target = package
        .dependency_method_completion_target_in_source_at(source, offset)
        .or_else(|| {
            offset.checked_sub(1).and_then(|fallback_offset| {
                package.dependency_method_completion_target_in_source_at(source, fallback_offset)
            })
        })?;
    workspace_source_dependency_target_from_open_docs(
        package,
        open_docs,
        target.manifest_path.as_path(),
        &target.source_path,
        |candidate_package, open_source| {
            candidate_package.public_struct_method_type_definition_in_source(
                &target.source_path,
                open_source,
                &target.struct_name,
                &token.text,
            )
        },
    )
}

fn dependency_struct_field_type_definition_target_with_open_docs(
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<DependencyDefinitionTarget> {
    let offset = position_to_offset(source, position)?;
    let token = dependency_identifier_token_at(source, position)?;
    let target = package
        .dependency_member_field_completion_target_in_source_at(source, offset)
        .or_else(|| {
            offset.checked_sub(1).and_then(|fallback_offset| {
                package
                    .dependency_member_field_completion_target_in_source_at(source, fallback_offset)
            })
        })?;
    workspace_source_dependency_target_from_open_docs(
        package,
        open_docs,
        target.manifest_path.as_path(),
        &target.source_path,
        |candidate_package, open_source| {
            candidate_package.public_struct_member_field_type_definition_in_source(
                &target.source_path,
                open_source,
                &target.struct_name,
                &token.text,
            )
        },
    )
}

fn dependency_type_definition_target_with_open_docs_at(
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<DependencyDefinitionTarget> {
    dependency_type_definition_target_at(source, analysis, package, position)
        .or_else(|| {
            dependency_struct_field_type_definition_target_with_open_docs(
                source, package, open_docs, position,
            )
        })
        .or_else(|| {
            dependency_method_type_definition_target_with_open_docs(
                source, package, open_docs, position,
            )
        })
}

fn dependency_occurrence_span_at(
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<Span> {
    let offset = position_to_offset(source, position)?;
    package
        .dependency_method_hover_in_source_at(source, offset)
        .map(|info| info.span)
        .or_else(|| {
            package
                .dependency_struct_field_hover_in_source_at(source, offset)
                .map(|info| info.span)
        })
        .or_else(|| {
            package
                .dependency_variant_hover_in_source_at(source, offset)
                .map(|info| info.span)
        })
        .or_else(|| {
            package
                .dependency_value_hover_in_source_at(source, offset)
                .map(|info| info.span)
        })
        .or_else(|| {
            package
                .dependency_hover_in_source_at(source, offset)
                .map(|info| info.span)
        })
}

fn dependency_occurrence_span_with_open_docs_at(
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<Span> {
    dependency_occurrence_span_at(source, package, position).or_else(|| {
        let token = dependency_identifier_token_at(source, position)?;
        dependency_method_definition_target_with_open_docs(source, package, open_docs, position)
            .or_else(|| {
                dependency_struct_field_definition_target_with_open_docs(
                    source, package, open_docs, position,
                )
            })
            .map(|_| token.span)
    })
}

fn same_dependency_definition_target(
    lhs: &DependencyDefinitionTarget,
    rhs: &DependencyDefinitionTarget,
) -> bool {
    lhs.package_name == rhs.package_name
        && canonicalize_or_clone(&lhs.manifest_path) == canonicalize_or_clone(&rhs.manifest_path)
        && lhs.source_path == rhs.source_path
        && lhs.kind == rhs.kind
        && lhs.name == rhs.name
        && lhs.span == rhs.span
}

fn same_dependency_definition_source_identity(
    lhs: &DependencyDefinitionTarget,
    rhs: &DependencyDefinitionTarget,
) -> bool {
    canonicalize_or_clone(&lhs.manifest_path) == canonicalize_or_clone(&rhs.manifest_path)
        && lhs.source_path == rhs.source_path
        && lhs.kind == rhs.kind
        && lhs.name == rhs.name
}

fn offset_hits_span(offset: usize, span: Span) -> bool {
    span.contains(offset) || span.end == offset
}

fn extend_workspace_dependency_reference_locations(
    package: &ql_analysis::PackageAnalysis,
    current_path: Option<&Path>,
    target: &DependencyDefinitionTarget,
    include_declaration: bool,
    locations: &mut Vec<Location>,
) {
    let open_docs = OpenDocuments::new();
    extend_workspace_dependency_reference_locations_with_open_docs(
        package,
        current_path,
        &open_docs,
        target,
        include_declaration,
        locations,
    );
}

fn extend_workspace_dependency_reference_locations_with_open_docs(
    package: &ql_analysis::PackageAnalysis,
    current_path: Option<&Path>,
    open_docs: &OpenDocuments,
    target: &DependencyDefinitionTarget,
    include_declaration: bool,
    locations: &mut Vec<Location>,
) {
    let Ok(source_paths) = collect_package_sources(package.manifest()) else {
        return;
    };

    for source_path in source_paths {
        if current_path
            .is_some_and(|path| canonicalize_or_clone(path) == canonicalize_or_clone(&source_path))
        {
            continue;
        }
        let (uri, source, owned_analysis) = if let Some((open_uri, open_source, open_analysis)) =
            open_document_snapshot(open_docs, &source_path)
        {
            (open_uri, open_source, Some(open_analysis))
        } else {
            let Ok(uri) = Url::from_file_path(&source_path) else {
                continue;
            };
            let Ok(source) = fs::read_to_string(&source_path) else {
                continue;
            };
            let source = source.replace("\r\n", "\n");
            let analysis = package
                .modules()
                .iter()
                .find(|module| {
                    canonicalize_or_clone(module.path()) == canonicalize_or_clone(&source_path)
                })
                .map(|module| module.analysis().clone())
                .or_else(|| analyze_source(&source).ok());
            (uri, source, analysis)
        };
        let analysis = owned_analysis.as_ref();
        let mut module_locations = lex(&source)
            .0
            .iter()
            .filter(|token| token.kind == TokenKind::Ident)
            .filter_map(|token| {
                let position = span_to_range(&source, token.span).start;
                let occurrence_span = dependency_occurrence_span_with_open_docs_at(
                    &source, package, open_docs, position,
                )?;
                if !include_declaration
                    && dependency_reference_is_definition_at(&source, analysis, package, position)
                        == Some(true)
                {
                    return None;
                }
                let occurrence_target = dependency_definition_target_with_open_docs_at(
                    &source, analysis, package, open_docs, position,
                )?;
                (occurrence_span == token.span
                    && same_dependency_definition_target(&occurrence_target, target))
                .then(|| Location::new(uri.clone(), span_to_range(&source, occurrence_span)))
            })
            .collect::<Vec<_>>();
        module_locations.sort_by_key(|location| {
            (
                location.range.start.line,
                location.range.start.character,
                location.range.end.line,
                location.range.end.character,
            )
        });
        module_locations.dedup_by(|left, right| same_location_anchor(left, right));
        locations.extend(module_locations);
    }
}

fn workspace_dependency_reference_locations(
    package: &ql_analysis::PackageAnalysis,
    current_path: Option<&Path>,
    target: &DependencyDefinitionTarget,
    include_declaration: bool,
) -> Vec<Location> {
    let open_docs = OpenDocuments::new();
    workspace_dependency_reference_locations_with_open_docs(
        package,
        current_path,
        &open_docs,
        target,
        include_declaration,
    )
}

fn workspace_dependency_reference_locations_with_open_docs(
    package: &ql_analysis::PackageAnalysis,
    current_path: Option<&Path>,
    open_docs: &OpenDocuments,
    target: &DependencyDefinitionTarget,
    include_declaration: bool,
) -> Vec<Location> {
    let mut locations = Vec::new();
    extend_workspace_dependency_reference_locations_with_open_docs(
        package,
        current_path,
        open_docs,
        target,
        include_declaration,
        &mut locations,
    );
    for candidate_manifest_path in
        source_preferred_manifest_paths_for_package(package.manifest().manifest_path.as_path())
    {
        let Some(member_package) = package_analysis_for_path(&candidate_manifest_path) else {
            continue;
        };
        extend_workspace_dependency_reference_locations_with_open_docs(
            &member_package,
            None,
            open_docs,
            target,
            include_declaration,
            &mut locations,
        );
    }
    locations.sort_by_key(|location| {
        (
            location.uri.to_string(),
            location.range.start.line,
            location.range.start.character,
            location.range.end.line,
            location.range.end.character,
        )
    });
    locations.dedup_by(|left, right| same_location_anchor(left, right));
    locations
}

fn workspace_source_location_for_dependency_target(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    target: &DependencyDefinitionTarget,
) -> Option<Location> {
    let open_docs = OpenDocuments::new();
    workspace_source_location_for_dependency_target_with_open_docs(
        uri, source, analysis, package, &open_docs, target,
    )
}

fn workspace_source_location_for_dependency_target_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    target: &DependencyDefinitionTarget,
) -> Option<Location> {
    let current_path = uri.to_file_path().ok();
    let mut matches = Vec::new();

    extend_workspace_dependency_definition_matches_with_open_docs(
        package,
        current_path.as_deref(),
        Some(source),
        analysis,
        open_docs,
        target,
        &mut matches,
    );

    for candidate_manifest_path in
        source_preferred_manifest_paths_for_package(package.manifest().manifest_path.as_path())
    {
        let Some(member_package) = package_analysis_for_path(&candidate_manifest_path) else {
            continue;
        };
        extend_workspace_dependency_definition_matches_with_open_docs(
            &member_package,
            None,
            None,
            None,
            open_docs,
            target,
            &mut matches,
        );
    }

    matches.sort_by_key(|location| {
        (
            location.uri.to_string(),
            location.range.start.line,
            location.range.start.character,
        )
    });
    matches.dedup();
    (matches.len() == 1).then(|| matches[0].clone())
}

fn named_type_expr_last_segment(ty: &TypeExpr) -> Option<&str> {
    let TypeExprKind::Named { path, .. } = &ty.kind else {
        return None;
    };
    path.segments.last().map(String::as_str)
}

fn implementation_target_matches_dependency_type_expr(
    package: &ql_analysis::PackageAnalysis,
    source: &str,
    analysis: &Analysis,
    target: &DependencyDefinitionTarget,
    ty: &TypeExpr,
) -> bool {
    if let Some(resolved_target) = package
        .dependency_type_definition_at(analysis, ty.span.start)
        .or_else(|| package.dependency_type_definition_in_source_at(source, ty.span.start))
    {
        return same_dependency_definition_target(&resolved_target, target)
            || same_dependency_definition_source_identity(&resolved_target, target);
    }

    canonicalize_or_clone(package.manifest().manifest_path.as_path())
        == canonicalize_or_clone(&target.manifest_path)
        && named_type_expr_last_segment(ty).is_some_and(|name| name == target.name)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TraitMethodImplementationQuery {
    trait_target: DependencyDefinitionTarget,
    method_name: String,
    method_span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct WorkspaceMethodDefinitionSite {
    location: Location,
    definition_target: ql_analysis::DefinitionTarget,
}

fn root_implementation_target_for_source(
    current_path: &Path,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<DependencyDefinitionTarget> {
    let offset = position_to_offset(source, position)?;
    let definition_target = analysis.definition_at(offset)?;
    if !matches!(
        definition_target.kind,
        ql_analysis::SymbolKind::Struct
            | ql_analysis::SymbolKind::Enum
            | ql_analysis::SymbolKind::Trait
    ) || !occurrence_matches_definition_target(analysis, offset, &definition_target)
    {
        return None;
    }

    let source_path = package_source_path_for_module(package, current_path)?;
    let package_name = manifest_package_name(package.manifest()).ok()?.to_owned();

    Some(DependencyDefinitionTarget {
        package_name,
        manifest_path: package.manifest().manifest_path.clone(),
        source_path,
        kind: definition_target.kind,
        name: definition_target.name,
        path: current_path.to_path_buf(),
        span: definition_target.span,
    })
}

fn broken_source_root_implementation_target_for_source(
    current_path: &Path,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<DependencyDefinitionTarget> {
    let offset = position_to_offset(source, position)?;
    let definition_target = broken_source_root_definition_sites_in_source(source)
        .into_iter()
        .find(|site| offset_hits_span(offset, site.span))?;
    let source_path = package_source_path_for_module(package, current_path)?;
    let package_name = manifest_package_name(package.manifest()).ok()?.to_owned();

    Some(DependencyDefinitionTarget {
        package_name,
        manifest_path: package.manifest().manifest_path.clone(),
        source_path,
        kind: definition_target.kind,
        name: definition_target.name,
        path: current_path.to_path_buf(),
        span: definition_target.span,
    })
}

fn workspace_source_root_implementation_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<GotoImplementationResponse> {
    let current_path = uri.to_file_path().ok()?;
    let target =
        root_implementation_target_for_source(&current_path, source, analysis, package, position)?;
    implementation_response_from_locations(workspace_implementation_locations_with_open_docs(
        package, open_docs, &target,
    ))
}

fn workspace_source_root_implementation_in_broken_source_with_open_docs(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<GotoImplementationResponse> {
    let current_path = uri.to_file_path().ok()?;
    let target = broken_source_root_implementation_target_for_source(
        &current_path,
        source,
        package,
        position,
    )?;

    implementation_response_from_locations(workspace_implementation_locations_with_open_docs(
        package, open_docs, &target,
    ))
}

fn trait_method_implementation_query_for_source(
    current_path: &Path,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<TraitMethodImplementationQuery> {
    let offset = position_to_offset(source, position)?;
    let source_path = package_source_path_for_module(package, current_path)?;
    let package_name = manifest_package_name(package.manifest()).ok()?.to_owned();

    analysis.ast().items.iter().find_map(|item| {
        let AstItemKind::Trait(trait_decl) = &item.kind else {
            return None;
        };
        let method = trait_decl
            .methods
            .iter()
            .find(|method| offset_hits_span(offset, method.name_span))?;

        Some(TraitMethodImplementationQuery {
            trait_target: DependencyDefinitionTarget {
                package_name: package_name.clone(),
                manifest_path: package.manifest().manifest_path.clone(),
                source_path: source_path.clone(),
                kind: ql_analysis::SymbolKind::Trait,
                name: trait_decl.name.clone(),
                path: current_path.to_path_buf(),
                span: item.span,
            },
            method_name: method.name.clone(),
            method_span: method.name_span,
        })
    })
}

fn broken_source_trait_method_implementation_query_for_source(
    current_path: &Path,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<TraitMethodImplementationQuery> {
    let offset = position_to_offset(source, position)?;
    let source_path = package_source_path_for_module(package, current_path)?;
    let package_name = manifest_package_name(package.manifest()).ok()?.to_owned();

    broken_source_trait_decl_sites_in_source(source)
        .into_iter()
        .find_map(|site| {
            let (method_name, method_span) = site
                .method_spans
                .into_iter()
                .find(|(_, span)| offset_hits_span(offset, *span))?;

            Some(TraitMethodImplementationQuery {
                trait_target: DependencyDefinitionTarget {
                    package_name: package_name.clone(),
                    manifest_path: package.manifest().manifest_path.clone(),
                    source_path: source_path.clone(),
                    kind: ql_analysis::SymbolKind::Trait,
                    name: site.trait_name,
                    path: current_path.to_path_buf(),
                    span: site.trait_span,
                },
                method_name,
                method_span,
            })
        })
}

fn local_trait_definition_target_for_item(
    current_path: &Path,
    package: &ql_analysis::PackageAnalysis,
    analysis: &Analysis,
    item_id: ql_hir::ItemId,
) -> Option<DependencyDefinitionTarget> {
    let ql_hir::ItemKind::Trait(trait_decl) = &analysis.hir().item(item_id).kind else {
        return None;
    };
    let source_path = package_source_path_for_module(package, current_path)?;
    let package_name = manifest_package_name(package.manifest()).ok()?.to_owned();

    Some(DependencyDefinitionTarget {
        package_name,
        manifest_path: package.manifest().manifest_path.clone(),
        source_path,
        kind: ql_analysis::SymbolKind::Trait,
        name: trait_decl.name.clone(),
        path: current_path.to_path_buf(),
        span: analysis.hir().item(item_id).span,
    })
}

fn trait_method_call_implementation_query_for_type_id(
    current_path: &Path,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    type_id: ql_hir::TypeId,
    method_name: &str,
    method_span: Span,
) -> Option<TraitMethodImplementationQuery> {
    let trait_target = match analysis.resolution().type_resolution(type_id)? {
        ql_resolve::TypeResolution::Item(item_id) => {
            local_trait_definition_target_for_item(current_path, package, analysis, *item_id)?
        }
        ql_resolve::TypeResolution::Import(_) => package
            .dependency_type_definition_at(analysis, analysis.hir().ty(type_id).span.start)
            .or_else(|| {
                package
                    .dependency_type_definition_in_source_at(source, analysis.hir().ty(type_id).span.start)
            })?,
        ql_resolve::TypeResolution::Generic(_) | ql_resolve::TypeResolution::Builtin(_) => {
            return None;
        }
    };
    if trait_target.kind != ql_analysis::SymbolKind::Trait {
        return None;
    }

    Some(TraitMethodImplementationQuery {
        trait_target,
        method_name: method_name.to_owned(),
        method_span,
    })
}

fn trait_method_call_receiver_type_id(
    analysis: &Analysis,
    function: &ql_hir::Function,
    object: ql_hir::ExprId,
) -> Option<ql_hir::TypeId> {
    match analysis.resolution().expr_resolution(object)? {
        ql_resolve::ValueResolution::Local(local_id) => analysis.hir().local(*local_id).ty,
        ql_resolve::ValueResolution::Param(binding) => match function.params.get(binding.index)? {
            ql_hir::Param::Regular(param) => Some(param.ty),
            ql_hir::Param::Receiver(_) => None,
        },
        ql_resolve::ValueResolution::SelfValue
        | ql_resolve::ValueResolution::Function(_)
        | ql_resolve::ValueResolution::Item(_)
        | ql_resolve::ValueResolution::Import(_) => None,
    }
}

fn trait_method_call_implementation_query_for_member(
    current_path: &Path,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    function: &ql_hir::Function,
    object: ql_hir::ExprId,
    method_name: &str,
    method_span: Span,
) -> Option<TraitMethodImplementationQuery> {
    let type_id = trait_method_call_receiver_type_id(analysis, function, object)?;
    trait_method_call_implementation_query_for_type_id(
        current_path,
        source,
        analysis,
        package,
        type_id,
        method_name,
        method_span,
    )
}

fn trait_method_call_implementation_query_in_expr(
    current_path: &Path,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    function: &ql_hir::Function,
    expr_id: ql_hir::ExprId,
    offset: usize,
) -> Option<TraitMethodImplementationQuery> {
    match &analysis.hir().expr(expr_id).kind {
        ql_hir::ExprKind::Tuple(items) | ql_hir::ExprKind::Array(items) => items.iter().find_map(
            |item| {
                trait_method_call_implementation_query_in_expr(
                    current_path,
                    source,
                    analysis,
                    package,
                    function,
                    *item,
                    offset,
                )
            },
        ),
        ql_hir::ExprKind::Block(block) | ql_hir::ExprKind::Unsafe(block) => {
            trait_method_call_implementation_query_in_block(
                current_path,
                source,
                analysis,
                package,
                function,
                *block,
                offset,
            )
        }
        ql_hir::ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => trait_method_call_implementation_query_in_expr(
            current_path,
            source,
            analysis,
            package,
            function,
            *condition,
            offset,
        )
        .or_else(|| {
            trait_method_call_implementation_query_in_block(
                current_path,
                source,
                analysis,
                package,
                function,
                *then_branch,
                offset,
            )
        })
        .or_else(|| {
            else_branch.and_then(|expr| {
                trait_method_call_implementation_query_in_expr(
                    current_path,
                    source,
                    analysis,
                    package,
                    function,
                    expr,
                    offset,
                )
            })
        }),
        ql_hir::ExprKind::Match { value, arms } => trait_method_call_implementation_query_in_expr(
            current_path,
            source,
            analysis,
            package,
            function,
            *value,
            offset,
        )
        .or_else(|| {
            arms.iter().find_map(|arm| {
                arm.guard
                    .and_then(|guard| {
                        trait_method_call_implementation_query_in_expr(
                            current_path,
                            source,
                            analysis,
                            package,
                            function,
                            guard,
                            offset,
                        )
                    })
                    .or_else(|| {
                        trait_method_call_implementation_query_in_expr(
                            current_path,
                            source,
                            analysis,
                            package,
                            function,
                            arm.body,
                            offset,
                        )
                    })
            })
        }),
        ql_hir::ExprKind::Closure { body, .. } => {
            trait_method_call_implementation_query_in_expr(
                current_path,
                source,
                analysis,
                package,
                function,
                *body,
                offset,
            )
        }
        ql_hir::ExprKind::Call { callee, args } => {
            if let ql_hir::ExprKind::Member {
                object,
                field,
                field_span,
            } = &analysis.hir().expr(*callee).kind
                && field_span.contains(offset)
            {
                return trait_method_call_implementation_query_for_member(
                    current_path,
                    source,
                    analysis,
                    package,
                    function,
                    *object,
                    field,
                    *field_span,
                );
            }

            trait_method_call_implementation_query_in_expr(
                current_path,
                source,
                analysis,
                package,
                function,
                *callee,
                offset,
            )
            .or_else(|| {
                args.iter().find_map(|arg| match arg {
                    ql_hir::CallArg::Positional(expr) => {
                        trait_method_call_implementation_query_in_expr(
                            current_path,
                            source,
                            analysis,
                            package,
                            function,
                            *expr,
                            offset,
                        )
                    }
                    ql_hir::CallArg::Named { value, .. } => {
                        trait_method_call_implementation_query_in_expr(
                            current_path,
                            source,
                            analysis,
                            package,
                            function,
                            *value,
                            offset,
                        )
                    }
                })
            })
        }
        ql_hir::ExprKind::Member { object, .. } | ql_hir::ExprKind::Question(object) => {
            trait_method_call_implementation_query_in_expr(
                current_path,
                source,
                analysis,
                package,
                function,
                *object,
                offset,
            )
        }
        ql_hir::ExprKind::Bracket { target, items } => {
            trait_method_call_implementation_query_in_expr(
                current_path,
                source,
                analysis,
                package,
                function,
                *target,
                offset,
            )
            .or_else(|| {
                items.iter().find_map(|item| {
                    trait_method_call_implementation_query_in_expr(
                        current_path,
                        source,
                        analysis,
                        package,
                        function,
                        *item,
                        offset,
                    )
                })
            })
        }
        ql_hir::ExprKind::StructLiteral { fields, .. } => fields.iter().find_map(|field| {
            trait_method_call_implementation_query_in_expr(
                current_path,
                source,
                analysis,
                package,
                function,
                field.value,
                offset,
            )
        }),
        ql_hir::ExprKind::Binary { left, right, .. } => {
            trait_method_call_implementation_query_in_expr(
                current_path,
                source,
                analysis,
                package,
                function,
                *left,
                offset,
            )
            .or_else(|| {
                trait_method_call_implementation_query_in_expr(
                    current_path,
                    source,
                    analysis,
                    package,
                    function,
                    *right,
                    offset,
                )
            })
        }
        ql_hir::ExprKind::Unary { expr, .. } => trait_method_call_implementation_query_in_expr(
            current_path,
            source,
            analysis,
            package,
            function,
            *expr,
            offset,
        ),
        ql_hir::ExprKind::Name(_)
        | ql_hir::ExprKind::Integer(_)
        | ql_hir::ExprKind::String { .. }
        | ql_hir::ExprKind::Bool(_)
        | ql_hir::ExprKind::NoneLiteral => None,
    }
}

fn trait_method_call_implementation_query_in_stmt(
    current_path: &Path,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    function: &ql_hir::Function,
    stmt_id: ql_hir::StmtId,
    offset: usize,
) -> Option<TraitMethodImplementationQuery> {
    match &analysis.hir().stmt(stmt_id).kind {
        ql_hir::StmtKind::Let { value, .. }
        | ql_hir::StmtKind::Return(Some(value))
        | ql_hir::StmtKind::Defer(value)
        | ql_hir::StmtKind::Expr { expr: value, .. } => {
            trait_method_call_implementation_query_in_expr(
                current_path,
                source,
                analysis,
                package,
                function,
                *value,
                offset,
            )
        }
        ql_hir::StmtKind::While { condition, body } => {
            trait_method_call_implementation_query_in_expr(
                current_path,
                source,
                analysis,
                package,
                function,
                *condition,
                offset,
            )
            .or_else(|| {
                trait_method_call_implementation_query_in_block(
                    current_path,
                    source,
                    analysis,
                    package,
                    function,
                    *body,
                    offset,
                )
            })
        }
        ql_hir::StmtKind::Loop { body } => trait_method_call_implementation_query_in_block(
            current_path,
            source,
            analysis,
            package,
            function,
            *body,
            offset,
        ),
        ql_hir::StmtKind::For { iterable, body, .. } => {
            trait_method_call_implementation_query_in_expr(
                current_path,
                source,
                analysis,
                package,
                function,
                *iterable,
                offset,
            )
            .or_else(|| {
                trait_method_call_implementation_query_in_block(
                    current_path,
                    source,
                    analysis,
                    package,
                    function,
                    *body,
                    offset,
                )
            })
        }
        ql_hir::StmtKind::Return(None)
        | ql_hir::StmtKind::Break
        | ql_hir::StmtKind::Continue => None,
    }
}

fn trait_method_call_implementation_query_in_block(
    current_path: &Path,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    function: &ql_hir::Function,
    block_id: ql_hir::BlockId,
    offset: usize,
) -> Option<TraitMethodImplementationQuery> {
    let block = analysis.hir().block(block_id);
    block
        .statements
        .iter()
        .find_map(|stmt_id| {
            trait_method_call_implementation_query_in_stmt(
                current_path,
                source,
                analysis,
                package,
                function,
                *stmt_id,
                offset,
            )
        })
        .or_else(|| {
            block.tail.and_then(|expr| {
                trait_method_call_implementation_query_in_expr(
                    current_path,
                    source,
                    analysis,
                    package,
                    function,
                    expr,
                    offset,
                )
            })
        })
}

fn trait_method_call_implementation_query_for_source(
    current_path: &Path,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<TraitMethodImplementationQuery> {
    let offset = position_to_offset(source, position)?;

    analysis.hir().items.iter().find_map(|item_id| match &analysis.hir().item(*item_id).kind {
        ql_hir::ItemKind::Function(function) => function.body.and_then(|body| {
            trait_method_call_implementation_query_in_block(
                current_path,
                source,
                analysis,
                package,
                function,
                body,
                offset,
            )
        }),
        ql_hir::ItemKind::Impl(impl_block) => impl_block.methods.iter().find_map(|method| {
            method.body.and_then(|body| {
                trait_method_call_implementation_query_in_block(
                    current_path,
                    source,
                    analysis,
                    package,
                    method,
                    body,
                    offset,
                )
            })
        }),
        ql_hir::ItemKind::Extend(extend_block) => extend_block.methods.iter().find_map(|method| {
            method.body.and_then(|body| {
                trait_method_call_implementation_query_in_block(
                    current_path,
                    source,
                    analysis,
                    package,
                    method,
                    body,
                    offset,
                )
            })
        }),
        _ => None,
    })
}

fn workspace_trait_method_implementation_sites_with_open_docs(
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    target: &DependencyDefinitionTarget,
    method_name: &str,
) -> Vec<WorkspaceMethodDefinitionSite> {
    if target.kind != ql_analysis::SymbolKind::Trait {
        return Vec::new();
    }

    let mut sites = Vec::new();

    for module in package.modules() {
        let canonical_module_path = canonicalize_or_clone(module.path());
        let (uri, source, analysis) = if let Some((open_uri, open_source)) =
            open_docs.get(&canonical_module_path)
        {
            if let Ok(open_analysis) = analyze_source(open_source) {
                (open_uri.clone(), open_source.clone(), open_analysis)
            } else {
                let mut module_sites = broken_source_trait_method_implementation_sites_in_source(
                    open_uri,
                    open_source,
                    package,
                    target,
                    method_name,
                );
                module_sites.sort_by_key(|site| {
                    (
                        site.location.range.start.line,
                        site.location.range.start.character,
                        site.location.range.end.line,
                        site.location.range.end.character,
                    )
                });
                module_sites
                    .dedup_by(|left, right| same_location_anchor(&left.location, &right.location));
                sites.extend(module_sites);
                continue;
            }
        } else {
            let Ok(uri) = Url::from_file_path(module.path()) else {
                continue;
            };
            let Ok(source) = fs::read_to_string(module.path()) else {
                continue;
            };
            (uri, source.replace("\r\n", "\n"), module.analysis().clone())
        };

        let mut module_sites = analysis
            .ast()
            .items
            .iter()
            .filter_map(|item| {
                let AstItemKind::Impl(impl_block) = &item.kind else {
                    return None;
                };
                impl_block
                    .trait_ty
                    .as_ref()
                    .filter(|trait_ty| {
                        implementation_target_matches_dependency_type_expr(
                            package, &source, &analysis, target, trait_ty,
                        )
                    })
                    .map(|_| {
                        impl_block
                            .methods
                            .iter()
                            .filter(|method| method.name == method_name)
                            .map(|method| WorkspaceMethodDefinitionSite {
                                location: Location::new(
                                    uri.clone(),
                                    span_to_range(&source, method.name_span),
                                ),
                                definition_target: ql_analysis::DefinitionTarget {
                                    kind: ql_analysis::SymbolKind::Method,
                                    name: method.name.clone(),
                                    span: method.name_span,
                                },
                            })
                            .collect::<Vec<_>>()
                    })
            })
            .flatten()
            .collect::<Vec<_>>();

        module_sites.sort_by_key(|site| {
            (
                site.location.range.start.line,
                site.location.range.start.character,
                site.location.range.end.line,
                site.location.range.end.character,
            )
        });
        module_sites.dedup_by(|left, right| same_location_anchor(&left.location, &right.location));
        sites.extend(module_sites);
    }

    sites
}

fn extend_workspace_trait_method_implementation_locations_with_open_docs(
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    target: &DependencyDefinitionTarget,
    method_name: &str,
    locations: &mut Vec<Location>,
) {
    locations.extend(
        workspace_trait_method_implementation_sites_with_open_docs(
            package,
            open_docs,
            target,
            method_name,
        )
        .into_iter()
        .map(|site| site.location),
    );
}

fn workspace_source_trait_method_references_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let current_path = uri.to_file_path().ok()?;
    let query = trait_method_implementation_query_for_source(
        &current_path,
        source,
        analysis,
        package,
        position,
    )?;
    let source_definition = Location::new(uri.clone(), span_to_range(source, query.method_span));
    let mut locations = Vec::new();

    if let Some(mut source_locations) =
        same_file_references_for_source_location_with_open_docs(&source_definition, open_docs)
    {
        if !include_declaration {
            source_locations.retain(|location| !same_location_anchor(location, &source_definition));
        }
        merge_unique_reference_locations(&mut locations, source_locations);
    }

    for candidate_manifest_path in
        visible_manifest_paths_for_package(package.manifest().manifest_path.as_path())
    {
        let Some(candidate_package) = package_analysis_for_path(&candidate_manifest_path) else {
            continue;
        };
        let implementation_sites = workspace_trait_method_implementation_sites_with_open_docs(
            &candidate_package,
            open_docs,
            &query.trait_target,
            &query.method_name,
        );

        for implementation_site in implementation_sites {
            if let Some(mut same_file_locations) =
                same_file_references_for_source_location_with_open_docs(
                    &implementation_site.location,
                    open_docs,
                )
            {
                if !include_declaration {
                    same_file_locations.retain(|location| {
                        !same_location_anchor(location, &implementation_site.location)
                    });
                }
                merge_unique_reference_locations(&mut locations, same_file_locations);
            }

            let implementation_path = implementation_site.location.uri.to_file_path().ok();
            merge_unique_reference_locations(
                &mut locations,
                workspace_visible_source_references_for_definition_with_open_docs(
                    package,
                    implementation_path.as_deref(),
                    open_docs,
                    &implementation_site.definition_target,
                    &implementation_site.location,
                    include_declaration,
                ),
            );
        }
    }

    if locations.is_empty() {
        return None;
    }

    if include_declaration {
        normalize_reference_locations_with_definition(&mut locations, &source_definition);
    }

    Some(locations)
}

fn workspace_source_trait_method_implementation_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<GotoImplementationResponse> {
    let current_path = uri.to_file_path().ok()?;
    let query = trait_method_implementation_query_for_source(
        &current_path,
        source,
        analysis,
        package,
        position,
    )?;
    implementation_response_from_locations(
        workspace_trait_method_implementation_locations_with_open_docs(
            package,
            open_docs,
            &query.trait_target,
            &query.method_name,
        ),
    )
}

fn workspace_source_trait_method_implementation_in_broken_source_with_open_docs(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<GotoImplementationResponse> {
    let current_path = uri.to_file_path().ok()?;
    let query = broken_source_trait_method_implementation_query_for_source(
        &current_path,
        source,
        package,
        position,
    )?;

    implementation_response_from_locations(
        workspace_trait_method_implementation_locations_with_open_docs(
            package,
            open_docs,
            &query.trait_target,
            &query.method_name,
        ),
    )
}

fn extend_workspace_dependency_implementation_locations_with_open_docs(
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    target: &DependencyDefinitionTarget,
    locations: &mut Vec<Location>,
) {
    if !matches!(
        target.kind,
        ql_analysis::SymbolKind::Struct
            | ql_analysis::SymbolKind::Enum
            | ql_analysis::SymbolKind::Trait
    ) {
        return;
    }

    for module in package.modules() {
        let canonical_module_path = canonicalize_or_clone(module.path());
        let (uri, source, analysis) =
            if let Some((open_uri, open_source)) = open_docs.get(&canonical_module_path) {
                if let Ok(open_analysis) = analyze_source(open_source) {
                    (open_uri.clone(), open_source.clone(), open_analysis)
                } else {
                    let mut module_locations = broken_source_implementation_locations_in_source(
                        open_uri,
                        open_source,
                        package,
                        target,
                    );
                    module_locations.sort_by_key(|location| {
                        (
                            location.range.start.line,
                            location.range.start.character,
                            location.range.end.line,
                            location.range.end.character,
                        )
                    });
                    module_locations.dedup_by(|left, right| same_location_anchor(left, right));
                    locations.extend(module_locations);
                    continue;
                }
            } else {
                let Ok(uri) = Url::from_file_path(module.path()) else {
                    continue;
                };
                let Ok(source) = fs::read_to_string(module.path()) else {
                    continue;
                };
                (uri, source.replace("\r\n", "\n"), module.analysis().clone())
            };

        let mut module_locations = analysis
            .ast()
            .items
            .iter()
            .filter_map(|item| match (&target.kind, &item.kind) {
                (
                    ql_analysis::SymbolKind::Struct | ql_analysis::SymbolKind::Enum,
                    AstItemKind::Impl(impl_block),
                ) if implementation_target_matches_dependency_type_expr(
                    package,
                    &source,
                    &analysis,
                    target,
                    &impl_block.target,
                ) =>
                {
                    Some(Location::new(
                        uri.clone(),
                        span_to_range(&source, item.span),
                    ))
                }
                (
                    ql_analysis::SymbolKind::Struct | ql_analysis::SymbolKind::Enum,
                    AstItemKind::Extend(extend_block),
                ) if implementation_target_matches_dependency_type_expr(
                    package,
                    &source,
                    &analysis,
                    target,
                    &extend_block.target,
                ) =>
                {
                    Some(Location::new(
                        uri.clone(),
                        span_to_range(&source, item.span),
                    ))
                }
                (ql_analysis::SymbolKind::Trait, AstItemKind::Impl(impl_block))
                    if impl_block.trait_ty.as_ref().is_some_and(|trait_ty| {
                        implementation_target_matches_dependency_type_expr(
                            package, &source, &analysis, target, trait_ty,
                        )
                    }) =>
                {
                    Some(Location::new(
                        uri.clone(),
                        span_to_range(&source, item.span),
                    ))
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        module_locations.sort_by_key(|location| {
            (
                location.range.start.line,
                location.range.start.character,
                location.range.end.line,
                location.range.end.character,
            )
        });
        module_locations.dedup_by(|left, right| same_location_anchor(left, right));
        locations.extend(module_locations);
    }
}

fn implementation_response_from_locations(
    mut locations: Vec<Location>,
) -> Option<GotoImplementationResponse> {
    if locations.is_empty() {
        return None;
    }

    locations.sort_by_key(|location| {
        (
            location.uri.to_string(),
            location.range.start.line,
            location.range.start.character,
            location.range.end.line,
            location.range.end.character,
        )
    });
    locations.dedup_by(|left, right| same_location_anchor(left, right));

    if locations.len() == 1 {
        Some(GotoImplementationResponse::Scalar(
            locations
                .into_iter()
                .next()
                .expect("single location exists"),
        ))
    } else {
        Some(GotoImplementationResponse::Array(locations))
    }
}

fn workspace_implementation_locations_with_open_docs(
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    target: &DependencyDefinitionTarget,
) -> Vec<Location> {
    let mut locations = Vec::new();

    for candidate_manifest_path in
        visible_manifest_paths_for_package(package.manifest().manifest_path.as_path())
    {
        let Some(candidate_package) = package_analysis_for_path(&candidate_manifest_path) else {
            continue;
        };
        extend_workspace_dependency_implementation_locations_with_open_docs(
            &candidate_package,
            open_docs,
            target,
            &mut locations,
        );
    }

    locations
}

fn workspace_trait_method_implementation_locations_with_open_docs(
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    target: &DependencyDefinitionTarget,
    method_name: &str,
) -> Vec<Location> {
    let mut locations = Vec::new();

    for candidate_manifest_path in
        visible_manifest_paths_for_package(package.manifest().manifest_path.as_path())
    {
        let Some(candidate_package) = package_analysis_for_path(&candidate_manifest_path) else {
            continue;
        };
        extend_workspace_trait_method_implementation_locations_with_open_docs(
            &candidate_package,
            open_docs,
            target,
            method_name,
            &mut locations,
        );
    }

    locations
}

fn workspace_source_implementation_for_dependency_with_open_docs(
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<GotoImplementationResponse> {
    let target = dependency_type_definition_target_with_open_docs_at(
        source, analysis, package, open_docs, position,
    )?;
    implementation_response_from_locations(workspace_implementation_locations_with_open_docs(
        package, open_docs, &target,
    ))
}

fn method_definition_location_at(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<Location> {
    let offset = position_to_offset(source, position)?;
    let definition = analysis.definition_at(offset)?;
    if definition.kind != ql_analysis::SymbolKind::Method || !definition.span.contains(offset) {
        return None;
    }

    Some(Location::new(uri.clone(), span_to_range(source, definition.span)))
}

fn workspace_source_method_implementation_for_dependency_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<GotoImplementationResponse> {
    if let Some(analysis) = analysis
        && let Some(query) = trait_method_call_implementation_query_for_source(
            &uri.to_file_path().ok()?,
            source,
            analysis,
            package,
            position,
        )
    {
        return implementation_response_from_locations(
            workspace_trait_method_implementation_locations_with_open_docs(
                package,
                open_docs,
                &query.trait_target,
                &query.method_name,
            ),
        );
    }

    let target = dependency_definition_target_with_open_docs_at(
        source, analysis, package, open_docs, position,
    )?;
    if target.kind != ql_analysis::SymbolKind::Method {
        return None;
    }

    let source_definition = analysis.and_then(|analysis| {
        method_definition_location_at(uri, source, analysis, position)
    });
    let implementation = workspace_source_location_for_dependency_target_with_open_docs(
        uri, source, analysis, package, open_docs, &target,
    )?;
    if source_definition
        .as_ref()
        .is_some_and(|source_definition| same_location_anchor(&implementation, source_definition))
    {
        return None;
    }

    Some(GotoImplementationResponse::Scalar(implementation))
}

fn workspace_source_method_implementation_for_local_source_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<GotoImplementationResponse> {
    let current_path = uri.to_file_path().ok()?;
    if let Some(query) = trait_method_call_implementation_query_for_source(
        &current_path,
        source,
        analysis,
        package,
        position,
    ) {
        return implementation_response_from_locations(
            workspace_trait_method_implementation_locations_with_open_docs(
                package,
                open_docs,
                &query.trait_target,
                &query.method_name,
            ),
        );
    }

    let Some(target) = local_source_dependency_target_with_analysis(
        uri, source, analysis, package, open_docs, position,
    ) else {
        return implementation_for_analysis(uri, source, analysis, position);
    };
    if target.kind != ql_analysis::SymbolKind::Method {
        return None;
    }

    let source_definition = method_definition_location_at(uri, source, analysis, position);
    let implementation = workspace_source_location_for_dependency_target_with_open_docs(
        uri,
        source,
        Some(analysis),
        package,
        open_docs,
        &target,
    )?;
    if source_definition
        .as_ref()
        .is_some_and(|source_definition| same_location_anchor(&implementation, source_definition))
    {
        return None;
    }

    Some(GotoImplementationResponse::Scalar(implementation))
}

fn broken_source_method_call_name_at(
    source: &str,
    position: tower_lsp::lsp_types::Position,
) -> Option<String> {
    let offset = position_to_offset(source, position)?;
    let (tokens, _) = lex(source);
    let (index, token) = tokens.iter().enumerate().find(|(_, token)| {
        token.kind == TokenKind::Ident && offset_hits_span(offset, token.span)
    })?;
    if tokens.get(index.checked_sub(1)?).map(|token| token.kind) != Some(TokenKind::Dot)
        || tokens.get(index + 1).map(|token| token.kind) != Some(TokenKind::LParen)
    {
        return None;
    }

    Some(token.text.clone())
}

fn repaired_analysis_for_broken_method_call(
    source: &str,
    position: tower_lsp::lsp_types::Position,
) -> Option<(String, Analysis, tower_lsp::lsp_types::Position)> {
    let offset = position_to_offset(source, position)?;
    let (tokens, _) = lex(source);
    let (index, _token) = tokens.iter().enumerate().find(|(_, token)| {
        token.kind == TokenKind::Ident && offset_hits_span(offset, token.span)
    })?;
    if tokens.get(index.checked_sub(1)?).map(|token| token.kind) != Some(TokenKind::Dot)
        || tokens.get(index + 1).map(|token| token.kind) != Some(TokenKind::LParen)
    {
        return None;
    }

    let insert_offset = tokens.get(index + 1)?.span.end;
    let mut repaired_source = source.to_owned();
    repaired_source.insert(insert_offset, ')');
    let analysis = analyze_source(&repaired_source).ok()?;
    Some((
        repaired_source,
        analysis,
        span_to_range(source, Span::new(insert_offset, insert_offset)).start,
    ))
}

fn map_position_after_repaired_method_call_insert(
    position: tower_lsp::lsp_types::Position,
    inserted_position: tower_lsp::lsp_types::Position,
) -> tower_lsp::lsp_types::Position {
    if position.line == inserted_position.line && position.character > inserted_position.character {
        tower_lsp::lsp_types::Position::new(position.line, position.character - 1)
    } else {
        position
    }
}

fn map_location_after_repaired_method_call_insert(
    current_uri: &Url,
    location: Location,
    inserted_position: tower_lsp::lsp_types::Position,
) -> Location {
    if location.uri != *current_uri {
        return location;
    }

    Location::new(
        location.uri,
        tower_lsp::lsp_types::Range::new(
            map_position_after_repaired_method_call_insert(location.range.start, inserted_position),
            map_position_after_repaired_method_call_insert(location.range.end, inserted_position),
        ),
    )
}

fn map_implementation_response_after_repaired_method_call_insert(
    current_uri: &Url,
    response: GotoImplementationResponse,
    inserted_position: tower_lsp::lsp_types::Position,
) -> GotoImplementationResponse {
    match response {
        GotoImplementationResponse::Scalar(location) => GotoImplementationResponse::Scalar(
            map_location_after_repaired_method_call_insert(current_uri, location, inserted_position),
        ),
        GotoImplementationResponse::Array(locations) => GotoImplementationResponse::Array(
            locations
                .into_iter()
                .map(|location| {
                    map_location_after_repaired_method_call_insert(
                        current_uri,
                        location,
                        inserted_position,
                    )
                })
                .collect(),
        ),
        GotoImplementationResponse::Link(links) => GotoImplementationResponse::Link(links),
    }
}

fn workspace_source_method_implementation_for_broken_source_with_open_docs(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<GotoImplementationResponse> {
    let (repaired_source, repaired_analysis, inserted_position) =
        repaired_analysis_for_broken_method_call(source, position)?;
    let implementation = workspace_source_method_implementation_for_dependency_with_open_docs(
        uri,
        &repaired_source,
        Some(&repaired_analysis),
        package,
        open_docs,
        position,
    )?;

    Some(map_implementation_response_after_repaired_method_call_insert(
        uri,
        implementation,
        inserted_position,
    ))
}

fn workspace_source_method_implementation_for_local_source_in_broken_source_with_open_docs(
    uri: &Url,
    source: &str,
    position: tower_lsp::lsp_types::Position,
) -> Option<GotoImplementationResponse> {
    let method_name = broken_source_method_call_name_at(source, position)?;
    let mut locations =
        broken_source_method_definition_locations_in_source(uri, source, &method_name);
    locations.sort_by_key(|location| {
        (
            location.range.start.line,
            location.range.start.character,
            location.range.end.line,
            location.range.end.character,
        )
    });
    locations.dedup_by(|left, right| same_location_anchor(left, right));
    (locations.len() == 1).then(|| {
        implementation_response_from_locations(locations)
            .expect("single broken-source method implementation exists")
    })
}

fn workspace_source_definition_for_dependency(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<GotoDefinitionResponse> {
    let open_docs = OpenDocuments::new();
    workspace_source_definition_for_dependency_with_open_docs(
        uri, source, analysis, package, &open_docs, position,
    )
}

fn workspace_source_definition_for_dependency_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<GotoDefinitionResponse> {
    let target = dependency_definition_target_with_open_docs_at(
        source, analysis, package, open_docs, position,
    )?;
    workspace_source_location_for_dependency_target_with_open_docs(
        uri, source, analysis, package, open_docs, &target,
    )
    .map(GotoDefinitionResponse::Scalar)
}

fn workspace_source_type_definition_for_dependency(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<GotoTypeDefinitionResponse> {
    let open_docs = OpenDocuments::new();
    workspace_source_type_definition_for_dependency_with_open_docs(
        uri, source, analysis, package, &open_docs, position,
    )
}

fn workspace_source_type_definition_for_dependency_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<GotoTypeDefinitionResponse> {
    let target = dependency_type_definition_target_with_open_docs_at(
        source, analysis, package, open_docs, position,
    )?;
    workspace_source_location_for_dependency_target_with_open_docs(
        uri, source, analysis, package, open_docs, &target,
    )
    .map(GotoTypeDefinitionResponse::Scalar)
}

fn workspace_source_hover_for_dependency(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<Hover> {
    let open_docs = OpenDocuments::new();
    workspace_source_hover_for_dependency_with_open_docs(
        uri, source, analysis, package, &open_docs, position,
    )
}

fn workspace_source_hover_for_dependency_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<Hover> {
    let occurrence_span =
        dependency_occurrence_span_with_open_docs_at(source, package, open_docs, position)?;
    let target = dependency_definition_target_with_open_docs_at(
        source, analysis, package, open_docs, position,
    )?;
    let source_location = workspace_source_location_for_dependency_target_with_open_docs(
        uri, source, analysis, package, open_docs, &target,
    )?;

    hover_from_workspace_source_location_with_open_docs(
        source,
        occurrence_span,
        source_location,
        open_docs,
    )
}

fn workspace_source_dependency_completion(
    source: &str,
    offset: usize,
    package: &ql_analysis::PackageAnalysis,
    target_manifest_path: &Path,
    target_source_path: &str,
    open_docs: &OpenDocuments,
    items_for_package: impl Fn(
        &ql_analysis::PackageAnalysis,
        Option<&str>,
    ) -> Option<Vec<ql_analysis::CompletionItem>>,
) -> Option<CompletionResponse> {
    for candidate_manifest_path in
        source_preferred_manifest_paths_for_package(package.manifest().manifest_path.as_path())
    {
        let Some(candidate_package) = package_analysis_for_path(&candidate_manifest_path) else {
            continue;
        };
        if canonicalize_or_clone(candidate_package.manifest().manifest_path.as_path())
            != canonicalize_or_clone(target_manifest_path)
        {
            continue;
        }
        let open_source = open_docs.iter().find_map(|(path, (_, open_source))| {
            candidate_package.modules().iter().find_map(|module| {
                (canonicalize_or_clone(module.path()) == canonicalize_or_clone(path)
                    && package_module_matches_dependency_source_path(
                        &candidate_package,
                        module.path(),
                        target_source_path,
                    ))
                .then_some(open_source.as_str())
            })
        });
        if let Some(items) = items_for_package(&candidate_package, open_source) {
            return completion_response(source, offset, items);
        }
    }
    None
}

fn workspace_source_variant_completions(
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<CompletionResponse> {
    let open_docs = OpenDocuments::new();
    workspace_source_variant_completions_with_open_docs(source, package, &open_docs, position)
}

fn workspace_source_variant_completions_with_open_docs(
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<CompletionResponse> {
    let offset = position_to_offset(source, position)?;
    let target = package.dependency_variant_completion_target_in_source_at(source, offset)?;
    workspace_source_dependency_completion(
        source,
        offset,
        package,
        target.manifest_path.as_path(),
        &target.source_path,
        open_docs,
        |candidate_package, open_source| {
            if let Some(open_source) = open_source {
                return candidate_package.public_enum_variant_completions_in_source(
                    &target.source_path,
                    open_source,
                    &target.enum_name,
                );
            }
            candidate_package
                .public_enum_variant_completions(&target.source_path, &target.enum_name)
        },
    )
}

fn workspace_source_struct_field_completions(
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<CompletionResponse> {
    let open_docs = OpenDocuments::new();
    workspace_source_struct_field_completions_with_open_docs(source, package, &open_docs, position)
}

fn workspace_source_struct_field_completions_with_open_docs(
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<CompletionResponse> {
    let offset = position_to_offset(source, position)?;
    let target = package.dependency_struct_field_completion_target_in_source_at(source, offset)?;
    workspace_source_dependency_completion(
        source,
        offset,
        package,
        target.target.manifest_path.as_path(),
        &target.target.source_path,
        open_docs,
        |candidate_package, open_source| {
            if let Some(open_source) = open_source {
                return candidate_package.public_struct_literal_field_completions_in_source(
                    &target.target.source_path,
                    open_source,
                    &target.target.struct_name,
                    &target.excluded_field_names,
                );
            }
            candidate_package.public_struct_literal_field_completions(
                &target.target.source_path,
                &target.target.struct_name,
                &target.excluded_field_names,
            )
        },
    )
}

fn workspace_source_member_field_completions(
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<CompletionResponse> {
    let open_docs = OpenDocuments::new();
    workspace_source_member_field_completions_with_open_docs(source, package, &open_docs, position)
}

fn workspace_source_member_field_completions_with_open_docs(
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<CompletionResponse> {
    let offset = position_to_offset(source, position)?;
    let target = package.dependency_member_field_completion_target_in_source_at(source, offset)?;
    workspace_source_dependency_completion(
        source,
        offset,
        package,
        target.manifest_path.as_path(),
        &target.source_path,
        open_docs,
        |candidate_package, open_source| {
            if let Some(open_source) = open_source {
                return candidate_package.public_struct_member_field_completions_in_source(
                    &target.source_path,
                    open_source,
                    &target.struct_name,
                );
            }
            candidate_package
                .public_struct_member_field_completions(&target.source_path, &target.struct_name)
        },
    )
}

fn workspace_source_method_completions(
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<CompletionResponse> {
    let open_docs = OpenDocuments::new();
    workspace_source_method_completions_with_open_docs(source, package, &open_docs, position)
}

fn workspace_source_method_completions_with_open_docs(
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<CompletionResponse> {
    let offset = position_to_offset(source, position)?;
    let target = package
        .dependency_method_completion_target_in_source_at(source, offset)
        .or_else(|| {
            offset.checked_sub(1).and_then(|fallback_offset| {
                package.dependency_method_completion_target_in_source_at(source, fallback_offset)
            })
        })?;
    workspace_source_dependency_completion(
        source,
        offset,
        package,
        target.manifest_path.as_path(),
        &target.source_path,
        open_docs,
        |candidate_package, open_source| {
            if let Some(open_source) = open_source {
                return candidate_package.public_struct_method_completions_in_source(
                    &target.source_path,
                    open_source,
                    &target.struct_name,
                );
            }
            candidate_package
                .public_struct_method_completions(&target.source_path, &target.struct_name)
        },
    )
}

fn workspace_source_references_for_import(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let open_docs = OpenDocuments::new();
    workspace_source_references_for_import_with_open_docs(
        uri,
        source,
        analysis,
        package,
        &open_docs,
        position,
        include_declaration,
    )
}

fn workspace_source_references_for_import_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let offset = position_to_offset(source, position)?;
    let occurrence = analyzed_import_binding_at(source, analysis, offset)?;
    let (imported_name, import_prefix) = occurrence.path_segments.split_last()?;
    let source_matches = workspace_source_locations_for_import_binding_with_open_docs(
        uri,
        source,
        Some(analysis),
        package,
        open_docs,
        import_prefix,
        imported_name,
        supports_workspace_import_definition,
    );
    if source_matches.is_empty() {
        return None;
    }

    let source_definition = (source_matches.len() == 1).then(|| source_matches[0].clone());
    let mut locations = references_for_package_analysis(
        uri,
        source,
        analysis,
        package,
        position,
        include_declaration,
    )?;
    if include_declaration && let Some(source_definition) = source_definition.as_ref() {
        normalize_reference_locations_with_definition(&mut locations, source_definition);
    }
    if let Some(source_definition) = source_definition.as_ref()
        && let Some(mut source_locations) =
            same_file_references_for_source_location_with_open_docs(source_definition, open_docs)
    {
        if !include_declaration {
            source_locations.retain(|location| !same_location_anchor(location, source_definition));
        }
        merge_unique_reference_locations(&mut locations, source_locations);
    }
    let current_path = uri.to_file_path().ok();
    merge_unique_reference_locations(
        &mut locations,
        workspace_import_reference_locations_with_open_docs(
            package,
            current_path.as_deref(),
            open_docs,
            &occurrence.path_segments,
            include_declaration,
        ),
    );
    if source_definition
        .as_ref()
        .and_then(|location| location.uri.to_file_path().ok())
        .is_some_and(|path| path.extension().is_none_or(|extension| extension != "qi"))
    {
        locations.retain(|location| {
            location
                .uri
                .to_file_path()
                .ok()
                .is_none_or(|path| path.extension().is_none_or(|extension| extension != "qi"))
        });
    }

    Some(locations)
}

fn broken_source_import_reference_locations_in_source(
    uri: &Url,
    source: &str,
    import_path: &[String],
    include_declaration: bool,
) -> Vec<Location> {
    let (tokens, _) = lex(source);
    let bindings = broken_source_import_bindings_in_tokens(&tokens);
    let mut local_name_counts = HashMap::<String, usize>::new();
    for binding in &bindings {
        *local_name_counts
            .entry(binding.local_name.clone())
            .or_insert(0usize) += 1;
    }

    let mut locations = Vec::new();
    for binding in bindings
        .into_iter()
        .filter(|binding| broken_source_import_binding_matches_path(binding, import_path))
    {
        if include_declaration {
            locations.push(Location::new(
                uri.clone(),
                span_to_range(source, binding.definition_span),
            ));
        }
        if local_name_counts.get(binding.local_name.as_str()) != Some(&1usize) {
            continue;
        }

        locations.extend(
            tokens
                .iter()
                .enumerate()
                .filter(|(_, token)| {
                    token.kind == TokenKind::Ident && token.text == binding.local_name
                })
                .filter(|(index, token)| {
                    token.span != binding.definition_span
                        && broken_source_import_token_matches_reference_context(&tokens, *index)
                })
                .map(|(_, token)| Location::new(uri.clone(), span_to_range(source, token.span))),
        );
    }

    locations.sort_by_key(|location| {
        (
            location.range.start.line,
            location.range.start.character,
            location.range.end.line,
            location.range.end.character,
        )
    });
    locations.dedup_by(|left, right| same_location_anchor(left, right));
    locations
}

fn workspace_broken_import_reference_locations_for_visible_sources(
    package_manifest_path: &Path,
    current_path: Option<&Path>,
    import_path: &[String],
    include_declaration: bool,
    open_docs: &OpenDocuments,
) -> Vec<Location> {
    let current_path = current_path.map(canonicalize_or_clone);
    let mut locations = Vec::new();

    for candidate_manifest_path in visible_manifest_paths_for_package(package_manifest_path) {
        let Some(candidate_package) = package_analysis_for_path(&candidate_manifest_path) else {
            continue;
        };
        let Ok(source_paths) = collect_package_sources(candidate_package.manifest()) else {
            continue;
        };

        for candidate_path in source_paths {
            let candidate_canonical = canonicalize_or_clone(&candidate_path);
            if current_path
                .as_ref()
                .is_some_and(|current_path| current_path == &candidate_canonical)
            {
                continue;
            }

            let Some((candidate_uri, candidate_source)) =
                open_or_disk_source_snapshot(open_docs, &candidate_path)
            else {
                continue;
            };

            let candidate_locations = if let Some((_, _, candidate_analysis)) =
                open_document_snapshot(open_docs, &candidate_path)
            {
                workspace_import_reference_locations_in_source(
                    &candidate_uri,
                    &candidate_source,
                    &candidate_analysis,
                    import_path,
                    include_declaration,
                )
            } else if analyze_source(&candidate_source).is_ok() {
                Vec::new()
            } else {
                broken_source_import_reference_locations_in_source(
                    &candidate_uri,
                    &candidate_source,
                    import_path,
                    include_declaration,
                )
            };

            merge_unique_reference_locations(&mut locations, candidate_locations);
        }
    }

    locations
}

fn workspace_source_references_for_import_in_broken_source(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let open_docs = OpenDocuments::new();
    workspace_source_references_for_import_in_broken_source_with_open_docs(
        uri,
        source,
        package,
        &open_docs,
        position,
        include_declaration,
    )
}

fn workspace_source_references_for_import_in_broken_source_with_open_docs(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let binding = broken_source_import_binding_at(source, position)?;
    let source_matches = workspace_source_locations_for_import_binding_with_open_docs(
        uri,
        source,
        None,
        package,
        open_docs,
        &binding.import_prefix,
        binding.imported_name.as_str(),
        supports_workspace_import_definition,
    );
    if source_matches.is_empty() {
        return None;
    }

    let (tokens, _) = lex(source);
    let source_definition = (source_matches.len() == 1).then(|| source_matches[0].clone());

    let mut locations = Vec::new();
    if include_declaration {
        if let Some(source_definition) = source_definition.as_ref() {
            locations.push(source_definition.clone());
        } else {
            locations.push(Location::new(
                uri.clone(),
                span_to_range(source, binding.definition_span),
            ));
        }
    }

    locations.extend(
        tokens
            .iter()
            .enumerate()
            .filter(|(_, token)| token.kind == TokenKind::Ident && token.text == binding.local_name)
            .filter(|(index, token)| {
                token.span != binding.definition_span
                    && broken_source_import_token_matches_reference_context(&tokens, *index)
            })
            .map(|(_, token)| Location::new(uri.clone(), span_to_range(source, token.span))),
    );

    if let Some(source_definition) = source_definition.as_ref()
        && let Some(mut source_locations) =
            same_file_references_for_source_location_with_open_docs(source_definition, open_docs)
    {
        if !include_declaration {
            source_locations.retain(|location| !same_location_anchor(location, source_definition));
        }
        merge_unique_reference_locations(&mut locations, source_locations);
    }
    let mut import_path = binding.import_prefix.clone();
    import_path.push(binding.imported_name.clone());
    let current_path = uri.to_file_path().ok();
    merge_unique_reference_locations(
        &mut locations,
        workspace_import_reference_locations_with_open_docs(
            package,
            current_path.as_deref(),
            open_docs,
            &import_path,
            include_declaration,
        ),
    );
    merge_unique_reference_locations(
        &mut locations,
        workspace_broken_import_reference_locations_for_visible_sources(
            package.manifest().manifest_path.as_path(),
            current_path.as_deref(),
            &import_path,
            include_declaration,
            open_docs,
        ),
    );

    (!locations.is_empty()).then_some(locations)
}

fn workspace_import_document_highlights_in_broken_source_with_open_docs(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<Vec<DocumentHighlight>> {
    let binding = broken_source_import_binding_at(source, position)?;
    let locations = workspace_source_references_for_import_in_broken_source_with_open_docs(
        uri, source, package, open_docs, position, true,
    )?;
    let mut highlights = document_highlights_from_locations(uri, locations).unwrap_or_default();
    let definition_range = span_to_range(source, binding.definition_span);
    if !highlights
        .iter()
        .any(|highlight| highlight.range == definition_range)
    {
        highlights.insert(
            0,
            DocumentHighlight {
                range: definition_range,
                kind: None,
            },
        );
    }
    Some(highlights)
}

fn workspace_import_document_highlights(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<Vec<DocumentHighlight>> {
    let open_docs = OpenDocuments::new();
    workspace_import_document_highlights_with_open_docs(
        uri, source, analysis, package, &open_docs, position,
    )
}

fn workspace_import_document_highlights_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<Vec<DocumentHighlight>> {
    let offset = position_to_offset(source, position)?;
    let (binding, _) = analysis.import_binding_at(offset)?;
    let locations = workspace_source_references_for_import_with_open_docs(
        uri, source, analysis, package, open_docs, position, true,
    )?;
    let mut highlights = document_highlights_from_locations(uri, locations).unwrap_or_default();
    let definition_range = span_to_range(source, binding.definition_span);
    if !highlights
        .iter()
        .any(|highlight| highlight.range == definition_range)
    {
        highlights.insert(
            0,
            DocumentHighlight {
                range: definition_range,
                kind: None,
            },
        );
    }
    Some(highlights)
}

fn workspace_dependency_document_highlights(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<Vec<DocumentHighlight>> {
    let open_docs = OpenDocuments::new();
    workspace_dependency_document_highlights_with_open_docs(
        uri, source, analysis, package, position, &open_docs,
    )
}

fn workspace_dependency_document_highlights_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
    open_docs: &OpenDocuments,
) -> Option<Vec<DocumentHighlight>> {
    let locations = workspace_source_references_for_dependency_with_open_docs(
        uri,
        source,
        Some(analysis),
        package,
        open_docs,
        position,
        true,
    )?;
    document_highlights_from_locations(uri, locations)
}

fn workspace_dependency_document_highlights_in_broken_source_with_open_docs(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
    open_docs: &OpenDocuments,
) -> Option<Vec<DocumentHighlight>> {
    let locations = workspace_source_references_for_dependency_in_broken_source_with_open_docs(
        uri, source, package, open_docs, position, true,
    )?;
    document_highlights_from_locations(uri, locations)
}

fn validate_rename_text(text: &str) -> std::result::Result<(), RenameError> {
    let escaped = text
        .strip_prefix('`')
        .and_then(|value| value.strip_suffix('`'));
    if let Some(inner) = escaped {
        return is_valid_identifier(inner)
            .then_some(())
            .ok_or_else(|| RenameError::InvalidIdentifier(text.to_owned()));
    }

    if !is_valid_identifier(text) {
        return Err(RenameError::InvalidIdentifier(text.to_owned()));
    }
    if is_keyword(text) {
        return Err(RenameError::Keyword(text.to_owned()));
    }

    Ok(())
}

fn supports_workspace_source_dependency_rename(kind: ql_analysis::SymbolKind) -> bool {
    matches!(
        kind,
        ql_analysis::SymbolKind::Variant
            | ql_analysis::SymbolKind::Field
            | ql_analysis::SymbolKind::Method
    )
}

fn workspace_source_dependency_prepare_rename_with_open_docs(
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<RenameTarget> {
    let offset = position_to_offset(source, position)?;
    package
        .dependency_prepare_rename_in_source_at(source, offset)
        .or_else(|| {
            let occurrence_span =
                dependency_occurrence_span_with_open_docs_at(source, package, open_docs, position)?;
            let target = dependency_definition_target_with_open_docs_at(
                source, analysis, package, open_docs, position,
            )?;
            supports_workspace_source_dependency_rename(target.kind).then_some(RenameTarget {
                kind: target.kind,
                name: source
                    .get(occurrence_span.start..occurrence_span.end)?
                    .to_owned(),
                span: occurrence_span,
            })
        })
}

fn prepare_rename_for_workspace_import_in_broken_source(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<PrepareRenameResponse> {
    let open_docs = OpenDocuments::new();
    prepare_rename_for_workspace_import_in_broken_source_with_open_docs(
        uri, source, package, &open_docs, position,
    )
}

fn prepare_rename_for_workspace_import_in_broken_source_with_open_docs(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<PrepareRenameResponse> {
    let binding = broken_source_import_binding_at(source, position)?;
    if workspace_source_locations_for_import_binding_with_open_docs(
        uri,
        source,
        None,
        package,
        open_docs,
        &binding.import_prefix,
        binding.imported_name.as_str(),
        supports_workspace_import_definition,
    )
    .is_empty()
    {
        return None;
    }

    let offset = position_to_offset(source, position)?;
    let (tokens, _) = lex(source);
    let token = tokens.iter().find(|token| {
        token.kind == TokenKind::Ident
            && token.text == binding.local_name
            && token.span.contains(offset)
    })?;
    let placeholder = source.get(token.span.start..token.span.end)?.to_owned();

    Some(PrepareRenameResponse::RangeWithPlaceholder {
        range: span_to_range(source, token.span),
        placeholder,
    })
}

fn prepare_rename_for_workspace_source_root_symbol_from_import_in_broken_source(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<PrepareRenameResponse> {
    let open_docs = OpenDocuments::new();
    prepare_rename_for_workspace_source_root_symbol_from_import_in_broken_source_with_open_docs(
        uri, source, package, &open_docs, position,
    )
}

fn prepare_rename_for_workspace_source_root_symbol_from_import_in_broken_source_with_open_docs(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<PrepareRenameResponse> {
    let binding = broken_source_import_binding_at(source, position)?;
    let occurrence_span =
        broken_source_import_occurrence_span_at(source, position, binding.local_name.as_str())?;
    let source_location = workspace_source_location_for_import_binding_with_open_docs(
        uri,
        source,
        None,
        package,
        open_docs,
        &binding.import_prefix,
        binding.imported_name.as_str(),
    )?;
    let source_path = source_location.uri.to_file_path().ok()?;
    let (definition_source, analysis) = if let Some((_, open_source, open_analysis)) =
        open_document_snapshot(open_docs, &source_path)
    {
        (open_source, open_analysis)
    } else {
        let definition_source = fs::read_to_string(source_path).ok()?.replace("\r\n", "\n");
        let analysis = analyze_source(&definition_source).ok()?;
        (definition_source, analysis)
    };
    let definition_target = definition_target_for_source_location(
        &analysis,
        &definition_source,
        source_location.range,
    )?;
    if !supports_workspace_source_root_definition_rename(definition_target.kind) {
        return None;
    }

    let placeholder = source
        .get(occurrence_span.start..occurrence_span.end)?
        .to_owned();
    Some(PrepareRenameResponse::RangeWithPlaceholder {
        range: span_to_range(source, occurrence_span),
        placeholder,
    })
}

fn prepare_rename_for_workspace_source_root_symbol_from_import(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<PrepareRenameResponse> {
    let open_docs = OpenDocuments::new();
    prepare_rename_for_workspace_source_root_symbol_from_import_with_open_docs(
        uri, source, analysis, package, &open_docs, position,
    )
}

fn prepare_rename_for_workspace_source_root_symbol_from_import_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<PrepareRenameResponse> {
    let offset = position_to_offset(source, position)?;
    let occurrence = analyzed_import_binding_at(source, analysis, offset)?;
    let (imported_name, import_prefix) = occurrence.path_segments.split_last()?;
    let source_location = workspace_source_location_for_import_binding_with_open_docs(
        uri,
        source,
        Some(analysis),
        package,
        open_docs,
        import_prefix,
        imported_name,
    )?;
    let source_path = source_location.uri.to_file_path().ok()?;
    let (definition_source, analysis) = if let Some((_, open_source, open_analysis)) =
        open_document_snapshot(open_docs, &source_path)
    {
        (open_source, open_analysis)
    } else {
        let definition_source = fs::read_to_string(source_path).ok()?.replace("\r\n", "\n");
        let analysis = analyze_source(&definition_source).ok()?;
        (definition_source, analysis)
    };
    let definition_target = definition_target_for_source_location(
        &analysis,
        &definition_source,
        source_location.range,
    )?;
    if !supports_workspace_source_root_definition_rename(definition_target.kind) {
        return None;
    }

    let placeholder = source
        .get(occurrence.occurrence_span.start..occurrence.occurrence_span.end)?
        .to_owned();

    Some(PrepareRenameResponse::RangeWithPlaceholder {
        range: span_to_range(source, occurrence.occurrence_span),
        placeholder,
    })
}

fn rename_for_workspace_import_in_broken_source(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
    new_name: &str,
) -> std::result::Result<Option<WorkspaceEdit>, RenameError> {
    let open_docs = OpenDocuments::new();
    rename_for_workspace_import_in_broken_source_with_open_docs(
        uri, source, package, &open_docs, position, new_name,
    )
}

fn rename_for_workspace_import_in_broken_source_with_open_docs(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
    new_name: &str,
) -> std::result::Result<Option<WorkspaceEdit>, RenameError> {
    validate_rename_text(new_name)?;

    let Some(binding) = broken_source_import_binding_at(source, position) else {
        return Ok(None);
    };
    if workspace_source_locations_for_import_binding_with_open_docs(
        uri,
        source,
        None,
        package,
        open_docs,
        &binding.import_prefix,
        binding.imported_name.as_str(),
        supports_workspace_import_definition,
    )
    .is_empty()
    {
        return Ok(None);
    }
    if binding.local_name == new_name {
        return Ok(None);
    }

    let mut edits = Vec::new();
    let definition_replacement = if binding.local_name == binding.imported_name {
        format!("{} as {}", binding.imported_name, new_name)
    } else {
        new_name.to_owned()
    };
    edits.push(TextEdit::new(
        span_to_range(source, binding.definition_span),
        definition_replacement,
    ));

    let (tokens, _) = lex(source);
    edits.extend(
        tokens
            .iter()
            .enumerate()
            .filter(|(_, token)| token.kind == TokenKind::Ident && token.text == binding.local_name)
            .filter(|(index, token)| {
                token.span != binding.definition_span
                    && broken_source_import_token_matches_reference_context(&tokens, *index)
            })
            .map(|(_, token)| {
                TextEdit::new(span_to_range(source, token.span), new_name.to_owned())
            }),
    );

    let mut changes = HashMap::new();
    changes.insert(uri.clone(), edits);
    Ok(Some(WorkspaceEdit::new(changes)))
}

fn rename_for_workspace_source_dependency_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
    new_name: &str,
) -> std::result::Result<Option<WorkspaceEdit>, RenameError> {
    validate_rename_text(new_name)?;

    let Some(target) = workspace_source_dependency_prepare_rename_with_open_docs(
        source, analysis, package, open_docs, position,
    ) else {
        return Ok(None);
    };
    if !supports_workspace_source_dependency_rename(target.kind) {
        return Ok(None);
    }
    if source.get(target.span.start..target.span.end) == Some(new_name) {
        return Ok(None);
    }

    let Some(locations) = workspace_source_references_for_dependency_with_open_docs(
        uri, source, analysis, package, open_docs, position, true,
    ) else {
        return Ok(None);
    };

    let replacement = new_name.to_owned();
    let mut changes = HashMap::<Url, Vec<TextEdit>>::new();
    for location in locations {
        changes
            .entry(location.uri)
            .or_default()
            .push(TextEdit::new(location.range, replacement.clone()));
    }
    for edits in changes.values_mut() {
        edits.sort_by_key(|edit| {
            (
                edit.range.start.line,
                edit.range.start.character,
                edit.range.end.line,
                edit.range.end.character,
            )
        });
    }

    Ok(Some(WorkspaceEdit::new(changes)))
}

fn broken_source_root_symbol_rename_edits_for_import_binding(
    source: &str,
    binding: &BrokenSourceImportBinding,
    new_name: &str,
) -> Vec<TextEdit> {
    let mut edits = vec![TextEdit::new(
        span_to_range(source, binding.imported_span),
        new_name.to_owned(),
    )];

    if binding.local_name == binding.imported_name {
        let (tokens, _) = lex(source);
        edits.extend(
            tokens
                .iter()
                .enumerate()
                .filter(|(_, token)| {
                    token.kind == TokenKind::Ident && token.text == binding.local_name
                })
                .filter(|(index, token)| {
                    token.span != binding.definition_span
                        && broken_source_import_token_matches_reference_context(&tokens, *index)
                })
                .map(|(_, token)| {
                    TextEdit::new(span_to_range(source, token.span), new_name.to_owned())
                }),
        );
    }

    edits.sort_by_key(|edit| {
        (
            edit.range.start.line,
            edit.range.start.character,
            edit.range.end.line,
            edit.range.end.character,
        )
    });
    edits.dedup_by(|left, right| left.range == right.range && left.new_text == right.new_text);
    edits
}

fn broken_source_import_binding_matches_path(
    binding: &BrokenSourceImportBinding,
    import_path: &[String],
) -> bool {
    import_path.last() == Some(&binding.imported_name)
        && binding.import_prefix.len() + 1 == import_path.len()
        && binding
            .import_prefix
            .iter()
            .zip(import_path.iter())
            .all(|(left, right)| left == right)
}

fn broken_source_root_symbol_rename_edits_for_import_path_in_source(
    source: &str,
    import_path: &[String],
    new_name: &str,
) -> Vec<TextEdit> {
    let (tokens, _) = lex(source);
    let bindings = broken_source_import_bindings_in_tokens(&tokens);
    let mut local_name_counts = HashMap::<String, usize>::new();
    for binding in &bindings {
        *local_name_counts
            .entry(binding.local_name.clone())
            .or_insert(0usize) += 1;
    }

    let mut edits = Vec::new();
    for binding in bindings
        .into_iter()
        .filter(|binding| broken_source_import_binding_matches_path(binding, import_path))
    {
        edits.push(TextEdit::new(
            span_to_range(source, binding.imported_span),
            new_name.to_owned(),
        ));

        if binding.local_name != binding.imported_name
            || local_name_counts.get(binding.local_name.as_str()) != Some(&1usize)
        {
            continue;
        }

        edits.extend(
            tokens
                .iter()
                .enumerate()
                .filter(|(_, token)| {
                    token.kind == TokenKind::Ident && token.text == binding.local_name
                })
                .filter(|(index, token)| {
                    token.span != binding.definition_span
                        && broken_source_import_token_matches_reference_context(&tokens, *index)
                })
                .map(|(_, token)| {
                    TextEdit::new(span_to_range(source, token.span), new_name.to_owned())
                }),
        );
    }

    edits.sort_by_key(|edit| {
        (
            edit.range.start.line,
            edit.range.start.character,
            edit.range.end.line,
            edit.range.end.character,
        )
    });
    edits.dedup_by(|left, right| left.range == right.range && left.new_text == right.new_text);
    edits
}

fn workspace_root_import_rename_edit_for_location(
    location: &Location,
    import_path: &[String],
    new_name: &str,
    open_docs: &OpenDocuments,
) -> Option<TextEdit> {
    let location_path = location.uri.to_file_path().ok()?;
    let (candidate_source, candidate_analysis) = if let Some((_, open_source, open_analysis)) =
        open_document_snapshot(open_docs, &location_path)
    {
        (open_source, open_analysis)
    } else {
        let candidate_source = fs::read_to_string(&location_path)
            .ok()?
            .replace("\r\n", "\n");
        let candidate_analysis = analyze_source(&candidate_source).ok()?;
        (candidate_source, candidate_analysis)
    };
    let offset = position_to_offset(&candidate_source, location.range.start)?;
    let occurrence = analyzed_import_binding_at(&candidate_source, &candidate_analysis, offset)?;
    if occurrence.path_segments.as_slice() != import_path
        || span_to_range(&candidate_source, occurrence.occurrence_span) != location.range
    {
        return None;
    }

    let replacement_range = if occurrence.definition_span == occurrence.imported_span {
        location.range
    } else {
        span_to_range(&candidate_source, occurrence.imported_span)
    };
    Some(TextEdit::new(replacement_range, new_name.to_owned()))
}

fn workspace_root_import_rename_edits_for_source(
    source: &str,
    analysis: &Analysis,
    import_path: &[String],
    new_name: &str,
) -> Vec<TextEdit> {
    let (tokens, _) = lex(source);
    let mut edits = tokens
        .iter()
        .filter(|token| token.kind == TokenKind::Ident)
        .filter_map(|token| {
            let occurrence = analyzed_import_binding_at(source, analysis, token.span.start)?;
            if occurrence.path_segments.as_slice() != import_path
                || occurrence.occurrence_span != token.span
            {
                return None;
            }

            let replacement_range = if occurrence.definition_span == occurrence.imported_span {
                span_to_range(source, token.span)
            } else {
                span_to_range(source, occurrence.imported_span)
            };
            Some(TextEdit::new(replacement_range, new_name.to_owned()))
        })
        .collect::<Vec<_>>();
    edits.sort_by_key(|edit| {
        (
            edit.range.start.line,
            edit.range.start.character,
            edit.range.end.line,
            edit.range.end.character,
        )
    });
    edits.dedup_by(|left, right| left.range == right.range && left.new_text == right.new_text);
    if edits.is_empty() {
        return Vec::new();
    }
    edits
}

fn open_or_disk_source_snapshot(open_docs: &OpenDocuments, path: &Path) -> Option<(Url, String)> {
    let canonical_path = canonicalize_or_clone(path);
    if let Some((uri, source)) = open_docs.get(&canonical_path) {
        return Some((uri.clone(), source.clone()));
    }

    let uri = Url::from_file_path(path).ok()?;
    let source = fs::read_to_string(path).ok()?.replace("\r\n", "\n");
    Some((uri, source))
}

fn visible_manifest_paths_for_package(package_manifest_path: &Path) -> Vec<PathBuf> {
    let mut manifest_paths = vec![package_manifest_path.to_path_buf()];
    manifest_paths.extend(source_preferred_manifest_paths_for_package(
        package_manifest_path,
    ));
    manifest_paths.sort_by_key(|manifest_path| {
        canonicalize_or_clone(manifest_path)
            .to_string_lossy()
            .into_owned()
    });
    manifest_paths
        .dedup_by(|left, right| canonicalize_or_clone(left) == canonicalize_or_clone(right));
    manifest_paths
}

fn visible_source_dependency_occurrence_matches_definition_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
    source_definition: &Location,
) -> bool {
    let Some(target) = dependency_definition_target_with_open_docs_at(
        source, analysis, package, open_docs, position,
    ) else {
        return false;
    };
    let Some(location) = workspace_source_location_for_dependency_target_with_open_docs(
        uri, source, analysis, package, open_docs, &target,
    ) else {
        return false;
    };
    same_location_anchor(&location, source_definition)
}

fn extend_workspace_root_import_rename_edits_for_visible_sources(
    package_manifest_path: &Path,
    current_path: Option<&Path>,
    import_path: &[String],
    new_name: &str,
    open_docs: &OpenDocuments,
    changes: &mut HashMap<Url, Vec<TextEdit>>,
) {
    let current_path = current_path.map(canonicalize_or_clone);
    for candidate_manifest_path in visible_manifest_paths_for_package(package_manifest_path) {
        let Some(candidate_package) = package_analysis_for_path(&candidate_manifest_path) else {
            continue;
        };
        let Ok(source_paths) = collect_package_sources(candidate_package.manifest()) else {
            continue;
        };

        for candidate_path in source_paths {
            let candidate_canonical = canonicalize_or_clone(&candidate_path);
            if current_path
                .as_ref()
                .is_some_and(|current_path| current_path == &candidate_canonical)
            {
                continue;
            }

            let Some((candidate_uri, candidate_source)) =
                open_or_disk_source_snapshot(open_docs, &candidate_path)
            else {
                continue;
            };

            let candidate_edits = if let Some((_, _, candidate_analysis)) =
                open_document_snapshot(open_docs, &candidate_path)
            {
                workspace_root_import_rename_edits_for_source(
                    &candidate_source,
                    &candidate_analysis,
                    import_path,
                    new_name,
                )
            } else if let Ok(candidate_analysis) = analyze_source(&candidate_source) {
                workspace_root_import_rename_edits_for_source(
                    &candidate_source,
                    &candidate_analysis,
                    import_path,
                    new_name,
                )
            } else {
                broken_source_root_symbol_rename_edits_for_import_path_in_source(
                    &candidate_source,
                    import_path,
                    new_name,
                )
            };
            if candidate_edits.is_empty() {
                continue;
            }

            changes
                .entry(candidate_uri)
                .or_default()
                .extend(candidate_edits);
        }
    }
}

fn rename_for_workspace_source_root_symbol_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
    new_name: &str,
) -> std::result::Result<Option<WorkspaceEdit>, RenameError> {
    validate_rename_text(new_name)?;

    let Some(offset) = position_to_offset(source, position) else {
        return Ok(None);
    };
    let Some(definition_target) = analysis.definition_at(offset) else {
        return Ok(None);
    };
    let supports_root_definition_rename =
        supports_workspace_source_root_definition_rename(definition_target.kind);
    let supports_root_member_rename =
        supports_workspace_source_root_member_rename(definition_target.kind);
    if (!supports_root_definition_rename && !supports_root_member_rename)
        || !occurrence_matches_definition_target(analysis, offset, &definition_target)
        || definition_target.name == new_name
    {
        return Ok(None);
    }

    let Some(rename) = analysis.rename_at(offset, new_name)? else {
        return Ok(None);
    };

    if supports_root_member_rename {
        let mut changes = HashMap::<Url, Vec<TextEdit>>::new();
        changes.insert(
            uri.clone(),
            rename
                .edits
                .into_iter()
                .map(|edit| TextEdit::new(span_to_range(source, edit.span), edit.replacement))
                .collect(),
        );

        if let Some(locations) = workspace_source_references_for_root_symbol_with_open_docs(
            uri, source, analysis, package, open_docs, position, true,
        ) {
            for location in locations {
                if location.uri == *uri {
                    continue;
                }

                changes
                    .entry(location.uri)
                    .or_default()
                    .push(TextEdit::new(location.range, new_name.to_owned()));
            }
        }

        for edits in changes.values_mut() {
            edits.sort_by_key(|edit| {
                (
                    edit.range.start.line,
                    edit.range.start.character,
                    edit.range.end.line,
                    edit.range.end.character,
                )
            });
            edits.dedup_by(|left, right| {
                left.range == right.range && left.new_text == right.new_text
            });
        }

        return Ok(Some(WorkspaceEdit::new(changes)));
    }

    let mut changes = HashMap::<Url, Vec<TextEdit>>::new();
    changes.insert(
        uri.clone(),
        rename
            .edits
            .into_iter()
            .map(|edit| TextEdit::new(span_to_range(source, edit.span), edit.replacement))
            .collect(),
    );

    let same_package_sources = collect_package_sources(package.manifest())
        .unwrap_or_default()
        .into_iter()
        .map(|path| canonicalize_or_clone(&path))
        .collect::<HashSet<_>>();
    let import_path = package_path_segments(source).map(|segments| {
        let mut path = segments.into_iter().map(str::to_owned).collect::<Vec<_>>();
        path.push(definition_target.name.clone());
        path
    });
    let source_definition =
        Location::new(uri.clone(), span_to_range(source, definition_target.span));

    if let Some(locations) = workspace_source_references_for_root_symbol_with_open_docs(
        uri, source, analysis, package, open_docs, position, true,
    ) {
        for location in locations {
            if same_location_anchor(&location, &source_definition) || location.uri == *uri {
                continue;
            }

            let Some(location_path) = location.uri.to_file_path().ok() else {
                continue;
            };
            let same_package_source =
                same_package_sources.contains(&canonicalize_or_clone(&location_path));

            let Some(import_path) = import_path.as_ref() else {
                if same_package_source {
                    changes
                        .entry(location.uri)
                        .or_default()
                        .push(TextEdit::new(location.range, new_name.to_owned()));
                }
                continue;
            };

            if let Some(edit) = workspace_root_import_rename_edit_for_location(
                &location,
                import_path,
                new_name,
                open_docs,
            ) {
                changes.entry(location.uri).or_default().push(edit);
                continue;
            }

            let Some((candidate_uri, candidate_source)) =
                open_or_disk_source_snapshot(open_docs, &location_path)
            else {
                if same_package_source {
                    changes
                        .entry(location.uri)
                        .or_default()
                        .push(TextEdit::new(location.range, new_name.to_owned()));
                }
                continue;
            };
            if analyze_source(&candidate_source).is_err() {
                let broken_edits = broken_source_root_symbol_rename_edits_for_import_path_in_source(
                    &candidate_source,
                    import_path,
                    new_name,
                );
                if !broken_edits.is_empty() {
                    changes
                        .entry(candidate_uri)
                        .or_default()
                        .extend(broken_edits);
                    continue;
                }
            }

            if same_package_source {
                changes
                    .entry(location.uri)
                    .or_default()
                    .push(TextEdit::new(location.range, new_name.to_owned()));
            }
        }
    }

    for edits in changes.values_mut() {
        edits.sort_by_key(|edit| {
            (
                edit.range.start.line,
                edit.range.start.character,
                edit.range.end.line,
                edit.range.end.character,
            )
        });
        edits.dedup_by(|left, right| left.range == right.range && left.new_text == right.new_text);
    }

    Ok(Some(WorkspaceEdit::new(changes)))
}

fn rename_for_workspace_source_root_symbol_from_import_in_broken_source_with_open_docs(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
    new_name: &str,
) -> std::result::Result<Option<WorkspaceEdit>, RenameError> {
    validate_rename_text(new_name)?;

    let Some(binding) = broken_source_import_binding_at(source, position) else {
        return Ok(None);
    };
    let Some(source_location) = workspace_source_location_for_import_binding(
        uri,
        source,
        None,
        package,
        &binding.import_prefix,
        binding.imported_name.as_str(),
    ) else {
        return Ok(None);
    };
    let Some(source_path) = source_location.uri.to_file_path().ok() else {
        return Ok(None);
    };
    let Some(source_package) = package_analysis_for_path(&source_path) else {
        return Ok(None);
    };
    let (source_uri, source_source, source_analysis) =
        if let Some((open_uri, open_source, open_analysis)) =
            open_document_snapshot(open_docs, &source_path)
        {
            (open_uri, open_source, open_analysis)
        } else {
            let Ok(source_uri) = Url::from_file_path(&source_path) else {
                return Ok(None);
            };
            let Ok(source_source) = fs::read_to_string(&source_path) else {
                return Ok(None);
            };
            let source_source = source_source.replace("\r\n", "\n");
            let Ok(source_analysis) = analyze_source(&source_source) else {
                return Ok(None);
            };
            (source_uri, source_source, source_analysis)
        };

    let Some(mut edit) = rename_for_workspace_source_root_symbol_with_open_docs(
        &source_uri,
        &source_source,
        &source_analysis,
        &source_package,
        open_docs,
        source_location.range.start,
        new_name,
    )?
    else {
        return Ok(None);
    };

    let current_source_edits =
        broken_source_root_symbol_rename_edits_for_import_binding(source, &binding, new_name);
    let changes = edit.changes.get_or_insert_with(HashMap::new);
    changes
        .entry(uri.clone())
        .or_default()
        .extend(current_source_edits);
    let mut import_path = binding.import_prefix.clone();
    import_path.push(binding.imported_name.clone());
    let current_path = uri.to_file_path().ok();
    extend_workspace_root_import_rename_edits_for_visible_sources(
        package.manifest().manifest_path.as_path(),
        current_path.as_deref(),
        &import_path,
        new_name,
        open_docs,
        changes,
    );
    for edits in changes.values_mut() {
        edits.sort_by_key(|edit| {
            (
                edit.range.start.line,
                edit.range.start.character,
                edit.range.end.line,
                edit.range.end.character,
            )
        });
        edits.dedup_by(|left, right| left.range == right.range && left.new_text == right.new_text);
    }

    Ok(Some(edit))
}

fn rename_for_workspace_source_root_symbol_from_import_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
    new_name: &str,
) -> std::result::Result<Option<WorkspaceEdit>, RenameError> {
    validate_rename_text(new_name)?;

    let Some(offset) = position_to_offset(source, position) else {
        return Ok(None);
    };
    let Some(occurrence) = analyzed_import_binding_at(source, analysis, offset) else {
        return Ok(None);
    };
    let Some((imported_name, import_prefix)) = occurrence.path_segments.split_last() else {
        return Ok(None);
    };
    let Some(source_location) = workspace_source_location_for_import_binding(
        uri,
        source,
        Some(analysis),
        package,
        import_prefix,
        imported_name,
    ) else {
        return Ok(None);
    };
    let Some(source_path) = source_location.uri.to_file_path().ok() else {
        return Ok(None);
    };
    let Some(source_package) = package_analysis_for_path(&source_path) else {
        return Ok(None);
    };
    let (source_uri, source_source, source_analysis) =
        if let Some((open_uri, open_source, open_analysis)) =
            open_document_snapshot(open_docs, &source_path)
        {
            (open_uri, open_source, open_analysis)
        } else {
            let Ok(source_uri) = Url::from_file_path(&source_path) else {
                return Ok(None);
            };
            let Ok(source_source) = fs::read_to_string(&source_path) else {
                return Ok(None);
            };
            let source_source = source_source.replace("\r\n", "\n");
            let Ok(source_analysis) = analyze_source(&source_source) else {
                return Ok(None);
            };
            (source_uri, source_source, source_analysis)
        };

    rename_for_workspace_source_root_symbol_with_open_docs(
        &source_uri,
        &source_source,
        &source_analysis,
        &source_package,
        open_docs,
        source_location.range.start,
        new_name,
    )
}

fn local_source_dependency_target_with_analysis(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
) -> Option<DependencyDefinitionTarget> {
    let offset = position_to_offset(source, position)?;
    let definition_span = analysis
        .references_at(offset)
        .and_then(|references| {
            references
                .into_iter()
                .find(|reference| reference.is_definition)
                .map(|reference| reference.span)
        })
        .or_else(|| {
            analysis
                .definition_at(offset)
                .map(|definition| definition.span)
        })?;
    let definition = analysis.definition_at(offset)?;
    if !supports_workspace_source_dependency_rename(definition.kind) {
        return None;
    }

    let source_definition = Location::new(uri.clone(), span_to_range(source, definition_span));
    let mut matches = Vec::new();

    for candidate_manifest_path in
        source_preferred_manifest_paths_for_package(package.manifest().manifest_path.as_path())
    {
        let Some(candidate_package) = package_analysis_for_path(&candidate_manifest_path) else {
            continue;
        };
        let Ok(source_paths) = collect_package_sources(candidate_package.manifest()) else {
            continue;
        };

        for source_path in source_paths {
            let canonical_source_path = canonicalize_or_clone(&source_path);
            let (candidate_uri, candidate_source, owned_analysis) =
                if let Some((open_uri, open_source)) = open_docs.get(&canonical_source_path) {
                    (
                        open_uri.clone(),
                        open_source.clone(),
                        analyze_source(open_source).ok(),
                    )
                } else {
                    let Ok(candidate_uri) = Url::from_file_path(&source_path) else {
                        continue;
                    };
                    let Ok(candidate_source) = fs::read_to_string(&source_path) else {
                        continue;
                    };
                    let candidate_source = candidate_source.replace("\r\n", "\n");
                    let candidate_analysis = candidate_package
                        .modules()
                        .iter()
                        .find(|module| {
                            canonicalize_or_clone(module.path()) == canonical_source_path
                        })
                        .map(|module| module.analysis().clone())
                        .or_else(|| analyze_source(&candidate_source).ok());
                    (candidate_uri, candidate_source, candidate_analysis)
                };
            let candidate_analysis = owned_analysis.as_ref();

            for token in lex(&candidate_source)
                .0
                .iter()
                .filter(|token| token.kind == TokenKind::Ident && token.text == definition.name)
            {
                let position = span_to_range(&candidate_source, token.span).start;
                let Some(occurrence_span) = dependency_occurrence_span_with_open_docs_at(
                    &candidate_source,
                    &candidate_package,
                    open_docs,
                    position,
                ) else {
                    continue;
                };
                if occurrence_span != token.span {
                    continue;
                }
                let Some(target) = dependency_definition_target_with_open_docs_at(
                    &candidate_source,
                    candidate_analysis,
                    &candidate_package,
                    open_docs,
                    position,
                ) else {
                    continue;
                };
                if target.kind != definition.kind || target.name != definition.name {
                    continue;
                }
                let Some(mapped_source) =
                    workspace_source_location_for_dependency_target_with_open_docs(
                        &candidate_uri,
                        &candidate_source,
                        candidate_analysis,
                        &candidate_package,
                        open_docs,
                        &target,
                    )
                else {
                    continue;
                };
                if same_location_anchor(&mapped_source, &source_definition)
                    && !matches
                        .iter()
                        .any(|existing| same_dependency_definition_target(existing, &target))
                {
                    matches.push(target);
                }
            }
        }
    }

    (matches.len() == 1).then(|| matches.remove(0))
}

fn rename_for_local_source_dependency_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
    new_name: &str,
) -> std::result::Result<Option<WorkspaceEdit>, RenameError> {
    validate_rename_text(new_name)?;

    let Some(offset) = position_to_offset(source, position) else {
        return Ok(None);
    };
    let Some(local_target) = local_source_dependency_target_with_analysis(
        uri, source, analysis, package, open_docs, position,
    ) else {
        return Ok(None);
    };
    let Some(rename) = analysis.rename_at(offset, new_name)? else {
        return Ok(None);
    };
    let external_locations = workspace_dependency_reference_locations_with_open_docs(
        package,
        uri.to_file_path().ok().as_deref(),
        open_docs,
        &local_target,
        false,
    );
    if external_locations.is_empty() {
        return Ok(None);
    }

    let mut changes = HashMap::<Url, Vec<TextEdit>>::new();
    changes.insert(
        uri.clone(),
        rename
            .edits
            .into_iter()
            .map(|edit| TextEdit::new(span_to_range(source, edit.span), edit.replacement))
            .collect(),
    );

    for location in external_locations {
        changes
            .entry(location.uri)
            .or_default()
            .push(TextEdit::new(location.range, new_name.to_owned()));
    }
    for edits in changes.values_mut() {
        edits.sort_by_key(|edit| {
            (
                edit.range.start.line,
                edit.range.start.character,
                edit.range.end.line,
                edit.range.end.character,
            )
        });
    }

    Ok(Some(WorkspaceEdit::new(changes)))
}

fn normalize_reference_locations_with_definition(
    locations: &mut Vec<Location>,
    source_definition: &Location,
) {
    if let Some(existing_index) = locations
        .iter()
        .position(|location| same_location_anchor(location, source_definition))
    {
        locations.swap(0, existing_index);
        locations[0] = source_definition.clone();
    } else if !locations.is_empty() {
        locations.insert(0, source_definition.clone());
    } else {
        locations.push(source_definition.clone());
    }
}

fn same_file_references_for_source_location_with_open_docs(
    source_location: &Location,
    open_docs: &OpenDocuments,
) -> Option<Vec<Location>> {
    let source_path = source_location.uri.to_file_path().ok()?;
    let (source, analysis) = if let Some((_, open_source, open_analysis)) =
        open_document_snapshot(open_docs, &source_path)
    {
        (open_source, open_analysis)
    } else {
        let source = fs::read_to_string(source_path).ok()?.replace("\r\n", "\n");
        let analysis = analyze_source(&source).ok()?;
        (source, analysis)
    };
    let definition_target =
        definition_target_for_source_location(&analysis, &source, source_location.range)?;
    let (tokens, _) = lex(&source);
    let mut locations = tokens
        .iter()
        .filter(|token| token.kind == TokenKind::Ident && token.text == definition_target.name)
        .filter(|token| {
            occurrence_matches_definition_target(&analysis, token.span.start, &definition_target)
        })
        .map(|token| {
            Location::new(
                source_location.uri.clone(),
                span_to_range(&source, token.span),
            )
        })
        .collect::<Vec<_>>();
    if locations.is_empty() {
        return None;
    }
    locations.sort_by_key(|location| {
        (
            location.range.start.line,
            location.range.start.character,
            location.range.end.line,
            location.range.end.character,
        )
    });
    locations.dedup_by(|left, right| same_location_anchor(left, right));
    Some(locations)
}

fn workspace_visible_source_references_for_definition_with_open_docs(
    package: &ql_analysis::PackageAnalysis,
    current_path: Option<&Path>,
    open_docs: &OpenDocuments,
    target: &ql_analysis::DefinitionTarget,
    source_definition: &Location,
    include_declaration: bool,
) -> Vec<Location> {
    let mut locations = Vec::new();
    for candidate_manifest_path in
        visible_manifest_paths_for_package(package.manifest().manifest_path.as_path())
    {
        let Some(candidate_package) = package_analysis_for_path(&candidate_manifest_path) else {
            continue;
        };
        let Ok(source_paths) = collect_package_sources(candidate_package.manifest()) else {
            continue;
        };

        for source_path in source_paths {
            if current_path.is_some_and(|path| {
                canonicalize_or_clone(path) == canonicalize_or_clone(&source_path)
            }) {
                continue;
            }

            let Some((uri, source)) = open_or_disk_source_snapshot(open_docs, &source_path) else {
                continue;
            };
            let owned_analysis = open_document_snapshot(open_docs, &source_path)
                .map(|(_, _, analysis)| analysis)
                .or_else(|| {
                    candidate_package
                        .modules()
                        .iter()
                        .find(|module| {
                            canonicalize_or_clone(module.path())
                                == canonicalize_or_clone(&source_path)
                        })
                        .map(|module| module.analysis().clone())
                })
                .or_else(|| analyze_source(&source).ok());
            let analysis = owned_analysis.as_ref();

            let mut module_locations = lex(&source)
                .0
                .iter()
                .filter(|token| token.kind == TokenKind::Ident && token.text == target.name)
                .filter_map(|token| {
                    let position = span_to_range(&source, token.span).start;
                    let matches_target = analysis.is_some_and(|analysis| {
                        occurrence_matches_definition_target(analysis, token.span.start, target)
                    })
                        || visible_source_dependency_occurrence_matches_definition_with_open_docs(
                            &uri,
                            &source,
                            analysis,
                            &candidate_package,
                            open_docs,
                            position,
                            source_definition,
                        );
                    if !matches_target {
                        return None;
                    }
                    if !include_declaration
                        && analysis.and_then(|analysis| {
                            analysis
                                .references_at(token.span.start)
                                .and_then(|references| {
                                    references
                                        .into_iter()
                                        .find(|reference| reference.span == token.span)
                                        .map(|reference| reference.is_definition)
                                })
                        }) == Some(true)
                    {
                        return None;
                    }
                    Some(Location::new(
                        uri.clone(),
                        span_to_range(&source, token.span),
                    ))
                })
                .collect::<Vec<_>>();
            module_locations.sort_by_key(|location| {
                (
                    location.range.start.line,
                    location.range.start.character,
                    location.range.end.line,
                    location.range.end.character,
                )
            });
            module_locations.dedup_by(|left, right| same_location_anchor(left, right));
            locations.extend(module_locations);
        }
    }

    locations
}

fn merge_unique_reference_locations(locations: &mut Vec<Location>, additional: Vec<Location>) {
    for location in additional {
        if !locations
            .iter()
            .any(|existing| same_location_anchor(existing, &location))
        {
            locations.push(location);
        }
    }
}

fn same_location_anchor(lhs: &Location, rhs: &Location) -> bool {
    lhs.uri == rhs.uri && ranges_overlap(lhs.range, rhs.range)
}

fn same_definition_target(
    lhs: &ql_analysis::DefinitionTarget,
    rhs: &ql_analysis::DefinitionTarget,
) -> bool {
    lhs.kind == rhs.kind && lhs.name == rhs.name && lhs.span == rhs.span
}

fn definition_target_for_source_location(
    analysis: &Analysis,
    source: &str,
    range: tower_lsp::lsp_types::Range,
) -> Option<ql_analysis::DefinitionTarget> {
    let start_offset = position_to_offset(source, range.start)?;
    let end_offset = position_to_offset(source, range.end)?;
    let (tokens, _) = lex(source);
    for token in tokens.iter().filter(|token| {
        token.kind == TokenKind::Ident
            && token.span.start >= start_offset
            && token.span.end <= end_offset
    }) {
        if let Some(target) = analysis.definition_at(token.span.start) {
            return Some(target);
        }
        if let Some(hover) = analysis.hover_at(token.span.start)
            && let Some(definition_span) = hover.definition_span
        {
            return Some(ql_analysis::DefinitionTarget {
                kind: hover.kind,
                name: hover.name,
                span: definition_span,
            });
        }
    }
    None
}

fn occurrence_matches_definition_target(
    analysis: &Analysis,
    offset: usize,
    target: &ql_analysis::DefinitionTarget,
) -> bool {
    if let Some(definition) = analysis.definition_at(offset)
        && same_definition_target(&definition, target)
    {
        return true;
    }
    analysis.hover_at(offset).is_some_and(|hover| {
        hover.kind == target.kind
            && hover.name == target.name
            && hover.definition_span == Some(target.span)
    })
}

fn dependency_reference_is_definition_at(
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<bool> {
    let offset = position_to_offset(source, position)?;
    let reference_at = |references: Vec<ql_analysis::ReferenceTarget>| {
        references
            .into_iter()
            .find(|reference| reference.span.contains(offset))
            .map(|reference| reference.is_definition)
    };

    if let Some(analysis) = analysis {
        if let Some(is_definition) = package
            .dependency_method_references_at(analysis, offset)
            .and_then(reference_at)
        {
            return Some(is_definition);
        }
        if let Some(is_definition) = package
            .dependency_struct_field_references_at(analysis, offset)
            .and_then(reference_at)
        {
            return Some(is_definition);
        }
        if let Some(is_definition) = package
            .dependency_variant_references_at(analysis, source, offset)
            .and_then(reference_at)
        {
            return Some(is_definition);
        }
    }

    package
        .dependency_value_references_in_source_at(source, offset)
        .and_then(reference_at)
        .or_else(|| {
            package
                .dependency_references_in_source_at(source, offset)
                .and_then(reference_at)
        })
}

fn dependency_reference_locations_in_source_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    target: &DependencyDefinitionTarget,
    include_declaration: bool,
) -> Vec<Location> {
    let mut locations = lex(source)
        .0
        .iter()
        .filter(|token| token.kind == TokenKind::Ident && token.text == target.name)
        .filter_map(|token| {
            let position = span_to_range(source, token.span).start;
            let occurrence_span =
                dependency_occurrence_span_with_open_docs_at(source, package, open_docs, position)?;
            if occurrence_span != token.span {
                return None;
            }
            if !include_declaration
                && dependency_reference_is_definition_at(source, analysis, package, position)
                    == Some(true)
            {
                return None;
            }
            let occurrence_target = dependency_definition_target_with_open_docs_at(
                source, analysis, package, open_docs, position,
            )?;
            same_dependency_definition_target(&occurrence_target, target)
                .then(|| Location::new(uri.clone(), span_to_range(source, occurrence_span)))
        })
        .collect::<Vec<_>>();
    locations.sort_by_key(|location| {
        (
            location.range.start.line,
            location.range.start.character,
            location.range.end.line,
            location.range.end.character,
        )
    });
    locations.dedup_by(|left, right| same_location_anchor(left, right));
    locations
}

fn ranges_overlap(lhs: tower_lsp::lsp_types::Range, rhs: tower_lsp::lsp_types::Range) -> bool {
    position_leq(lhs.start, rhs.end) && position_leq(rhs.start, lhs.end)
}

fn position_leq(lhs: tower_lsp::lsp_types::Position, rhs: tower_lsp::lsp_types::Position) -> bool {
    (lhs.line, lhs.character) <= (rhs.line, rhs.character)
}

fn dependency_references_for_position(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    if let Some(analysis) = analysis {
        return references_for_package_analysis(
            uri,
            source,
            analysis,
            package,
            position,
            include_declaration,
        );
    }

    references_for_dependency_imports(uri, source, package, position, include_declaration)
        .or_else(|| {
            references_for_dependency_values(uri, source, package, position, include_declaration)
        })
        .or_else(|| {
            references_for_dependency_methods(uri, source, package, position, include_declaration)
        })
        .or_else(|| {
            references_for_dependency_variants(uri, source, package, position, include_declaration)
        })
        .or_else(|| {
            references_for_dependency_struct_fields(
                uri,
                source,
                package,
                position,
                include_declaration,
            )
        })
}

fn workspace_source_references_for_root_symbol_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let offset = position_to_offset(source, position)?;
    let definition_target = analysis.definition_at(offset)?;
    if !supports_workspace_source_root_definition_references(definition_target.kind) {
        return None;
    }
    if !occurrence_matches_definition_target(analysis, offset, &definition_target) {
        return None;
    }
    if let Some(locations) = workspace_source_trait_method_references_with_open_docs(
        uri,
        source,
        analysis,
        package,
        open_docs,
        position,
        include_declaration,
    ) {
        return Some(locations);
    }

    let source_definition =
        Location::new(uri.clone(), span_to_range(source, definition_target.span));
    let mut locations = Vec::new();

    if let Some(mut same_file_locations) =
        same_file_references_for_source_location_with_open_docs(&source_definition, open_docs)
    {
        if !include_declaration {
            same_file_locations
                .retain(|location| !same_location_anchor(location, &source_definition));
        }
        merge_unique_reference_locations(&mut locations, same_file_locations);
    }

    let current_path = uri.to_file_path().ok();
    merge_unique_reference_locations(
        &mut locations,
        workspace_visible_source_references_for_definition_with_open_docs(
            package,
            current_path.as_deref(),
            open_docs,
            &definition_target,
            &source_definition,
            include_declaration,
        ),
    );

    if supports_workspace_source_root_definition_rename(definition_target.kind) {
        let package_segments = package_path_segments(source)?;
        let mut import_path = package_segments
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        import_path.push(definition_target.name.clone());
        merge_unique_reference_locations(
            &mut locations,
            workspace_import_reference_locations_with_open_docs(
                package,
                current_path.as_deref(),
                open_docs,
                &import_path,
                include_declaration,
            ),
        );
        merge_unique_reference_locations(
            &mut locations,
            workspace_broken_import_reference_locations_for_visible_sources(
                package.manifest().manifest_path.as_path(),
                current_path.as_deref(),
                &import_path,
                include_declaration,
                open_docs,
            ),
        );
    }

    (!locations.is_empty()).then_some(locations)
}

fn workspace_source_references_for_dependency(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let open_docs = OpenDocuments::new();
    workspace_source_references_for_dependency_with_open_docs(
        uri,
        source,
        analysis,
        package,
        &open_docs,
        position,
        include_declaration,
    )
}

fn workspace_source_references_for_dependency_with_open_docs(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let target = dependency_definition_target_with_open_docs_at(
        source, analysis, package, open_docs, position,
    )?;
    let source_definition = workspace_source_location_for_dependency_target_with_open_docs(
        uri, source, analysis, package, open_docs, &target,
    )?;
    let interface_path = canonicalize_or_clone(&target.path);
    let mut locations = dependency_references_for_position(
        uri,
        source,
        analysis,
        package,
        position,
        include_declaration,
    )
    .unwrap_or_else(|| {
        dependency_reference_locations_in_source_with_open_docs(
            uri,
            source,
            analysis,
            package,
            open_docs,
            &target,
            include_declaration,
        )
    });
    if locations.is_empty() {
        return None;
    }
    if include_declaration {
        normalize_reference_locations_with_definition(&mut locations, &source_definition);
    }
    if let Some(mut source_locations) =
        same_file_references_for_source_location_with_open_docs(&source_definition, open_docs)
    {
        if !include_declaration {
            source_locations.retain(|location| !same_location_anchor(location, &source_definition));
        }
        merge_unique_reference_locations(&mut locations, source_locations);
    }
    let current_path = uri.to_file_path().ok();
    merge_unique_reference_locations(
        &mut locations,
        workspace_dependency_reference_locations_with_open_docs(
            package,
            current_path.as_deref(),
            open_docs,
            &target,
            include_declaration,
        ),
    );
    if source_definition
        .uri
        .to_file_path()
        .ok()
        .is_some_and(|path| canonicalize_or_clone(&path) != interface_path)
    {
        locations.retain(|location| {
            location
                .uri
                .to_file_path()
                .ok()
                .is_none_or(|path| canonicalize_or_clone(&path) != interface_path)
        });
    }

    Some(locations)
}

fn workspace_source_references_for_dependency_in_broken_source(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let open_docs = OpenDocuments::new();
    workspace_source_references_for_dependency_in_broken_source_with_open_docs(
        uri,
        source,
        package,
        &open_docs,
        position,
        include_declaration,
    )
}

fn workspace_source_references_for_dependency_in_broken_source_with_open_docs(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    open_docs: &OpenDocuments,
    position: tower_lsp::lsp_types::Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    workspace_source_references_for_dependency_with_open_docs(
        uri,
        source,
        None,
        package,
        open_docs,
        position,
        include_declaration,
    )
}

fn document_highlights_from_locations(
    uri: &Url,
    locations: Vec<Location>,
) -> Option<Vec<DocumentHighlight>> {
    let highlights = locations
        .into_iter()
        .filter(|location| location.uri == *uri)
        .map(|location| DocumentHighlight {
            range: location.range,
            kind: None,
        })
        .collect::<Vec<_>>();
    (!highlights.is_empty()).then_some(highlights)
}

fn document_highlights_for_analysis_at(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<Vec<DocumentHighlight>> {
    let locations = references_for_analysis(uri, source, analysis, position, true)?;
    document_highlights_from_locations(uri, locations)
}

fn document_highlights_for_package_analysis_at(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<Vec<DocumentHighlight>> {
    let locations =
        references_for_package_analysis(uri, source, analysis, package, position, true)?;
    document_highlights_from_locations(uri, locations)
}

fn fallback_document_highlights_for_package_at(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<Vec<DocumentHighlight>> {
    let open_docs = OpenDocuments::new();
    fallback_document_highlights_for_package_at_with_open_docs(
        uri, source, package, position, &open_docs,
    )
}

fn fallback_document_highlights_for_package_at_with_open_docs(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
    open_docs: &OpenDocuments,
) -> Option<Vec<DocumentHighlight>> {
    if let Some(highlights) = workspace_import_document_highlights_in_broken_source_with_open_docs(
        uri, source, package, open_docs, position,
    ) {
        return Some(highlights);
    }
    if let Some(highlights) =
        workspace_dependency_document_highlights_in_broken_source_with_open_docs(
            uri, source, package, position, open_docs,
        )
    {
        return Some(highlights);
    }
    if let Some(locations) = references_for_dependency_imports(uri, source, package, position, true)
    {
        return document_highlights_from_locations(uri, locations);
    }
    if let Some(locations) = references_for_dependency_values(uri, source, package, position, true)
    {
        return document_highlights_from_locations(uri, locations);
    }
    if let Some(locations) = references_for_dependency_methods(uri, source, package, position, true)
    {
        return document_highlights_from_locations(uri, locations);
    }
    if let Some(locations) =
        references_for_dependency_variants(uri, source, package, position, true)
    {
        return document_highlights_from_locations(uri, locations);
    }
    let locations = references_for_dependency_struct_fields(uri, source, package, position, true)?;
    document_highlights_from_locations(uri, locations)
}

fn completion_options() -> CompletionOptions {
    CompletionOptions {
        trigger_characters: Some(vec![".".to_owned()]),
        ..CompletionOptions::default()
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        *self.workspace_roots.write().await = configure_workspace_roots(&params);
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
                declaration_provider: Some(DeclarationCapability::Simple(true)),
                type_definition_provider: Some(TypeDefinitionProviderCapability::Simple(true)),
                implementation_provider: Some(ImplementationProviderCapability::Simple(true)),
                references_provider: Some(OneOf::Left(true)),
                document_highlight_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                completion_provider: Some(completion_options()),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
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
            let open_docs = self.open_file_documents().await;
            let analysis = analyze_source(&source).ok();
            if let Some(analysis) = analysis.as_ref() {
                if let Some(hover) = workspace_source_hover_for_import_with_open_docs(
                    &uri, &source, analysis, &package, &open_docs, position,
                ) {
                    return Ok(Some(hover));
                }
            } else if let Some(hover) =
                workspace_source_hover_for_import_in_broken_source_with_open_docs(
                    &uri, &source, &package, &open_docs, position,
                )
            {
                return Ok(Some(hover));
            }
            if let Some(hover) = workspace_source_hover_for_dependency_with_open_docs(
                &uri,
                &source,
                analysis.as_ref(),
                &package,
                &open_docs,
                position,
            ) {
                return Ok(Some(hover));
            }
            if let Some(hover) = hover_for_dependency_imports(&source, &package, position) {
                return Ok(Some(hover));
            }
            if let Some(hover) = hover_for_dependency_methods(&source, &package, position) {
                return Ok(Some(hover));
            }
            if let Some(hover) = hover_for_dependency_struct_fields(&source, &package, position) {
                return Ok(Some(hover));
            }
            if let Some(hover) = hover_for_dependency_variants(&source, &package, position) {
                return Ok(Some(hover));
            }
            let Some(analysis) = analysis else {
                return Ok(hover_for_dependency_values(&source, &package, position));
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
            let open_docs = self.open_file_documents().await;
            let analysis = analyze_source(&source).ok();
            if let Some(analysis) = analysis.as_ref()
                && let Some(definition) = workspace_source_definition_for_import_with_open_docs(
                    &uri, &source, analysis, &package, &open_docs, position,
                )
            {
                return Ok(Some(definition));
            }
            if analysis.is_none()
                && let Some(definition) =
                    workspace_source_definition_for_import_in_broken_source_with_open_docs(
                        &uri, &source, &package, &open_docs, position,
                    )
            {
                return Ok(Some(definition));
            }
            if let Some(definition) = workspace_source_definition_for_dependency_with_open_docs(
                &uri,
                &source,
                analysis.as_ref(),
                &package,
                &open_docs,
                position,
            ) {
                return Ok(Some(definition));
            }
            if let Some(analysis) = analysis {
                return Ok(definition_for_package_analysis(
                    &uri, &source, &analysis, &package, position,
                ));
            }
            return Ok(
                definition_for_dependency_imports(&source, &package, position)
                    .or_else(|| definition_for_dependency_methods(&source, &package, position))
                    .or_else(|| {
                        definition_for_dependency_struct_fields(&source, &package, position)
                    })
                    .or_else(|| definition_for_dependency_variants(&source, &package, position))
                    .or_else(|| definition_for_dependency_values(&source, &package, position)),
            );
        }

        let Ok(analysis) = analyze_source(&source) else {
            return Ok(None);
        };
        Ok(crate::bridge::definition_for_analysis(
            &uri, &source, &analysis, position,
        ))
    }

    async fn goto_declaration(
        &self,
        params: GotoDeclarationParams,
    ) -> Result<Option<GotoDeclarationResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let Some(source) = self.documents.get(&uri).await else {
            return Ok(None);
        };

        if let Some(package) = self.package_analysis_for_uri(&uri) {
            let open_docs = self.open_file_documents().await;
            let analysis = analyze_source(&source).ok();
            if let Some(analysis) = analysis.as_ref()
                && let Some(GotoDefinitionResponse::Scalar(location)) =
                    workspace_source_definition_for_import_with_open_docs(
                        &uri, &source, analysis, &package, &open_docs, position,
                    )
            {
                return Ok(Some(GotoDeclarationResponse::Scalar(location)));
            }
            if analysis.is_none()
                && let Some(GotoDefinitionResponse::Scalar(location)) =
                    workspace_source_definition_for_import_in_broken_source_with_open_docs(
                        &uri, &source, &package, &open_docs, position,
                    )
            {
                return Ok(Some(GotoDeclarationResponse::Scalar(location)));
            }
            if let Some(GotoDefinitionResponse::Scalar(location)) =
                workspace_source_definition_for_dependency_with_open_docs(
                    &uri,
                    &source,
                    analysis.as_ref(),
                    &package,
                    &open_docs,
                    position,
                )
            {
                return Ok(Some(GotoDeclarationResponse::Scalar(location)));
            }
            if let Some(analysis) = analysis {
                return Ok(declaration_for_package_analysis(
                    &uri, &source, &analysis, &package, position,
                ));
            }
            return Ok(
                declaration_for_dependency_imports(&source, &package, position)
                    .or_else(|| declaration_for_dependency_methods(&source, &package, position))
                    .or_else(|| {
                        declaration_for_dependency_struct_fields(&source, &package, position)
                    })
                    .or_else(|| declaration_for_dependency_variants(&source, &package, position))
                    .or_else(|| declaration_for_dependency_values(&source, &package, position)),
            );
        }

        let Ok(analysis) = analyze_source(&source) else {
            return Ok(None);
        };
        Ok(crate::bridge::declaration_for_analysis(
            &uri, &source, &analysis, position,
        ))
    }

    async fn goto_type_definition(
        &self,
        params: GotoTypeDefinitionParams,
    ) -> Result<Option<GotoTypeDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let Some(source) = self.documents.get(&uri).await else {
            return Ok(None);
        };

        if let Some(package) = self.package_analysis_for_uri(&uri) {
            let open_docs = self.open_file_documents().await;
            let analysis = analyze_source(&source).ok();
            if let Some(analysis) = analysis.as_ref() {
                if let Some(definition) = workspace_source_type_definition_for_import_with_open_docs(
                    &uri, &source, analysis, &package, &open_docs, position,
                ) {
                    return Ok(Some(definition));
                }
            } else if let Some(definition) =
                workspace_source_type_definition_for_import_in_broken_source_with_open_docs(
                    &uri, &source, &package, &open_docs, position,
                )
            {
                return Ok(Some(definition));
            }
            if let Some(definition) = workspace_source_type_definition_for_dependency_with_open_docs(
                &uri,
                &source,
                analysis.as_ref(),
                &package,
                &open_docs,
                position,
            ) {
                return Ok(Some(definition));
            }
            if let Some(analysis) = analysis {
                return Ok(type_definition_for_package_analysis(
                    &uri, &source, &analysis, &package, position,
                ));
            }
            return Ok(
                type_definition_for_dependency_imports(&source, &package, position).or_else(|| {
                    type_definition_for_dependency_values(&source, &package, position).or_else(
                        || {
                            type_definition_for_dependency_variants(&source, &package, position)
                                .or_else(|| {
                                    type_definition_for_dependency_struct_field_types(
                                        &source, &package, position,
                                    )
                                    .or_else(|| {
                                        type_definition_for_dependency_method_types(
                                            &source, &package, position,
                                        )
                                    })
                                })
                        },
                    )
                }),
            );
        }

        let Ok(analysis) = analyze_source(&source) else {
            return Ok(None);
        };
        Ok(type_definition_for_analysis(
            &uri, &source, &analysis, position,
        ))
    }

    async fn goto_implementation(
        &self,
        params: GotoImplementationParams,
    ) -> Result<Option<GotoImplementationResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let Some(source) = self.documents.get(&uri).await else {
            return Ok(None);
        };
        if let Some(package) = self.package_analysis_for_uri(&uri) {
            let open_docs = self.open_file_documents().await;
            let analysis = analyze_source(&source).ok();
            if let Some(analysis) = analysis.as_ref()
                && let Some(implementation) = workspace_source_root_implementation_with_open_docs(
                    &uri, &source, analysis, &package, &open_docs, position,
                )
            {
                return Ok(Some(implementation));
            }
            if analysis.is_none()
                && let Some(implementation) =
                    workspace_source_root_implementation_in_broken_source_with_open_docs(
                        &uri, &source, &package, &open_docs, position,
                    )
            {
                return Ok(Some(implementation));
            }
            if let Some(analysis) = analysis.as_ref()
                && let Some(implementation) =
                    workspace_source_trait_method_implementation_with_open_docs(
                        &uri, &source, analysis, &package, &open_docs, position,
                    )
            {
                return Ok(Some(implementation));
            }
            if analysis.is_none()
                && let Some(implementation) =
                    workspace_source_trait_method_implementation_in_broken_source_with_open_docs(
                        &uri, &source, &package, &open_docs, position,
                    )
            {
                return Ok(Some(implementation));
            }
            if let Some(analysis) = analysis.as_ref()
                && let Some(implementation) =
                    workspace_source_implementation_for_dependency_with_open_docs(
                        &source,
                        Some(analysis),
                        &package,
                        &open_docs,
                        position,
                    )
            {
                return Ok(Some(implementation));
            }
            if analysis.is_none()
                && let Some(implementation) =
                    workspace_source_implementation_for_dependency_with_open_docs(
                        &source,
                        analysis.as_ref(),
                        &package,
                        &open_docs,
                        position,
                    )
            {
                return Ok(Some(implementation));
            }
            if let Some(analysis) = analysis.as_ref()
                && let Some(implementation) =
                    workspace_source_method_implementation_for_local_source_with_open_docs(
                        &uri, &source, analysis, &package, &open_docs, position,
                    )
            {
                return Ok(Some(implementation));
            }
            if let Some(implementation) =
                workspace_source_method_implementation_for_dependency_with_open_docs(
                    &uri,
                    &source,
                    analysis.as_ref(),
                    &package,
                    &open_docs,
                    position,
                )
            {
                return Ok(Some(implementation));
            }
            if analysis.is_none()
                && let Some(implementation) =
                    workspace_source_method_implementation_for_broken_source_with_open_docs(
                        &uri, &source, &package, &open_docs, position,
                    )
            {
                return Ok(Some(implementation));
            }
            if analysis.is_none()
                && let Some(implementation) =
                    workspace_source_method_implementation_for_local_source_in_broken_source_with_open_docs(
                        &uri, &source, position,
                    )
            {
                return Ok(Some(implementation));
            }
            let Some(analysis) = analysis else {
                return Ok(None);
            };
            return Ok(implementation_for_analysis(
                &uri, &source, &analysis, position,
            ));
        }

        let Ok(analysis) = analyze_source(&source) else {
            return Ok(None);
        };
        Ok(implementation_for_analysis(
            &uri, &source, &analysis, position,
        ))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let Some(source) = self.documents.get(&uri).await else {
            return Ok(None);
        };

        if let Some(package) = self.package_analysis_for_uri(&uri) {
            let open_docs = self.open_file_documents().await;
            let analysis = analyze_source(&source).ok();
            if let Some(analysis) = analysis.as_ref() {
                if let Some(references) = workspace_source_references_for_root_symbol_with_open_docs(
                    &uri,
                    &source,
                    analysis,
                    &package,
                    &open_docs,
                    position,
                    params.context.include_declaration,
                ) {
                    return Ok(Some(references));
                }
                if let Some(references) = workspace_source_references_for_import_with_open_docs(
                    &uri,
                    &source,
                    analysis,
                    &package,
                    &open_docs,
                    position,
                    params.context.include_declaration,
                ) {
                    return Ok(Some(references));
                }
            }
            if analysis.is_none()
                && let Some(references) =
                    workspace_source_references_for_import_in_broken_source_with_open_docs(
                        &uri,
                        &source,
                        &package,
                        &open_docs,
                        position,
                        params.context.include_declaration,
                    )
            {
                return Ok(Some(references));
            }
            if analysis.is_none()
                && let Some(references) =
                    workspace_source_references_for_dependency_in_broken_source_with_open_docs(
                        &uri,
                        &source,
                        &package,
                        &open_docs,
                        position,
                        params.context.include_declaration,
                    )
            {
                return Ok(Some(references));
            }

            if let Some(analysis) = analysis.as_ref()
                && let Some(references) = workspace_source_references_for_dependency_with_open_docs(
                    &uri,
                    &source,
                    Some(analysis),
                    &package,
                    &open_docs,
                    position,
                    params.context.include_declaration,
                )
            {
                return Ok(Some(references));
            }

            return Ok(dependency_references_for_position(
                &uri,
                &source,
                analysis.as_ref(),
                &package,
                position,
                params.context.include_declaration,
            ));
        }

        let Ok(analysis) = analyze_source(&source) else {
            return Ok(None);
        };
        Ok(references_for_analysis(
            &uri,
            &source,
            &analysis,
            position,
            params.context.include_declaration,
        ))
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let Some(source) = self.documents.get(&uri).await else {
            return Ok(None);
        };

        if let Some(package) = self.package_analysis_for_uri(&uri) {
            let open_docs = self.open_file_documents().await;
            let Ok(analysis) = analyze_source(&source) else {
                return Ok(fallback_document_highlights_for_package_at_with_open_docs(
                    &uri, &source, &package, position, &open_docs,
                ));
            };
            if let Some(highlights) = workspace_import_document_highlights_with_open_docs(
                &uri, &source, &analysis, &package, &open_docs, position,
            ) {
                return Ok(Some(highlights));
            }
            if let Some(highlights) = workspace_dependency_document_highlights_with_open_docs(
                &uri, &source, &analysis, &package, position, &open_docs,
            ) {
                return Ok(Some(highlights));
            }
            return Ok(document_highlights_for_package_analysis_at(
                &uri, &source, &analysis, &package, position,
            ));
        }

        let Ok(analysis) = analyze_source(&source) else {
            return Ok(None);
        };
        Ok(document_highlights_for_analysis_at(
            &uri, &source, &analysis, position,
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
            let open_docs = self.open_file_documents().await;
            if let Some(completion) = completion_for_dependency_imports(&source, package, position)
            {
                return Ok(Some(completion));
            }
            if let Some(completion) = workspace_source_struct_field_completions_with_open_docs(
                &source, package, &open_docs, position,
            ) {
                return Ok(Some(completion));
            }
            if let Some(completion) =
                completion_for_dependency_struct_fields(&source, package, position)
            {
                return Ok(Some(completion));
            }
            if let Some(completion) = workspace_source_member_field_completions_with_open_docs(
                &source, package, &open_docs, position,
            ) {
                return Ok(Some(completion));
            }
            if let Some(completion) =
                completion_for_dependency_member_fields(&source, package, position)
            {
                return Ok(Some(completion));
            }
            if let Some(completion) = workspace_source_method_completions_with_open_docs(
                &source, package, &open_docs, position,
            ) {
                return Ok(Some(completion));
            }
            if let Some(completion) = completion_for_dependency_methods(&source, package, position)
            {
                return Ok(Some(completion));
            }
            if let Some(completion) = workspace_source_variant_completions_with_open_docs(
                &source, package, &open_docs, position,
            ) {
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

    async fn code_action(
        &self,
        params: CodeActionParams,
    ) -> Result<Option<Vec<CodeActionOrCommand>>> {
        let uri = params.text_document.uri;
        let Some(source) = self.documents.get(&uri).await else {
            return Ok(None);
        };
        let documents = self.documents.entries().await;
        let workspace_roots = self.workspace_roots.read().await.clone();
        let actions = auto_import_code_actions_for_source(
            &uri,
            &source,
            &params.context.diagnostics,
            documents.clone(),
            &workspace_roots,
        );
        let mut actions = actions;
        actions.extend(import_missing_dependency_code_actions_for_position(
            &uri,
            &source,
            params.range.start,
            documents,
            &workspace_roots,
        ));
        Ok((!actions.is_empty()).then_some(actions))
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let Some(source) = self.documents.get(&uri).await else {
            return Ok(None);
        };

        match document_formatting_edits(&source) {
            Ok(edits) => Ok(Some(edits)),
            Err(message) => {
                self.client.log_message(MessageType::WARNING, message).await;
                Ok(None)
            }
        }
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let Some((source, analysis)) = self.analyzed_document(&uri).await else {
            return Ok(None);
        };

        Ok(Some(document_symbols_for_analysis(&source, &analysis)))
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let documents = self.documents.entries().await;
        let workspace_roots = self.workspace_roots.read().await.clone();
        Ok(Some(workspace_symbols_for_documents_and_roots(
            documents,
            &workspace_roots,
            &params.query,
        )))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let Some(source) = self.documents.get(&uri).await else {
            return Ok(None);
        };

        if let Some(package) = self.package_analysis_for_uri(&uri) {
            let open_docs = self.open_file_documents().await;
            if let Ok(analysis) = analyze_source(&source) {
                return Ok(Some(
                    semantic_tokens_for_workspace_package_analysis_with_open_docs(
                        &uri, &source, &analysis, &package, &open_docs,
                    ),
                ));
            }
            return Ok(Some(
                semantic_tokens_for_workspace_dependency_fallback_with_open_docs(
                    &uri, &source, &package, &open_docs,
                ),
            ));
        }

        let Ok(analysis) = analyze_source(&source) else {
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
        let Some(source) = self.documents.get(&uri).await else {
            return Ok(None);
        };
        if let Some(package) = self.package_analysis_for_uri(&uri) {
            let open_docs = self.open_file_documents().await;
            if let Some(rename) = prepare_rename_for_dependency_imports(&source, &package, position)
            {
                return Ok(Some(rename));
            }
            let analysis = analyze_source(&source).ok();
            if let Some(rename_target) = workspace_source_dependency_prepare_rename_with_open_docs(
                &source,
                analysis.as_ref(),
                &package,
                &open_docs,
                position,
            ) && supports_workspace_source_dependency_rename(rename_target.kind)
            {
                return Ok(Some(PrepareRenameResponse::RangeWithPlaceholder {
                    range: span_to_range(&source, rename_target.span),
                    placeholder: rename_target.name,
                }));
            }
            if analysis.is_none() {
                if let Some(rename) =
                    prepare_rename_for_workspace_source_root_symbol_from_import_in_broken_source_with_open_docs(
                        &uri,
                        &source,
                        &package,
                        &open_docs,
                        position,
                    )
                {
                    return Ok(Some(rename));
                }
                if let Some(rename) =
                    prepare_rename_for_workspace_import_in_broken_source_with_open_docs(
                        &uri, &source, &package, &open_docs, position,
                    )
                {
                    return Ok(Some(rename));
                }
                if position_to_offset(&source, position)
                    .and_then(|offset| package.dependency_hover_in_source_at(&source, offset))
                    .is_some()
                {
                    return Ok(None);
                }
                return Ok(None);
            }
            if let Some(rename) =
                prepare_rename_for_workspace_source_root_symbol_from_import_with_open_docs(
                    &uri,
                    &source,
                    analysis.as_ref().expect("analysis checked above"),
                    &package,
                    &open_docs,
                    position,
                )
            {
                return Ok(Some(rename));
            }

            return Ok(prepare_rename_for_analysis(
                &source,
                analysis.as_ref().expect("analysis checked above"),
                position,
            ));
        }

        let Ok(analysis) = analyze_source(&source) else {
            return Ok(None);
        };
        Ok(prepare_rename_for_analysis(&source, &analysis, position))
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let Some(source) = self.documents.get(&uri).await else {
            return Ok(None);
        };
        if let Some(package) = self.package_analysis_for_uri(&uri) {
            let analysis = analyze_source(&source).ok();
            let open_docs = self.open_file_documents().await;
            if let Some(analysis) = analysis.as_ref()
                && let Some(edit) = rename_for_local_source_dependency_with_open_docs(
                    &uri,
                    &source,
                    analysis,
                    &package,
                    &open_docs,
                    position,
                    &params.new_name,
                )
                .map_err(|error| Error::invalid_params(error.to_string()))?
            {
                return Ok(Some(edit));
            }
            if let Some(edit) = rename_for_workspace_source_dependency_with_open_docs(
                &uri,
                &source,
                analysis.as_ref(),
                &package,
                &open_docs,
                position,
                &params.new_name,
            )
            .map_err(|error| Error::invalid_params(error.to_string()))?
            {
                return Ok(Some(edit));
            }
            if let Some(analysis) = analysis.as_ref()
                && let Some(edit) = rename_for_workspace_source_root_symbol_with_open_docs(
                    &uri,
                    &source,
                    analysis,
                    &package,
                    &open_docs,
                    position,
                    &params.new_name,
                )
                .map_err(|error| Error::invalid_params(error.to_string()))?
            {
                return Ok(Some(edit));
            }
            if let Some(analysis) = analysis.as_ref()
                && let Some(edit) =
                    rename_for_workspace_source_root_symbol_from_import_with_open_docs(
                        &uri,
                        &source,
                        analysis,
                        &package,
                        &open_docs,
                        position,
                        &params.new_name,
                    )
                    .map_err(|error| Error::invalid_params(error.to_string()))?
            {
                return Ok(Some(edit));
            }
            if let Some(edit) =
                rename_for_dependency_imports(&uri, &source, &package, position, &params.new_name)
                    .map_err(|error| Error::invalid_params(error.to_string()))?
            {
                return Ok(Some(edit));
            }
            if analysis.is_none() {
                if let Some(edit) =
                    rename_for_workspace_source_root_symbol_from_import_in_broken_source_with_open_docs(
                        &uri,
                        &source,
                        &package,
                        &open_docs,
                        position,
                        &params.new_name,
                    )
                    .map_err(|error| Error::invalid_params(error.to_string()))?
                {
                    return Ok(Some(edit));
                }
                if let Some(edit) = rename_for_workspace_import_in_broken_source_with_open_docs(
                    &uri,
                    &source,
                    &package,
                    &open_docs,
                    position,
                    &params.new_name,
                )
                .map_err(|error| Error::invalid_params(error.to_string()))?
                {
                    return Ok(Some(edit));
                }
                if position_to_offset(&source, position)
                    .and_then(|offset| package.dependency_hover_in_source_at(&source, offset))
                    .is_some()
                {
                    return Ok(None);
                }
                return Ok(None);
            }

            return rename_for_analysis(
                &uri,
                &source,
                analysis.as_ref().expect("analysis checked above"),
                position,
                &params.new_name,
            )
            .map_err(|error| Error::invalid_params(error.to_string()));
        }

        let Ok(analysis) = analyze_source(&source) else {
            return Ok(None);
        };
        rename_for_analysis(&uri, &source, &analysis, position, &params.new_name)
            .map_err(|error| Error::invalid_params(error.to_string()))
    }
}

#[cfg(test)]
mod tests;
