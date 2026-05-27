use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

use ql_project::{InterfaceArtifactStaleReason, InterfaceArtifactStatus, ManifestBuildProfile};
use serde_json::{Value as JsonValue, json};

use crate::cli_utils::{
    normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error,
};
use crate::project_dependencies::project_dependency_json;
use crate::project_targets::project_target_display_path;

#[cfg(test)]
use super::collection::ProjectStatusInterface;
use super::collection::{ProjectStatusMember, ProjectStatusSelectionFailure};

pub(super) fn render_project_status_preflight_failure_json(
    path: &Path,
    error: &ql_project::ProjectError,
) -> String {
    let manifest_path = project_status_load_error_manifest_path(error).map(normalize_path);
    let rendered = serde_json::to_string_pretty(&json!({
        "schema": "ql.project.status.v1",
        "path": normalize_path(path),
        "project_manifest_path": manifest_path,
        "kind": Option::<String>::None,
        "status": "failed",
        "members": [],
        "failure": {
            "kind": "preflight",
            "preflight_failure": {
                "stage": "manifest-load",
                "message": project_status_load_error_message(error),
                "manifest_path": manifest_path,
            },
        },
    }))
    .expect("project status preflight failure json should serialize");
    format!("{rendered}\n")
}

pub(super) fn render_project_status(
    manifest: &ql_project::ProjectManifest,
    members: &[ProjectStatusMember],
) -> String {
    let mut rendered = String::new();
    rendered.push_str(&format!(
        "project_manifest: {}\n",
        normalize_path(&manifest.manifest_path)
    ));
    rendered.push_str(&format!(
        "kind: {}\n",
        if manifest.workspace.is_some() {
            "workspace"
        } else {
            "package"
        }
    ));
    rendered.push_str(&format!("status: {}\n", project_status_label(members)));
    if members.is_empty() {
        rendered.push_str("members: []\n");
        return rendered;
    }

    rendered.push_str("members:\n");
    for member in members {
        rendered.push_str(&format!("  - {}\n", project_status_member_label(member)));
        rendered.push_str(&format!(
            "    manifest: {}\n",
            normalize_path(&member.manifest_path)
        ));
        if let Some(default_profile) = member.default_profile {
            rendered.push_str(&format!("    profile: {}\n", default_profile.as_str()));
        }
        rendered.push_str(&format!(
            "    interface: {} ({})\n",
            member.interface.status.label(),
            normalize_path(&member.interface.path)
        ));
        if let Some(detail) = member.interface.detail.as_deref() {
            rendered.push_str(&format!("    interface_detail: {detail}\n"));
        }
        append_project_status_stale_reasons(&mut rendered, member, "    ");
        append_project_status_targets(&mut rendered, member, "    ");
        append_project_status_dependencies(&mut rendered, member, "    ");
    }
    rendered
}

pub(super) fn render_project_status_json(
    path: &Path,
    manifest: &ql_project::ProjectManifest,
    members: &[ProjectStatusMember],
) -> String {
    let rendered = serde_json::to_string_pretty(&json!({
        "schema": "ql.project.status.v1",
        "path": normalize_path(path),
        "project_manifest_path": normalize_path(&manifest.manifest_path),
        "kind": if manifest.workspace.is_some() { "workspace" } else { "package" },
        "status": project_status_label(members),
        "members": members
            .iter()
            .map(project_status_member_json)
            .collect::<Vec<_>>(),
    }))
    .expect("project status json should serialize");
    format!("{rendered}\n")
}

