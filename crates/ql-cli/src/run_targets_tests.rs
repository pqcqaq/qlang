use std::path::{Path, PathBuf};

use ql_project::{BuildTargetKind, WorkspaceBuildTargets};

use super::*;

fn member(package_name: &str, targets: Vec<BuildTarget>) -> WorkspaceBuildTargets {
    WorkspaceBuildTargets {
        member_manifest_path: PathBuf::from(format!("packages/{package_name}/qlang.toml")),
        package_name: package_name.to_owned(),
        default_profile: None,
        targets,
    }
}

fn target(kind: BuildTargetKind, path: &str) -> BuildTarget {
    BuildTarget {
        kind,
        path: PathBuf::from(path),
    }
}

#[test]
fn runnable_target_selection_ignores_libraries() {
    let members = [member(
        "app",
        vec![
            target(BuildTargetKind::Library, "packages/app/src/lib.ql"),
            target(BuildTargetKind::Binary, "packages/app/src/main.ql"),
        ],
    )];

    let selected =
        select_runnable_project_target(Path::new("workspace"), &members, &Default::default())
            .expect("single runnable target should be selected");

    assert_eq!(selected.package_name, "app");
    assert_eq!(
        selected.target.path,
        PathBuf::from("packages/app/src/main.ql")
    );
}

#[test]
fn run_json_target_selection_reports_multiple_runnable_targets() {
    let members = [
        member(
            "app",
            vec![target(BuildTargetKind::Binary, "packages/app/src/main.ql")],
        ),
        member(
            "tool",
            vec![target(BuildTargetKind::Binary, "packages/tool/src/main.ql")],
        ),
    ];

    let failure = select_runnable_project_target_for_run_json(
        Path::new("workspace"),
        &members,
        &Default::default(),
    )
    .expect_err("multiple runnable targets should fail");

    assert_eq!(failure["error_kind"], "project");
    assert_eq!(failure["stage"], "target-selection");
    assert_eq!(failure["target_count"], 2);
}
