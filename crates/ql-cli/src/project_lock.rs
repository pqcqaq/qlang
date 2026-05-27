use std::fs;
use std::path::Path;

use ql_driver::{BuildError, acquire_build_output_locks, write_file_atomically};
use ql_project::{load_project_manifest, project_lockfile_path, render_project_lockfile};

use crate::cli_utils::{
    normalize_line_endings, normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error,
};
use crate::project_targets::resolve_project_workspace_member_command_request_root;

mod json_report;

use json_report::{
    ProjectLockJsonReport, render_project_lock_manifest_failure_json,
    render_project_lock_render_failure_json,
};

#[derive(Debug, PartialEq, Eq)]
enum ProjectLockCheckStatus {
    UpToDate,
    Stale,
    Missing,
    ReadError(String),
}

fn project_lockfile_check_status(lockfile_path: &Path, expected: &str) -> ProjectLockCheckStatus {
    match fs::read_to_string(lockfile_path) {
        Ok(actual) => {
            if normalize_line_endings(&actual) == normalize_line_endings(expected) {
                ProjectLockCheckStatus::UpToDate
            } else {
                ProjectLockCheckStatus::Stale
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            ProjectLockCheckStatus::Missing
        }
        Err(error) => ProjectLockCheckStatus::ReadError(error.to_string()),
    }
}

pub(crate) fn project_lock_path(path: &Path, check_only: bool, json: bool) -> Result<(), u8> {
    let command_label = if check_only {
        "`ql project lock --check`"
    } else {
        "`ql project lock`"
    };

    let request_root = resolve_project_workspace_member_command_request_root(path);
    let manifest = match load_project_manifest(request_root.as_deref().unwrap_or(path)) {
        Ok(manifest) => manifest,
        Err(error) => {
            if json {
                print!(
                    "{}",
                    render_project_lock_manifest_failure_json(path, check_only, &error)
                );
            } else {
                report_project_lock_load_error(path, check_only, command_label, &error);
            }
            return Err(1);
        }
    };

    let lockfile_path = project_lockfile_path(&manifest);
    let rendered = match render_project_lockfile(&manifest) {
        Ok(rendered) => rendered,
        Err(error) => {
            if json {
                print!(
                    "{}",
                    render_project_lock_render_failure_json(
                        path,
                        &manifest,
                        &lockfile_path,
                        check_only,
                        &error,
                    )
                );
            } else {
                report_project_lock_render_error(&manifest, check_only, command_label, &error);
            }
            return Err(1);
        }
    };

    if json {
        let mut report =
            ProjectLockJsonReport::new(path, &manifest, &lockfile_path, check_only, &rendered);
        let rerun_command = format_project_lock_command(&manifest.manifest_path, false);
        let _lock = match acquire_build_output_locks(vec![lockfile_path.clone()])
            .map_err(|error| project_lock_output_lock_error_message(&lockfile_path, error))
        {
            Ok(lock) => lock,
            Err(message) => {
                report.record_failure("lock", message, Some(rerun_command));
                print!("{}", report.into_json());
                return Err(1);
            }
        };

        if check_only {
            match project_lockfile_check_status(&lockfile_path, &rendered) {
                ProjectLockCheckStatus::UpToDate => {
                    print!("{}", report.into_json());
                    return Ok(());
                }
                ProjectLockCheckStatus::Stale => {
                    report.record_failure(
                        "stale",
                        format!("lockfile `{}` is stale", normalize_path(&lockfile_path)),
                        Some(rerun_command),
                    );
                    print!("{}", report.into_json());
                    return Err(1);
                }
                ProjectLockCheckStatus::Missing => {
                    report.record_failure(
                        "missing",
                        format!("lockfile `{}` is missing", normalize_path(&lockfile_path)),
                        Some(rerun_command),
                    );
                    print!("{}", report.into_json());
                    return Err(1);
                }
                ProjectLockCheckStatus::ReadError(error) => {
                    report.record_failure(
                        "read",
                        format!(
                            "failed to read lockfile `{}`: {error}",
                            normalize_path(&lockfile_path)
                        ),
                        Some(rerun_command),
                    );
                    print!("{}", report.into_json());
                    return Err(1);
                }
            }
        }

        if let Err(error) = write_file_atomically(&lockfile_path, &rendered) {
            report.record_failure(
                "write",
                format!(
                    "failed to write lockfile `{}`: {error}",
                    normalize_path(&lockfile_path)
                ),
                Some(format_project_lock_command(&manifest.manifest_path, false)),
            );
            print!("{}", report.into_json());
            return Err(1);
        }

        print!("{}", report.into_json());
        return Ok(());
    }

    let _lock = acquire_build_output_locks(vec![lockfile_path.clone()]).map_err(|error| {
        report_project_lock_output_lock_error(
            command_label,
            &manifest.manifest_path,
            &lockfile_path,
            error,
        );
        1
    })?;

    if check_only {
        return check_project_lockfile(&manifest, &lockfile_path, &rendered);
    }

    write_file_atomically(&lockfile_path, rendered).map_err(|error| {
        eprintln!(
            "error: {command_label} failed to write lockfile `{}`: {error}",
            normalize_path(&lockfile_path)
        );
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(&manifest.manifest_path)
        );
        eprintln!(
            "hint: rerun `ql project lock {}` after fixing the lockfile output path",
            normalize_path(&manifest.manifest_path)
        );
        1
    })?;

    println!("wrote lockfile: {}", normalize_path(&lockfile_path));
    Ok(())
}

fn project_lock_output_lock_error_message(lockfile_path: &Path, error: BuildError) -> String {
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

fn report_project_lock_output_lock_error(
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

fn report_project_lock_load_error(
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

fn report_project_lock_render_error(
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

fn check_project_lockfile(
    manifest: &ql_project::ProjectManifest,
    lockfile_path: &Path,
    expected: &str,
) -> Result<(), u8> {
    let normalized_lockfile_path = normalize_path(lockfile_path);
    let rerun_command = format!(
        "ql project lock {}",
        normalize_path(&manifest.manifest_path)
    );

    match project_lockfile_check_status(lockfile_path, expected) {
        ProjectLockCheckStatus::UpToDate => return Ok(()),
        ProjectLockCheckStatus::Stale => {
            eprintln!(
                "error: `ql project lock --check` lockfile `{normalized_lockfile_path}` is stale"
            );
        }
        ProjectLockCheckStatus::Missing => {
            eprintln!(
                "error: `ql project lock --check` lockfile `{normalized_lockfile_path}` is missing"
            );
        }
        ProjectLockCheckStatus::ReadError(error) => {
            eprintln!(
                "error: `ql project lock --check` failed to read lockfile `{normalized_lockfile_path}`: {error}"
            );
        }
    }

    eprintln!(
        "note: failing package manifest: {}",
        normalize_path(&manifest.manifest_path)
    );
    eprintln!("hint: rerun `{rerun_command}` to regenerate `qlang.lock`");
    Err(1)
}

fn format_project_lock_command(manifest_path: &Path, check_only: bool) -> String {
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

#[cfg(test)]
#[path = "project_lock_tests.rs"]
mod tests;
