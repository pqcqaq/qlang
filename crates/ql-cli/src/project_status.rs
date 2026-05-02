use std::path::{Path, PathBuf};

use ql_project::{
    BuildTarget, InterfaceArtifactStaleReason, InterfaceArtifactStatus, ManifestBuildProfile,
    default_interface_path, discover_package_build_targets, interface_artifact_stale_reasons,
    interface_artifact_status, interface_artifact_status_detail, load_project_manifest,
    package_name,
};
use serde_json::{Value as JsonValue, json};

use super::{
    normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error, project_target_display_path,
    resolve_project_workspace_member_command_request_root, validate_project_package_name,
};

use crate::project_dependencies::{
    ProjectDependencyMember, find_workspace_member_dependencies, project_dependency_json,
};

struct ProjectStatusMember {
    member: Option<String>,
    package_name: String,
    manifest_path: PathBuf,
    default_profile: Option<ManifestBuildProfile>,
    targets: Vec<BuildTarget>,
    dependencies: Vec<ProjectDependencyMember>,
    interface: ProjectStatusInterface,
}

struct ProjectStatusInterface {
    path: PathBuf,
    status: InterfaceArtifactStatus,
    detail: Option<String>,
    stale_reasons: Vec<InterfaceArtifactStaleReason>,
}

pub(crate) fn project_status_path(
    path: &Path,
    package_name: Option<&str>,
    json: bool,
) -> Result<(), u8> {
    let request_root = resolve_project_workspace_member_command_request_root(path);
    let manifest = load_project_manifest(request_root.as_deref().unwrap_or(path))
        .map_err(|error| report_project_status_load_error(path, &error))?;
    let members = collect_project_status_members(&manifest, package_name).map_err(|message| {
        eprintln!("error: `ql project status` {message}");
        1
    })?;
    let rendered = if json {
        render_project_status_json(path, &manifest, &members)
    } else {
        render_project_status(&manifest, &members)
    };
    print!("{rendered}");
    Ok(())
}

fn report_project_status_load_error(path: &Path, error: &ql_project::ProjectError) -> u8 {
    if let ql_project::ProjectError::ManifestNotFound { start } = error {
        eprintln!(
            "error: `ql project status` requires a package or workspace manifest; could not find `qlang.toml` starting from `{}`",
            normalize_path(start)
        );
    } else if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error)
    {
        eprintln!(
            "error: `ql project status` manifest `{}` does not declare `[package].name`",
            normalize_path(manifest_path)
        );
    } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(error) {
        eprintln!("error: `ql project status` {error}");
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(manifest_path)
        );
    } else {
        eprintln!("error: `ql project status` {error}");
    }
    eprintln!("note: requested path: {}", normalize_path(path));
    1
}

fn collect_project_status_members(
    manifest: &ql_project::ProjectManifest,
    selected_package_name: Option<&str>,
) -> Result<Vec<ProjectStatusMember>, String> {
    if let Some(selected_package_name) = selected_package_name {
        validate_project_package_name(selected_package_name)?;
    }

    if let Some(workspace) = manifest.workspace.as_ref() {
        let workspace_root = manifest.manifest_path.parent().unwrap_or(Path::new("."));
        let workspace_profile = manifest.profile.as_ref().map(|profile| profile.default);
        let mut members = Vec::new();
        for member in &workspace.members {
            let member_manifest =
                load_project_manifest(&workspace_root.join(member)).map_err(|error| {
                    format!("failed to inspect workspace member `{member}`: {error}")
                })?;
            let member_package_name = package_name(&member_manifest).map_err(|error| {
                format!("failed to inspect workspace member `{member}`: {error}")
            })?;
            if selected_package_name.is_some_and(|expected| expected != member_package_name) {
                continue;
            }

            let default_profile = member_manifest
                .profile
                .as_ref()
                .map(|profile| profile.default)
                .or(workspace_profile);
            members.push(collect_project_status_member(
                manifest,
                Some(member.clone()),
                &member_manifest,
                default_profile,
            )?);
        }

        if let Some(selected_package_name) = selected_package_name {
            if members.is_empty() {
                return Err(format!(
                    "workspace manifest `{}` does not contain package `{selected_package_name}`",
                    normalize_path(&manifest.manifest_path)
                ));
            }
        }
        return Ok(members);
    }

    let actual_package_name =
        package_name(manifest).map_err(|error| format!("failed to inspect package: {error}"))?;
    if let Some(selected_package_name) = selected_package_name {
        if selected_package_name != actual_package_name {
            return Err(format!(
                "package selector expected `{selected_package_name}` but `{}` resolves to package `{actual_package_name}`",
                normalize_path(&manifest.manifest_path)
            ));
        }
    }
    Ok(vec![collect_project_status_member(
        manifest,
        None,
        manifest,
        manifest.profile.as_ref().map(|profile| profile.default),
    )?])
}

