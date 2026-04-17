use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use ql_analysis::{
    Analysis, DependencyInterface, PackageAnalysisError, analyze_available_package_dependencies,
    analyze_package, analyze_package_with_available_dependencies, analyze_source,
};
use ql_project::{collect_package_sources, load_project_manifest};
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
    TextDocumentSyncOptions, TypeDefinitionProviderCapability, Url, WorkspaceEdit,
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
    rename_for_dependency_imports, semantic_tokens_for_analysis,
    semantic_tokens_for_package_analysis, semantic_tokens_legend, span_to_range,
    type_definition_for_analysis, type_definition_for_dependency_imports,
    type_definition_for_dependency_method_types, type_definition_for_dependency_struct_field_types,
    type_definition_for_dependency_values, type_definition_for_dependency_variants,
    type_definition_for_package_analysis, workspace_symbols_for_analysis,
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
            append_dependency_workspace_symbols(&manifest_path, symbols, query);
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

        let Ok(module_uri) = Url::from_file_path(&module_path) else {
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

        let Ok(source_uri) = Url::from_file_path(&source_path) else {
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

#[allow(deprecated)]
fn append_dependency_workspace_symbols(
    package_path: &Path,
    symbols: &mut Vec<SymbolInformation>,
    query: &str,
) {
    if let Ok(dependencies) = analyze_available_package_dependencies(package_path) {
        symbols.extend(workspace_symbols_for_dependencies(&dependencies, query));
    }
}

#[allow(deprecated)]
fn append_workspace_member_symbols(
    member_manifest_path: &Path,
    open_docs: &HashMap<PathBuf, (Url, String)>,
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
            append_dependency_workspace_symbols(member_manifest_path, symbols, query);
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
            append_dependency_workspace_symbols(member_manifest_path, symbols, query);
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
            append_dependency_workspace_symbols(member_manifest_path, symbols, query);
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
            append_dependency_workspace_symbols(member_manifest_path, symbols, query);
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
                    true,
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

                let workspace_member_manifests =
                    workspace_member_manifest_paths_for_package(manifest.manifest_path.as_path());

                append_dependency_workspace_symbols(&path, &mut symbols, &normalized_query);

                for member_manifest_path in workspace_member_manifests {
                    if !searched_packages.insert(member_manifest_path.clone()) {
                        continue;
                    }
                    append_workspace_member_symbols(
                        &member_manifest_path,
                        &open_docs,
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

                let workspace_member_manifests =
                    workspace_member_manifest_paths_for_package(manifest.manifest_path.as_path());

                append_dependency_workspace_symbols(&path, &mut symbols, &normalized_query);

                for member_manifest_path in workspace_member_manifests {
                    if !searched_packages.insert(member_manifest_path.clone()) {
                        continue;
                    }
                    append_workspace_member_symbols(
                        &member_manifest_path,
                        &open_docs,
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
                append_dependency_workspace_symbols(&path, &mut symbols, &normalized_query);

                for member_manifest_path in
                    workspace_member_manifest_paths_for_package(manifest.manifest_path.as_path())
                {
                    if !searched_packages.insert(member_manifest_path.clone()) {
                        continue;
                    }
                    append_workspace_member_symbols(
                        &member_manifest_path,
                        &open_docs,
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

fn extend_workspace_import_definition_matches(
    package: &ql_analysis::PackageAnalysis,
    current_path: Option<&Path>,
    current_source: Option<&str>,
    current_analysis: Option<&Analysis>,
    import_prefix: &[String],
    imported_name: &str,
    matches: &mut Vec<Location>,
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
            if symbol.name != imported_name || !supports_workspace_import_definition(symbol.kind) {
                continue;
            }
            matches.push(Location::new(
                module_uri.clone(),
                span_to_range(module_source, symbol.span),
            ));
        }
    }
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
    let current_path = uri.to_file_path().ok();
    let mut matches = Vec::new();

    extend_workspace_import_definition_matches(
        package,
        current_path.as_deref(),
        Some(source),
        Some(analysis),
        import_prefix,
        imported_name,
        &mut matches,
    );

    for member_manifest_path in
        workspace_member_manifest_paths_for_package(package.manifest().manifest_path.as_path())
    {
        let Some(member_package) = package_analysis_for_path(&member_manifest_path) else {
            continue;
        };
        extend_workspace_import_definition_matches(
            &member_package,
            None,
            None,
            None,
            import_prefix,
            imported_name,
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
    (matches.len() == 1).then(|| GotoDefinitionResponse::Scalar(matches[0].clone()))
}

fn workspace_source_references_for_import(
    uri: &Url,
    source: &str,
    analysis: &Analysis,
    package: &ql_analysis::PackageAnalysis,
    position: tower_lsp::lsp_types::Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    if !include_declaration {
        return None;
    }

    let GotoDefinitionResponse::Scalar(source_definition) =
        workspace_source_definition_for_import(uri, source, analysis, package, position)?
    else {
        return None;
    };

    let mut locations =
        references_for_package_analysis(uri, source, analysis, package, position, true)?;
    if let Some(existing_index) = locations
        .iter()
        .position(|location| *location == source_definition)
    {
        locations.swap(0, existing_index);
    } else if let Some(first_location) = locations.first_mut() {
        *first_location = source_definition;
    } else {
        locations.push(source_definition);
    }

    Some(locations)
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
            let Ok(analysis) = analyze_source(&source) else {
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
            if let Some(definition) = definition_for_dependency_imports(&source, &package, position)
            {
                return Ok(Some(definition));
            }
            if let Some(definition) = definition_for_dependency_methods(&source, &package, position)
            {
                return Ok(Some(definition));
            }
            if let Some(definition) =
                definition_for_dependency_struct_fields(&source, &package, position)
            {
                return Ok(Some(definition));
            }
            if let Some(definition) =
                definition_for_dependency_variants(&source, &package, position)
            {
                return Ok(Some(definition));
            }
            let Ok(analysis) = analyze_source(&source) else {
                return Ok(definition_for_dependency_values(
                    &source, &package, position,
                ));
            };
            if let Some(definition) =
                workspace_source_definition_for_import(&uri, &source, &analysis, &package, position)
            {
                return Ok(Some(definition));
            }
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
            if let Some(declaration) =
                declaration_for_dependency_imports(&source, &package, position)
            {
                return Ok(Some(declaration));
            }
            if let Some(declaration) =
                declaration_for_dependency_methods(&source, &package, position)
            {
                return Ok(Some(declaration));
            }
            if let Some(declaration) =
                declaration_for_dependency_struct_fields(&source, &package, position)
            {
                return Ok(Some(declaration));
            }
            if let Some(declaration) =
                declaration_for_dependency_variants(&source, &package, position)
            {
                return Ok(Some(declaration));
            }
            let Ok(analysis) = analyze_source(&source) else {
                return Ok(declaration_for_dependency_values(
                    &source, &package, position,
                ));
            };
            if let Some(GotoDefinitionResponse::Scalar(location)) =
                workspace_source_definition_for_import(&uri, &source, &analysis, &package, position)
            {
                return Ok(Some(GotoDeclarationResponse::Scalar(location)));
            }
            return Ok(declaration_for_package_analysis(
                &uri, &source, &analysis, &package, position,
            ));
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
            let Ok(analysis) = analyze_source(&source) else {
                return Ok(
                    type_definition_for_dependency_imports(&source, &package, position).or_else(
                        || {
                            type_definition_for_dependency_values(&source, &package, position)
                                .or_else(|| {
                                    type_definition_for_dependency_variants(
                                        &source, &package, position,
                                    )
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
                                })
                        },
                    ),
                );
            };
            return Ok(type_definition_for_package_analysis(
                &uri, &source, &analysis, &package, position,
            ));
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
            let Ok(analysis) = analyze_source(&source) else {
                if let Some(references) = references_for_dependency_imports(
                    &uri,
                    &source,
                    &package,
                    position,
                    params.context.include_declaration,
                ) {
                    return Ok(Some(references));
                }
                if let Some(references) = references_for_dependency_values(
                    &uri,
                    &source,
                    &package,
                    position,
                    params.context.include_declaration,
                ) {
                    return Ok(Some(references));
                }
                if let Some(references) = references_for_dependency_methods(
                    &uri,
                    &source,
                    &package,
                    position,
                    params.context.include_declaration,
                ) {
                    return Ok(Some(references));
                }
                if let Some(references) = references_for_dependency_variants(
                    &uri,
                    &source,
                    &package,
                    position,
                    params.context.include_declaration,
                ) {
                    return Ok(Some(references));
                }
                return Ok(references_for_dependency_struct_fields(
                    &uri,
                    &source,
                    &package,
                    position,
                    params.context.include_declaration,
                ));
            };
            if let Some(references) = workspace_source_references_for_import(
                &uri,
                &source,
                &analysis,
                &package,
                position,
                params.context.include_declaration,
            ) {
                return Ok(Some(references));
            }
            return Ok(references_for_package_analysis(
                &uri,
                &source,
                &analysis,
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
        let Some((source, analysis)) = self.analyzed_document(&uri).await else {
            return Ok(None);
        };

        if let Some(package) = self.package_analysis_for_uri(&uri) {
            return Ok(Some(semantic_tokens_for_package_analysis(
                &source, &analysis, &package,
            )));
        }

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
            if position_to_offset(&source, position)
                .and_then(|offset| package.dependency_hover_in_source_at(&source, offset))
                .is_some()
            {
                return Ok(None);
            }
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
            if position_to_offset(&source, position)
                .and_then(|offset| package.dependency_hover_in_source_at(&source, offset))
                .is_some()
            {
                return Ok(None);
            }
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
        document_highlights_for_analysis_at, document_highlights_for_package_analysis_at,
        package_analysis_for_path, workspace_source_definition_for_import,
        workspace_source_references_for_import, workspace_symbols_for_documents,
        workspace_symbols_for_documents_and_roots,
    };
    use ql_analysis::{SymbolKind as AnalysisSymbolKind, analyze_source};
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};
    use tower_lsp::lsp_types::{
        GotoDefinitionResponse, Location, Position, SymbolInformation, SymbolKind, Url,
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
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");

        let references = workspace_source_references_for_import(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            true,
        )
        .expect("workspace import references should exist");

        assert_eq!(references.len(), 3);
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
}
