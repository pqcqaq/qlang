use std::path::{Path, PathBuf};

use ql_project::ProjectManifest;

use crate::cli_utils::{normalize_path, validate_project_package_name};
use crate::project_workspace::{
    WorkspacePackageSelectionFailure, resolve_project_package_manifest,
    resolve_project_workspace_manifest, resolve_project_workspace_member_package_name,
    resolve_selected_workspace_member_manifest,
    resolve_selected_workspace_member_manifest_for_json,
};

pub(super) enum ProjectDependencyQueryContextError {
    Json {
        workspace_manifest: ProjectManifest,
        package_name: String,
        failure: ProjectDependencySelectionFailure,
    },
    Exit(u8),
}

pub(super) struct ProjectDependencySelectionFailure {
    pub(super) message: String,
    pub(super) selector: Option<String>,
    pub(super) target_count: Option<usize>,
}

pub(super) fn resolve_project_dependency_query_context(
    path: &Path,
    package_name: Option<&str>,
    command_label: &str,
    selector_option: &str,
    json: bool,
    allow_standalone_package: bool,
) -> Result<(ProjectManifest, String, PathBuf), ProjectDependencyQueryContextError> {
    let workspace_manifest = match resolve_project_workspace_manifest(path) {
        Ok(workspace_manifest) => workspace_manifest,
        Err(workspace_error) => {
            if allow_standalone_package {
                return resolve_standalone_package_query_context(
                    path,
                    package_name,
                    command_label,
                    json,
                );
            }
            eprintln!("error: {command_label} {workspace_error}");
            return Err(ProjectDependencyQueryContextError::Exit(1));
        }
    };
    let package_name = match package_name {
        Some(package_name) => {
            if let Err(message) = validate_project_package_name(package_name) {
                if json {
                    return Err(ProjectDependencyQueryContextError::Json {
                        workspace_manifest,
                        package_name: package_name.to_owned(),
                        failure: ProjectDependencySelectionFailure {
                            message: format!("{command_label} {message}"),
                            selector: Some(format!("package `{package_name}`")),
                            target_count: None,
                        },
                    });
                }
                eprintln!("error: {command_label} {message}");
                return Err(ProjectDependencyQueryContextError::Exit(1));
            }
            package_name.to_owned()
        }
        None => resolve_project_workspace_member_package_name(path, None, command_label)
            .map_err(ProjectDependencyQueryContextError::Exit)?,
    };
    let (_, member_manifest) = if json {
        resolve_selected_workspace_member_manifest_for_json(
            &workspace_manifest,
            &package_name,
            command_label,
        )
        .map_err(|failure| ProjectDependencyQueryContextError::Json {
            workspace_manifest: workspace_manifest.clone(),
            package_name: package_name.clone(),
            failure: failure.into(),
        })?
    } else {
        resolve_selected_workspace_member_manifest(
            &workspace_manifest,
            path,
            &package_name,
            command_label,
            selector_option,
        )
        .map_err(ProjectDependencyQueryContextError::Exit)?
    };
    Ok((
        workspace_manifest,
        package_name,
        member_manifest.manifest_path,
    ))
}

fn resolve_standalone_package_query_context(
    path: &Path,
    selected_package_name: Option<&str>,
    command_label: &str,
    json: bool,
) -> Result<(ProjectManifest, String, PathBuf), ProjectDependencyQueryContextError> {
    let package_manifest = resolve_project_package_manifest(path).map_err(|message| {
        eprintln!("error: {command_label} {message}");
        ProjectDependencyQueryContextError::Exit(1)
    })?;
    let actual_package_name = ql_project::package_name(&package_manifest)
        .map_err(|error| {
            eprintln!("error: {command_label} failed to inspect package: {error}");
            ProjectDependencyQueryContextError::Exit(1)
        })?
        .to_owned();

    if let Some(selected_package_name) = selected_package_name {
        if let Err(message) = validate_project_package_name(selected_package_name) {
            if json {
                return Err(ProjectDependencyQueryContextError::Json {
                    workspace_manifest: package_manifest,
                    package_name: selected_package_name.to_owned(),
                    failure: ProjectDependencySelectionFailure {
                        message: format!("{command_label} {message}"),
                        selector: Some(format!("package `{selected_package_name}`")),
                        target_count: None,
                    },
                });
            }
            eprintln!("error: {command_label} {message}");
            return Err(ProjectDependencyQueryContextError::Exit(1));
        }
        if selected_package_name != actual_package_name {
            if json {
                return Err(ProjectDependencyQueryContextError::Json {
                    workspace_manifest: package_manifest,
                    package_name: selected_package_name.to_owned(),
                    failure: ProjectDependencySelectionFailure {
                        message: format!(
                            "{command_label} package selector expected `{selected_package_name}` but `{}` resolves to package `{actual_package_name}`",
                            normalize_path(path)
                        ),
                        selector: Some(format!("package `{selected_package_name}`")),
                        target_count: Some(0),
                    },
                });
            }
            eprintln!(
                "error: {command_label} package selector expected `{selected_package_name}` but `{}` resolves to package `{actual_package_name}`",
                normalize_path(path)
            );
            return Err(ProjectDependencyQueryContextError::Exit(1));
        }
    }

    Ok((
        package_manifest.clone(),
        actual_package_name,
        package_manifest.manifest_path,
    ))
}

impl From<WorkspacePackageSelectionFailure> for ProjectDependencySelectionFailure {
    fn from(failure: WorkspacePackageSelectionFailure) -> Self {
        Self {
            message: failure.message,
            selector: Some(failure.selector),
            target_count: failure.target_count,
        }
    }
}
