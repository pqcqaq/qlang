use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use ql_analysis::{
    DependencyInterface, analyze_available_package_dependencies, analyze_package, analyze_source,
};
use ql_project::{collect_package_sources, load_project_manifest};
use tower_lsp::lsp_types::{Location, SymbolInformation, Url};

use crate::bridge::{
    document_symbol_kind, span_to_range, symbol_information, workspace_symbols_for_analysis,
};

use super::{
    OpenDocuments, canonicalize_or_clone, local_dependency_manifest_paths_for_package,
    should_fallback_to_manifest_sources, workspace_member_manifest_paths_for_package,
};

struct WorkspaceSymbolRequestContext {
    open_docs: OpenDocuments,
    non_file_docs: Vec<(Url, String)>,
    workspace_roots: Vec<PathBuf>,
    query: String,
}

#[derive(Default)]
struct WorkspaceSymbolIndex {
    searched_packages: HashSet<PathBuf>,
    covered_files: HashSet<PathBuf>,
    symbols: Vec<SymbolInformation>,
}

impl WorkspaceSymbolIndex {
    fn append_manifest_and_workspace_symbols(
        &mut self,
        manifest: &ql_project::ProjectManifest,
        open_docs: &OpenDocuments,
        query: &str,
    ) {
        if manifest.package.is_some() {
            let manifest_path = manifest.manifest_path.clone();
            if self.searched_packages.insert(manifest_path.clone()) {
                self.append_manifest_source_workspace_symbols(manifest, open_docs, query);
                let preferred_local_dependency_manifest_paths = self
                    .append_local_dependency_workspace_symbols(
                        manifest_path.as_path(),
                        open_docs,
                        query,
                    );
                self.append_dependency_workspace_symbols_excluding(
                    &manifest_path,
                    &preferred_local_dependency_manifest_paths,
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
            if !self.searched_packages.insert(member_manifest_path.clone()) {
                continue;
            }
            self.append_workspace_member_symbols(&member_manifest_path, open_docs, query);
        }
    }

    fn append_package_workspace_symbols(
        &mut self,
        package: &ql_analysis::PackageAnalysis,
        open_docs: &OpenDocuments,
        query: &str,
        include_dependencies: bool,
    ) {
        for module in package.modules() {
            let module_path = module.path().to_path_buf();
            self.covered_files.insert(module_path.clone());

            if let Some((open_uri, open_source)) = open_docs.get(&module_path)
                && let Ok(analysis) = analyze_source(open_source)
            {
                self.symbols.extend(workspace_symbols_for_analysis(
                    open_uri,
                    open_source,
                    &analysis,
                    query,
                ));
                continue;
            }

            let module_location_path =
                fs::canonicalize(&module_path).unwrap_or(module_path.clone());
            let Ok(module_uri) = Url::from_file_path(&module_location_path) else {
                continue;
            };
            let Ok(module_source) = fs::read_to_string(&module_path) else {
                continue;
            };
            self.symbols.extend(workspace_symbols_for_analysis(
                &module_uri,
                &module_source,
                module.analysis(),
                query,
            ));
        }

        if include_dependencies {
            self.symbols.extend(workspace_symbols_for_dependencies(
                package.dependencies(),
                query,
            ));
        }
    }

    fn append_manifest_source_workspace_symbols(
        &mut self,
        manifest: &ql_project::ProjectManifest,
        open_docs: &OpenDocuments,
        query: &str,
    ) {
        let Ok(source_paths) = collect_package_sources(manifest) else {
            return;
        };

        for source_path in source_paths {
            self.covered_files.insert(source_path.clone());

            if let Some((open_uri, open_source)) = open_docs.get(&source_path) {
                if let Ok(analysis) = analyze_source(open_source) {
                    self.symbols.extend(workspace_symbols_for_analysis(
                        open_uri,
                        open_source,
                        &analysis,
                        query,
                    ));
                }
                continue;
            }

            let source_location_path =
                fs::canonicalize(&source_path).unwrap_or(source_path.clone());
            let Ok(source_uri) = Url::from_file_path(&source_location_path) else {
                continue;
            };
            let Ok(source) = fs::read_to_string(&source_path) else {
                continue;
            };
            let Ok(analysis) = analyze_source(&source) else {
                continue;
            };
            self.symbols.extend(workspace_symbols_for_analysis(
                &source_uri,
                &source,
                &analysis,
                query,
            ));
        }
    }

    fn append_dependency_workspace_symbols_excluding(
        &mut self,
        package_path: &Path,
        excluded_manifest_paths: &HashSet<PathBuf>,
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
            self.symbols.extend(workspace_symbols_for_dependencies(
                &filtered_dependencies,
                query,
            ));
        }
    }

    fn append_visible_dependency_workspace_symbols(
        &mut self,
        package_manifest_path: &Path,
        open_docs: &OpenDocuments,
        query: &str,
    ) {
        let preferred_local_dependency_manifest_paths =
            self.append_local_dependency_workspace_symbols(package_manifest_path, open_docs, query);
        self.append_dependency_workspace_symbols_excluding(
            package_manifest_path,
            &preferred_local_dependency_manifest_paths,
            query,
        );
    }

    fn append_analyzed_package_workspace_symbols(
        &mut self,
        package: &ql_analysis::PackageAnalysis,
        open_docs: &OpenDocuments,
        query: &str,
    ) {
        self.append_package_workspace_symbols(package, open_docs, query, false);
        self.append_visible_dependency_workspace_symbols(
            package.manifest().manifest_path.as_path(),
            open_docs,
            query,
        );
    }

    fn append_manifest_fallback_workspace_symbols(
        &mut self,
        manifest: &ql_project::ProjectManifest,
        open_docs: &OpenDocuments,
        query: &str,
    ) {
        self.append_manifest_source_workspace_symbols(manifest, open_docs, query);
        self.append_visible_dependency_workspace_symbols(
            manifest.manifest_path.as_path(),
            open_docs,
            query,
        );
    }

    fn append_workspace_member_symbols_for_package(
        &mut self,
        package_manifest_path: &Path,
        open_docs: &OpenDocuments,
        query: &str,
    ) {
        for member_manifest_path in
            workspace_member_manifest_paths_for_package(package_manifest_path)
        {
            if !self.searched_packages.insert(member_manifest_path.clone()) {
                continue;
            }
            self.append_workspace_member_symbols(&member_manifest_path, open_docs, query);
        }
    }

    fn append_local_dependency_workspace_symbols(
        &mut self,
        package_manifest_path: &Path,
        open_docs: &OpenDocuments,
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

            if !self
                .searched_packages
                .insert(local_dependency_manifest_path.clone())
            {
                continue;
            }

            self.append_workspace_member_symbols(&local_dependency_manifest_path, open_docs, query);
        }

        preferred_manifest_paths
    }

