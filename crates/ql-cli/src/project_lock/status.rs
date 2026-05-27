use std::fs;
use std::path::Path;

use crate::cli_utils::{normalize_line_endings, normalize_path};
use crate::project_lock::reporting::format_project_lock_command;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ProjectLockCheckStatus {
    UpToDate,
    Stale,
    Missing,
    ReadError(String),
}

pub(super) fn project_lockfile_check_status(
    lockfile_path: &Path,
    expected: &str,
) -> ProjectLockCheckStatus {
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

pub(super) fn check_project_lockfile(
    manifest: &ql_project::ProjectManifest,
    lockfile_path: &Path,
    expected: &str,
) -> Result<(), u8> {
    let normalized_lockfile_path = normalize_path(lockfile_path);
    let rerun_command = format_project_lock_command(&manifest.manifest_path, false);

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
