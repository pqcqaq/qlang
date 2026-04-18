use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use ql_analysis::{
    Analysis, DependencyDefinitionTarget, DependencyInterface, PackageAnalysisError, RenameError,
    analyze_available_package_dependencies, analyze_package,
    analyze_package_with_available_dependencies, analyze_source,
};
use ql_lexer::{Token, TokenKind, is_keyword, is_valid_identifier, lex};
use ql_project::{collect_package_sources, load_project_manifest};
use ql_span::Span;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::{Error, Result};
use tower_lsp::lsp_types::request::{
    GotoDeclarationParams, GotoDeclarationResponse, GotoTypeDefinitionParams,
    GotoTypeDefinitionResponse,
};
use tower_lsp::lsp_types::{
    CompletionOptions, CompletionParams, CompletionResponse, DeclarationCapability,
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DocumentHighlight, DocumentHighlightParams, DocumentSymbolParams, DocumentSymbolResponse,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams, HoverProviderCapability,
    InitializeParams, InitializeResult, InitializedParams, Location, MessageType, OneOf,
    PrepareRenameResponse, ReferenceParams, RenameOptions, RenameParams, SemanticTokensFullOptions,
    SemanticTokensOptions, SemanticTokensParams, SemanticTokensResult,
    SemanticTokensServerCapabilities, ServerCapabilities, ServerInfo, SymbolInformation,
    TextDocumentPositionParams, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextDocumentSyncOptions, TextEdit, TypeDefinitionProviderCapability, Url, WorkspaceEdit,
    WorkspaceSymbolParams,
};
use tower_lsp::{Client, LanguageServer};

