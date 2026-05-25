use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ql_analysis::{Analysis, analyze_source};
use tower_lsp::lsp_types::Url;

use super::canonicalize_or_clone;

pub(super) type OpenDocuments = HashMap<PathBuf, (Url, String)>;

pub(super) struct WorkspaceRequestContext {
    pub(super) analysis: Option<Analysis>,
    pub(super) package: ql_analysis::PackageAnalysis,
    pub(super) open_docs: OpenDocuments,
}

pub(super) fn file_open_documents(documents: Vec<(Url, String)>) -> OpenDocuments {
    let mut open_docs = HashMap::new();
    for (uri, source) in documents {
        let Ok(path) = uri.to_file_path() else {
            continue;
        };
        open_docs.insert(canonicalize_or_clone(&path), (uri, source));
    }
    open_docs
}

pub(super) fn open_document_snapshot(
    open_docs: &OpenDocuments,
    path: &Path,
) -> Option<(Url, String, Analysis)> {
    let canonical_path = canonicalize_or_clone(path);
    let (uri, source) = open_docs.get(&canonical_path)?;
    let analysis = analyze_source(source).ok()?;
    Some((uri.clone(), source.clone(), analysis))
}
