use crate::cli_utils::{
    normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error,
};

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