    fn append_workspace_member_symbols(
        &mut self,
        member_manifest_path: &Path,
        open_docs: &OpenDocuments,
        query: &str,
    ) {
        match analyze_package(member_manifest_path) {
            Ok(member_package) => {
                self.append_analyzed_package_workspace_symbols(&member_package, open_docs, query);
            }
            Err(error) if should_fallback_to_manifest_sources(&error) => {
                let Ok(member_manifest) = load_project_manifest(member_manifest_path) else {
                    return;
                };
                self.append_manifest_fallback_workspace_symbols(&member_manifest, open_docs, query);
            }
            Err(_) => {}
        }
    }

    fn append_standalone_document_workspace_symbols(
        &mut self,
        path: &Path,
        uri: &Url,
        source: &str,
        query: &str,
    ) {
        self.covered_files.insert(path.to_path_buf());
        if let Ok(analysis) = analyze_source(source) {
            self.symbols.extend(workspace_symbols_for_analysis(
                uri, source, &analysis, query,
            ));
        }
    }

    fn append_open_file_document(
        &mut self,
        path: &Path,
        uri: &Url,
        source: &str,
        open_docs: &OpenDocuments,
        query: &str,
    ) {
        if self.covered_files.contains(path) {
            return;
        }

        match analyze_package(path) {
            Ok(package) => {
                let manifest_path = package.manifest().manifest_path.clone();
                if !self.searched_packages.insert(manifest_path.clone()) {
                    return;
                }
                self.append_analyzed_package_workspace_symbols(&package, open_docs, query);
                self.append_workspace_member_symbols_for_package(&manifest_path, open_docs, query);
            }
            Err(error) if should_fallback_to_manifest_sources(&error) => {
                let Ok(manifest) = load_project_manifest(path) else {
                    self.append_standalone_document_workspace_symbols(path, uri, source, query);
                    return;
                };

                let manifest_path = manifest.manifest_path.clone();
                if !self.searched_packages.insert(manifest_path.clone()) {
                    return;
                }

                self.append_manifest_fallback_workspace_symbols(&manifest, open_docs, query);
                self.append_workspace_member_symbols_for_package(&manifest_path, open_docs, query);
            }
            Err(_) => {
                self.append_standalone_document_workspace_symbols(path, uri, source, query);
            }
        }
    }

