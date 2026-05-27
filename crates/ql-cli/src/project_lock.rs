#[cfg(test)]
use std::fs;
use std::path::Path;

use ql_driver::acquire_build_output_locks;
use ql_driver::write_file_atomically;
use ql_project::{load_project_manifest, project_lockfile_path, render_project_lockfile};

use crate::cli_utils::normalize_path;
use crate::project_targets::resolve_project_workspace_member_command_request_root;

mod json_report;
mod reporting;
mod status;

use json_report::{
    ProjectLockJsonReport, render_project_lock_manifest_failure_json,
    render_project_lock_render_failure_json,
};
use reporting::{
    format_project_lock_command, project_lock_output_lock_error_message,
    report_project_lock_load_error, report_project_lock_output_lock_error,
    report_project_lock_render_error,
};
use status::{ProjectLockCheckStatus, check_project_lockfile, project_lockfile_check_status};

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

#[cfg(test)]
#[path = "project_lock_tests.rs"]
mod tests;