use crate::bridge::{
    completion_for_analysis, completion_for_dependency_imports,
    completion_for_dependency_member_fields, completion_for_dependency_methods,
    completion_for_dependency_struct_fields, completion_for_dependency_variants,
    completion_for_package_analysis, declaration_for_dependency_imports,
    declaration_for_dependency_methods, declaration_for_dependency_struct_fields,
    declaration_for_dependency_values, declaration_for_dependency_variants,
    declaration_for_package_analysis, definition_for_dependency_imports,
    definition_for_dependency_methods, definition_for_dependency_struct_fields,
    definition_for_dependency_values, definition_for_dependency_variants,
    definition_for_package_analysis, diagnostics_to_lsp, document_symbol_kind,
    document_symbols_for_analysis, hover_for_dependency_imports, hover_for_dependency_methods,
    hover_for_dependency_struct_fields, hover_for_dependency_values, hover_for_dependency_variants,
    hover_for_package_analysis, position_to_offset, prepare_rename_for_analysis,
    prepare_rename_for_dependency_imports, references_for_analysis,
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
            let preferred_local_dependency_package_names =
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
                &preferred_local_dependency_package_names,
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
    excluded_package_names: &HashSet<String>,
    symbols: &mut Vec<SymbolInformation>,
    query: &str,
) {
    if let Ok(dependencies) = analyze_available_package_dependencies(package_path) {
        let filtered_dependencies = dependencies
            .into_iter()
            .filter(|dependency| {
                dependency
                    .manifest()
                    .package
                    .as_ref()
                    .is_none_or(|package| !excluded_package_names.contains(&package.name))
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
) -> HashSet<String> {
    let mut preferred_package_names = HashSet::new();

    for local_dependency_manifest_path in
        local_dependency_manifest_paths_for_package(package_manifest_path)
    {
        if !manifest_has_workspace_symbol_source(&local_dependency_manifest_path, open_docs) {
            continue;
        }

        if let Ok(local_dependency_manifest) =
            load_project_manifest(&local_dependency_manifest_path)
            && let Some(package) = local_dependency_manifest.package.as_ref()
        {
            preferred_package_names.insert(package.name.clone());
        }

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

    preferred_package_names
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
            let preferred_local_dependency_package_names =
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
                &preferred_local_dependency_package_names,
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
            let preferred_local_dependency_package_names =
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
                &preferred_local_dependency_package_names,
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
            let preferred_local_dependency_package_names =
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
                &preferred_local_dependency_package_names,
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
            let preferred_local_dependency_package_names =
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
                &preferred_local_dependency_package_names,
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
                let preferred_local_dependency_package_names =
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
                    &preferred_local_dependency_package_names,
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
                let preferred_local_dependency_package_names =
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
                    &preferred_local_dependency_package_names,
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
                let preferred_local_dependency_package_names =
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
                    &preferred_local_dependency_package_names,
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
                let preferred_local_dependency_package_names =
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
                    &preferred_local_dependency_package_names,
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

        for symbol in dependency.symbols() {
            if !query.is_empty() && !symbol.name.to_ascii_lowercase().contains(query) {
                continue;
            }
            let Some(span) = dependency.definition_span_for_symbol(symbol) else {
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct WorkspaceSourceSymbolMatch {
    location: Location,
    kind: ql_analysis::SymbolKind,
}

fn extend_workspace_import_symbol_matches(
    package: &ql_analysis::PackageAnalysis,
    current_path: Option<&Path>,
    current_source: Option<&str>,
    current_analysis: Option<&Analysis>,
    import_prefix: &[String],
    imported_name: &str,
    supports_kind: fn(ql_analysis::SymbolKind) -> bool,
    matches: &mut Vec<WorkspaceSourceSymbolMatch>,
) {
    for module in package.modules() {
        let module_path = module.path();
        let owned_source = if current_path
            .is_some_and(|path| canonicalize_or_clone(path) == canonicalize_or_clone(module_path))
        {
            None
        } else {
            let Ok(source) = fs::read_to_string(module_path) else {
                continue;
            };
            Some(source.replace("\r\n", "\n"))
        };
        let module_source = owned_source
            .as_deref()
            .unwrap_or_else(|| current_source.unwrap_or_default());
        let module_analysis = if owned_source.is_some() {
            module.analysis()
        } else {
            current_analysis.unwrap_or(module.analysis())
        };
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
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BrokenSourceImportBinding {
    imported_name: String,
    import_prefix: Vec<String>,
    local_name: String,
    definition_span: Span,
}

fn workspace_source_symbol_matches_for_import_binding(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    import_prefix: &[String],
    imported_name: &str,
    supports_kind: fn(ql_analysis::SymbolKind) -> bool,
) -> Vec<WorkspaceSourceSymbolMatch> {
    let current_path = uri.to_file_path().ok();
    let mut matches = Vec::new();

    extend_workspace_import_symbol_matches(
        package,
        current_path.as_deref(),
        Some(source),
        analysis,
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
        extend_workspace_import_symbol_matches(
            &member_package,
            None,
            None,
            None,
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

fn workspace_source_locations_for_import_binding(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    import_prefix: &[String],
    imported_name: &str,
    supports_kind: fn(ql_analysis::SymbolKind) -> bool,
) -> Vec<Location> {
    workspace_source_symbol_matches_for_import_binding(
        uri,
        source,
        analysis,
        package,
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
    let matches = workspace_source_locations_for_import_binding(
        uri,
        source,
        analysis,
        package,
        import_prefix,
        imported_name,
        supports_workspace_import_definition,
    );
    (matches.len() == 1).then(|| matches[0].clone())
}

fn workspace_source_type_definition_location_for_import_binding(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    import_prefix: &[String],
    imported_name: &str,
) -> Option<Location> {
    let matches = workspace_source_locations_for_import_binding(
        uri,
        source,
        analysis,
        package,
        import_prefix,
        imported_name,
        supports_workspace_import_type_definition,
    );
    (matches.len() == 1).then(|| matches[0].clone())
}

fn workspace_source_kind_for_import_binding(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    import_prefix: &[String],
    imported_name: &str,
    supports_kind: fn(ql_analysis::SymbolKind) -> bool,
) -> Option<ql_analysis::SymbolKind> {
    let matches = workspace_source_symbol_matches_for_import_binding(
        uri,
        source,
        analysis,
        package,
        import_prefix,
        imported_name,
        supports_kind,
    );
    (matches.len() == 1).then(|| matches[0].kind)
}

fn extend_workspace_import_reference_locations(
    package: &ql_analysis::PackageAnalysis,
    current_path: Option<&Path>,
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
        let Ok(source) = fs::read_to_string(module.path()) else {
            continue;
        };
        let source = source.replace("\r\n", "\n");
        let Ok(uri) = Url::from_file_path(module.path()) else {
            continue;
        };
        let (tokens, _) = lex(&source);
        let mut module_locations = tokens
            .iter()
            .filter(|token| token.kind == TokenKind::Ident)
            .filter_map(|token| {
                let (binding, occurrence_span) =
                    module.analysis().import_binding_at(token.span.start)?;
                (binding.path.segments.as_slice() == import_path
                    && occurrence_span == token.span
                    && (include_declaration || occurrence_span != binding.definition_span))
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

fn workspace_import_reference_locations(
    package: &ql_analysis::PackageAnalysis,
    current_path: Option<&Path>,
    import_path: &[String],
    include_declaration: bool,
) -> Vec<Location> {
    let mut locations = Vec::new();
    extend_workspace_import_reference_locations(
        package,
        current_path,
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
        extend_workspace_import_reference_locations(
            &member_package,
            None,
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
    let offset = position_to_offset(source, position)?;
    let (binding, _) = analysis.import_binding_at(offset)?;
    let (imported_name, import_prefix) = binding.path.segments.split_last()?;
    workspace_source_location_for_import_binding(
        uri,
        source,
        Some(analysis),
        package,
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
    let offset = position_to_offset(source, position)?;
    let binding = analysis.type_import_binding_at(offset)?;
    let (imported_name, import_prefix) = binding.path.segments.split_last()?;
    workspace_source_type_definition_location_for_import_binding(
        uri,
        source,
        Some(analysis),
        package,
        import_prefix,
        imported_name,
    )
    .map(GotoTypeDefinitionResponse::Scalar)
}

fn hover_from_workspace_source_location(
    current_source: &str,
    current_span: Span,
    source_location: Location,
) -> Option<Hover> {
    let source_path = source_location.uri.to_file_path().ok()?;
    let source = fs::read_to_string(source_path).ok()?.replace("\r\n", "\n");
    let analysis = analyze_source(&source).ok()?;
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
    let offset = position_to_offset(source, position)?;
    let (binding, occurrence_span) = analysis.import_binding_at(offset)?;
    let (imported_name, import_prefix) = binding.path.segments.split_last()?;
    let source_location = workspace_source_location_for_import_binding(
        uri,
        source,
        Some(analysis),
        package,
        import_prefix,
        imported_name,
    )?;

    hover_from_workspace_source_location(source, occurrence_span, source_location)
}

fn workspace_source_definition_for_import_in_broken_source(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<GotoDefinitionResponse> {
    let binding = broken_source_import_binding_at(source, position)?;
    workspace_source_location_for_import_binding(
        uri,
        source,
        None,
        package,
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
    let binding = broken_source_import_binding_at(source, position)?;
    workspace_source_type_definition_location_for_import_binding(
        uri,
        source,
        None,
        package,
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
    let binding = broken_source_import_binding_at(source, position)?;
    let occurrence_span =
        broken_source_import_occurrence_span_at(source, position, binding.local_name.as_str())?;
    let source_location = workspace_source_location_for_import_binding(
        uri,
        source,
        None,
        package,
        &binding.import_prefix,
        binding.imported_name.as_str(),
    )?;

    hover_from_workspace_source_location(source, occurrence_span, source_location)
}

fn workspace_import_semantic_tokens_in_analysis(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
) -> Vec<ql_analysis::SemanticTokenOccurrence> {
    let (tokens, _) = lex(source);
    let bindings = broken_source_import_bindings_in_tokens(&tokens);
    let mut local_name_counts = HashMap::<String, usize>::new();
    let mut local_name_kinds = HashMap::<String, ql_analysis::SymbolKind>::new();

    for binding in bindings {
        let Some(kind) = workspace_source_kind_for_import_binding(
            uri,
            source,
            Some(analysis),
            package,
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

fn workspace_import_semantic_tokens_in_broken_source(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
) -> Vec<ql_analysis::SemanticTokenOccurrence> {
    let (tokens, _) = lex(source);
    let mut unique_bindings = HashMap::<String, (Span, ql_analysis::SymbolKind)>::new();
    let mut local_name_counts = HashMap::<String, usize>::new();

    for binding in broken_source_import_bindings_in_tokens(&tokens) {
        let Some(kind) = workspace_source_kind_for_import_binding(
            uri,
            source,
            None,
            package,
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

fn semantic_tokens_for_workspace_package_analysis(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
) -> SemanticTokensResult {
    let mut tokens = analysis.semantic_tokens();
    let dependency_import_root_tokens =
        package.dependency_import_root_semantic_tokens_in_source(source);
    let workspace_import_root_tokens =
        workspace_import_semantic_tokens_in_analysis(uri, source, analysis, package);
    let overridden_import_spans = dependency_import_root_tokens
        .iter()
        .chain(workspace_import_root_tokens.iter())
        .map(|token| (token.span.start, token.span.end))
        .collect::<HashSet<_>>();

    tokens.retain(|token| {
        token.kind != ql_analysis::SymbolKind::Import
            || !overridden_import_spans.contains(&(token.span.start, token.span.end))
    });
    tokens.extend(package.dependency_semantic_tokens_in_source(source));
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
    let mut tokens = package.dependency_fallback_semantic_tokens_in_source(source);
    tokens.extend(workspace_import_semantic_tokens_in_broken_source(
        uri, source, package,
    ));
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
        )
    ) || matches!(prev_kind, Some(TokenKind::Colon | TokenKind::Arrow))
}

fn extend_workspace_dependency_definition_matches(
    package: &ql_analysis::PackageAnalysis,
    current_path: Option<&Path>,
    current_source: Option<&str>,
    current_analysis: Option<&Analysis>,
    target: &DependencyDefinitionTarget,
    matches: &mut Vec<Location>,
) {
    if !supports_workspace_dependency_definition(target.kind) {
        return;
    }

    for module in package.modules() {
        let module_path = module.path();
        if !package_module_matches_dependency_source_path(package, module_path, &target.source_path)
        {
            continue;
        }

        let owned_source = if current_path
            .is_some_and(|path| canonicalize_or_clone(path) == canonicalize_or_clone(module_path))
        {
            None
        } else {
            let Ok(source) = fs::read_to_string(module_path) else {
                continue;
            };
            Some(source.replace("\r\n", "\n"))
        };
        let module_source = owned_source
            .as_deref()
            .unwrap_or_else(|| current_source.unwrap_or_default());
        let module_analysis = if owned_source.is_some() {
            module.analysis()
        } else {
            current_analysis.unwrap_or(module.analysis())
        };

        let Ok(module_uri) = Url::from_file_path(module_path) else {
            continue;
        };
        for symbol in module_analysis.document_symbols() {
            if symbol.name != target.name || symbol.kind != target.kind {
                continue;
            }
            matches.push(Location::new(
                module_uri.clone(),
                span_to_range(module_source, symbol.span),
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

fn same_dependency_definition_target(
    lhs: &DependencyDefinitionTarget,
    rhs: &DependencyDefinitionTarget,
) -> bool {
    lhs.package_name == rhs.package_name
        && lhs.source_path == rhs.source_path
        && lhs.kind == rhs.kind
        && lhs.name == rhs.name
        && lhs.span == rhs.span
}

fn extend_workspace_dependency_reference_locations(
    package: &ql_analysis::PackageAnalysis,
    current_path: Option<&Path>,
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
        let Ok(source) = fs::read_to_string(&source_path) else {
            continue;
        };
        let source = source.replace("\r\n", "\n");
        let Ok(uri) = Url::from_file_path(&source_path) else {
            continue;
        };
        let owned_analysis = package
            .modules()
            .iter()
            .find(|module| {
                canonicalize_or_clone(module.path()) == canonicalize_or_clone(&source_path)
            })
            .map(|module| module.analysis().clone())
            .or_else(|| analyze_source(&source).ok());
        let analysis = owned_analysis.as_ref();
        let mut module_locations = lex(&source)
            .0
            .iter()
            .filter(|token| token.kind == TokenKind::Ident)
            .filter_map(|token| {
                let position = span_to_range(&source, token.span).start;
                let occurrence_span = dependency_occurrence_span_at(&source, package, position)?;
                if !include_declaration
                    && dependency_reference_is_definition_at(&source, analysis, package, position)
                        == Some(true)
                {
                    return None;
                }
                let occurrence_target =
                    dependency_definition_target_at(&source, analysis, package, position)?;
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
    let mut locations = Vec::new();
    extend_workspace_dependency_reference_locations(
        package,
        current_path,
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
        extend_workspace_dependency_reference_locations(
            &member_package,
            None,
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
    let current_path = uri.to_file_path().ok();
    let mut matches = Vec::new();

    extend_workspace_dependency_definition_matches(
        package,
        current_path.as_deref(),
        Some(source),
        analysis,
        target,
        &mut matches,
    );

    for candidate_manifest_path in
        source_preferred_manifest_paths_for_package(package.manifest().manifest_path.as_path())
    {
        let Some(member_package) = package_analysis_for_path(&candidate_manifest_path) else {
            continue;
        };
        extend_workspace_dependency_definition_matches(
            &member_package,
            None,
            None,
            None,
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

fn workspace_source_definition_for_dependency(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<GotoDefinitionResponse> {
    let target = dependency_definition_target_at(source, analysis, package, position)?;
    workspace_source_location_for_dependency_target(uri, source, analysis, package, &target)
        .map(GotoDefinitionResponse::Scalar)
}

fn workspace_source_type_definition_for_dependency(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<GotoTypeDefinitionResponse> {
    let target = dependency_type_definition_target_at(source, analysis, package, position)?;
    workspace_source_location_for_dependency_target(uri, source, analysis, package, &target)
        .map(GotoTypeDefinitionResponse::Scalar)
}

fn workspace_source_hover_for_dependency(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<Hover> {
    let occurrence_span = dependency_occurrence_span_at(source, package, position)?;
    let target = dependency_definition_target_at(source, analysis, package, position)?;
    let source_location =
        workspace_source_location_for_dependency_target(uri, source, analysis, package, &target)?;

    hover_from_workspace_source_location(source, occurrence_span, source_location)
}

fn workspace_source_references_for_import(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let offset = position_to_offset(source, position)?;
    let (binding, _) = analysis.import_binding_at(offset)?;
    let (imported_name, import_prefix) = binding.path.segments.split_last()?;
    let source_matches = workspace_source_locations_for_import_binding(
        uri,
        source,
        Some(analysis),
        package,
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
            same_file_references_for_source_location(source_definition)
    {
        if !include_declaration {
            source_locations.retain(|location| !same_location_anchor(location, source_definition));
        }
        merge_unique_reference_locations(&mut locations, source_locations);
    }
    let current_path = uri.to_file_path().ok();
    merge_unique_reference_locations(
        &mut locations,
        workspace_import_reference_locations(
            package,
            current_path.as_deref(),
            &binding.path.segments,
            include_declaration,
        ),
    );

    Some(locations)
}

fn workspace_source_references_for_import_in_broken_source(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let binding = broken_source_import_binding_at(source, position)?;
    let source_matches = workspace_source_locations_for_import_binding(
        uri,
        source,
        None,
        package,
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
            same_file_references_for_source_location(source_definition)
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
        workspace_import_reference_locations(
            package,
            current_path.as_deref(),
            &import_path,
            include_declaration,
        ),
    );

    (!locations.is_empty()).then_some(locations)
}

fn workspace_import_document_highlights_in_broken_source(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<Vec<DocumentHighlight>> {
    let binding = broken_source_import_binding_at(source, position)?;
    let locations = workspace_source_references_for_import_in_broken_source(
        uri, source, package, position, true,
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

fn workspace_dependency_document_highlights_in_broken_source(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<Vec<DocumentHighlight>> {
    let locations = workspace_source_references_for_dependency_in_broken_source(
        uri, source, package, position, true,
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

fn prepare_rename_for_workspace_import_in_broken_source(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
) -> Option<PrepareRenameResponse> {
    let binding = broken_source_import_binding_at(source, position)?;
    if workspace_source_locations_for_import_binding(
        uri,
        source,
        None,
        package,
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

fn rename_for_workspace_import_in_broken_source(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
    new_name: &str,
) -> std::result::Result<Option<WorkspaceEdit>, RenameError> {
    validate_rename_text(new_name)?;

    let Some(binding) = broken_source_import_binding_at(source, position) else {
        return Ok(None);
    };
    if workspace_source_locations_for_import_binding(
        uri,
        source,
        None,
        package,
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
    } else if let Some(first_location) = locations.first_mut() {
        *first_location = source_definition.clone();
    } else {
        locations.push(source_definition.clone());
    }
}

fn same_file_references_for_source_location(source_location: &Location) -> Option<Vec<Location>> {
    let source_path = source_location.uri.to_file_path().ok()?;
    let source = fs::read_to_string(source_path).ok()?.replace("\r\n", "\n");
    let analysis = analyze_source(&source).ok()?;
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

fn workspace_source_references_for_dependency(
    uri: &Url,
    source: &str,
    analysis: Option<&Analysis>,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let target = dependency_definition_target_at(source, analysis, package, position)?;
    let source_definition =
        workspace_source_location_for_dependency_target(uri, source, analysis, package, &target)?;
    let mut locations = dependency_references_for_position(
        uri,
        source,
        analysis,
        package,
        position,
        include_declaration,
    )?;
    if include_declaration {
        normalize_reference_locations_with_definition(&mut locations, &source_definition);
    }
    if let Some(mut source_locations) = same_file_references_for_source_location(&source_definition)
    {
        if !include_declaration {
            source_locations.retain(|location| !same_location_anchor(location, &source_definition));
        }
        merge_unique_reference_locations(&mut locations, source_locations);
    }
    let current_path = uri.to_file_path().ok();
    merge_unique_reference_locations(
        &mut locations,
        workspace_dependency_reference_locations(
            package,
            current_path.as_deref(),
            &target,
            include_declaration,
        ),
    );

    Some(locations)
}

fn workspace_source_references_for_dependency_in_broken_source(
    uri: &Url,
    source: &str,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    workspace_source_references_for_dependency(
        uri,
        source,
        None,
        package,
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
    if let Some(highlights) =
        workspace_import_document_highlights_in_broken_source(uri, source, package, position)
    {
        return Some(highlights);
    }
    if let Some(highlights) =
        workspace_dependency_document_highlights_in_broken_source(uri, source, package, position)
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
                references_provider: Some(OneOf::Left(true)),
                document_highlight_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
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
            let analysis = analyze_source(&source).ok();
            if let Some(analysis) = analysis.as_ref() {
                if let Some(hover) =
                    workspace_source_hover_for_import(&uri, &source, analysis, &package, position)
                {
                    return Ok(Some(hover));
                }
            } else if let Some(hover) = workspace_source_hover_for_import_in_broken_source(
                &uri, &source, &package, position,
            ) {
                return Ok(Some(hover));
            }
            if let Some(hover) = workspace_source_hover_for_dependency(
                &uri,
                &source,
                analysis.as_ref(),
                &package,
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
            let analysis = analyze_source(&source).ok();
            if let Some(analysis) = analysis.as_ref()
                && let Some(definition) = workspace_source_definition_for_import(
                    &uri, &source, analysis, &package, position,
                )
            {
                return Ok(Some(definition));
            }
            if analysis.is_none()
                && let Some(definition) = workspace_source_definition_for_import_in_broken_source(
                    &uri, &source, &package, position,
                )
            {
                return Ok(Some(definition));
            }
            if let Some(definition) = workspace_source_definition_for_dependency(
                &uri,
                &source,
                analysis.as_ref(),
                &package,
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
            let analysis = analyze_source(&source).ok();
            if let Some(analysis) = analysis.as_ref()
                && let Some(GotoDefinitionResponse::Scalar(location)) =
                    workspace_source_definition_for_import(
                        &uri, &source, analysis, &package, position,
                    )
            {
                return Ok(Some(GotoDeclarationResponse::Scalar(location)));
            }
            if analysis.is_none()
                && let Some(GotoDefinitionResponse::Scalar(location)) =
                    workspace_source_definition_for_import_in_broken_source(
                        &uri, &source, &package, position,
                    )
            {
                return Ok(Some(GotoDeclarationResponse::Scalar(location)));
            }
            if let Some(GotoDefinitionResponse::Scalar(location)) =
                workspace_source_definition_for_dependency(
                    &uri,
                    &source,
                    analysis.as_ref(),
                    &package,
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
            let analysis = analyze_source(&source).ok();
            if let Some(analysis) = analysis.as_ref() {
                if let Some(definition) = workspace_source_type_definition_for_import(
                    &uri, &source, analysis, &package, position,
                ) {
                    return Ok(Some(definition));
                }
            } else if let Some(definition) =
                workspace_source_type_definition_for_import_in_broken_source(
                    &uri, &source, &package, position,
                )
            {
                return Ok(Some(definition));
            }
            if let Some(definition) = workspace_source_type_definition_for_dependency(
                &uri,
                &source,
                analysis.as_ref(),
                &package,
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

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let Some(source) = self.documents.get(&uri).await else {
            return Ok(None);
        };

        if let Some(package) = self.package_analysis_for_uri(&uri) {
            let analysis = analyze_source(&source).ok();
            if let Some(analysis) = analysis.as_ref() {
                if let Some(references) = workspace_source_references_for_import(
                    &uri,
                    &source,
                    analysis,
                    &package,
                    position,
                    params.context.include_declaration,
                ) {
                    return Ok(Some(references));
                }
            }
            if analysis.is_none()
                && let Some(references) = workspace_source_references_for_import_in_broken_source(
                    &uri,
                    &source,
                    &package,
                    position,
                    params.context.include_declaration,
                )
            {
                return Ok(Some(references));
            }
            if analysis.is_none()
                && let Some(references) =
                    workspace_source_references_for_dependency_in_broken_source(
                        &uri,
                        &source,
                        &package,
                        position,
                        params.context.include_declaration,
                    )
            {
                return Ok(Some(references));
            }

            if let Some(analysis) = analysis.as_ref() {
                if let Some(references) = workspace_source_references_for_dependency(
                    &uri,
                    &source,
                    Some(analysis),
                    &package,
                    position,
                    params.context.include_declaration,
                ) {
                    return Ok(Some(references));
                }
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
            let Ok(analysis) = analyze_source(&source) else {
                return Ok(fallback_document_highlights_for_package_at(
                    &uri, &source, &package, position,
                ));
            };
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
            if let Some(completion) = completion_for_dependency_imports(&source, package, position)
            {
                return Ok(Some(completion));
            }
            if let Some(completion) =
                completion_for_dependency_struct_fields(&source, package, position)
            {
                return Ok(Some(completion));
            }
            if let Some(completion) =
                completion_for_dependency_member_fields(&source, package, position)
            {
                return Ok(Some(completion));
            }
            if let Some(completion) = completion_for_dependency_methods(&source, package, position)
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
            if let Ok(analysis) = analyze_source(&source) {
                return Ok(Some(semantic_tokens_for_workspace_package_analysis(
                    &uri, &source, &analysis, &package,
                )));
            }
            return Ok(Some(semantic_tokens_for_workspace_dependency_fallback(
                &uri, &source, &package,
            )));
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
            if let Some(rename) = prepare_rename_for_dependency_imports(&source, &package, position)
            {
                return Ok(Some(rename));
            }
            let analysis = analyze_source(&source).ok();
            if analysis.is_none() {
                if let Some(rename) = prepare_rename_for_workspace_import_in_broken_source(
                    &uri, &source, &package, position,
                ) {
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
            if let Some(edit) =
                rename_for_dependency_imports(&uri, &source, &package, position, &params.new_name)
                    .map_err(|error| Error::invalid_params(error.to_string()))?
            {
                return Ok(Some(edit));
            }
            let analysis = analyze_source(&source).ok();
            if analysis.is_none() {
                if let Some(edit) = rename_for_workspace_import_in_broken_source(
                    &uri,
                    &source,
                    &package,
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
mod tests {
    use super::{
        GotoTypeDefinitionResponse, document_highlights_for_analysis_at,
        document_highlights_for_package_analysis_at, fallback_document_highlights_for_package_at,
        package_analysis_for_path, prepare_rename_for_dependency_imports,
        prepare_rename_for_workspace_import_in_broken_source, rename_for_dependency_imports,
        rename_for_workspace_import_in_broken_source,
        semantic_tokens_for_workspace_dependency_fallback,
        semantic_tokens_for_workspace_package_analysis, workspace_source_definition_for_dependency,
        workspace_source_definition_for_import,
        workspace_source_definition_for_import_in_broken_source,
        workspace_source_hover_for_dependency, workspace_source_hover_for_import,
        workspace_source_hover_for_import_in_broken_source,
        workspace_source_references_for_dependency,
        workspace_source_references_for_dependency_in_broken_source,
        workspace_source_references_for_import,
        workspace_source_references_for_import_in_broken_source,
        workspace_source_type_definition_for_dependency,
        workspace_source_type_definition_for_import,
        workspace_source_type_definition_for_import_in_broken_source,
        workspace_symbols_for_documents, workspace_symbols_for_documents_and_roots,
    };
    use crate::bridge::{semantic_tokens_legend, span_to_range};
    use ql_analysis::{RenameError, SymbolKind as AnalysisSymbolKind, analyze_source};
    use ql_span::Span;
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};
    use tower_lsp::lsp_types::{
        GotoDefinitionResponse, HoverContents, Location, Position, PrepareRenameResponse,
        SemanticTokenType, SemanticTokensResult, SymbolInformation, SymbolKind, TextEdit, Url,
        WorkspaceEdit,
    };

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos();
            let path = env::temp_dir().join(format!("{prefix}-{unique}"));
            fs::create_dir_all(&path).expect("create temporary test directory");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write(&self, relative: &str, contents: &str) -> PathBuf {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent directories");
            }
            fs::write(&path, contents).expect("write file");
            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn nth_offset(source: &str, needle: &str, occurrence: usize) -> usize {
        source
            .match_indices(needle)
            .nth(occurrence.saturating_sub(1))
            .map(|(start, _)| start)
            .expect("needle occurrence should exist")
    }

    fn offset_to_position(source: &str, offset: usize) -> Position {
        let prefix = &source[..offset];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
        let line_start = prefix.rfind('\n').map(|index| index + 1).unwrap_or(0);
        Position::new(line, prefix[line_start..].chars().count() as u32)
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

    fn assert_workspace_edit(edit: WorkspaceEdit, uri: &Url, expected: Vec<TextEdit>) {
        let changes = edit
            .changes
            .expect("workspace edit should contain direct changes");
        let edits = changes
            .get(uri)
            .expect("workspace edit should target current document");
        assert_eq!(edits, &expected);
    }

    fn assert_single_dependency_method_symbol(
        symbols: Vec<SymbolInformation>,
        name: &str,
        interface_path: &Path,
        line: u32,
        start: u32,
        end: u32,
        package_name: &str,
    ) {
        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: name.to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(line, start),
                        tower_lsp::lsp_types::Position::new(line, end),
                    ),
                ),
                container_name: Some(package_name.to_owned()),
            }]
        );
    }

    fn assert_single_source_symbol(
        symbols: Vec<SymbolInformation>,
        name: &str,
        kind: SymbolKind,
        source_path: &Path,
        source: &str,
        occurrence: usize,
    ) {
        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: name.to_owned(),
                kind,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(source_path).expect("source path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        offset_to_position(source, nth_offset(source, name, occurrence)),
                        offset_to_position(
                            source,
                            nth_offset(source, name, occurrence) + name.len(),
                        ),
                    ),
                ),
                container_name: None,
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_includes_package_modules_for_open_documents() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-package");
        let root = temp.path().join("app");
        let main_path = temp.write(
            "app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let _ = main_path;
        let open_path = temp.write(
            "app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        let helper_path = temp.write(
            "app/src/helper.ql",
            r#"
fn helper_value() -> Int {
    return 1
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "helper");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "helper_value".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(&helper_path).expect("helper path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(1, 3),
                        tower_lsp::lsp_types::Position::new(1, 15),
                    ),
                ),
                container_name: None,
            }]
        );

        let _ = root;
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_includes_workspace_member_modules_for_open_documents() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-members");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"
"#,
        );
        let helper_path = temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "helper");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "tool_helper".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(&helper_path).expect("helper path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(1, 3),
                        tower_lsp::lsp_types::Position::new(1, 14),
                    ),
                ),
                container_name: None,
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_workspace_member_modules_for_open_packages_when_member_has_source_diagnostics()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-open-broken-member");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"
"#,
        );
        let helper_path = temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/tool/src/broken.ql",
            r#"
fn broken() -> Int {
    let value: Int = "oops"
    return value
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "helper");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "tool_helper".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(&helper_path).expect("helper path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(1, 3),
                        tower_lsp::lsp_types::Position::new(1, 14),
                    ),
                ),
                container_name: None,
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_member_dependency_methods_for_open_packages_when_member_has_source_diagnostics()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-open-broken-member-method");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "dep"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"

[references]
packages = ["../dep"]
"#,
        );
        temp.write(
            "workspace/tool/src/helper.ql",
            r#"
use demo.dep.Config as Cfg

fn tool_helper(config: Cfg) -> Int {
    return config.get()
}
"#,
        );
        temp.write(
            "workspace/tool/src/broken.ql",
            r#"
fn broken() -> Int {
    let value: Int = "oops"
    return value
}
"#,
        );
        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "get");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "get".to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(12, 8),
                        tower_lsp::lsp_types::Position::new(12, 27),
                    ),
                ),
                container_name: Some("dep".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_dependency_symbols_for_broken_open_packages() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-broken-dependency");

        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported(value: Int) -> Int
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.dep.exported as run

fn main() -> Int {
    let broken: Int = "oops"
    return run(1)
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "exported");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "exported".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(7, 4),
                        tower_lsp::lsp_types::Position::new(7, 34),
                    ),
                ),
                container_name: Some("dep".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_prefers_local_dependency_source_symbols_for_broken_open_packages() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-broken-local-dependency-source");

        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_source_path = temp.write(
            "workspace/dep/src/lib.ql",
            r#"
package demo.dep

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported(value: Int) -> Int
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.dep.exported as run

fn main() -> Int {
    let broken: Int = "oops"
    return run(1)
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");
        let dependency_source =
            fs::read_to_string(&dependency_source_path).expect("dependency source should read");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "exported");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "exported".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(&dependency_source_path)
                        .expect("dependency source path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        offset_to_position(
                            &dependency_source,
                            nth_offset(&dependency_source, "exported", 1),
                        ),
                        offset_to_position(
                            &dependency_source,
                            nth_offset(&dependency_source, "exported", 1) + "exported".len(),
                        ),
                    ),
                ),
                container_name: None,
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_prefers_local_dependency_source_methods_for_broken_open_packages() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-broken-local-dependency-source-method");

        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_source_path = temp.write(
            "workspace/dep/src/lib.ql",
            r#"
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int {
        return self.value
    }
}
"#,
        );
        temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.dep.Config as Cfg

fn main(config: Cfg) -> Int {
    let broken: Int = "oops"
    return config.get()
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");
        let dependency_source =
            fs::read_to_string(&dependency_source_path).expect("dependency source should read");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "get");

        assert_single_source_symbol(
            symbols,
            "get",
            SymbolKind::METHOD,
            &dependency_source_path,
            &dependency_source,
            1,
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_prefers_local_dependency_source_trait_and_extend_methods_for_broken_open_packages()
     {
        let temp = TempDir::new(
            "ql-lsp-workspace-symbol-broken-local-dependency-source-trait-extend-methods",
        );

        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_source_path = temp.write(
            "workspace/dep/src/lib.ql",
            r#"
package demo.dep

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int {
        return 2
    }
}
"#,
        );
        temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

fn main() -> Int {
    let broken: Int = "oops"
    return 0
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");
        let dependency_source =
            fs::read_to_string(&dependency_source_path).expect("dependency source should read");

        let trait_symbols =
            workspace_symbols_for_documents(vec![(open_uri.clone(), open_source.clone())], "poll");
        let extend_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "twice");

        assert_single_source_symbol(
            trait_symbols,
            "poll",
            SymbolKind::METHOD,
            &dependency_source_path,
            &dependency_source,
            1,
        );
        assert_single_source_symbol(
            extend_symbols,
            "twice",
            SymbolKind::METHOD,
            &dependency_source_path,
            &dependency_source,
            1,
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_dependency_methods_for_broken_open_packages() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-broken-dependency-method");

        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.dep.Config as Cfg

fn main(config: Cfg) -> Int {
    let broken: Int = "oops"
    return config.get()
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "get");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "get".to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(12, 8),
                        tower_lsp::lsp_types::Position::new(12, 27),
                    ),
                ),
                container_name: Some("dep".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_package_and_workspace_member_modules_when_dependency_interfaces_fail()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-open-missing-dependency");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "dep"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.dep.exported as run

fn main() -> Int {
    return run(0)
}
"#,
        );
        let app_helper_path = temp.write(
            "workspace/app/src/helper.ql",
            r#"
fn app_helper() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"
"#,
        );
        let tool_helper_path = temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        temp.write(
            "workspace/dep/src/lib.ql",
            r#"
package demo.dep

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "helper");

        assert_eq!(
            symbols,
            vec![
                SymbolInformation {
                    name: "app_helper".to_owned(),
                    kind: SymbolKind::FUNCTION,
                    tags: None,
                    deprecated: None,
                    location: Location::new(
                        Url::from_file_path(&app_helper_path)
                            .expect("helper path should convert to URI"),
                        tower_lsp::lsp_types::Range::new(
                            tower_lsp::lsp_types::Position::new(1, 3),
                            tower_lsp::lsp_types::Position::new(1, 13),
                        ),
                    ),
                    container_name: None,
                },
                SymbolInformation {
                    name: "tool_helper".to_owned(),
                    kind: SymbolKind::FUNCTION,
                    tags: None,
                    deprecated: None,
                    location: Location::new(
                        Url::from_file_path(&tool_helper_path)
                            .expect("helper path should convert to URI"),
                        tower_lsp::lsp_types::Range::new(
                            tower_lsp::lsp_types::Position::new(1, 3),
                            tower_lsp::lsp_types::Position::new(1, 14),
                        ),
                    ),
                    container_name: None,
                },
            ]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_workspace_member_modules_when_member_dependency_interfaces_fail()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-member-missing-dependency");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "dep"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"

[references]
packages = ["../dep"]
"#,
        );
        let helper_path = temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        temp.write(
            "workspace/dep/src/lib.ql",
            r#"
package demo.dep

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "helper");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "tool_helper".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(&helper_path).expect("helper path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(1, 3),
                        tower_lsp::lsp_types::Position::new(1, 14),
                    ),
                ),
                container_name: None,
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_workspace_member_modules_for_broken_open_packages() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-broken-members");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    let broken: Int = "oops"
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"
"#,
        );
        let helper_path = temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "helper");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "tool_helper".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(&helper_path).expect("helper path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(1, 3),
                        tower_lsp::lsp_types::Position::new(1, 14),
                    ),
                ),
                container_name: None,
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_workspace_member_modules_for_broken_open_packages_when_dependency_interfaces_fail()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-broken-members-missing-dependency");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "dep"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.dep.exported as run

fn main() -> Int {
    let broken: Int = "oops"
    return run(0)
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"
"#,
        );
        let helper_path = temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        temp.write(
            "workspace/dep/src/lib.ql",
            r#"
package demo.dep

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "helper");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "tool_helper".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(&helper_path).expect("helper path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(1, 3),
                        tower_lsp::lsp_types::Position::new(1, 14),
                    ),
                ),
                container_name: None,
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_includes_dependency_interface_symbols_for_open_packages() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-dependency");

        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported(value: Int) -> Int
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.dep.exported as run

fn main() -> Int {
    return run(1)
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "exported");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "exported".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(7, 4),
                        tower_lsp::lsp_types::Position::new(7, 34),
                    ),
                ),
                container_name: Some("dep".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_prefers_local_dependency_source_symbols_for_open_packages() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-local-dependency-source");

        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_source_path = temp.write(
            "workspace/dep/src/lib.ql",
            r#"
package demo.dep

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported(value: Int) -> Int
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.dep.exported as run

fn main() -> Int {
    return run(1)
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");
        let dependency_source =
            fs::read_to_string(&dependency_source_path).expect("dependency source should read");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "exported");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "exported".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(&dependency_source_path)
                        .expect("dependency source path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        offset_to_position(
                            &dependency_source,
                            nth_offset(&dependency_source, "exported", 1),
                        ),
                        offset_to_position(
                            &dependency_source,
                            nth_offset(&dependency_source, "exported", 1) + "exported".len(),
                        ),
                    ),
                ),
                container_name: None,
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_prefers_local_dependency_source_methods_for_open_packages() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-local-dependency-source-method");

        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_source_path = temp.write(
            "workspace/dep/src/lib.ql",
            r#"
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int {
        return self.value
    }
}
"#,
        );
        temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.dep.Config as Cfg

fn main(config: Cfg) -> Int {
    return config.get()
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");
        let dependency_source =
            fs::read_to_string(&dependency_source_path).expect("dependency source should read");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "get");

        assert_single_source_symbol(
            symbols,
            "get",
            SymbolKind::METHOD,
            &dependency_source_path,
            &dependency_source,
            1,
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_prefers_local_dependency_source_trait_and_extend_methods_for_open_packages()
     {
        let temp =
            TempDir::new("ql-lsp-workspace-symbol-local-dependency-source-trait-extend-methods");

        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_source_path = temp.write(
            "workspace/dep/src/lib.ql",
            r#"
package demo.dep

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int {
        return 2
    }
}
"#,
        );
        temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");
        let dependency_source =
            fs::read_to_string(&dependency_source_path).expect("dependency source should read");

        let trait_symbols =
            workspace_symbols_for_documents(vec![(open_uri.clone(), open_source.clone())], "poll");
        let extend_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "twice");

        assert_single_source_symbol(
            trait_symbols,
            "poll",
            SymbolKind::METHOD,
            &dependency_source_path,
            &dependency_source,
            1,
        );
        assert_single_source_symbol(
            extend_symbols,
            "twice",
            SymbolKind::METHOD,
            &dependency_source_path,
            &dependency_source,
            1,
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_includes_dependency_interface_methods_for_open_packages() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-dependency-method");

        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.dep.Config as Cfg

fn main(config: Cfg) -> Int {
    return config.get()
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "get");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "get".to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(12, 8),
                        tower_lsp::lsp_types::Position::new(12, 27),
                    ),
                ),
                container_name: Some("dep".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_includes_dependency_interface_trait_and_extend_methods_for_open_packages()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-dependency-trait-extend-methods");

        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let trait_symbols =
            workspace_symbols_for_documents(vec![(open_uri.clone(), open_source.clone())], "poll");
        let extend_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "twice");

        assert_eq!(
            trait_symbols,
            vec![SymbolInformation {
                name: "poll".to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(8, 4),
                        tower_lsp::lsp_types::Position::new(8, 24),
                    ),
                ),
                container_name: Some("dep".to_owned()),
            }]
        );
        assert_eq!(
            extend_symbols,
            vec![SymbolInformation {
                name: "twice".to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(16, 8),
                        tower_lsp::lsp_types::Position::new(16, 29),
                    ),
                ),
                container_name: Some("dep".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_includes_dependency_interface_symbols_for_workspace_members() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-member-dependency");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "dep"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"

[references]
packages = ["../dep"]
"#,
        );
        temp.write(
            "workspace/tool/src/helper.ql",
            r#"
use demo.dep.exported as run

fn tool_helper() -> Int {
    return run(1)
}
"#,
        );
        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported(value: Int) -> Int
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "exported");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "exported".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(7, 4),
                        tower_lsp::lsp_types::Position::new(7, 34),
                    ),
                ),
                container_name: Some("dep".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_includes_dependency_interface_methods_for_workspace_members() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-member-dependency-method");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "dep"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"

[references]
packages = ["../dep"]
"#,
        );
        temp.write(
            "workspace/tool/src/helper.ql",
            r#"
use demo.dep.Config as Cfg

fn tool_helper(config: Cfg) -> Int {
    return config.get()
}
"#,
        );
        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "get");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "get".to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(12, 8),
                        tower_lsp::lsp_types::Position::new(12, 27),
                    ),
                ),
                container_name: Some("dep".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_available_dependency_symbols_when_one_package_interface_is_missing()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-partial-dependency");

        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub fn exported(value: Int) -> Int
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package]
name = "bad"
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../good", "../bad"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "exported");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "exported".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(7, 4),
                        tower_lsp::lsp_types::Position::new(7, 34),
                    ),
                ),
                container_name: Some("good".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_includes_dependency_interface_trait_and_extend_methods_for_workspace_members()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-member-dependency-trait-extend-methods");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "dep"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"

[references]
packages = ["../dep"]
"#,
        );
        temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let trait_symbols =
            workspace_symbols_for_documents(vec![(open_uri.clone(), open_source.clone())], "poll");
        let extend_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "twice");

        assert_eq!(
            trait_symbols,
            vec![SymbolInformation {
                name: "poll".to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(8, 4),
                        tower_lsp::lsp_types::Position::new(8, 24),
                    ),
                ),
                container_name: Some("dep".to_owned()),
            }]
        );
        assert_eq!(
            extend_symbols,
            vec![SymbolInformation {
                name: "twice".to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(16, 8),
                        tower_lsp::lsp_types::Position::new(16, 29),
                    ),
                ),
                container_name: Some("dep".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_available_dependency_methods_when_one_package_interface_is_missing()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-partial-dependency-method");

        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package]
name = "bad"
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../good", "../bad"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "get");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "get".to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(12, 8),
                        tower_lsp::lsp_types::Position::new(12, 27),
                    ),
                ),
                container_name: Some("good".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_available_dependency_methods_when_reference_manifest_is_invalid()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-invalid-reference-manifest-method");

        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package
name = "bad"
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../good", "../bad"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "get");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "get".to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(12, 8),
                        tower_lsp::lsp_types::Position::new(12, 27),
                    ),
                ),
                container_name: Some("good".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_available_member_dependency_symbols_when_one_member_interface_is_missing()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-partial-member-dependency");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "good", "bad"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"

[references]
packages = ["../good", "../bad"]
"#,
        );
        temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub fn exported(value: Int) -> Int
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package]
name = "bad"
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "exported");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "exported".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(7, 4),
                        tower_lsp::lsp_types::Position::new(7, 34),
                    ),
                ),
                container_name: Some("good".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_available_member_dependency_methods_when_one_member_interface_is_missing()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-partial-member-dependency-method");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "good", "bad"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"

[references]
packages = ["../good", "../bad"]
"#,
        );
        temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package]
name = "bad"
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "get");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "get".to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(12, 8),
                        tower_lsp::lsp_types::Position::new(12, 27),
                    ),
                ),
                container_name: Some("good".to_owned()),
            }]
        );
    }

    #[test]
    fn package_analysis_path_keeps_available_dependency_completions_when_one_interface_is_missing()
    {
        let temp = TempDir::new("ql-lsp-package-fallback-partial-dependency");

        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub struct Buffer[T] {
    value: T,
}
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package]
name = "bad"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.good.Bu

fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../good", "../bad"]
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");

        let package =
            package_analysis_for_path(&open_path).expect("fallback package analysis should exist");
        let completions = package
            .dependency_completions_at(&open_source, nth_offset(&open_source, "Bu", 1) + 2)
            .expect("dependency completions should exist");

        assert!(completions.iter().any(|item| {
            item.label == "Buffer"
                && item.kind == AnalysisSymbolKind::Struct
                && item.detail.starts_with("struct Buffer[T] {")
        }));
    }

    #[test]
    fn package_analysis_path_keeps_available_dependency_definitions_for_source_diagnostics() {
        let temp = TempDir::new("ql-lsp-package-fallback-source-diagnostics");

        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub fn exported(value: Int) -> Int
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package]
name = "bad"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.good.exported as run

fn main() -> Int {
    let value: Missing = run(1)
    return value
}
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../good", "../bad"]
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");

        let package =
            package_analysis_for_path(&open_path).expect("fallback package analysis should exist");
        let definition = package
            .dependency_definition_in_source_at(&open_source, nth_offset(&open_source, "run", 2))
            .expect("dependency definition should exist");

        assert_eq!(definition.kind, AnalysisSymbolKind::Function);
        assert_eq!(definition.name, "exported");
        assert!(definition.path.ends_with("good.qi"));
    }

    #[test]
    fn package_analysis_path_keeps_available_dependency_completions_when_one_reference_manifest_is_invalid()
     {
        let temp = TempDir::new("ql-lsp-package-fallback-invalid-reference-manifest");

        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub struct Buffer[T] {
    value: T,
}
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package
name = "bad"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.good.Bu

fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../good", "../bad"]
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");

        let package =
            package_analysis_for_path(&open_path).expect("fallback package analysis should exist");
        let completions = package
            .dependency_completions_at(&open_source, nth_offset(&open_source, "Bu", 1) + 2)
            .expect("dependency completions should exist");

        assert!(completions.iter().any(|item| {
            item.label == "Buffer"
                && item.kind == AnalysisSymbolKind::Struct
                && item.detail.starts_with("struct Buffer[T] {")
        }));
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_available_member_dependency_symbols_when_member_reference_manifest_is_invalid()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-invalid-member-reference-manifest");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "good", "bad"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"

[references]
packages = ["../good", "../bad"]
"#,
        );
        temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub fn exported(value: Int) -> Int
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package
name = "bad"
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "exported");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "exported".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(7, 4),
                        tower_lsp::lsp_types::Position::new(7, 34),
                    ),
                ),
                container_name: Some("good".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_available_member_dependency_methods_when_member_reference_manifest_is_invalid()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-invalid-member-reference-manifest-method");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "good", "bad"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"

[references]
packages = ["../good", "../bad"]
"#,
        );
        temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package
name = "bad"
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "get");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "get".to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(12, 8),
                        tower_lsp::lsp_types::Position::new(12, 27),
                    ),
                ),
                container_name: Some("good".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_member_dependency_trait_and_extend_methods_for_open_packages_when_member_has_source_diagnostics()
     {
        let temp =
            TempDir::new("ql-lsp-workspace-symbol-open-broken-member-trait-and-extend-methods");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "dep"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"

[references]
packages = ["../dep"]
"#,
        );
        temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/tool/src/broken.ql",
            r#"
fn broken() -> Int {
    let value: Int = "oops"
    return value
}
"#,
        );
        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let trait_symbols =
            workspace_symbols_for_documents(vec![(open_uri.clone(), open_source.clone())], "poll");
        let extend_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "twice");

        assert_single_dependency_method_symbol(
            trait_symbols,
            "poll",
            &dependency_interface_path,
            8,
            4,
            24,
            "dep",
        );
        assert_single_dependency_method_symbol(
            extend_symbols,
            "twice",
            &dependency_interface_path,
            16,
            8,
            29,
            "dep",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_dependency_trait_and_extend_methods_for_broken_open_packages()
    {
        let temp =
            TempDir::new("ql-lsp-workspace-symbol-broken-dependency-trait-and-extend-methods");

        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

fn main() -> Int {
    let broken: Int = "oops"
    return 0
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let trait_symbols =
            workspace_symbols_for_documents(vec![(open_uri.clone(), open_source.clone())], "poll");
        let extend_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "twice");

        assert_single_dependency_method_symbol(
            trait_symbols,
            "poll",
            &dependency_interface_path,
            8,
            4,
            24,
            "dep",
        );
        assert_single_dependency_method_symbol(
            extend_symbols,
            "twice",
            &dependency_interface_path,
            16,
            8,
            29,
            "dep",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_available_dependency_trait_and_extend_methods_when_one_package_interface_is_missing()
     {
        let temp =
            TempDir::new("ql-lsp-workspace-symbol-partial-dependency-trait-and-extend-methods");

        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package]
name = "bad"
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../good", "../bad"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let trait_symbols =
            workspace_symbols_for_documents(vec![(open_uri.clone(), open_source.clone())], "poll");
        let extend_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "twice");

        assert_single_dependency_method_symbol(
            trait_symbols,
            "poll",
            &dependency_interface_path,
            8,
            4,
            24,
            "good",
        );
        assert_single_dependency_method_symbol(
            extend_symbols,
            "twice",
            &dependency_interface_path,
            16,
            8,
            29,
            "good",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_available_dependency_trait_and_extend_methods_when_reference_manifest_is_invalid()
     {
        let temp = TempDir::new(
            "ql-lsp-workspace-symbol-invalid-reference-manifest-trait-and-extend-methods",
        );

        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package
name = "bad"
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../good", "../bad"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let trait_symbols =
            workspace_symbols_for_documents(vec![(open_uri.clone(), open_source.clone())], "poll");
        let extend_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "twice");

        assert_single_dependency_method_symbol(
            trait_symbols,
            "poll",
            &dependency_interface_path,
            8,
            4,
            24,
            "good",
        );
        assert_single_dependency_method_symbol(
            extend_symbols,
            "twice",
            &dependency_interface_path,
            16,
            8,
            29,
            "good",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_available_member_dependency_trait_and_extend_methods_when_one_member_interface_is_missing()
     {
        let temp = TempDir::new(
            "ql-lsp-workspace-symbol-partial-member-dependency-trait-and-extend-methods",
        );

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "good", "bad"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"

[references]
packages = ["../good", "../bad"]
"#,
        );
        temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package]
name = "bad"
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let trait_symbols =
            workspace_symbols_for_documents(vec![(open_uri.clone(), open_source.clone())], "poll");
        let extend_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "twice");

        assert_single_dependency_method_symbol(
            trait_symbols,
            "poll",
            &dependency_interface_path,
            8,
            4,
            24,
            "good",
        );
        assert_single_dependency_method_symbol(
            extend_symbols,
            "twice",
            &dependency_interface_path,
            16,
            8,
            29,
            "good",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_available_member_dependency_trait_and_extend_methods_when_member_reference_manifest_is_invalid()
     {
        let temp = TempDir::new(
            "ql-lsp-workspace-symbol-invalid-member-reference-manifest-trait-and-extend-methods",
        );

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "good", "bad"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"

[references]
packages = ["../good", "../bad"]
"#,
        );
        temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package
name = "bad"
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let trait_symbols =
            workspace_symbols_for_documents(vec![(open_uri.clone(), open_source.clone())], "poll");
        let extend_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "twice");

        assert_single_dependency_method_symbol(
            trait_symbols,
            "poll",
            &dependency_interface_path,
            8,
            4,
            24,
            "good",
        );
        assert_single_dependency_method_symbol(
            extend_symbols,
            "twice",
            &dependency_interface_path,
            16,
            8,
            29,
            "good",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_uses_workspace_roots_without_open_documents() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-roots");
        let workspace_root = temp.path().join("workspace");
        let helper_path = temp.write(
            "workspace/packages/tool/src/helper.ql",
            r#"
package demo.tool

pub fn helper() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

pub fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/packages/tool/qlang.toml",
            r#"
[package]
name = "tool"
"#,
        );

        let symbols =
            workspace_symbols_for_documents_and_roots(Vec::new(), &[workspace_root], "helper");

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "helper");
        assert_eq!(
            symbols[0]
                .location
                .uri
                .to_file_path()
                .expect("workspace symbol path should convert")
                .canonicalize()
                .expect("workspace symbol path should canonicalize"),
            helper_path
                .canonicalize()
                .expect("helper path should canonicalize"),
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_uses_workspace_roots_and_prefers_local_dependency_source_symbols() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-root-local-dependency-source");
        let workspace_root = temp.path().join("workspace");
        let dependency_source_path = temp.write(
            "workspace/vendor/dep/src/lib.ql",
            r#"
package demo.dep

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[dependencies]
dep = { path = "../../vendor/dep" }
"#,
        );
        temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.dep.exported as run

pub fn main() -> Int {
    return run(1)
}
"#,
        );
        temp.write(
            "workspace/vendor/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        temp.write(
            "workspace/vendor/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported(value: Int) -> Int
"#,
        );

        let dependency_source =
            fs::read_to_string(&dependency_source_path).expect("dependency source should read");
        let symbols =
            workspace_symbols_for_documents_and_roots(Vec::new(), &[workspace_root], "exported");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "exported".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(&dependency_source_path)
                        .expect("dependency source path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        offset_to_position(
                            &dependency_source,
                            nth_offset(&dependency_source, "exported", 1),
                        ),
                        offset_to_position(
                            &dependency_source,
                            nth_offset(&dependency_source, "exported", 1) + "exported".len(),
                        ),
                    ),
                ),
                container_name: None,
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_uses_workspace_roots_and_prefers_local_dependency_source_methods() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-root-local-dependency-source-method");
        let workspace_root = temp.path().join("workspace");
        let dependency_source_path = temp.write(
            "workspace/vendor/dep/src/lib.ql",
            r#"
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int {
        return self.value
    }
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[dependencies]
dep = { path = "../../vendor/dep" }
"#,
        );
        temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.dep.Config as Cfg

pub fn main(config: Cfg) -> Int {
    return config.get()
}
"#,
        );
        temp.write(
            "workspace/vendor/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        temp.write(
            "workspace/vendor/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
        );

        let dependency_source =
            fs::read_to_string(&dependency_source_path).expect("dependency source should read");
        let symbols =
            workspace_symbols_for_documents_and_roots(Vec::new(), &[workspace_root], "get");

        assert_single_source_symbol(
            symbols,
            "get",
            SymbolKind::METHOD,
            &dependency_source_path,
            &dependency_source,
            1,
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_uses_workspace_roots_and_prefers_local_dependency_source_trait_and_extend_methods()
     {
        let temp = TempDir::new(
            "ql-lsp-workspace-symbol-root-local-dependency-source-trait-extend-methods",
        );
        let workspace_root = temp.path().join("workspace");
        let dependency_source_path = temp.write(
            "workspace/vendor/dep/src/lib.ql",
            r#"
package demo.dep

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int {
        return 2
    }
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[dependencies]
dep = { path = "../../vendor/dep" }
"#,
        );
        temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
pub fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/vendor/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        temp.write(
            "workspace/vendor/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
}
"#,
        );

        let dependency_source =
            fs::read_to_string(&dependency_source_path).expect("dependency source should read");
        let trait_symbols = workspace_symbols_for_documents_and_roots(
            Vec::new(),
            &[workspace_root.clone()],
            "poll",
        );
        let extend_symbols =
            workspace_symbols_for_documents_and_roots(Vec::new(), &[workspace_root], "twice");

        assert_single_source_symbol(
            trait_symbols,
            "poll",
            SymbolKind::METHOD,
            &dependency_source_path,
            &dependency_source,
            1,
        );
        assert_single_source_symbol(
            extend_symbols,
            "twice",
            SymbolKind::METHOD,
            &dependency_source_path,
            &dependency_source,
            1,
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_uses_workspace_roots_and_prefers_local_dependency_source_symbols_for_broken_members()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-root-broken-local-dependency-source");
        let workspace_root = temp.path().join("workspace");
        let dependency_source_path = temp.write(
            "workspace/vendor/dep/src/lib.ql",
            r#"
package demo.dep

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[dependencies]
dep = { path = "../../vendor/dep" }
"#,
        );
        temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.dep.exported as run

pub fn main() -> Int {
    let broken: Int = "oops"
    return run(1)
}
"#,
        );
        temp.write(
            "workspace/vendor/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        temp.write(
            "workspace/vendor/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported(value: Int) -> Int
"#,
        );

        let dependency_source =
            fs::read_to_string(&dependency_source_path).expect("dependency source should read");
        let symbols =
            workspace_symbols_for_documents_and_roots(Vec::new(), &[workspace_root], "exported");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "exported".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(&dependency_source_path)
                        .expect("dependency source path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        offset_to_position(
                            &dependency_source,
                            nth_offset(&dependency_source, "exported", 1),
                        ),
                        offset_to_position(
                            &dependency_source,
                            nth_offset(&dependency_source, "exported", 1) + "exported".len(),
                        ),
                    ),
                ),
                container_name: None,
            }]
        );
    }

    #[test]
    fn workspace_import_definition_prefers_workspace_member_source_over_interface_artifact() {
        let temp = TempDir::new("ql-lsp-workspace-import-source-definition");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    return run(1)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let definition = workspace_source_definition_for_import(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        )
        .expect("workspace import definition should exist");

        let GotoDefinitionResponse::Scalar(location) = definition else {
            panic!("workspace import definition should resolve to one location")
        };
        assert_eq!(
            location
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
    }

    #[test]
    fn local_dependency_import_definition_prefers_dependency_source_over_interface_artifact() {
        let temp = TempDir::new("ql-lsp-local-dependency-import-source-definition");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.dep.exported as run

pub fn main() -> Int {
    return run(1)
}
"#,
        );
        let dep_source_path = temp.write(
            "workspace/vendor/dep/src/lib.ql",
            r#"
package demo.dep

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../../vendor/dep"]
"#,
        );
        temp.write(
            "workspace/vendor/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        temp.write(
            "workspace/vendor/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let definition = workspace_source_definition_for_import(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        )
        .expect("local dependency import definition should exist");

        let GotoDefinitionResponse::Scalar(location) = definition else {
            panic!("local dependency import definition should resolve to one location")
        };
        assert_eq!(
            location
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            dep_source_path
                .canonicalize()
                .expect("dependency source path should canonicalize"),
        );
    }

    #[test]
    fn workspace_import_hover_prefers_workspace_member_source_over_interface_artifact() {
        let temp = TempDir::new("ql-lsp-workspace-import-source-hover");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int, extra: Int) -> Int {
    return value + extra
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let hover = workspace_source_hover_for_import(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 1)),
        )
        .expect("workspace import hover should exist");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("hover should use markdown")
        };
        assert!(
            markup
                .value
                .contains("fn exported(value: Int, extra: Int) -> Int")
        );
        assert!(!markup.value.contains("fn exported(value: Int) -> Int"));
    }

    #[test]
    fn local_dependency_import_semantic_tokens_prefer_dependency_symbol_kinds() {
        let temp = TempDir::new("ql-lsp-local-dependency-import-semantic-tokens");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Command as Cmd
use demo.core.Config as Cfg

pub fn main(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2 }
    let command = Cmd.Retry(1)
    return built.value + config.value + command.unwrap_or(0)
}
"#,
        );
        temp.write(
            "workspace/vendor/core/src/lib.ql",
            r#"
package demo.core

pub enum Command {
    Retry(Int),
}

impl Command {
    pub fn unwrap_or(self, fallback: Int) -> Int {
        match self {
            Command.Retry(value) => value,
        }
    }
}

pub struct Config {
    value: Int,
    limit: Int,
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[dependencies]
core = { path = "../../vendor/core" }
"#,
        );
        temp.write(
            "workspace/vendor/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/vendor/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub enum Command {
    Retry(Int),
}

pub struct Config {
    value: Int,
    limit: Int,
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let SemanticTokensResult::Tokens(tokens) =
            semantic_tokens_for_workspace_package_analysis(&uri, &source, &analysis, &package)
        else {
            panic!("expected full semantic tokens")
        };
        let decoded = decode_semantic_tokens(&tokens.data);
        let legend = semantic_tokens_legend();
        let namespace_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::NAMESPACE)
            .expect("namespace legend entry should exist") as u32;
        let class_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::CLASS)
            .expect("class legend entry should exist") as u32;
        let enum_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::ENUM)
            .expect("enum legend entry should exist") as u32;

        for (needle, occurrence, token_type) in [
            ("Cfg", 1usize, class_type),
            ("Cfg", 2usize, class_type),
            ("Cfg", 3usize, class_type),
            ("Cmd", 1usize, enum_type),
            ("Cmd", 2usize, enum_type),
        ] {
            let span = Span::new(
                nth_offset(&source, needle, occurrence),
                nth_offset(&source, needle, occurrence) + needle.len(),
            );
            let range = span_to_range(&source, span);
            assert!(decoded.contains(&(
                range.start.line,
                range.start.character,
                range.end.character - range.start.character,
                token_type,
            )));
        }

        for (needle, occurrence) in [("Cfg", 1usize), ("Cmd", 1usize)] {
            let span = Span::new(
                nth_offset(&source, needle, occurrence),
                nth_offset(&source, needle, occurrence) + needle.len(),
            );
            let range = span_to_range(&source, span);
            assert!(!decoded.contains(&(
                range.start.line,
                range.start.character,
                range.end.character - range.start.character,
                namespace_type,
            )));
        }
    }

    #[test]
    fn workspace_import_semantic_tokens_prefer_workspace_member_symbol_kinds() {
        let temp = TempDir::new("ql-lsp-workspace-import-semantic-tokens");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Command as Cmd
use demo.core.Config as Cfg

pub fn main(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2 }
    let command = Cmd.Retry(1)
    return built.value
}
"#,
        );
        temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub enum Command {
    Retry(Int),
}

