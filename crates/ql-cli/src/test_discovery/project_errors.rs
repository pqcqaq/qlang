use std::path::Path;

use crate::build_reporting::build_json_project_error;
use crate::cli_utils::{
    normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error,
};
use crate::test_command::TestCommandOptions;
use crate::test_reporting::render_test_json_preflight_failure_report;

pub(super) fn report_ql_test_project_preflight_error(
    request_path: &Path,
    command_options: &TestCommandOptions,
    error: &ql_project::ProjectError,
    stage: &str,
) -> u8 {
    if command_options.json {
        print!(
            "{}",
            render_test_json_preflight_failure_report(
                request_path,
                command_options,
                build_json_project_error(request_path, error, stage),
            )
        );
        return 1;
    }

    report_ql_test_project_error(error)
}

pub(super) fn report_ql_test_project_error(error: &ql_project::ProjectError) -> u8 {
    for line in ql_test_project_error_message(error) {
        eprintln!("{line}");
    }
    1
}

pub(super) fn ql_test_project_error_message(error: &ql_project::ProjectError) -> Vec<String> {
    if let ql_project::ProjectError::ManifestNotFound { start } = error {
        return vec![format!(
            "error: `ql test` requires a package or workspace manifest; could not find `qlang.toml` starting from `{}`",
            normalize_path(start)
        )];
    }

    if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error) {
        return vec![format!(
            "error: `ql test` manifest `{}` does not declare `[package].name`",
            normalize_path(manifest_path)
        )];
    }

    if let ql_project::ProjectError::PackageSourceRootNotFound { path } = error {
        return vec![format!(
            "error: `ql test` package source directory `{}` does not exist",
            normalize_path(path)
        )];
    }

    if let Some(manifest_path) = package_check_manifest_path_from_project_error(error) {
        return vec![
            format!("error: `ql test` {error}"),
            format!(
                "note: failing package manifest: {}",
                normalize_path(manifest_path)
            ),
        ];
    }

    vec![format!("error: `ql test` {error}")]
}
