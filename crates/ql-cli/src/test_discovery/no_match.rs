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

pub(crate) fn report_no_tests_discovered(path: &Path, package_name: Option<&str>) {
    let normalized_path = normalize_path(path);
    eprintln!("error: {}", test_no_tests_message(path, package_name));
    if let Some(package_name) = package_name {
        eprintln!(
            "hint: add standalone smoke tests under `tests/**/*.ql`, rerun `ql test {normalized_path} --package {package_name} --list`, or adjust `--package`"
        );
        return;
    }
    eprintln!(
        "hint: add standalone smoke tests under `tests/**/*.ql`, or rerun `ql test <file.ql>` for a single file"
    );
}

pub(crate) fn report_no_matching_tests(path: &Path, filter: &str, package_name: Option<&str>) {
    let normalized_path = normalize_path(path);
    eprintln!(
        "error: {}",
        test_no_matching_filter_message(path, filter, package_name)
    );
    if let Some(package_name) = package_name {
        eprintln!(
            "hint: rerun `ql test {normalized_path} --package {package_name} --list` to inspect the discovered tests, or adjust `--filter` / `--package`"
        );
        return;
    }
    eprintln!(
        "hint: rerun `ql test {normalized_path} --list` to inspect the discovered tests, or adjust `--filter`"
    );
}

pub(crate) fn report_no_matching_test_target(
    path: &Path,
    target_path: &str,
    package_name: Option<&str>,
) {
    let normalized_path = normalize_path(path);
    eprintln!(
        "error: {}",
        test_no_matching_target_message(path, target_path, package_name)
    );
    if let Some(package_name) = package_name {
        eprintln!(
            "hint: rerun `ql test {normalized_path} --package {package_name} --list` to inspect the discovered tests, or adjust `--target` / `--package`"
        );
        return;
    }

    eprintln!(
        "hint: rerun `ql test {normalized_path} --list` to inspect the discovered tests, or adjust `--target`"
    );
}
