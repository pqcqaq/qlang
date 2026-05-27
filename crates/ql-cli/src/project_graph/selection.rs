use std::path::Path;

use ql_project::package_name;

use super::reporting::report_project_graph_manifest_failure;
use crate::cli_utils::{normalize_path, validate_project_package_name};
use crate::project_workspace::{
    WorkspacePackageSelectionFailure, resolve_selected_workspace_member_manifest,
    resolve_selected_workspace_member_manifest_for_json,
};

pub(super) fn resolve_project_graph_package_manifest(
    path: &Path,
    manifest: &ql_project::ProjectManifest,
    selected_package_name: &str,
    json: bool,
) -> Result<ql_project::ProjectManifest, ProjectGraphPackageSelectionError> {
    if let Err(message) = validate_project_package_name(selected_package_name) {
        if json {
            return Err(ProjectGraphPackageSelectionError::Json(
                ProjectGraphSelectionFailure {
                    message: format!("`ql project graph` {message}"),
                    selector: Some(format!("package `{selected_package_name}`")),
                    target_count: None,
                },
            ));
        }
        eprintln!("error: `ql project graph` {message}");
        return Err(ProjectGraphPackageSelectionError::Exit(1));
    }

    if manifest.workspace.is_some() {
        if json {
            return resolve_project_graph_package_manifest_json(manifest, selected_package_name);
        }
        let (_, member_manifest) = resolve_selected_workspace_member_manifest(
            manifest,
            path,
            selected_package_name,
            "`ql project graph`",
            "--package",
        )
        .map_err(ProjectGraphPackageSelectionError::Exit)?;
        return Ok(member_manifest);
    }

    let actual_package_name = package_name(manifest).map_err(|error| {
        eprintln!("error: `ql project graph` {error}");
        report_project_graph_manifest_failure(&manifest.manifest_path);
        ProjectGraphPackageSelectionError::Exit(1)
    })?;
    if actual_package_name != selected_package_name {
        if json {
            return Err(ProjectGraphPackageSelectionError::Json(
                ProjectGraphSelectionFailure {
                    message: format!(
                        "`ql project graph` package selector expected `{selected_package_name}` but `{}` resolves to package `{actual_package_name}`",
                        normalize_path(path)
                    ),
                    selector: Some(format!("package `{selected_package_name}`")),
                    target_count: Some(0),
                },
            ));
        }
        eprintln!(
            "error: `ql project graph` package selector expected `{selected_package_name}` but `{}` resolves to package `{actual_package_name}`",
            normalize_path(path)
        );
        return Err(ProjectGraphPackageSelectionError::Exit(1));
    }

    Ok(manifest.clone())
}

pub(super) enum ProjectGraphPackageSelectionError {
    Json(ProjectGraphSelectionFailure),
    Exit(u8),
}

pub(super) struct ProjectGraphSelectionFailure {
    pub(super) message: String,
    pub(super) selector: Option<String>,
    pub(super) target_count: Option<usize>,
}

fn resolve_project_graph_package_manifest_json(
    manifest: &ql_project::ProjectManifest,
    selected_package_name: &str,
) -> Result<ql_project::ProjectManifest, ProjectGraphPackageSelectionError> {
    resolve_selected_workspace_member_manifest_for_json(
        manifest,
        selected_package_name,
        "`ql project graph`",
    )
    .map(|(_, member_manifest)| member_manifest)
    .map_err(|failure| ProjectGraphPackageSelectionError::Json(failure.into()))
}

impl From<WorkspacePackageSelectionFailure> for ProjectGraphSelectionFailure {
    fn from(failure: WorkspacePackageSelectionFailure) -> Self {
        Self {
            message: failure.message,
            selector: Some(failure.selector),
            target_count: failure.target_count,
        }
    }
}
