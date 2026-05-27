use std::path::Path;

use ql_driver::BuildError;

use crate::cli_utils::{
    normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error,
};

pub(super) fn project_lock_output_lock_error_message(
    lockfile_path: &Path,
    error: BuildError,
) -> String {
    match error {
        BuildError::Io { path, error } => format!(
            "failed to acquire lockfile output lock `{}` for `{}`: {error}",
            normalize_path(&path),
            normalize_path(lockfile_path)
        ),
        BuildError::InvalidInput(message) => message,
        BuildError::Diagnostics { path, .. } => format!(
            "failed to acquire lockfile output lock while diagnostics were reported for `{}`",
            normalize_path(&path)
        ),
        BuildError::Toolchain { error, .. } => format!("{error}"),
    }
}

pub(super) fn report_project_lock_output_lock_error(
    command_label: &str,
    manifest_path: &Path,
    lockfile_path: &Path,
    error: BuildError,
) {
    eprintln!(
        "error: {command_label} failed to lock lockfile `{}`: {}",
        normalize_path(lockfile_path),
        project_lock_output_lock_error_message(lockfile_path, error)
    );
    eprintln!(
        "note: failing package manifest: {}",
        normalize_path(manifest_path)
    );
    eprintln!(
        "hint: rerun `ql project lock {}` after the lockfile is no longer in use",
        normalize_path(manifest_path)
    );
}

pub(super) fn report_project_lock_load_error(
    path: &Path,
    check_only: bool,
    command_label: &str,
    error: &ql_project::ProjectError,
) {
    if let ql_project::ProjectError::ManifestNotFound { start } = error {
        eprintln!(
            "error: {command_label} requires a package or workspace manifest; could not find `qlang.toml` starting from `{}`",
            normalize_path(start)
        );
        report_project_lock_package_context_failure(path, check_only);
    } else if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error)
    {
        eprintln!(
            "error: {command_label} manifest `{}` does not declare `[package].name`",
            normalize_path(manifest_path)
        );
        report_project_lock_manifest_failure(manifest_path, check_only);
    } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(error) {
        eprintln!("error: {command_label} {error}");
        report_project_lock_manifest_failure(manifest_path, check_only);
    } else {
        eprintln!("error: {command_label} {error}");
    }
}

pub(super) fn report_project_lock_render_error(
    manifest: &ql_project::ProjectManifest,
    check_only: bool,
    command_label: &str,
    error: &ql_project::ProjectError,
) {
    if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error) {
        eprintln!(
            "error: {command_label} manifest `{}` does not declare `[package].name`",
            normalize_path(manifest_path)
        );
        report_project_lock_manifest_failure(manifest_path, check_only);
    } else if let ql_project::ProjectError::PackageSourceRootNotFound { path } = error {
        eprintln!(
            "error: {command_label} package source directory `{}` does not exist",
            normalize_path(path)
        );
        eprintln!(
            "hint: rerun `{}` after fixing the package source root",
            format_project_lock_command(&manifest.manifest_path, check_only)
        );
    } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(error) {
        eprintln!("error: {command_label} {error}");
        report_project_lock_manifest_failure(manifest_path, check_only);
    } else {
        eprintln!("error: {command_label} {error}");
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(&manifest.manifest_path)
        );
    }
}

pub(super) fn format_project_lock_command(manifest_path: &Path, check_only: bool) -> String {
    let manifest_path = normalize_path(manifest_path);
    if check_only {
        format!("ql project lock {manifest_path} --check")
    } else {
        format!("ql project lock {manifest_path}")
    }
}

fn report_project_lock_manifest_failure(manifest_path: &Path, check_only: bool) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command = if check_only {
        format!("ql project lock {manifest_path} --check")
    } else {
        format!("ql project lock {manifest_path}")
    };
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!("hint: rerun `{rerun_command}` after fixing the package manifest");
}

fn report_project_lock_package_context_failure(path: &Path, check_only: bool) {
    let normalized_path = normalize_path(path);
    let rerun_command = if check_only {
        format!("ql project lock {normalized_path} --check")
    } else {
        format!("ql project lock {normalized_path}")
    };
    eprintln!(
        "note: `ql project lock` only writes or checks package/workspace lockfiles for packages or workspace members discoverable from `qlang.toml`"
    );
    eprintln!("hint: rerun `{rerun_command}` after adding `qlang.toml` for this path");
}