pub(super) fn render_project_status_selection_failure_json(
    path: &Path,
    manifest: &ql_project::ProjectManifest,
    failure: ProjectStatusSelectionFailure,
) -> String {
    let rendered = serde_json::to_string_pretty(&json!({
        "schema": "ql.project.status.v1",
        "path": normalize_path(path),
        "project_manifest_path": normalize_path(&manifest.manifest_path),
        "kind": if manifest.workspace.is_some() { "workspace" } else { "package" },
        "status": "failed",
        "members": [],
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
    .expect("project status selection failure json should serialize");
    format!("{rendered}\n")
}

fn project_status_load_error_message(error: &ql_project::ProjectError) -> String {
    if let ql_project::ProjectError::ManifestNotFound { start } = error {
        return format!(
            "`ql project status` requires a package or workspace manifest; could not find `qlang.toml` starting from `{}`",
            normalize_path(start)
        );
    }
    if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error) {
        return format!(
            "`ql project status` manifest `{}` does not declare `[package].name`",
            normalize_path(manifest_path)
        );
    }
    format!("`ql project status` {error}")
}

fn project_status_load_error_manifest_path(error: &ql_project::ProjectError) -> Option<&Path> {
    package_missing_name_manifest_path_from_project_error(error)
        .or_else(|| package_check_manifest_path_from_project_error(error))
}

fn project_status_label(members: &[ProjectStatusMember]) -> &'static str {
    if members.iter().any(|member| {
        matches!(
            member.interface.status,
            InterfaceArtifactStatus::Invalid | InterfaceArtifactStatus::Unreadable
        )
    }) {
        return "needs-attention";
    }
    if members
        .iter()
        .any(|member| member.interface.status != InterfaceArtifactStatus::Valid)
    {
        return "needs-interface-sync";
    }
    "ok"
}

fn project_status_member_label(member: &ProjectStatusMember) -> String {
    if let Some(workspace_member) = member.member.as_deref() {
        format!("{workspace_member} ({})", member.package_name)
    } else {
        member.package_name.clone()
    }
}

fn append_project_status_stale_reasons(
    rendered: &mut String,
    member: &ProjectStatusMember,
    indent: &str,
) {
    if member.interface.stale_reasons.is_empty() {
        return;
    }

    rendered.push_str(&format!("{indent}stale_reasons:\n"));
    for reason in &member.interface.stale_reasons {
        rendered.push_str(&format!(
            "{indent}  - {}: {}\n",
            interface_stale_reason_kind(reason),
            normalize_path(interface_stale_reason_path(reason))
        ));
    }
}

fn append_project_status_targets(
    rendered: &mut String,
    member: &ProjectStatusMember,
    indent: &str,
) {
    if member.targets.is_empty() {
        rendered.push_str(&format!("{indent}targets: []\n"));
        return;
    }

    rendered.push_str(&format!("{indent}targets:\n"));
    for target in &member.targets {
        rendered.push_str(&format!(
            "{indent}  - {}: {}\n",
            target.kind.as_str(),
            project_target_display_path(&member.manifest_path, target.path.as_path())
        ));
    }
}

fn append_project_status_dependencies(
    rendered: &mut String,
    member: &ProjectStatusMember,
    indent: &str,
) {
    if member.dependencies.is_empty() {
        rendered.push_str(&format!("{indent}dependencies: []\n"));
        return;
    }

    rendered.push_str(&format!("{indent}dependencies:\n"));
    for dependency in &member.dependencies {
        if let Some(workspace_member) = dependency.member.as_deref() {
            rendered.push_str(&format!(
                "{indent}  - {} ({})\n",
                workspace_member, dependency.package_name
            ));
        } else {
            rendered.push_str(&format!(
                "{indent}  - {} ({}, local)\n",
                dependency.dependency_path, dependency.package_name
            ));
        }
    }
}

fn project_status_member_json(member: &ProjectStatusMember) -> JsonValue {
    json!({
        "member": member.member.as_deref(),
        "package_name": member.package_name.as_str(),
        "manifest_path": normalize_path(&member.manifest_path),
        "default_profile": member.default_profile.map(ManifestBuildProfile::as_str),
        "interface": {
            "path": normalize_path(&member.interface.path),
            "status": member.interface.status.label(),
            "detail": member.interface.detail.as_deref(),
            "stale_reasons": member.interface.stale_reasons
                .iter()
                .map(|reason| json!({
                    "kind": interface_stale_reason_kind(reason),
                    "path": normalize_path(interface_stale_reason_path(reason)),
                }))
                .collect::<Vec<_>>(),
        },
        "targets": member.targets
            .iter()
            .map(|target| json!({
                "kind": target.kind.as_str(),
                "path": project_target_display_path(&member.manifest_path, target.path.as_path()),
            }))
            .collect::<Vec<_>>(),
        "dependencies": member.dependencies
            .iter()
            .map(project_dependency_json)
            .collect::<Vec<_>>(),
    })
}

fn interface_stale_reason_kind(reason: &InterfaceArtifactStaleReason) -> &'static str {
    match reason {
        InterfaceArtifactStaleReason::ManifestNewer { .. } => "manifest",
        InterfaceArtifactStaleReason::SourceNewer { .. } => "source",
    }
}

fn interface_stale_reason_path(reason: &InterfaceArtifactStaleReason) -> &Path {
    match reason {
        InterfaceArtifactStaleReason::ManifestNewer { path }
        | InterfaceArtifactStaleReason::SourceNewer { path } => path,
    }
}

#[cfg(test)]
#[path = "../project_status_tests.rs"]
mod tests;
