use std::path::{Path, PathBuf};

use ql_project::{ProjectError, ProjectManifest, load_project_manifest, package_name};

use super::{
    normalize_path, package_missing_name_manifest_path_from_project_error,
    resolve_project_member_request_root, validate_project_package_name,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WorkspaceMemberLookupError {
    Missing,
    Ambiguous { matches: Vec<String> },
    InspectionFailure { member: String, message: String },
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

pub(crate) fn resolve_project_selected_package_manifest(
    path: &Path,
    target_package_name: Option<&str>,
    command_label: &str,
) -> Result<(ProjectManifest, ProjectManifest), u8> {
    if let Some(target_package_name) = target_package_name {
        let workspace_manifest = resolve_project_workspace_manifest(path).map_err(|message| {
            eprintln!("error: {command_label} {message}");
            1
        })?;
        if let Err(message) = validate_project_package_name(target_package_name) {
            eprintln!("error: {command_label} {message}");
            return Err(1);
        }

        let (_, member_manifest_path) = resolve_workspace_member_entry_by_package_name(
            &workspace_manifest,
            target_package_name,
        )
        .map_err(|error| {
            eprintln!(
                "error: {command_label} {}",
                render_workspace_member_lookup_error(
                    &workspace_manifest,
                    target_package_name,
                    &error,
                )
            );
            1
        })?;
        let package_manifest = load_project_manifest(&member_manifest_path).map_err(|error| {
            eprintln!("error: {command_label} {error}");
            1
        })?;
        return Ok((workspace_manifest, package_manifest));
    }

    let package_manifest = resolve_project_package_manifest(path).map_err(|message| {
        eprintln!("error: {command_label} {message}");
        1
    })?;
    let workspace_manifest =
        resolve_project_workspace_manifest(path).unwrap_or_else(|_| package_manifest.clone());
    Ok((workspace_manifest, package_manifest))
}

pub(crate) fn resolve_project_workspace_manifest(path: &Path) -> Result<ProjectManifest, String> {
    let manifest = load_project_manifest(path)
        .map_err(|error| project_workspace_manifest_error(path, &error))?;
    let workspace_manifest_path = if manifest.workspace.is_some() {
        manifest.manifest_path.clone()
    } else {
        resolve_project_member_request_root(&manifest.manifest_path)
    };
    let workspace_manifest = load_project_manifest(&workspace_manifest_path)
        .map_err(|error| project_workspace_manifest_error(path, &error))?;

    if workspace_manifest.workspace.is_none() {
        return Err(format!(
            "requires an existing workspace manifest; `{}` resolves to package manifest `{}`",
            normalize_path(path),
            normalize_path(&workspace_manifest.manifest_path)
        ));
    }

    Ok(workspace_manifest)
}

pub(crate) fn resolve_project_package_manifest(path: &Path) -> Result<ProjectManifest, String> {
    let manifest = load_project_manifest(path)
        .map_err(|error| project_workspace_manifest_error(path, &error))?;
    if manifest.package.is_none() {
        return Err(format!(
            "requires an existing package manifest; `{}` resolves to workspace manifest `{}`",
            normalize_path(path),
            normalize_path(&manifest.manifest_path)
        ));
    }
    Ok(manifest)
}

pub(crate) fn resolve_project_workspace_member_package_name(
    path: &Path,
    selected_package_name: Option<&str>,
    command_label: &str,
) -> Result<String, u8> {
    if let Some(package_name) = selected_package_name {
        if let Err(message) = validate_project_package_name(package_name) {
            eprintln!("error: {command_label} {message}");
            return Err(1);
        }
        return Ok(package_name.to_owned());
    }

    let package_manifest = load_project_manifest(path).map_err(|error| {
        eprintln!(
            "error: {command_label} {}",
            project_workspace_manifest_error(path, &error)
        );
        1
    })?;
    if package_manifest.package.is_none() {
        eprintln!(
            "error: {command_label} could not derive a package name from `{}`; rerun with `--name <package>`",
            normalize_path(path)
        );
        return Err(1);
    }

    package_name(&package_manifest)
        .map(str::to_owned)
        .map_err(|error| {
            eprintln!("error: {command_label} {error}");
            1
        })
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

fn project_workspace_manifest_error(path: &Path, error: &ProjectError) -> String {
    match error {
        ProjectError::ManifestNotFound { start } => format!(
            "requires a package or workspace manifest; could not find `qlang.toml` starting from `{}`",
            normalize_path(start)
        ),
        ProjectError::PackageSourceRootNotFound {
            path: manifest_path,
        } => format!(
            "manifest `{}` does not have a project source root discoverable from `{}`",
            normalize_path(manifest_path),
            normalize_path(path)
        ),
        other => other.to_string(),
    }
}
