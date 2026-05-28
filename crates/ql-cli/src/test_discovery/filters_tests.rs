use std::path::PathBuf;

use ql_driver::BuildOptions;

use crate::test_reporting::{TestTarget, TestTargetKind};

use super::filters::{filter_test_targets, select_test_targets_by_path};

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
fn select_test_targets_does_not_match_other_package_relative_path() {
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

    let selected = select_test_targets_by_path(targets, "tests/basic.ql", Some("missing"));

    assert!(selected.is_empty());
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
