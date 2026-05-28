use std::path::{Path, PathBuf};

use crate::cli_scan::collect_ql_files;
use crate::cli_utils::normalize_path;

pub(super) fn collect_project_test_files(package_root: &Path) -> Result<Vec<PathBuf>, u8> {
    let tests_root = project_tests_root(package_root);
    if !tests_root.is_dir() {
        return Ok(Vec::new());
    }

    collect_ql_files(&tests_root).map_err(|error| {
        eprintln!(
            "{}",
            project_test_files_read_error_message(&tests_root, error)
        );
        1
    })
}

fn project_tests_root(package_root: &Path) -> PathBuf {
    package_root.join("tests")
}

pub(super) fn project_test_files_read_error_message(
    tests_root: &Path,
    error: impl std::fmt::Display,
) -> String {
    format!(
        "error: `ql test` failed to read `{}`: {error}",
        normalize_path(tests_root)
    )
}
