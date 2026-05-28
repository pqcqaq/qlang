use std::path::PathBuf;

use ql_driver::BuildOptions;

use crate::test_reporting::{TestTarget, TestTargetKind};

use super::listing::render_test_target_listing;

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
