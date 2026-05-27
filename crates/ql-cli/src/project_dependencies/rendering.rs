use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

use serde_json::{Value as JsonValue, json};

use crate::cli_utils::normalize_path;

use super::{ProjectDependencyMember, ProjectDependencySelectionFailure, ProjectDependentMember};

pub(super) fn render_project_dependents(
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

pub(super) fn render_project_dependents_json(
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

pub(super) fn render_project_dependents_selection_failure_json(
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

pub(super) fn render_project_dependencies(
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

pub(super) fn render_project_dependencies_json(
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

pub(super) fn render_project_dependencies_selection_failure_json(
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
#[path = "../project_dependencies_tests.rs"]
mod tests;
