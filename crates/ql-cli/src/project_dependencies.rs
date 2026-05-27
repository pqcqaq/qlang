use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ql_project::{ProjectManifest, load_project_manifest, load_reference_manifests, package_name};

use crate::cli_utils::{normalize_path, relative_path_from, validate_project_package_name};
use crate::project_workspace::{
    WorkspacePackageSelectionFailure, resolve_project_package_manifest,
    resolve_project_workspace_manifest, resolve_project_workspace_member_package_name,
    resolve_selected_workspace_member_manifest,
    resolve_selected_workspace_member_manifest_for_json,
};

mod rendering;

pub(crate) use rendering::project_dependency_json;
use rendering::{
    render_project_dependencies, render_project_dependencies_json,
    render_project_dependencies_selection_failure_json, render_project_dependents,
    render_project_dependents_json, render_project_dependents_selection_failure_json,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProjectDependentMember {
    pub(crate) member: String,
    pub(crate) package_name: String,
    pub(crate) manifest_path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProjectDependencyMember {
    pub(crate) member: Option<String>,
    pub(crate) dependency_path: String,
    pub(crate) package_name: String,
    pub(crate) manifest_path: PathBuf,
}

enum ProjectDependencyQueryContextError {
    Json {
        workspace_manifest: ProjectManifest,
        package_name: String,
        failure: ProjectDependencySelectionFailure,
    },
    Exit(u8),
}

struct ProjectDependencySelectionFailure {
    message: String,
    selector: Option<String>,
    target_count: Option<usize>,
}

pub(crate) fn project_dependents_path(
    path: &Path,
    package_name: Option<&str>,
    json: bool,
) -> Result<(), u8> {
    let (workspace_manifest, package_name, member_manifest_path) =
        match resolve_project_dependency_query_context(
            path,
            package_name,
            "`ql project dependents`",
            "--name",
            json,
            true,
        ) {
            Ok(context) => context,
            Err(ProjectDependencyQueryContextError::Json {
                workspace_manifest,
                package_name,
                failure,
            }) => {
                print!(
                    "{}",
                    render_project_dependents_selection_failure_json(
                        path,
                        &workspace_manifest,
                        &package_name,
                        failure,
                    )
                );
                return Err(1);
            }
            Err(ProjectDependencyQueryContextError::Exit(code)) => return Err(code),
        };
    let dependents = find_workspace_member_dependents(&workspace_manifest, &member_manifest_path)
        .map_err(|message| {
        eprintln!("error: `ql project dependents` {message}");
        1
    })?;
    let rendered = if json {
        render_project_dependents_json(path, &workspace_manifest, &package_name, &dependents)
    } else {
        render_project_dependents(&workspace_manifest, &package_name, &dependents)
    };
    print!("{rendered}");
    Ok(())
}

pub(crate) fn project_dependencies_path(
    path: &Path,
    package_name: Option<&str>,
    json: bool,
) -> Result<(), u8> {
    let (workspace_manifest, package_name, member_manifest_path) =
        match resolve_project_dependency_query_context(
            path,
            package_name,
            "`ql project dependencies`",
            "--name",
            json,
            true,
        ) {
            Ok(context) => context,
            Err(ProjectDependencyQueryContextError::Json {
                workspace_manifest,
                package_name,
                failure,
            }) => {
                print!(
                    "{}",
                    render_project_dependencies_selection_failure_json(
                        path,
                        &workspace_manifest,
                        &package_name,
                        failure,
                    )
                );
                return Err(1);
            }
            Err(ProjectDependencyQueryContextError::Exit(code)) => return Err(code),
        };
    let dependencies =
        find_workspace_member_dependencies(&workspace_manifest, &member_manifest_path).map_err(
            |message| {
                eprintln!("error: `ql project dependencies` {message}");
                1
            },
        )?;
    let rendered = if json {
        render_project_dependencies_json(path, &workspace_manifest, &package_name, &dependencies)
    } else {
        render_project_dependencies(&workspace_manifest, &package_name, &dependencies)
    };
    print!("{rendered}");
    Ok(())
}

fn resolve_project_dependency_query_context(
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

pub(crate) fn find_workspace_member_dependents(
    workspace_manifest: &ql_project::ProjectManifest,
    dependency_manifest_path: &Path,
) -> Result<Vec<ProjectDependentMember>, String> {
    let Some(workspace) = workspace_manifest.workspace.as_ref() else {
        return Ok(Vec::new());
    };

    let workspace_root = workspace_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    let dependency_manifest_path = normalize_path(dependency_manifest_path);
    let mut dependents = Vec::new();

    for member in &workspace.members {
        let member_manifest = load_project_manifest(&workspace_root.join(member)).map_err(|error| {
            format!(
                "failed to inspect workspace member `{member}` while checking whether `{}` can be removed: {error}",
                dependency_manifest_path
            )
        })?;
        if normalize_path(&member_manifest.manifest_path) == dependency_manifest_path {
            continue;
        }

        let member_package_name = package_name(&member_manifest).map_err(|error| {
            format!(
                "failed to inspect workspace member `{member}` while checking whether `{}` can be removed: {error}",
                dependency_manifest_path
            )
        })?;
        let references = load_reference_manifests(&member_manifest).map_err(|error| {
            format!(
                "failed to inspect local dependencies for workspace member `{member}` while checking whether `{}` can be removed: {error}",
                dependency_manifest_path
            )
        })?;
        if references
            .iter()
            .any(|reference| normalize_path(&reference.manifest_path) == dependency_manifest_path)
        {
            dependents.push(ProjectDependentMember {
                member: member.clone(),
                package_name: member_package_name.to_owned(),
                manifest_path: member_manifest.manifest_path,
            });
        }
    }

    Ok(dependents)
}

pub(crate) fn find_workspace_member_dependencies(
    workspace_manifest: &ql_project::ProjectManifest,
    member_manifest_path: &Path,
) -> Result<Vec<ProjectDependencyMember>, String> {
    let member_manifest_path = normalize_path(member_manifest_path);
    let member_manifest = load_project_manifest(Path::new(&member_manifest_path)).map_err(|error| {
        format!(
            "failed to inspect workspace member `{member_manifest_path}` while resolving local dependencies: {error}"
        )
    })?;
    let references = load_reference_manifests(&member_manifest).map_err(|error| {
        format!(
            "failed to inspect local dependencies for workspace member `{member_manifest_path}`: {error}"
        )
    })?;
    if references.is_empty() {
        return Ok(Vec::new());
    }

    let workspace_root = workspace_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    let member_root = member_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    let mut workspace_member_paths = BTreeMap::new();
    if let Some(workspace) = workspace_manifest.workspace.as_ref() {
        for member in &workspace.members {
            let dependency_manifest =
                load_project_manifest(&workspace_root.join(member)).map_err(|error| {
                    format!(
                        "failed to inspect workspace member `{member}` while resolving local dependencies for `{member_manifest_path}`: {error}"
                    )
                })?;
            workspace_member_paths.insert(
                normalize_path(&dependency_manifest.manifest_path),
                member.clone(),
            );
        }
    }

    let mut seen_reference_manifest_paths = BTreeSet::new();
    let mut dependencies = Vec::new();

    for dependency_manifest in references {
        let dependency_manifest_path = normalize_path(&dependency_manifest.manifest_path);
        if dependency_manifest_path == member_manifest_path
            || !seen_reference_manifest_paths.insert(dependency_manifest_path.clone())
        {
            continue;
        }

        let dependency_package_name = package_name(&dependency_manifest).map_err(|error| {
            format!(
                "failed to inspect local dependency `{dependency_manifest_path}` while resolving local dependencies for `{member_manifest_path}`: {error}"
            )
        })?;
        let dependency_root = dependency_manifest
            .manifest_path
            .parent()
            .unwrap_or(Path::new("."));
        dependencies.push(ProjectDependencyMember {
            member: workspace_member_paths
                .get(&dependency_manifest_path)
                .cloned(),
            dependency_path: relative_path_from(member_root, dependency_root),
            package_name: dependency_package_name.to_owned(),
            manifest_path: dependency_manifest.manifest_path,
        });
    }

    Ok(dependencies)
}
