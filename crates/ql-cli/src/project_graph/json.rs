use std::path::Path;

use ql_project::package_name;
use serde_json::json;

use super::selection::ProjectGraphSelectionFailure;
use crate::cli_utils::{
    normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error,
};

pub(super) fn render_project_graph_preflight_failure_json(
    path: &Path,
    error: &ql_project::ProjectError,
) -> String {
    let manifest_path = project_graph_load_error_manifest_path(error).map(normalize_path);
    let rendered = serde_json::to_string_pretty(&json!({
        "schema": "ql.project.graph.v1",
        "path": normalize_path(path),
        "manifest_path": manifest_path,
        "package_name": Option::<String>::None,
        "workspace_members": [],
        "workspace_packages": [],
        "failure": {
            "kind": "preflight",
            "preflight_failure": {
                "stage": "manifest-load",
                "message": project_graph_load_error_message(error),
                "manifest_path": manifest_path,
            },
        },
    }))
    .expect("project graph preflight failure json should serialize");
    format!("{rendered}\n")
}

pub(super) fn render_project_graph_selection_failure_json(
    path: &Path,
    manifest: &ql_project::ProjectManifest,
    failure: ProjectGraphSelectionFailure,
) -> String {
    let rendered = serde_json::to_string_pretty(&json!({
        "schema": "ql.project.graph.v1",
        "path": normalize_path(path),
        "manifest_path": normalize_path(&manifest.manifest_path),
        "package_name": package_name(manifest).ok(),
        "workspace_members": manifest
            .workspace
            .as_ref()
            .map(|workspace| workspace.members.clone())
            .unwrap_or_default(),
        "workspace_packages": [],
        "failure": {
            "kind": "selection",
            "selection_failure": {
                "stage": "package-selection",
                "message": failure.message,
                "selector": failure.selector,
                "target_count": failure.target_count,
            },
        },
    }))
    .expect("project graph selection failure json should serialize");
    format!("{rendered}\n")
}

fn project_graph_load_error_message(error: &ql_project::ProjectError) -> String {
    if let ql_project::ProjectError::ManifestNotFound { start } = error {
        return format!(
            "`ql project graph` requires a package or workspace manifest; could not find `qlang.toml` starting from `{}`",
            normalize_path(start)
        );
    }
    if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error) {
        return format!(
            "`ql project graph` manifest `{}` does not declare `[package].name`",
            normalize_path(manifest_path)
        );
    }
    if package_check_manifest_path_from_project_error(error).is_some() {
        return format!("`ql project graph` {error}");
    }
    error.to_string()
}

fn project_graph_load_error_manifest_path(error: &ql_project::ProjectError) -> Option<&Path> {
    package_missing_name_manifest_path_from_project_error(error)
        .or_else(|| package_check_manifest_path_from_project_error(error))
}
