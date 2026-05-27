use std::path::{Path, PathBuf};

use ql_project::{
    BuildTarget, InterfaceArtifactStaleReason, InterfaceArtifactStatus, ManifestBuildProfile,
    default_interface_path, discover_package_build_targets, interface_artifact_stale_reasons,
    interface_artifact_status, interface_artifact_status_detail, load_project_manifest,
    package_name,
};

use crate::cli_utils::{normalize_path, validate_project_package_name};
use crate::project_dependencies::{ProjectDependencyMember, find_workspace_member_dependencies};
use crate::project_workspace::{
    WorkspacePackageSelectionFailure, resolve_selected_workspace_member_manifest,
    resolve_selected_workspace_member_manifest_for_json,
};

pub(super) struct ProjectStatusMember {
    pub(super) member: Option<String>,
    pub(super) package_name: String,
    pub(super) manifest_path: PathBuf,
    pub(super) default_profile: Option<ManifestBuildProfile>,
    pub(super) targets: Vec<BuildTarget>,
    pub(super) dependencies: Vec<ProjectDependencyMember>,
    pub(super) interface: ProjectStatusInterface,
}

pub(super) struct ProjectStatusInterface {
    pub(super) path: PathBuf,
    pub(super) status: InterfaceArtifactStatus,
    pub(super) detail: Option<String>,
    pub(super) stale_reasons: Vec<InterfaceArtifactStaleReason>,
}

pub(super) enum ProjectStatusMemberSelectionError {
    Message(String),
    Json(ProjectStatusSelectionFailure),
    Exit(u8),
}

pub(super) struct ProjectStatusSelectionFailure {
    pub(super) message: String,
    pub(super) selector: Option<String>,
    pub(super) target_count: Option<usize>,
}

pub(super) fn collect_project_status_members(
    manifest: &ql_project::ProjectManifest,
    request_path: &Path,
    selected_package_name: Option<&str>,
    json: bool,
) -> Result<Vec<ProjectStatusMember>, ProjectStatusMemberSelectionError> {
    if let Some(selected_package_name) = selected_package_name {
        if let Err(message) = validate_project_package_name(selected_package_name) {
            if json {
                return Err(ProjectStatusMemberSelectionError::Json(
                    ProjectStatusSelectionFailure {
                        message: format!("`ql project status` {message}"),
                        selector: Some(format!("package `{selected_package_name}`")),
                        target_count: None,
                    },
                ));
            }
            return Err(ProjectStatusMemberSelectionError::Message(message));
        }
    }

    if let Some(workspace) = manifest.workspace.as_ref() {
        let workspace_root = manifest.manifest_path.parent().unwrap_or(Path::new("."));
        let workspace_profile = manifest.profile.as_ref().map(|profile| profile.default);
        if let Some(selected_package_name) = selected_package_name {
            let (member, member_manifest) = if json {
                resolve_project_status_workspace_member_json(manifest, selected_package_name)?
            } else {
                resolve_selected_workspace_member_manifest(
                    manifest,
                    request_path,
                    selected_package_name,
                    "`ql project status`",
                    "--package",
                )
                .map_err(ProjectStatusMemberSelectionError::Exit)?
            };
            let default_profile = member_manifest
                .profile
                .as_ref()
                .map(|profile| profile.default)
                .or(workspace_profile);
            return Ok(vec![
                collect_project_status_member(
                    manifest,
                    Some(member),
                    &member_manifest,
                    default_profile,
                )
                .map_err(ProjectStatusMemberSelectionError::Message)?,
            ]);
        }

        let mut members = Vec::new();
        for member in &workspace.members {
            let member_manifest = load_project_manifest(&workspace_root.join(member))
                .map_err(|error| format!("failed to inspect workspace member `{member}`: {error}"))
                .map_err(ProjectStatusMemberSelectionError::Message)?;
            let member_package_name = package_name(&member_manifest)
                .map_err(|error| format!("failed to inspect workspace member `{member}`: {error}"))
                .map_err(ProjectStatusMemberSelectionError::Message)?;
            if selected_package_name.is_some_and(|expected| expected != member_package_name) {
                continue;
            }

            let default_profile = member_manifest
                .profile
                .as_ref()
                .map(|profile| profile.default)
                .or(workspace_profile);
            members.push(
                collect_project_status_member(
                    manifest,
                    Some(member.clone()),
                    &member_manifest,
                    default_profile,
                )
                .map_err(ProjectStatusMemberSelectionError::Message)?,
            );
        }

        return Ok(members);
    }

    let actual_package_name = package_name(manifest)
        .map_err(|error| format!("failed to inspect package: {error}"))
        .map_err(ProjectStatusMemberSelectionError::Message)?;
    if let Some(selected_package_name) = selected_package_name {
        if selected_package_name != actual_package_name {
            if json {
                return Err(ProjectStatusMemberSelectionError::Json(
                    ProjectStatusSelectionFailure {
                        message: format!(
                            "`ql project status` package selector expected `{selected_package_name}` but `{}` resolves to package `{actual_package_name}`",
                            normalize_path(&manifest.manifest_path)
                        ),
                        selector: Some(format!("package `{selected_package_name}`")),
                        target_count: Some(0),
                    },
                ));
            }
            return Err(ProjectStatusMemberSelectionError::Message(format!(
                "package selector expected `{selected_package_name}` but `{}` resolves to package `{actual_package_name}`",
                normalize_path(&manifest.manifest_path)
            )));
        }
    }
    Ok(vec![
        collect_project_status_member(
            manifest,
            None,
            manifest,
            manifest.profile.as_ref().map(|profile| profile.default),
        )
        .map_err(ProjectStatusMemberSelectionError::Message)?,
    ])
}

fn resolve_project_status_workspace_member_json(
    manifest: &ql_project::ProjectManifest,
    selected_package_name: &str,
) -> Result<(String, ql_project::ProjectManifest), ProjectStatusMemberSelectionError> {
    resolve_selected_workspace_member_manifest_for_json(
        manifest,
        selected_package_name,
        "`ql project status`",
    )
    .map_err(|failure| ProjectStatusMemberSelectionError::Json(failure.into()))
}

impl From<WorkspacePackageSelectionFailure> for ProjectStatusSelectionFailure {
    fn from(failure: WorkspacePackageSelectionFailure) -> Self {
        Self {
            message: failure.message,
            selector: Some(failure.selector),
            target_count: failure.target_count,
        }
    }
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
