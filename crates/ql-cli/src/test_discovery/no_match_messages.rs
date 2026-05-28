use std::path::Path;

use crate::cli_utils::normalize_path;

pub(crate) fn test_no_tests_message(path: &Path, package_name: Option<&str>) -> String {
    let normalized_path = normalize_path(path);
    if let Some(package_name) = package_name {
        return format!(
            "`ql test` found no `.ql` test files for package `{package_name}` under `{normalized_path}`"
        );
    }
    format!("`ql test` found no `.ql` test files under `{normalized_path}`")
}

pub(crate) fn test_no_matching_filter_message(
    path: &Path,
    filter: &str,
    package_name: Option<&str>,
) -> String {
    let normalized_path = normalize_path(path);
    if let Some(package_name) = package_name {
        return format!(
            "`ql test` found no test files matching `{filter}` for package `{package_name}` under `{normalized_path}`"
        );
    }
    format!("`ql test` found no test files matching `{filter}` under `{normalized_path}`")
}

pub(crate) fn test_no_matching_target_message(
    path: &Path,
    target_path: &str,
    package_name: Option<&str>,
) -> String {
    let normalized_path = normalize_path(path);
    if let Some(package_name) = package_name {
        return format!(
            "`ql test` found no test target `{target_path}` for package `{package_name}` under `{normalized_path}`"
        );
    }
    format!("`ql test` found no test target `{target_path}` under `{normalized_path}`")
}
