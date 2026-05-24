use std::path::{Path, PathBuf};

use ql_driver::{BuildError, BuildOutputLock, acquire_build_output_locks};

use super::{atomic_write::write_file_atomically, normalize_path};

pub(crate) fn acquire_locked_project_manifest_edits(
    manifest_paths: impl IntoIterator<Item = PathBuf>,
) -> Result<Vec<BuildOutputLock>, String> {
    acquire_build_output_locks(manifest_paths).map_err(project_manifest_output_lock_error_message)
}

pub(crate) fn write_locked_project_manifest(
    manifest_path: &Path,
    contents: String,
) -> Result<(), std::io::Error> {
    write_file_atomically(manifest_path, contents)
}

fn project_manifest_output_lock_error_message(error: BuildError) -> String {
    match error {
        BuildError::Io { path, error } => {
            format!(
                "failed to acquire manifest output lock `{}`: {error}",
                normalize_path(&path)
            )
        }
        BuildError::InvalidInput(message) => message,
        BuildError::Diagnostics { path, .. } => format!(
            "failed to acquire manifest output lock while diagnostics were reported for `{}`",
            normalize_path(&path)
        ),
        BuildError::Toolchain { error, .. } => format!("{error}"),
    }
}
