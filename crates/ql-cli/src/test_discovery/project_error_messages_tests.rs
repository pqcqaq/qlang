use std::path::PathBuf;

use super::project_error_messages::ql_test_project_error_message;

#[test]
fn ql_test_project_error_message_names_missing_manifest_start() {
    let message = ql_test_project_error_message(&ql_project::ProjectError::ManifestNotFound {
        start: PathBuf::from("missing"),
    });

    assert_eq!(
        message,
        vec![
            "error: `ql test` requires a package or workspace manifest; could not find `qlang.toml` starting from `missing`"
                .to_owned()
        ]
    );
}

#[test]
fn ql_test_project_error_message_names_missing_package_manifest() {
    let message = ql_test_project_error_message(&ql_project::ProjectError::PackageNotDefined {
        path: PathBuf::from("pkg/qlang.toml"),
    });

    assert_eq!(
        message,
        vec!["error: `ql test` manifest `pkg/qlang.toml` does not declare `[package].name`"]
    );
}

#[test]
fn ql_test_project_error_message_names_missing_source_root() {
    let message =
        ql_test_project_error_message(&ql_project::ProjectError::PackageSourceRootNotFound {
            path: PathBuf::from("pkg/src"),
        });

    assert_eq!(
        message,
        vec!["error: `ql test` package source directory `pkg/src` does not exist"]
    );
}
