use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ql_project::{load_project_manifest, load_reference_manifests, package_name};
use serde_json::{Value as JsonValue, json};

use super::{
    find_workspace_member_entries_by_package_name, normalize_path, relative_path_from,
    resolve_project_workspace_manifest, resolve_project_workspace_member_package_name,
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
    let workspace_manifest = resolve_project_workspace_manifest(path).map_err(|message| {
        eprintln!("error: `ql project dependents` {message}");
        1
    })?;
    let package_name = resolve_project_workspace_member_package_name(
        path,
        package_name,
        "`ql project dependents`",
    )?;
    let member_entries =
        find_workspace_member_entries_by_package_name(&workspace_manifest, &package_name);
    if member_entries.is_empty() {
        eprintln!(
            "error: `ql project dependents` workspace manifest `{}` does not contain package `{package_name}`",
            normalize_path(&workspace_manifest.manifest_path)
        );
        return Err(1);
    }
    if member_entries.len() > 1 {
        let matching_members = member_entries
            .iter()
            .map(|(member, _)| member.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!(
            "error: `ql project dependents` workspace manifest `{}` contains multiple members for package `{package_name}`: {matching_members}",
            normalize_path(&workspace_manifest.manifest_path)
        );
        return Err(1);
    }

    let (_, member_manifest_path) = &member_entries[0];
    let dependents = find_workspace_member_dependents(&workspace_manifest, member_manifest_path)
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
    let workspace_manifest = resolve_project_workspace_manifest(path).map_err(|message| {
        eprintln!("error: `ql project dependencies` {message}");
        1
    })?;
    let package_name = resolve_project_workspace_member_package_name(
        path,
        package_name,
        "`ql project dependencies`",
    )?;
    let member_entries =
        find_workspace_member_entries_by_package_name(&workspace_manifest, &package_name);
    if member_entries.is_empty() {
        eprintln!(
            "error: `ql project dependencies` workspace manifest `{}` does not contain package `{package_name}`",
            normalize_path(&workspace_manifest.manifest_path)
        );
        return Err(1);
    }
    if member_entries.len() > 1 {
        let matching_members = member_entries
            .iter()
            .map(|(member, _)| member.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!(
            "error: `ql project dependencies` workspace manifest `{}` contains multiple members for package `{package_name}`: {matching_members}",
            normalize_path(&workspace_manifest.manifest_path)
        );
        return Err(1);
    }

    let (_, member_manifest_path) = &member_entries[0];
    let dependencies =
        find_workspace_member_dependencies(&workspace_manifest, member_manifest_path).map_err(
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
