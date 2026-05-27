use std::path::PathBuf;

use super::*;

#[test]
fn manifest_path_collection_deduplicates_project_smoke_targets() {
    let manifest = PathBuf::from("workspace/app/qlang.toml");
    let targets = [
        TestTarget {
            display_path: "tests/one.ql".to_owned(),
            kind: TestTargetKind::Smoke {
                source_path: PathBuf::from("workspace/app/tests/one.ql"),
                working_directory: PathBuf::from("workspace/app"),
                build_options: ql_driver::BuildOptions::default(),
                package_manifest_path: Some(manifest.clone()),
            },
        },
        TestTarget {
            display_path: "tests/two.ql".to_owned(),
            kind: TestTargetKind::Smoke {
                source_path: PathBuf::from("workspace/app/tests/two.ql"),
                working_directory: PathBuf::from("workspace/app"),
                build_options: ql_driver::BuildOptions::default(),
                package_manifest_path: Some(manifest.clone()),
            },
        },
    ];

    assert_eq!(test_target_manifest_paths(&targets), vec![manifest]);
}
