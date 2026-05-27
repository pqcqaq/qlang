use std::path::Path;

use ql_project::{ProjectError, ProjectManifest, load_project_manifest, package_name};

pub(crate) use self::members::{
    WorkspaceMemberLookupError, WorkspacePackageSelectionFailure,
    render_workspace_member_lookup_error, resolve_selected_workspace_member_manifest,
    resolve_selected_workspace_member_manifest_for_json,
    resolve_workspace_member_entry_by_package_name, select_workspace_members,
};
use crate::cli_utils::{normalize_path, validate_project_package_name};
use crate::project_targets::resolve_project_member_request_root;

mod members;

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
