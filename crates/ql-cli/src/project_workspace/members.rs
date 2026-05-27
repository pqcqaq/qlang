use std::path::{Path, PathBuf};

use ql_project::{ProjectError, ProjectManifest, load_project_manifest, package_name};

use crate::cli_utils::{
    normalize_path, package_missing_name_manifest_path_from_project_error,
    validate_project_package_name,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WorkspaceMemberLookupError {
    Missing,
    Ambiguous { matches: Vec<String> },
    InspectionFailure { member: String, message: String },
}

pub(crate) struct WorkspacePackageSelectionFailure {
    pub(crate) message: String,
    pub(crate) selector: String,
    pub(crate) target_count: Option<usize>,
}

pub(crate) fn select_workspace_members(
    manifest: &ProjectManifest,
    request_path: &Path,
    package_name: Option<&str>,
    command_label: &str,
    selector_option: &str,
) -> Result<Vec<String>, u8> {
    let Some(workspace) = &manifest.workspace else {
        return Ok(Vec::new());
    };
    let Some(package_name) = package_name else {
        return Ok(workspace.members.clone());
    };
    if let Err(message) = validate_project_package_name(package_name) {
        eprintln!("error: {command_label} {message}");
        return Err(1);
    }

    let matching_members = find_workspace_member_entries_by_package_name(manifest, package_name)
        .map_err(|error| {
            eprintln!(
                "error: {command_label} {}",
                render_workspace_member_lookup_error(manifest, package_name, &error)
            );
            1
        })?;
    if matching_members.is_empty() {
        let normalized_path = normalize_path(request_path);
        let rerun_command = format!("{} {normalized_path}", command_label.trim_matches('`'));
        eprintln!(
            "error: {command_label} package selector matched no workspace members under `{normalized_path}`"
        );
        eprintln!("note: selector: package `{package_name}`");
        eprintln!(
            "hint: rerun `{rerun_command}` to inspect all workspace members, or adjust `{selector_option}`"
        );
        return Err(1);
    }
    if matching_members.len() > 1 {
        let manifest_path = normalize_path(&manifest.manifest_path);
        let rendered_members = matching_members
            .iter()
            .map(|(member, member_manifest)| {
                format!("{member} ({})", normalize_path(member_manifest))
            })
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!(
            "error: {command_label} workspace manifest `{manifest_path}` contains multiple members for package `{package_name}`: {rendered_members}"
        );
        return Err(1);
    }

    Ok(vec![
        matching_members
            .into_iter()
            .next()
            .expect("non-empty workspace check package matches should contain one entry")
            .0,
    ])
}

pub(crate) fn resolve_selected_workspace_member_manifest(
    workspace_manifest: &ProjectManifest,
    request_path: &Path,
    package_name: &str,
    command_label: &str,
    selector_option: &str,
) -> Result<(String, ProjectManifest), u8> {
    let selected_member = select_workspace_members(
        workspace_manifest,
        request_path,
        Some(package_name),
        command_label,
        selector_option,
    )?
    .into_iter()
    .next()
    .expect("workspace member selector should resolve exactly one member");
    let workspace_root = workspace_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    let member_manifest =
        load_project_manifest(&workspace_root.join(&selected_member)).map_err(|error| {
            eprintln!(
                "error: {command_label} failed to inspect workspace member `{selected_member}`: {}",
                normalize_workspace_member_package_error(&error)
            );
            1
        })?;
    Ok((selected_member, member_manifest))
}

pub(crate) fn resolve_selected_workspace_member_manifest_for_json(
    workspace_manifest: &ProjectManifest,
    package_name: &str,
    command_label: &str,
) -> Result<(String, ProjectManifest), WorkspacePackageSelectionFailure> {
    let (member, member_manifest_path) =
        resolve_workspace_member_entry_by_package_name(workspace_manifest, package_name).map_err(
            |error| {
                workspace_package_selection_failure_from_lookup(
                    workspace_manifest,
                    package_name,
                    command_label,
                    &error,
                )
            },
        )?;
    let member_manifest = load_project_manifest(&member_manifest_path).map_err(|error| {
        WorkspacePackageSelectionFailure {
            message: format!(
                "{command_label} failed to inspect selected workspace member `{member}`: {error}"
            ),
            selector: format!("package `{package_name}`"),
            target_count: None,
        }
    })?;
    Ok((member, member_manifest))
}

fn workspace_package_selection_failure_from_lookup(
    workspace_manifest: &ProjectManifest,
    package_name: &str,
    command_label: &str,
    error: &WorkspaceMemberLookupError,
) -> WorkspacePackageSelectionFailure {
    WorkspacePackageSelectionFailure {
        message: render_workspace_package_selection_failure_message(
            workspace_manifest,
            package_name,
            command_label,
            error,
        ),
        selector: format!("package `{package_name}`"),
        target_count: workspace_member_lookup_target_count(error),
    }
}

fn render_workspace_package_selection_failure_message(
    workspace_manifest: &ProjectManifest,
    package_name: &str,
    command_label: &str,
    error: &WorkspaceMemberLookupError,
) -> String {
    match error {
        WorkspaceMemberLookupError::Missing => format!(
            "{command_label} package selector matched no workspace members under `{}`",
            normalize_path(
                workspace_manifest
                    .manifest_path
                    .parent()
                    .unwrap_or(Path::new("."))
            )
        ),
        _ => format!(
            "{command_label} {}",
            render_workspace_member_lookup_error(workspace_manifest, package_name, error)
        ),
    }
}

fn workspace_member_lookup_target_count(error: &WorkspaceMemberLookupError) -> Option<usize> {
    match error {
        WorkspaceMemberLookupError::Missing => Some(0),
        WorkspaceMemberLookupError::Ambiguous { matches } => Some(matches.len()),
        WorkspaceMemberLookupError::InspectionFailure { .. } => None,
    }
}

pub(crate) fn resolve_workspace_member_entry_by_package_name(
    workspace_manifest: &ProjectManifest,
    wanted_package_name: &str,
) -> Result<(String, PathBuf), WorkspaceMemberLookupError> {
    let member_entries =
        find_workspace_member_entries_by_package_name(workspace_manifest, wanted_package_name)?;
    if member_entries.is_empty() {
        return Err(WorkspaceMemberLookupError::Missing);
    }
    if member_entries.len() > 1 {
        return Err(WorkspaceMemberLookupError::Ambiguous {
            matches: member_entries
                .into_iter()
                .map(|(member, _)| member)
                .collect(),
        });
    }

    Ok(member_entries
        .into_iter()
        .next()
        .expect("non-empty workspace member lookup should contain one entry"))
}

pub(crate) fn render_workspace_member_lookup_error(
    workspace_manifest: &ProjectManifest,
    package_name: &str,
    error: &WorkspaceMemberLookupError,
) -> String {
    match error {
        WorkspaceMemberLookupError::Missing => format!(
            "workspace manifest `{}` does not contain package `{package_name}`",
            normalize_path(&workspace_manifest.manifest_path)
        ),
        WorkspaceMemberLookupError::Ambiguous { matches } => format!(
            "workspace manifest `{}` contains multiple members for package `{package_name}`: {}",
            normalize_path(&workspace_manifest.manifest_path),
            matches.join(", ")
        ),
        WorkspaceMemberLookupError::InspectionFailure { member, message } => {
            format!("failed to inspect workspace member `{member}`: {message}")
        }
    }
}

pub(crate) fn find_workspace_member_entries_by_package_name(
    workspace_manifest: &ProjectManifest,
    wanted_package_name: &str,
) -> Result<Vec<(String, PathBuf)>, WorkspaceMemberLookupError> {
    let workspace_root = workspace_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    let Some(workspace) = workspace_manifest.workspace.as_ref() else {
        return Ok(Vec::new());
    };
    let mut member_entries = Vec::new();
    for member in &workspace.members {
        let member_manifest =
            load_project_manifest(&workspace_root.join(member)).map_err(|error| {
                WorkspaceMemberLookupError::InspectionFailure {
                    member: member.clone(),
                    message: normalize_workspace_member_package_error(&error),
                }
            })?;
        let existing_package_name = package_name(&member_manifest).map_err(|error| {
            WorkspaceMemberLookupError::InspectionFailure {
                member: member.clone(),
                message: normalize_workspace_member_package_error(&error),
            }
        })?;
        if existing_package_name == wanted_package_name {
            member_entries.push((member.clone(), member_manifest.manifest_path));
        }
    }
    Ok(member_entries)
}

fn normalize_workspace_member_package_error(error: &ProjectError) -> String {
    if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error) {
        return format!(
            "manifest `{}` does not declare `[package].name`",
            normalize_path(manifest_path)
        );
    }
    error.to_string()
}
