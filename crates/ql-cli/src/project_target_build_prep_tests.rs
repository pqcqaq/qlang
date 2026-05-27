use super::*;
use ql_driver::{BuildEmit, BuildProfile};
use ql_project::{BuildTarget, ManifestBuildProfile};

fn path_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| component.as_os_str().to_str().map(str::to_owned))
        .collect()
}

fn plan_member(
    package_name: &str,
    require_targets: bool,
    target_kind: BuildTargetKind,
    target_path: &str,
) -> ProjectBuildPlanMember {
    ProjectBuildPlanMember {
        member: WorkspaceBuildTargets {
            member_manifest_path: PathBuf::from(format!("{package_name}/qlang.toml")),
            package_name: package_name.to_owned(),
            default_profile: Some(ManifestBuildProfile::Release),
            targets: vec![BuildTarget {
                kind: target_kind,
                path: PathBuf::from(target_path),
            }],
        },
        emit_interface: false,
        require_targets,
    }
}

#[test]
fn dependency_link_inputs_skip_selected_members() {
    let plan = vec![
        plan_member("dep", false, BuildTargetKind::Library, "dep/src/lib.ql"),
        plan_member("app", true, BuildTargetKind::Library, "app/src/lib.ql"),
    ];
    let options = BuildOptions {
        emit: BuildEmit::Executable,
        profile: BuildProfile::Debug,
        ..BuildOptions::default()
    };

    let inputs = project_dependency_link_inputs(&plan, &options, false);

    assert_eq!(inputs.len(), 1);
    assert_eq!(
        path_components(&inputs[0]),
        ["dep", "target", "ql", "release", "lib.lib"]
    );
}

#[test]
fn selected_library_link_inputs_skip_dependency_members() {
    let plan = vec![
        plan_member("dep", false, BuildTargetKind::Library, "dep/src/lib.ql"),
        plan_member("app", true, BuildTargetKind::Library, "app/src/lib.ql"),
        plan_member("tool", true, BuildTargetKind::Binary, "tool/src/main.ql"),
    ];
    let options = BuildOptions {
        emit: BuildEmit::Executable,
        profile: BuildProfile::Debug,
        ..BuildOptions::default()
    };

    let inputs = project_selected_library_link_inputs(&plan, &options, false);

    assert_eq!(inputs.len(), 1);
    assert_eq!(
        path_components(&inputs[0]),
        ["app", "target", "ql", "release", "lib.lib"]
    );
}
