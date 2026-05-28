use std::path::{Path, PathBuf};

use ql_driver::{BuildOptions, BuildProfile};

use super::listing::render_test_target_listing;
use super::package_selector::package_selector_mismatch_message;
use super::paths::{package_test_command_path, project_test_output_path};
use super::project_errors::ql_test_project_error_message;
use super::ui::is_project_ui_test;
use super::*;
use crate::test_reporting::TestTargetKind;

fn smoke_target(display_path: &str, source_path: &str) -> TestTarget {
    TestTarget {
        display_path: display_path.to_owned(),
        kind: TestTargetKind::Smoke {
            source_path: PathBuf::from(source_path),
            working_directory: PathBuf::from("."),
            build_options: BuildOptions::default(),
            package_manifest_path: None,
        },
    }
}

#[test]
fn select_test_targets_matches_workspace_package_relative_path() {
    let targets = vec![
        smoke_target(
            "packages/core/tests/basic.ql",
            "workspace/packages/core/tests/basic.ql",
        ),
        smoke_target(
            "packages/app/tests/basic.ql",
            "workspace/packages/app/tests/basic.ql",
        ),
    ];

    let selected = select_test_targets_by_path(targets, "tests/basic.ql", Some("core"));

    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].display_path, "packages/core/tests/basic.ql");
}

#[test]
fn select_test_targets_matches_normalized_source_path() {
    let targets = vec![smoke_target(
        "packages/core/tests/basic.ql",
        "workspace/packages/core/tests/basic.ql",
    )];

    let selected =
        select_test_targets_by_path(targets, "workspace/packages/core/tests/basic.ql", None);

    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].display_path, "packages/core/tests/basic.ql");
}

#[test]
fn filter_test_targets_keeps_display_path_substring_matches() {
    let targets = vec![
        smoke_target("tests/basic.ql", "tests/basic.ql"),
        smoke_target("tests/slow/path.ql", "tests/slow/path.ql"),
    ];

    let selected = filter_test_targets(targets, Some("slow"));

    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].display_path, "tests/slow/path.ql");
}

#[test]
fn render_test_target_listing_preserves_paths_and_summary() {
    let targets = vec![
        smoke_target("tests/basic.ql", "tests/basic.ql"),
        smoke_target("tests/slow/path.ql", "tests/slow/path.ql"),
    ];

    assert_eq!(
        render_test_target_listing(&targets),
        "tests/basic.ql\ntests/slow/path.ql\n\ntest listing: 2 discovered\n"
    );
}

#[test]
fn no_tests_message_includes_package_selector_context() {
    let message = test_no_tests_message(Path::new("workspace"), Some("core"));

    assert_eq!(
        message,
        "`ql test` found no `.ql` test files for package `core` under `workspace`"
    );
}

#[test]
fn no_matching_filter_message_includes_package_selector_context() {
    let message = test_no_matching_filter_message(Path::new("workspace"), "slow", Some("core"));

    assert_eq!(
        message,
        "`ql test` found no test files matching `slow` for package `core` under `workspace`"
    );
}

#[test]
fn no_matching_target_message_includes_package_selector_context() {
    let message =
        test_no_matching_target_message(Path::new("workspace"), "tests/smoke.ql", Some("core"));

    assert_eq!(
        message,
        "`ql test` found no test target `tests/smoke.ql` for package `core` under `workspace`"
    );
}

#[test]
fn package_selector_mismatch_message_names_request_root() {
    let message = package_selector_mismatch_message(Path::new("workspace"));

    assert_eq!(
        message,
        "package selector matched no workspace members under `workspace`"
    );
}

#[test]
fn project_test_output_path_preserves_nested_test_layout() {
    let output_path = project_test_output_path(
        Path::new("pkg/qlang.toml"),
        Path::new("pkg/tests/integration/math.ql"),
        BuildProfile::Release,
    );
    let executable_name = if cfg!(windows) { "math.exe" } else { "math" };

    assert_eq!(
        output_path,
        PathBuf::from("pkg")
            .join("target")
            .join("ql")
            .join("release")
            .join("tests")
            .join("integration")
            .join(executable_name)
    );
}

#[test]
fn package_test_command_path_is_package_relative() {
    assert_eq!(
        package_test_command_path(Path::new("pkg"), Path::new("pkg/tests/ui/basic.ql")),
        PathBuf::from("tests").join("ui").join("basic.ql")
    );
}

#[test]
fn project_ui_test_detection_requires_tests_ui_prefix() {
    assert!(is_project_ui_test(
        Path::new("pkg"),
        Path::new("pkg/tests/ui/basic.ql")
    ));
    assert!(!is_project_ui_test(
        Path::new("pkg"),
        Path::new("pkg/tests/smoke.ql")
    ));
    assert!(!is_project_ui_test(
        Path::new("pkg"),
        Path::new("other/tests/ui/basic.ql")
    ));
}

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
