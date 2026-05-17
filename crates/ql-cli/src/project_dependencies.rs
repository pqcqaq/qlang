use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ql_project::{ProjectManifest, load_project_manifest, load_reference_manifests, package_name};
use serde_json::{Value as JsonValue, json};

use crate::project_workspace::{
    WorkspacePackageSelectionFailure, resolve_selected_workspace_member_manifest_for_json,
};

use super::{
    normalize_path, relative_path_from, resolve_project_package_manifest,
    resolve_project_workspace_manifest, resolve_project_workspace_member_package_name,
    resolve_selected_workspace_member_manifest, validate_project_package_name,
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
            false,
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
            if allow_standalone_package && package_name.is_none() {
                let package_manifest =
                    resolve_project_package_manifest(path).map_err(|message| {
                        eprintln!("error: {command_label} {message}");
                        ProjectDependencyQueryContextError::Exit(1)
                    })?;
                let package_name = ql_project::package_name(&package_manifest)
                    .map_err(|error| {
                        eprintln!("error: {command_label} failed to inspect package: {error}");
                        ProjectDependencyQueryContextError::Exit(1)
                    })?
                    .to_owned();
                return Ok((
                    package_manifest.clone(),
                    package_name,
                    package_manifest.manifest_path,
                ));
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

fn render_project_dependents(
    workspace_manifest: &ql_project::ProjectManifest,
    package_name: &str,
    dependents: &[ProjectDependentMember],
) -> String {
    let mut rendered = String::new();
    rendered.push_str(&format!(
        "workspace_manifest: {}\n",
        normalize_path(&workspace_manifest.manifest_path)
    ));
    rendered.push_str(&format!("package: {package_name}\n"));
    if dependents.is_empty() {
        rendered.push_str("dependents: []\n");
        return rendered;
    }

    rendered.push_str("dependents:\n");
    for dependent in dependents {
        rendered.push_str(&format!(
            "  - {} ({})\n",
            dependent.member, dependent.package_name
        ));
    }
    rendered
}

fn render_project_dependents_json(
    path: &Path,
    workspace_manifest: &ql_project::ProjectManifest,
    package_name: &str,
    dependents: &[ProjectDependentMember],
) -> String {
    let rendered = serde_json::to_string_pretty(&json!({
        "schema": "ql.project.dependents.v1",
        "path": normalize_path(path),
        "workspace_manifest_path": normalize_path(&workspace_manifest.manifest_path),
        "package_name": package_name,
        "dependents": dependents
            .iter()
            .map(|dependent| json!({
                "member": dependent.member,
                "package_name": dependent.package_name,
                "manifest_path": normalize_path(&dependent.manifest_path),
            }))
            .collect::<Vec<_>>(),
    }))
    .expect("project dependents json should serialize");
    format!("{rendered}\n")
}

fn render_project_dependents_selection_failure_json(
    path: &Path,
    workspace_manifest: &ql_project::ProjectManifest,
    package_name: &str,
    failure: ProjectDependencySelectionFailure,
) -> String {
    let rendered = serde_json::to_string_pretty(&json!({
        "schema": "ql.project.dependents.v1",
        "path": normalize_path(path),
        "workspace_manifest_path": normalize_path(&workspace_manifest.manifest_path),
        "package_name": package_name,
        "dependents": [],
        "failure": project_dependency_selection_failure_json(failure),
    }))
    .expect("project dependents selection failure json should serialize");
    format!("{rendered}\n")
}

fn render_project_dependencies(
    workspace_manifest: &ql_project::ProjectManifest,
    package_name: &str,
    dependencies: &[ProjectDependencyMember],
) -> String {
    let mut rendered = String::new();
    rendered.push_str(&format!(
        "workspace_manifest: {}\n",
        normalize_path(&workspace_manifest.manifest_path)
    ));
    rendered.push_str(&format!("package: {package_name}\n"));
    if dependencies.is_empty() {
        rendered.push_str("dependencies: []\n");
        return rendered;
    }

    rendered.push_str("dependencies:\n");
    for dependency in dependencies {
        if let Some(member) = dependency.member.as_deref() {
            rendered.push_str(&format!("  - {} ({})\n", member, dependency.package_name));
        } else {
            rendered.push_str(&format!(
                "  - {} ({}, local)\n",
                dependency.dependency_path, dependency.package_name
            ));
        }
    }
    rendered
}

fn render_project_dependencies_json(
    path: &Path,
    workspace_manifest: &ql_project::ProjectManifest,
    package_name: &str,
    dependencies: &[ProjectDependencyMember],
) -> String {
    let rendered = serde_json::to_string_pretty(&json!({
        "schema": "ql.project.dependencies.v1",
        "path": normalize_path(path),
        "workspace_manifest_path": normalize_path(&workspace_manifest.manifest_path),
        "package_name": package_name,
        "dependencies": dependencies
            .iter()
            .map(project_dependency_json)
            .collect::<Vec<_>>(),
    }))
    .expect("project dependencies json should serialize");
    format!("{rendered}\n")
}

fn render_project_dependencies_selection_failure_json(
    path: &Path,
    workspace_manifest: &ql_project::ProjectManifest,
    package_name: &str,
    failure: ProjectDependencySelectionFailure,
) -> String {
    let rendered = serde_json::to_string_pretty(&json!({
        "schema": "ql.project.dependencies.v1",
        "path": normalize_path(path),
        "workspace_manifest_path": normalize_path(&workspace_manifest.manifest_path),
        "package_name": package_name,
        "dependencies": [],
        "failure": project_dependency_selection_failure_json(failure),
    }))
    .expect("project dependencies selection failure json should serialize");
    format!("{rendered}\n")
}

fn project_dependency_selection_failure_json(
    failure: ProjectDependencySelectionFailure,
) -> JsonValue {
    json!({
        "kind": "selection",
        "selection_failure": {
            "stage": "package-selection",
            "message": failure.message,
            "selector": failure.selector,
            "target_count": failure.target_count,
        },
    })
}

pub(crate) fn project_dependency_json(dependency: &ProjectDependencyMember) -> JsonValue {
    json!({
        "kind": if dependency.member.is_some() { "workspace" } else { "local" },
        "member": dependency.member.as_deref(),
        "dependency_path": dependency.dependency_path.as_str(),
        "package_name": dependency.package_name.as_str(),
        "manifest_path": normalize_path(&dependency.manifest_path),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_manifest() -> ql_project::ProjectManifest {
        ql_project::ProjectManifest {
            manifest_path: PathBuf::from("workspace/qlang.toml"),
            package: None,
            workspace: None,
            references: ql_project::ReferencesManifest::default(),
            profile: None,
            lib: None,
            bins: Vec::new(),
        }
    }

    #[test]
    fn dependencies_text_marks_workspace_and_external_local_dependencies() {
        let dependencies = vec![
            ProjectDependencyMember {
                member: Some("packages/core".to_owned()),
                dependency_path: "../core".to_owned(),
                package_name: "core".to_owned(),
                manifest_path: PathBuf::from("workspace/packages/core/qlang.toml"),
            },
            ProjectDependencyMember {
                member: None,
                dependency_path: "../vendor/log".to_owned(),
                package_name: "log".to_owned(),
                manifest_path: PathBuf::from("vendor/log/qlang.toml"),
            },
        ];

        let rendered = render_project_dependencies(&workspace_manifest(), "app", &dependencies);

        assert!(rendered.contains("  - packages/core (core)\n"));
        assert!(rendered.contains("  - ../vendor/log (log, local)\n"));
    }

    #[test]
    fn dependents_json_renders_stable_schema() {
        let dependents = vec![ProjectDependentMember {
            member: "packages/app".to_owned(),
            package_name: "app".to_owned(),
            manifest_path: PathBuf::from("workspace/packages/app/qlang.toml"),
        }];

        let rendered = render_project_dependents_json(
            Path::new("workspace"),
            &workspace_manifest(),
            "core",
            &dependents,
        );

        assert!(rendered.contains("\"schema\": \"ql.project.dependents.v1\""));
        assert!(rendered.contains("\"package_name\": \"core\""));
        assert!(rendered.contains("\"member\": \"packages/app\""));
    }

    #[test]
    fn dependency_json_marks_dependency_kind() {
        let workspace_dependency = ProjectDependencyMember {
            member: Some("packages/core".to_owned()),
            dependency_path: "../core".to_owned(),
            package_name: "core".to_owned(),
            manifest_path: PathBuf::from("workspace/packages/core/qlang.toml"),
        };
        let local_dependency = ProjectDependencyMember {
            member: None,
            dependency_path: "../vendor/log".to_owned(),
            package_name: "log".to_owned(),
            manifest_path: PathBuf::from("vendor/log/qlang.toml"),
        };

        assert_eq!(
            project_dependency_json(&workspace_dependency)["kind"],
            "workspace"
        );
        assert_eq!(project_dependency_json(&local_dependency)["kind"], "local");
    }
}
