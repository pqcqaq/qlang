use std::path::Path;

use crate::cli_utils::{
    normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error,
};

pub(super) fn report_project_graph_load_error(path: &Path, error: &ql_project::ProjectError) -> u8 {
    if let ql_project::ProjectError::ManifestNotFound { start } = error {
        eprintln!(
            "error: `ql project graph` requires a package or workspace manifest; could not find `qlang.toml` starting from `{}`",
            normalize_path(start)
        );
        report_project_graph_package_context_failure(path);
    } else if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error)
    {
        eprintln!(
            "error: `ql project graph` manifest `{}` does not declare `[package].name`",
            normalize_path(manifest_path)
        );
        report_project_graph_manifest_failure(manifest_path);
    } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(error) {
        eprintln!("error: `ql project graph` {error}");
        report_project_graph_manifest_failure(manifest_path);
    } else {
        eprintln!("error: {error}");
    }
    1
}

pub(super) fn report_project_graph_manifest_failure(manifest_path: &Path) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command = format_project_graph_command(&manifest_path);
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!("hint: rerun `{rerun_command}` after fixing the package manifest");
}

fn report_project_graph_package_context_failure(path: &Path) {
    let normalized_path = normalize_path(path);
    let rerun_command = format_project_graph_command(&normalized_path);
    eprintln!(
        "note: `ql project graph` only renders package/workspace graphs for packages or workspace members discoverable from `qlang.toml`"
    );
    eprintln!("hint: rerun `{rerun_command}` after adding `qlang.toml` for this path");
}

pub(super) fn format_project_graph_command(normalized_path: &str) -> String {
    format!("ql project graph {normalized_path}")
}