    fn append_workspace_root(
        &mut self,
        workspace_root: &Path,
        open_docs: &OpenDocuments,
        query: &str,
    ) {
        let Ok(manifest) = load_project_manifest(workspace_root) else {
            return;
        };
        self.append_manifest_and_workspace_symbols(&manifest, open_docs, query);
    }

    fn append_non_file_document(&mut self, uri: &Url, source: &str, query: &str) {
        if let Ok(analysis) = analyze_source(source) {
            self.symbols.extend(workspace_symbols_for_analysis(
                uri, source, &analysis, query,
            ));
        }
    }

    fn finish(mut self) -> Vec<SymbolInformation> {
        self.symbols.sort_by_key(|symbol| {
            (
                symbol.name.to_ascii_lowercase(),
                symbol.location.uri.to_string(),
                symbol.location.range.start.line,
                symbol.location.range.start.character,
            )
        });
        self.symbols.dedup();
        self.symbols
    }
}

fn manifest_has_workspace_symbol_source(manifest_path: &Path, open_docs: &OpenDocuments) -> bool {
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

#[cfg(test)]
pub(super) fn workspace_symbols_for_documents(
    documents: Vec<(Url, String)>,
    query: &str,
) -> Vec<SymbolInformation> {
    workspace_symbols_for_context(workspace_symbol_request_context(documents, &[], query))
}

fn workspace_symbol_request_context(
    documents: Vec<(Url, String)>,
    workspace_roots: &[PathBuf],
    query: &str,
) -> WorkspaceSymbolRequestContext {
    let mut open_docs = HashMap::<PathBuf, (Url, String)>::new();
    let mut non_file_docs = Vec::<(Url, String)>::new();
    for (uri, source) in documents {
        if let Ok(path) = uri.to_file_path() {
            open_docs.insert(canonicalize_or_clone(&path), (uri, source));
        } else {
            non_file_docs.push((uri, source));
        }
    }

    let mut workspace_roots = workspace_roots
        .iter()
        .map(|path| canonicalize_or_clone(path))
        .collect::<Vec<_>>();
    workspace_roots.sort();
    workspace_roots.dedup();

    WorkspaceSymbolRequestContext {
        open_docs,
        non_file_docs,
        workspace_roots,
        query: query.trim().to_ascii_lowercase(),
    }
}

pub(super) fn workspace_symbols_for_documents_and_roots(
    documents: Vec<(Url, String)>,
    workspace_roots: &[PathBuf],
    query: &str,
) -> Vec<SymbolInformation> {
    workspace_symbols_for_context(workspace_symbol_request_context(
        documents,
        workspace_roots,
        query,
    ))
}

fn workspace_symbols_for_context(context: WorkspaceSymbolRequestContext) -> Vec<SymbolInformation> {
    let WorkspaceSymbolRequestContext {
        open_docs,
        non_file_docs,
        workspace_roots,
        query,
    } = context;
    let mut index = WorkspaceSymbolIndex::default();

    let mut file_paths = open_docs.keys().cloned().collect::<Vec<_>>();
    file_paths.sort();

    for path in file_paths {
        let Some((uri, source)) = open_docs.get(&path) else {
            continue;
        };
        index.append_open_file_document(&path, uri, source, &open_docs, &query);
    }

    for workspace_root in workspace_roots {
        index.append_workspace_root(&workspace_root, &open_docs, &query);
    }

    for (uri, source) in non_file_docs {
        index.append_non_file_document(&uri, &source, &query);
    }

    index.finish()
}

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

            symbols.push(symbol_information(
                symbol.name.clone(),
                document_symbol_kind(symbol.kind),
                Location::new(uri.clone(), span_to_range(&source, span)),
                Some(symbol.package_name.clone()),
            ));
        }
    }

    symbols
}
