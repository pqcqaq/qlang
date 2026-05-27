use std::path::{Path, PathBuf};

use ql_driver::{BuildOptions, BuildProfile};

use super::*;

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
fn no_tests_message_includes_package_selector_context() {
    let message = test_no_tests_message(Path::new("workspace"), Some("core"));

    assert_eq!(
        message,
        "`ql test` found no `.ql` test files for package `core` under `workspace`"
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
