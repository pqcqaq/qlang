use std::path::Path;

use ql_project::{WorkspaceBuildTargets, discover_workspace_build_targets, load_project_manifest};

use crate::cli_utils::{
    normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error,
};

use super::{
    rendering::render_project_targets_preflight_failure_json,
    resolve_project_workspace_member_command_request_root,
};

pub(super) fn load_project_target_members_for_workspace_member_path(
    path: &Path,
    command_label: &str,
    json: bool,
) -> Result<Vec<WorkspaceBuildTargets>, u8> {
    let request_root = resolve_project_workspace_member_command_request_root(path);
    let request_root = request_root.as_deref().unwrap_or(path);
    if json {
        return load_project_target_members_for_json(path, request_root, command_label);
    }

    load_workspace_build_targets_for_command_from_request_root(path, request_root, command_label)
}

pub(crate) fn load_workspace_build_targets_for_command_from_request_root(
    _request_path: &Path,
    request_root: &Path,
    command_label: &str,
) -> Result<Vec<WorkspaceBuildTargets>, u8> {
    let manifest = load_project_manifest(request_root).map_err(|error| {
        report_project_target_manifest_load_error(command_label, &error);
        1
    })?;

    discover_workspace_build_targets(&manifest).map_err(|error| {
        report_project_target_discovery_error(command_label, &error);
        1
    })
}

fn report_project_target_manifest_load_error(
    command_label: &str,
    error: &ql_project::ProjectError,
) {
    if let ql_project::ProjectError::ManifestNotFound { start } = error {
        eprintln!(
            "error: {command_label} requires a package or workspace manifest; could not find `qlang.toml` starting from `{}`",
            normalize_path(start)
        );
    } else if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error)
    {
        eprintln!(
            "error: {command_label} manifest `{}` does not declare `[package].name`",
            normalize_path(manifest_path)
        );
    } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(error) {
        eprintln!("error: {command_label} {error}");
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(manifest_path)
        );
    } else {
        eprintln!("error: {command_label} {error}");
    }
}

fn report_project_target_discovery_error(command_label: &str, error: &ql_project::ProjectError) {
    if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error) {
        eprintln!(
            "error: {command_label} manifest `{}` does not declare `[package].name`",
            normalize_path(manifest_path)
        );
    } else if let ql_project::ProjectError::PackageSourceRootNotFound { path } = error {
        eprintln!(
            "error: {command_label} package source directory `{}` does not exist",
            normalize_path(path)
        );
    } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(error) {
        eprintln!("error: {command_label} {error}");
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(manifest_path)
        );
    } else {
        eprintln!("error: {command_label} {error}");
    }
}

fn load_project_target_members_for_json(
    path: &Path,
    request_root: &Path,
    command_label: &str,
) -> Result<Vec<WorkspaceBuildTargets>, u8> {
    let manifest = load_project_manifest(request_root).map_err(|error| {
        print!(
            "{}",
            render_project_targets_preflight_failure_json(
                path,
                "manifest-load",
                project_targets_load_error_message(command_label, &error),
                project_targets_error_manifest_path(&error),
            )
        );
        1
    })?;

    discover_workspace_build_targets(&manifest).map_err(|error| {
        print!(
            "{}",
            render_project_targets_preflight_failure_json(
                path,
                "target-discovery",
                project_targets_discovery_error_message(command_label, &error),
                project_targets_error_manifest_path(&error),
            )
        );
        1
    })
}

fn project_targets_load_error_message(
    command_label: &str,
    error: &ql_project::ProjectError,
) -> String {
    if let ql_project::ProjectError::ManifestNotFound { start } = error {
        return format!(
            "{command_label} requires a package or workspace manifest; could not find `qlang.toml` starting from `{}`",
            normalize_path(start)
        );
    }
    if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error) {
        return format!(
            "{command_label} manifest `{}` does not declare `[package].name`",
            normalize_path(manifest_path)
        );
    }
    format!("{command_label} {error}")
}

fn project_targets_discovery_error_message(
    command_label: &str,
    error: &ql_project::ProjectError,
) -> String {
    if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error) {
        return format!(
            "{command_label} manifest `{}` does not declare `[package].name`",
            normalize_path(manifest_path)
        );
    }
    if let ql_project::ProjectError::PackageSourceRootNotFound { path } = error {
        return format!(
            "{command_label} package source directory `{}` does not exist",
            normalize_path(path)
        );
    }
    format!("{command_label} {error}")
}

fn project_targets_error_manifest_path(error: &ql_project::ProjectError) -> Option<&Path> {
    package_missing_name_manifest_path_from_project_error(error)
        .or_else(|| package_check_manifest_path_from_project_error(error))
}
