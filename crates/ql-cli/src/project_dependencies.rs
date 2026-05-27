use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ql_project::{load_project_manifest, load_reference_manifests, package_name};

use crate::cli_utils::{normalize_path, relative_path_from};

mod query_context;
mod rendering;

use query_context::{ProjectDependencyQueryContextError, resolve_project_dependency_query_context};
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