pub struct Config {
    value: Int,
    limit: Int,
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub enum Command {
    Retry(Int),
}

pub struct Config {
    value: Int,
    limit: Int,
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let SemanticTokensResult::Tokens(tokens) =
            semantic_tokens_for_workspace_package_analysis(&uri, &source, &analysis, &package)
        else {
            panic!("expected full semantic tokens")
        };
        let decoded = decode_semantic_tokens(&tokens.data);
        let legend = semantic_tokens_legend();
        let namespace_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::NAMESPACE)
            .expect("namespace legend entry should exist") as u32;
        let class_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::CLASS)
            .expect("class legend entry should exist") as u32;
        let enum_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::ENUM)
            .expect("enum legend entry should exist") as u32;

        for (needle, occurrence, token_type) in [
            ("Cfg", 1usize, class_type),
            ("Cfg", 2usize, class_type),
            ("Cfg", 3usize, class_type),
            ("Cmd", 1usize, enum_type),
            ("Cmd", 2usize, enum_type),
        ] {
            let span = Span::new(
                nth_offset(&source, needle, occurrence),
                nth_offset(&source, needle, occurrence) + needle.len(),
            );
            let range = span_to_range(&source, span);
            assert!(decoded.contains(&(
                range.start.line,
                range.start.character,
                range.end.character - range.start.character,
                token_type,
            )));
        }

        for (needle, occurrence) in [("Cfg", 1usize), ("Cmd", 1usize)] {
            let span = Span::new(
                nth_offset(&source, needle, occurrence),
                nth_offset(&source, needle, occurrence) + needle.len(),
            );
            let range = span_to_range(&source, span);
            assert!(!decoded.contains(&(
                range.start.line,
                range.start.character,
                range.end.character - range.start.character,
                namespace_type,
            )));
        }
    }

    #[test]
    fn workspace_import_semantic_tokens_survive_parse_errors() {
        let temp = TempDir::new("ql-lsp-workspace-import-semantic-tokens-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Command as Cmd
use demo.core.Config as Cfg

pub fn main(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2
    let command = Cmd.Retry(1)
    return built.value
"#,
        );
        temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub enum Command {
    Retry(Int),
}

pub struct Config {
    value: Int,
    limit: Int,
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub enum Command {
    Retry(Int),
}

pub struct Config {
    value: Int,
    limit: Int,
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let SemanticTokensResult::Tokens(tokens) =
            semantic_tokens_for_workspace_dependency_fallback(&uri, &source, &package)
        else {
            panic!("expected full semantic tokens")
        };
        let decoded = decode_semantic_tokens(&tokens.data);
        let legend = semantic_tokens_legend();
        let class_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::CLASS)
            .expect("class legend entry should exist") as u32;
        let enum_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::ENUM)
            .expect("enum legend entry should exist") as u32;

        for (needle, occurrence, token_type) in [
            ("Cfg", 1usize, class_type),
            ("Cfg", 2usize, class_type),
            ("Cfg", 3usize, class_type),
            ("Cmd", 1usize, enum_type),
            ("Cmd", 2usize, enum_type),
        ] {
            let span = Span::new(
                nth_offset(&source, needle, occurrence),
                nth_offset(&source, needle, occurrence) + needle.len(),
            );
            let range = span_to_range(&source, span);
            assert!(decoded.contains(&(
                range.start.line,
                range.start.character,
                range.end.character - range.start.character,
                token_type,
            )));
        }
    }

    #[test]
    fn workspace_type_import_type_definition_prefers_workspace_member_source_over_interface_artifact()
     {
        let temp = TempDir::new("ql-lsp-workspace-type-import-source-type-definition");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config

pub fn main(value: Config) -> Config {
    return value
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
    extra: Int,
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let definition = workspace_source_type_definition_for_import(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, nth_offset(&source, "Config", 2)),
        )
        .expect("workspace import type definition should exist");

        let GotoTypeDefinitionResponse::Scalar(location) = definition else {
            panic!("workspace import type definition should resolve to one location")
        };
        assert_eq!(
            location
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
    }

    #[test]
    fn workspace_import_definition_survives_parse_errors_and_prefers_workspace_member_source() {
        let temp = TempDir::new("ql-lsp-workspace-import-source-definition-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    let next = run(1)
    return next
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let definition = workspace_source_definition_for_import_in_broken_source(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        )
        .expect("broken-source workspace import definition should exist");

        let GotoDefinitionResponse::Scalar(location) = definition else {
            panic!("workspace import definition should resolve to one location")
        };
        assert_eq!(
            location
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
    }

    #[test]
    fn workspace_type_import_type_definition_survives_parse_errors_and_prefers_workspace_member_source()
     {
        let temp = TempDir::new("ql-lsp-workspace-type-import-source-type-definition-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config

pub fn main(value: Config) -> Config {
    return value
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let definition = workspace_source_type_definition_for_import_in_broken_source(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "Config", 2)),
        )
        .expect("broken-source workspace import type definition should exist");

        let GotoTypeDefinitionResponse::Scalar(location) = definition else {
            panic!("workspace import type definition should resolve to one location")
        };
        assert_eq!(
            location
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
    }

    #[test]
    fn workspace_type_import_definition_survives_parse_errors_and_keeps_type_context() {
        let temp = TempDir::new("ql-lsp-workspace-type-import-source-definition-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config

pub fn main(value: Config) -> Config {
    return Config { value
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let definition = workspace_source_definition_for_import_in_broken_source(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "Config", 2)),
        )
        .expect("broken-source workspace type import definition should exist");

        let GotoDefinitionResponse::Scalar(location) = definition else {
            panic!("workspace import definition should resolve to one location")
        };
        assert_eq!(
            location
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
    }

    #[test]
    fn workspace_import_hover_survives_parse_errors_and_prefers_workspace_member_source() {
        let temp = TempDir::new("ql-lsp-workspace-import-source-hover-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    let next = run(1)
    return next
"#,
        );
        temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int, extra: Int) -> Int {
    return value + extra
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let hover = workspace_source_hover_for_import_in_broken_source(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        )
        .expect("broken-source workspace import hover should exist");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("hover should use markdown")
        };
        assert!(
            markup
                .value
                .contains("fn exported(value: Int, extra: Int) -> Int")
        );
        assert!(!markup.value.contains("fn exported(value: Int) -> Int"));
    }

    #[test]
    fn workspace_import_references_prefer_workspace_member_source_over_interface_artifact() {
        let temp = TempDir::new("ql-lsp-workspace-import-source-references");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    return run(1)
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.exported as call

pub fn task() -> Int {
    return call(2)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

        let references = workspace_source_references_for_import(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            true,
        )
        .expect("workspace import references should exist");
        assert_eq!(references.len(), 6);
        assert_eq!(
            references[0]
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            references[0].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 1)),
        );
        assert_eq!(references[1].uri, uri);
        assert_eq!(
            references[1].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 1)),
        );
        assert_eq!(references[2].uri, uri);
        assert_eq!(
            references[2].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        );
        assert_eq!(
            references[3]
                .uri
                .to_file_path()
                .expect("source reference URI should convert to a file path")
                .canonicalize()
                .expect("source reference path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            references[3].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 2)),
        );
        assert_eq!(references[4].uri, task_uri);
        assert_eq!(
            references[4].range.start,
            offset_to_position(&task_source, nth_offset(&task_source, "call", 1)),
        );
        assert_eq!(references[5].uri, task_uri);
        assert_eq!(
            references[5].range.start,
            offset_to_position(&task_source, nth_offset(&task_source, "call", 2)),
        );
    }

    #[test]
    fn local_dependency_import_references_prefer_dependency_source_over_interface_artifact() {
        let temp = TempDir::new("ql-lsp-local-dependency-import-source-references");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    return run(1)
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.exported as call

pub fn task() -> Int {
    return call(2)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/vendor/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[dependencies]
core = { path = "../../vendor/core" }
"#,
        );
        temp.write(
            "workspace/vendor/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/vendor/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

        let references = workspace_source_references_for_import(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            true,
        )
        .expect("local dependency import references should exist");
        assert_eq!(references.len(), 6);
        assert_eq!(
            references[0]
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            references[0].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 1)),
        );
        assert_eq!(references[1].uri, uri);
        assert_eq!(
            references[1].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 1)),
        );
        assert_eq!(references[2].uri, uri);
        assert_eq!(
            references[2].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        );
        assert_eq!(
            references[3]
                .uri
                .to_file_path()
                .expect("source reference URI should convert to a file path")
                .canonicalize()
                .expect("source reference path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            references[3].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 2)),
        );
        assert_eq!(references[4].uri, task_uri);
        assert_eq!(
            references[4].range.start,
            offset_to_position(&task_source, nth_offset(&task_source, "call", 1)),
        );
        assert_eq!(references[5].uri, task_uri);
        assert_eq!(
            references[5].range.start,
            offset_to_position(&task_source, nth_offset(&task_source, "call", 2)),
        );
    }

    #[test]
    fn workspace_import_references_without_declaration_include_other_workspace_uses() {
        let temp = TempDir::new("ql-lsp-workspace-import-source-references-no-decl");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    return run(1)
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.exported as call

pub fn task() -> Int {
    return call(2)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

        let references = workspace_source_references_for_import(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            false,
        )
        .expect("workspace import references without declaration should exist");

        assert_eq!(references.len(), 3);
        assert_eq!(references[0].uri, uri);
        assert_eq!(
            references[0].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        );
        assert_eq!(
            references[1]
                .uri
                .to_file_path()
                .expect("source reference URI should convert to a file path")
                .canonicalize()
                .expect("source reference path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            references[1].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 2)),
        );
        assert_eq!(references[2].uri, task_uri);
        assert_eq!(
            references[2].range.start,
            offset_to_position(&task_source, nth_offset(&task_source, "call", 2)),
        );
    }

    #[test]
    fn workspace_import_references_survive_parse_errors_and_prefer_workspace_member_source() {
        let temp = TempDir::new("ql-lsp-workspace-import-source-references-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    let first = run(1)
    let second = run(first)
    return second
"#,
        );
        temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.exported as call

pub fn task() -> Int {
    return call(2)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let references = workspace_source_references_for_import_in_broken_source(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            true,
        )
        .expect("broken-source workspace import references should exist");

        assert_eq!(references.len(), 4);
        assert_eq!(
            references[0]
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            references[0].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 1)),
        );
        assert_eq!(references[1].uri, uri);
        assert_eq!(
            references[1].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        );
        assert_eq!(references[2].uri, uri);
        assert_eq!(
            references[2].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 3)),
        );
        assert_eq!(
            references[3]
                .uri
                .to_file_path()
                .expect("source reference URI should convert to a file path")
                .canonicalize()
                .expect("source reference path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            references[3].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 2)),
        );
    }

    #[test]
    fn local_dependency_import_references_survive_parse_errors_and_prefer_dependency_source() {
        let temp = TempDir::new("ql-lsp-local-dependency-import-source-references-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    let first = run(1)
    let second = run(first)
    return second
"#,
        );
        let core_source_path = temp.write(
            "workspace/vendor/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[dependencies]
core = { path = "../../vendor/core" }
"#,
        );
        temp.write(
            "workspace/vendor/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/vendor/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let references = workspace_source_references_for_import_in_broken_source(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            true,
        )
        .expect("broken-source local dependency import references should exist");

        assert_eq!(references.len(), 4);
        assert_eq!(
            references[0]
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            references[0].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 1)),
        );
        assert_eq!(references[1].uri, uri);
        assert_eq!(
            references[1].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        );
        assert_eq!(references[2].uri, uri);
        assert_eq!(
            references[2].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 3)),
        );
        assert_eq!(
            references[3]
                .uri
                .to_file_path()
                .expect("source reference URI should convert to a file path")
                .canonicalize()
                .expect("source reference path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            references[3].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 2)),
        );
    }

    #[test]
    fn workspace_import_references_without_declaration_survive_parse_errors() {
        let temp = TempDir::new("ql-lsp-workspace-import-source-references-parse-errors-no-decl");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config

pub fn main(value: Config) -> Config {
    let current: Config = Config { value: 1
    return current
"#,
        );
        temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let references = workspace_source_references_for_import_in_broken_source(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "Config", 2)),
            false,
        )
        .expect("broken-source workspace import references without declaration should exist");

        assert_eq!(references.len(), 4);
        assert!(references.iter().all(|location| location.uri == uri));
        assert_eq!(
            references[0].range.start,
            offset_to_position(&source, nth_offset(&source, "Config", 2)),
        );
        assert_eq!(
            references[1].range.start,
            offset_to_position(&source, nth_offset(&source, "Config", 3)),
        );
        assert_eq!(
            references[2].range.start,
            offset_to_position(&source, nth_offset(&source, "Config", 4)),
        );
        assert_eq!(
            references[3].range.start,
            offset_to_position(&source, nth_offset(&source, "Config", 5)),
        );
    }

    #[test]
    fn workspace_import_prepare_rename_survives_parse_errors() {
        let temp = TempDir::new("ql-lsp-workspace-import-prepare-rename-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported

pub fn main() -> Int {
    let first = exported(1)
    let second = exported(first)
    return second
"#,
        );
        temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let use_offset = nth_offset(&source, "exported", 2);

        assert_eq!(
            prepare_rename_for_workspace_import_in_broken_source(
                &uri,
                &source,
                &package,
                offset_to_position(&source, use_offset),
            ),
            Some(PrepareRenameResponse::RangeWithPlaceholder {
                range: span_to_range(
                    &source,
                    Span::new(use_offset, use_offset + "exported".len()),
                ),
                placeholder: "exported".to_owned(),
            }),
        );
    }

    #[test]
    fn workspace_import_rename_survives_parse_errors_and_inserts_alias() {
        let temp = TempDir::new("ql-lsp-workspace-import-rename-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported

pub fn main() -> Int {
    let first = exported(1)
    let second = exported(first)
    return second
"#,
        );
        temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let use_offset = nth_offset(&source, "exported", 2);

        let edit = rename_for_workspace_import_in_broken_source(
            &uri,
            &source,
            &package,
            offset_to_position(&source, use_offset),
            "run",
        )
        .expect("rename should validate")
        .expect("broken-source workspace import rename should produce edits");

        assert_workspace_edit(
            edit,
            &uri,
            vec![
                TextEdit::new(
                    span_to_range(
                        &source,
                        Span::new(
                            nth_offset(&source, "exported", 1),
                            nth_offset(&source, "exported", 1) + "exported".len(),
                        ),
                    ),
                    "exported as run".to_owned(),
                ),
                TextEdit::new(
                    span_to_range(
                        &source,
                        Span::new(
                            nth_offset(&source, "exported", 2),
                            nth_offset(&source, "exported", 2) + "exported".len(),
                        ),
                    ),
                    "run".to_owned(),
                ),
                TextEdit::new(
                    span_to_range(
                        &source,
                        Span::new(
                            nth_offset(&source, "exported", 3),
                            nth_offset(&source, "exported", 3) + "exported".len(),
                        ),
                    ),
                    "run".to_owned(),
                ),
            ],
        );

        assert_eq!(
            rename_for_workspace_import_in_broken_source(
                &uri,
                &source,
                &package,
                offset_to_position(&source, use_offset),
                "match",
            ),
            Err(RenameError::Keyword("match".to_owned())),
        );
    }

    #[test]
    fn workspace_dependency_definitions_prefer_workspace_member_source_over_interface_artifact() {
        let temp = TempDir::new("ql-lsp-workspace-dependency-source-definitions");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Command as Cmd
use demo.core.Config as Cfg
use demo.core.exported as run

pub fn main(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2 }
    let command = Cmd.Retry(1)
    let result = config.ping()
    return run(result) + built.value + command.unwrap_or(0)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub enum Command {
    Retry(Int),
}

impl Command {
    pub fn unwrap_or(self, fallback: Int) -> Int {
        match self {
            Command.Retry(value) => value,
        }
    }
}

pub struct Config {
    value: Int,
    limit: Int,
}

impl Config {
    pub fn ping(self) -> Int {
        return self.value + self.limit
    }

    pub fn use_ping(self) -> Int {
        return self.ping()
    }
}

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub enum Command {
    Retry(Int),
}

impl Command {
    pub fn unwrap_or(self, fallback: Int) -> Int
}

pub struct Config {
    value: Int,
    limit: Int,
}

impl Config {
    pub fn ping(self) -> Int
}

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");

        for (needle, occurrence, expected_symbol, expected_occurrence) in [
            ("run", 2usize, "exported", 1usize),
            ("Retry", 1usize, "Retry", 1usize),
            ("ping", 1usize, "ping", 1usize),
            ("value", 2usize, "value", 3usize),
        ] {
            let definition = workspace_source_definition_for_dependency(
                &uri,
                &source,
                Some(&analysis),
                &package,
                offset_to_position(&source, nth_offset(&source, needle, occurrence)),
            )
            .unwrap_or_else(|| panic!("workspace dependency definition should exist for {needle}"));

            let GotoDefinitionResponse::Scalar(location) = definition else {
                panic!("workspace dependency definition should resolve to one location")
            };
            assert_eq!(
                location
                    .uri
                    .to_file_path()
                    .expect("definition URI should convert to a file path")
                    .canonicalize()
                    .expect("definition path should canonicalize"),
                core_source_path
                    .canonicalize()
                    .expect("core source path should canonicalize"),
            );
            assert_eq!(
                location.range.start,
                offset_to_position(
                    &core_source,
                    nth_offset(&core_source, expected_symbol, expected_occurrence)
                ),
            );
        }
    }

    #[test]
    fn workspace_dependency_value_queries_survive_parse_errors_and_prefer_workspace_member_source()
    {
        let temp = TempDir::new("ql-lsp-workspace-dependency-value-source-queries-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config as Cfg

pub fn main(config: Cfg) -> Int {
    let current = config
    return current.value
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
    extra: Int,
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let current_position = offset_to_position(&source, nth_offset(&source, "current", 2));

        let hover =
            workspace_source_hover_for_dependency(&uri, &source, None, &package, current_position)
                .expect("broken-source workspace dependency hover should exist");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("hover should use markdown")
        };
        assert!(markup.value.contains("struct Config"));

        let definition = workspace_source_definition_for_dependency(
            &uri,
            &source,
            None,
            &package,
            current_position,
        )
        .expect("broken-source workspace dependency definition should exist");
        let GotoDefinitionResponse::Scalar(definition_location) = definition else {
            panic!("workspace dependency definition should resolve to one location")
        };
        assert_eq!(
            definition_location
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );

        let type_definition = workspace_source_type_definition_for_dependency(
            &uri,
            &source,
            None,
            &package,
            current_position,
        )
        .expect("broken-source workspace dependency type definition should exist");
        let GotoTypeDefinitionResponse::Scalar(type_location) = type_definition else {
            panic!("workspace dependency type definition should resolve to one location")
        };
        assert_eq!(
            type_location
                .uri
                .to_file_path()
                .expect("type definition URI should convert to a file path")
                .canonicalize()
                .expect("type definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
    }

    #[test]
    fn workspace_dependency_type_definitions_prefer_workspace_member_source_over_interface_artifact()
     {
        let temp = TempDir::new("ql-lsp-workspace-dependency-source-type-definitions");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Command as Cmd
use demo.core.Config as Cfg
use demo.core.Holder as Hold

pub fn main(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2 }
    let holder = Hold { child: config.clone_self() }
    let command = Cmd.Retry(1)
    return holder.child.value + built.value + command.unwrap_or(0)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub enum Command {
    Retry(Int),
}

pub struct Config {
    value: Int,
    limit: Int,
}

pub struct Holder {
    child: Config,
}

impl Command {
    pub fn unwrap_or(self, fallback: Int) -> Int {
        match self {
            Command.Retry(value) => value,
        }
    }
}

impl Config {
    pub fn clone_self(self) -> Config {
        return self
    }
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub enum Command {
    Retry(Int),
}

pub struct Config {
    value: Int,
    limit: Int,
}

pub struct Holder {
    child: Config,
}

impl Command {
    pub fn unwrap_or(self, fallback: Int) -> Int
}

impl Config {
    pub fn clone_self(self) -> Config
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");

        for (needle, occurrence, expected_symbol, expected_occurrence) in [
            ("Cfg", 2usize, "Config", 1usize),
            ("built", 2usize, "Config", 1usize),
            ("clone_self", 1usize, "Config", 1usize),
            ("Retry", 1usize, "Command", 1usize),
            ("child", 2usize, "Config", 1usize),
        ] {
            let definition = workspace_source_type_definition_for_dependency(
                &uri,
                &source,
                Some(&analysis),
                &package,
                offset_to_position(&source, nth_offset(&source, needle, occurrence)),
            )
            .unwrap_or_else(|| {
                panic!("workspace dependency type definition should exist for {needle}")
            });

            let GotoTypeDefinitionResponse::Scalar(location) = definition else {
                panic!("workspace dependency type definition should resolve to one location")
            };
            assert_eq!(
                location
                    .uri
                    .to_file_path()
                    .expect("definition URI should convert to a file path")
                    .canonicalize()
                    .expect("definition path should canonicalize"),
                core_source_path
                    .canonicalize()
                    .expect("core source path should canonicalize"),
            );
            assert_eq!(
                location.range.start,
                offset_to_position(
                    &core_source,
                    nth_offset(&core_source, expected_symbol, expected_occurrence)
                ),
            );
        }
    }

    #[test]
    fn workspace_dependency_references_prefer_workspace_member_source_over_interface_artifact() {
        let temp = TempDir::new("ql-lsp-workspace-dependency-source-references");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Command as Cmd
use demo.core.Config as Cfg
use demo.core.exported as run

pub fn main(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2 }
    let command = Cmd.Retry(1)
    let result = config.ping()
    return run(result) + built.value + command.unwrap_or(0)
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.Command as OtherCmd
use demo.core.Config as OtherCfg
use demo.core.exported as call

pub fn task(config: OtherCfg) -> Int {
    let command = OtherCmd.Retry(2)
    let result = config.ping()
    return call(result) + config.value + command.unwrap_or(0)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub enum Command {
    Retry(Int),
}

impl Command {
    pub fn unwrap_or(self, fallback: Int) -> Int {
        match self {
            Command.Retry(value) => value,
        }
    }
}

pub struct Config {
    value: Int,
    limit: Int,
}

impl Config {
    pub fn ping(self) -> Int {
        return self.value + self.limit
    }

    pub fn use_ping(self) -> Int {
        return self.ping()
    }
}

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub enum Command {
    Retry(Int),
}

impl Command {
    pub fn unwrap_or(self, fallback: Int) -> Int
}

pub struct Config {
    value: Int,
    limit: Int,
}

impl Config {
    pub fn ping(self) -> Int
}

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

        for (
            needle,
            occurrence,
            expected_symbol,
            expected_occurrence,
            expected_count,
            local_occurrences,
            source_occurrence,
            task_needle,
            task_occurrences,
        ) in [
            (
                "Retry",
                1usize,
                "Retry",
                1usize,
                4usize,
                vec![1usize],
                Some(2usize),
                "Retry",
                vec![1usize],
            ),
            (
                "ping",
                1usize,
                "ping",
                1usize,
                4usize,
                vec![1usize],
                Some(3usize),
                "ping",
                vec![1usize],
            ),
            (
                "value",
                2usize,
                "value",
                3usize,
                5usize,
                vec![1usize, 2usize],
                Some(4usize),
                "value",
                vec![1usize],
            ),
            (
                "run",
                2usize,
                "exported",
                1usize,
                6usize,
                vec![1usize, 2usize],
                Some(2usize),
                "call",
                vec![1usize, 2usize],
            ),
        ] {
            let references = workspace_source_references_for_dependency(
                &uri,
                &source,
                Some(&analysis),
                &package,
                offset_to_position(&source, nth_offset(&source, needle, occurrence)),
                true,
            )
            .unwrap_or_else(|| panic!("workspace dependency references should exist for {needle}"));

            assert_eq!(references.len(), expected_count, "{needle}");
            assert_eq!(
                references[0]
                    .uri
                    .to_file_path()
                    .expect("definition URI should convert to a file path")
                    .canonicalize()
                    .expect("definition path should canonicalize"),
                core_source_path
                    .canonicalize()
                    .expect("core source path should canonicalize"),
            );
            assert_eq!(
                references[0].range.start,
                offset_to_position(
                    &core_source,
                    nth_offset(&core_source, expected_symbol, expected_occurrence)
                ),
            );

            for (reference, local_occurrence) in
                references[1..].iter().zip(local_occurrences.into_iter())
            {
                assert_eq!(reference.uri, uri);
                assert_eq!(
                    reference.range.start,
                    offset_to_position(&source, nth_offset(&source, needle, local_occurrence)),
                );
            }

            if let Some(source_occurrence) = source_occurrence {
                assert!(
                    references.iter().any(|reference| {
                        reference
                            .uri
                            .to_file_path()
                            .ok()
                            .and_then(|path| path.canonicalize().ok())
                            == core_source_path.canonicalize().ok()
                            && reference.range.start
                                == offset_to_position(
                                    &core_source,
                                    nth_offset(&core_source, expected_symbol, source_occurrence),
                                )
                    }),
                    "{needle} should include workspace source occurrence",
                );
            }

            for task_occurrence in task_occurrences {
                assert!(
                    references.iter().any(|reference| {
                        reference.uri == task_uri
                            && reference.range.start
                                == offset_to_position(
                                    &task_source,
                                    nth_offset(&task_source, task_needle, task_occurrence),
                                )
                    }),
                    "{needle} should include task file occurrence",
                );
            }
        }
    }

    #[test]
    fn local_dependency_queries_prefer_dependency_source_over_interface_artifact() {
        let temp = TempDir::new("ql-lsp-local-dependency-source-queries");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config as Cfg
use demo.core.exported as run

pub fn main(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2 }
    return run(config.ping()) + built.value
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.Config as OtherCfg
use demo.core.exported as call

pub fn task(config: OtherCfg) -> Int {
    return call(config.ping()) + config.value
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/vendor/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
    limit: Int,
}

impl Config {
    pub fn ping(self) -> Int {
        return self.value + self.limit
    }
}

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[dependencies]
core = { path = "../../vendor/core" }
"#,
        );
        temp.write(
            "workspace/vendor/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/vendor/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
    limit: Int,
}

impl Config {
    pub fn ping(self) -> Int
}

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

        let definition = workspace_source_definition_for_dependency(
            &uri,
            &source,
            Some(&analysis),
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        )
        .expect("local dependency definition should exist");
        let GotoDefinitionResponse::Scalar(definition_location) = definition else {
            panic!("local dependency definition should resolve to one location")
        };
        assert_eq!(
            definition_location
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            definition_location.range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 1)),
        );

        let type_definition = workspace_source_type_definition_for_dependency(
            &uri,
            &source,
            Some(&analysis),
            &package,
            offset_to_position(&source, nth_offset(&source, "Cfg", 2)),
        )
        .expect("local dependency type definition should exist");
        let GotoTypeDefinitionResponse::Scalar(type_location) = type_definition else {
            panic!("local dependency type definition should resolve to one location")
        };
        assert_eq!(
            type_location
                .uri
                .to_file_path()
                .expect("type definition URI should convert to a file path")
                .canonicalize()
                .expect("type definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            type_location.range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "Config", 1)),
        );

        let references = workspace_source_references_for_dependency(
            &uri,
            &source,
            Some(&analysis),
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            true,
        )
        .expect("local dependency references should exist");
        assert_eq!(references.len(), 6);
        assert_eq!(
            references[0]
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            references[0].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 1)),
        );

        for (reference, local_occurrence) in references[1..3].iter().zip([1usize, 2usize]) {
            assert_eq!(reference.uri, uri);
            assert_eq!(
                reference.range.start,
                offset_to_position(&source, nth_offset(&source, "run", local_occurrence)),
            );
        }
        assert_eq!(
            references[3]
                .uri
                .to_file_path()
                .expect("reference URI should convert to a file path")
                .canonicalize()
                .expect("reference path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            references[3].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 2)),
        );
        for (reference, local_occurrence) in references[4..].iter().zip([1usize, 2usize]) {
            assert_eq!(reference.uri, task_uri);
            assert_eq!(
                reference.range.start,
                offset_to_position(
                    &task_source,
                    nth_offset(&task_source, "call", local_occurrence)
                ),
            );
        }
    }

    #[test]
    fn workspace_dependency_references_without_declaration_include_other_workspace_uses() {
        let temp = TempDir::new("ql-lsp-workspace-dependency-source-references-no-decl");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config as Cfg

pub fn main(config: Cfg) -> Int {
    return config.ping()
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.Config as OtherCfg

pub fn task(config: OtherCfg) -> Int {
    return config.ping()
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    pub fn ping(self) -> Int {
        return self.value
    }

    pub fn use_ping(self) -> Int {
        return self.ping()
    }
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    pub fn ping(self) -> Int
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

        let references = workspace_source_references_for_dependency(
            &uri,
            &source,
            Some(&analysis),
            &package,
            offset_to_position(&source, nth_offset(&source, "ping", 1)),
            false,
        )
        .expect("workspace dependency references without declaration should exist");

        assert_eq!(references.len(), 3);
        assert_eq!(references[0].uri, uri);
        assert_eq!(
            references[0].range.start,
            offset_to_position(&source, nth_offset(&source, "ping", 1)),
        );
        assert!(
            references.iter().any(|reference| {
                reference
                    .uri
                    .to_file_path()
                    .ok()
                    .and_then(|path| path.canonicalize().ok())
                    == core_source_path.canonicalize().ok()
                    && reference.range.start
                        == offset_to_position(&core_source, nth_offset(&core_source, "ping", 3))
            }),
            "references should include workspace source method use",
        );
        assert!(
            references.iter().any(|reference| {
                reference.uri == task_uri
                    && reference.range.start
                        == offset_to_position(&task_source, nth_offset(&task_source, "ping", 1))
            }),
            "references should include other workspace file method use",
        );
    }

    #[test]
    fn workspace_dependency_value_references_survive_parse_errors_and_prefer_workspace_member_source()
     {
        let temp = TempDir::new("ql-lsp-workspace-dependency-source-references-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main(value: Int) -> Int {
    let result = run(value)
    return result
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.exported as call

pub fn task(value: Int) -> Int {
    return call(value)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");
        let references = workspace_source_references_for_dependency_in_broken_source(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            true,
        )
        .expect("broken-source workspace dependency value references should exist");

        assert_eq!(references.len(), 5);
        assert_eq!(
            references[0]
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            references[0].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 1)),
        );
        assert_eq!(references[1].uri, uri);
        assert_eq!(
            references[1].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 1)),
        );
        assert_eq!(references[2].uri, uri);
        assert_eq!(
            references[2].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        );
        assert!(
            references.iter().any(|reference| {
                reference.uri == task_uri
                    && reference.range.start
                        == offset_to_position(&task_source, nth_offset(&task_source, "call", 1))
            }),
            "run should include task alias definition",
        );
        assert!(
            references.iter().any(|reference| {
                reference.uri == task_uri
                    && reference.range.start
                        == offset_to_position(&task_source, nth_offset(&task_source, "call", 2))
            }),
            "run should include task call occurrence",
        );
    }

    #[test]
    fn workspace_dependency_value_references_without_declaration_survive_parse_errors() {
        let temp =
            TempDir::new("ql-lsp-workspace-dependency-source-references-parse-errors-no-decl");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main(value: Int) -> Int {
    return run(value
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.exported as call

pub fn task(value: Int) -> Int {
    return call(value)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

        let references = workspace_source_references_for_dependency_in_broken_source(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            false,
        )
        .expect(
            "broken-source workspace dependency value references without declaration should exist",
        );

        assert_eq!(references.len(), 3);
        assert_eq!(references[0].uri, uri);
        assert_eq!(
            references[0].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        );
        assert!(
            references.iter().any(|reference| {
                reference
                    .uri
                    .to_file_path()
                    .ok()
                    .and_then(|path| path.canonicalize().ok())
                    == core_source_path.canonicalize().ok()
                    && reference.range.start
                        == offset_to_position(&core_source, nth_offset(&core_source, "exported", 2))
            }),
            "references should include workspace source occurrence",
        );
        assert!(
            references.iter().any(|reference| {
                reference.uri == task_uri
                    && reference.range.start
                        == offset_to_position(&task_source, nth_offset(&task_source, "call", 2))
            }),
            "references should include other workspace file occurrence",
        );
    }

    #[test]
    fn document_highlight_keeps_same_file_definition_and_usages() {
        let temp = TempDir::new("ql-lsp-document-highlight-same-file");
        let source_path = temp.write(
            "pkg/src/main.ql",
            r#"
pub fn helper() -> Int {
    return 1
}

pub fn main() -> Int {
    let first = helper()
    return helper() + first
}
"#,
        );
        let source = fs::read_to_string(&source_path).expect("source should read");
        let analysis = analyze_source(&source).expect("source should analyze");
        let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");

        let highlights = document_highlights_for_analysis_at(
            &uri,
            &source,
            &analysis,
            offset_to_position(&source, nth_offset(&source, "helper", 2)),
        )
        .expect("same-file document highlight should exist");

        let actual = highlights
            .into_iter()
            .map(|highlight| highlight.range.start)
            .collect::<Vec<_>>();
        let expected = vec![
            offset_to_position(&source, nth_offset(&source, "helper", 1)),
            offset_to_position(&source, nth_offset(&source, "helper", 2)),
            offset_to_position(&source, nth_offset(&source, "helper", 3)),
        ];

        assert_eq!(actual, expected);
    }

    #[test]
    fn document_highlight_keeps_package_import_occurrences_in_current_file() {
        let temp = TempDir::new("ql-lsp-document-highlight-package-import");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    return run(1)
}
"#,
        );
        temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let highlights = document_highlights_for_package_analysis_at(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        )
        .expect("package-aware document highlight should exist");

        let actual = highlights
            .into_iter()
            .map(|highlight| highlight.range.start)
            .collect::<Vec<_>>();
        let expected = vec![
            offset_to_position(&source, nth_offset(&source, "run", 1)),
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        ];

        assert_eq!(actual, expected);
    }

    #[test]
    fn document_highlight_keeps_workspace_import_occurrences_in_broken_source() {
        let temp = TempDir::new("ql-lsp-document-highlight-package-import-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    let first = run(1)
    let second = run(first)
    return second
"#,
        );
        temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let highlights = fallback_document_highlights_for_package_at(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        )
        .expect("broken-source workspace import document highlight should exist");

        let actual = highlights
            .into_iter()
            .map(|highlight| highlight.range.start)
            .collect::<Vec<_>>();
        let expected = vec![
            offset_to_position(&source, nth_offset(&source, "run", 1)),
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            offset_to_position(&source, nth_offset(&source, "run", 3)),
        ];

        assert_eq!(actual, expected);
    }

    #[test]
    fn document_highlight_keeps_dependency_value_occurrences_in_broken_source() {
        let temp = TempDir::new("ql-lsp-document-highlight-dependency-value-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    let first = run(1)
    return run(first)
"#,
        );
        temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.exported as call

pub fn task(value: Int) -> Int {
    return call(value)
}
"#,
        );
        temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let highlights = fallback_document_highlights_for_package_at(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 3)),
        )
        .expect("broken-source dependency value document highlight should exist");

        let actual = highlights
            .into_iter()
            .map(|highlight| highlight.range.start)
            .collect::<Vec<_>>();
        let expected = vec![
            offset_to_position(&source, nth_offset(&source, "run", 1)),
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            offset_to_position(&source, nth_offset(&source, "run", 3)),
        ];

        assert_eq!(actual, expected);
    }

    #[test]
    fn document_highlight_keeps_dependency_structured_root_indexed_value_occurrences_in_broken_source()
     {
        let temp = TempDir::new(
            "ql-lsp-document-highlight-dependency-structured-root-indexed-value-parse-errors",
        );
        let app_path = temp.write(
            "workspace/app/src/lib.ql",
            r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {
    let first = (if flag { maybe_children()? } else { maybe_children()? })[0]
    let second = (match flag { true => maybe_children()?, false => maybe_children()? })[1]
    return first.value + second.value + first.value
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
        );
        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub fn maybe_children() -> Option[[Child; 2]]
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let highlights = fallback_document_highlights_for_package_at(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "first", 3)),
        )
        .expect(
            "broken-source dependency structured root-indexed value document highlight should exist",
        );

        let actual = highlights
            .into_iter()
            .map(|highlight| highlight.range.start)
            .collect::<Vec<_>>();
        let expected = vec![
            offset_to_position(&source, nth_offset(&source, "first", 1)),
            offset_to_position(&source, nth_offset(&source, "first", 2)),
            offset_to_position(&source, nth_offset(&source, "first", 3)),
        ];

        assert_eq!(actual, expected);
    }

    #[test]
    fn dependency_value_prepare_rename_and_rename_survive_structured_root_indexed_parse_errors() {
        let temp =
            TempDir::new("ql-lsp-dependency-value-rename-structured-root-indexed-parse-errors");
        let app_path = temp.write(
            "workspace/app/src/lib.ql",
            r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {
    let first = (if flag { maybe_children()? } else { maybe_children()? })[0]
    let second = (match flag { true => maybe_children()?, false => maybe_children()? })[1]
    return first.value + second.value + first.value
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
        );
        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub fn maybe_children() -> Option[[Child; 2]]
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let use_offset = nth_offset(&source, "first", 2);

        assert_eq!(
            prepare_rename_for_dependency_imports(
                &source,
                &package,
                offset_to_position(&source, use_offset),
            ),
            Some(PrepareRenameResponse::RangeWithPlaceholder {
                range: span_to_range(&source, Span::new(use_offset, use_offset + "first".len())),
                placeholder: "first".to_owned(),
            }),
        );

        let edit = rename_for_dependency_imports(
            &uri,
            &source,
            &package,
            offset_to_position(&source, use_offset),
            "current_child",
        )
        .expect("rename should succeed")
        .expect("rename should return workspace edits");
        let changes = edit
            .changes
            .expect("rename should use simple workspace changes");
        let edits = changes
            .get(&uri)
            .expect("rename should edit current document");
        assert_eq!(
            edits,
            &vec![
                TextEdit::new(
                    span_to_range(
                        &source,
                        Span::new(
                            nth_offset(&source, "first", 1),
                            nth_offset(&source, "first", 1) + "first".len(),
                        ),
                    ),
                    "current_child".to_owned(),
                ),
                TextEdit::new(
                    span_to_range(
                        &source,
                        Span::new(
                            nth_offset(&source, "first", 2),
                            nth_offset(&source, "first", 2) + "first".len(),
                        ),
                    ),
                    "current_child".to_owned(),
                ),
                TextEdit::new(
                    span_to_range(
                        &source,
                        Span::new(
                            nth_offset(&source, "first", 3),
                            nth_offset(&source, "first", 3) + "first".len(),
                        ),
                    ),
                    "current_child".to_owned(),
                ),
            ],
        );
    }

    #[test]
    fn workspace_import_references_skip_non_workspace_dependency_in_broken_source() {
        let temp = TempDir::new("ql-lsp-workspace-import-references-skip-dependency");
        let app_path = temp.write(
            "app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    return run(
"#,
        );
        temp.write(
            "app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "core/core.qi",
            r#"
// qlang interface v1
// package: core
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let references = workspace_source_references_for_import_in_broken_source(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            true,
        )
        .is_none();

        assert!(references);
    }
}