fn collect_project_status_member(
    root_manifest: &ql_project::ProjectManifest,
    member: Option<String>,
    member_manifest: &ql_project::ProjectManifest,
    default_profile: Option<ManifestBuildProfile>,
) -> Result<ProjectStatusMember, String> {
    let package_name = package_name(member_manifest)
        .map_err(|error| format!("failed to inspect package manifest: {error}"))?;
    let targets = discover_package_build_targets(member_manifest).map_err(|error| {
        format!("failed to discover build targets for package `{package_name}`: {error}")
    })?;
    let dependencies =
        find_workspace_member_dependencies(root_manifest, &member_manifest.manifest_path)?;
    let interface_path = default_interface_path(member_manifest)
        .map_err(|error| format!("failed to resolve default interface path: {error}"))?;
    let status = interface_artifact_status(member_manifest, &interface_path);
    let detail = interface_artifact_status_detail(&interface_path, status);
    let stale_reasons = if status == InterfaceArtifactStatus::Stale {
        interface_artifact_stale_reasons(member_manifest, &interface_path)
    } else {
        Vec::new()
    };

    Ok(ProjectStatusMember {
        member,
        package_name: package_name.to_owned(),
        manifest_path: member_manifest.manifest_path.clone(),
        default_profile,
        targets,
        dependencies,
        interface: ProjectStatusInterface {
            path: interface_path,
            status,
            detail,
            stale_reasons,
        },
    })
}

fn render_project_status(
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

fn render_project_status_json(
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
mod tests {
    use super::*;

    fn member_with_status(status: InterfaceArtifactStatus) -> ProjectStatusMember {
        ProjectStatusMember {
            member: Some("packages/app".to_owned()),
            package_name: "app".to_owned(),
            manifest_path: PathBuf::from("packages/app/qlang.toml"),
            default_profile: None,
            targets: Vec::new(),
            dependencies: Vec::new(),
            interface: ProjectStatusInterface {
                path: PathBuf::from("packages/app/target/qlang/app.qi"),
                status,
                detail: None,
                stale_reasons: Vec::new(),
            },
        }
    }

    #[test]
    fn status_label_prioritizes_invalid_or_unreadable_interfaces() {
        let members = vec![
            member_with_status(InterfaceArtifactStatus::Valid),
            member_with_status(InterfaceArtifactStatus::Invalid),
        ];

        assert_eq!(project_status_label(&members), "needs-attention");
    }

    #[test]
    fn status_label_reports_interface_sync_for_missing_or_stale_interfaces() {
        let members = vec![member_with_status(InterfaceArtifactStatus::Missing)];

        assert_eq!(project_status_label(&members), "needs-interface-sync");
    }

    #[test]
    fn member_label_includes_workspace_member_when_present() {
        let member = member_with_status(InterfaceArtifactStatus::Valid);

        assert_eq!(project_status_member_label(&member), "packages/app (app)");
    }
}
