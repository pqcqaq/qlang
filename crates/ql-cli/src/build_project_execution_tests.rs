use super::*;
use ql_driver::{BuildCHeaderOptions, BuildEmit, BuildProfile, CHeaderSurface};
use ql_project::ManifestBuildProfile;

fn plan_member(require_targets: bool) -> ProjectBuildPlanMember {
    ProjectBuildPlanMember {
        member: WorkspaceBuildTargets {
            member_manifest_path: PathBuf::from("pkg/qlang.toml"),
            package_name: "pkg".to_owned(),
            default_profile: Some(ManifestBuildProfile::Release),
            targets: vec![BuildTarget {
                kind: BuildTargetKind::Library,
                path: PathBuf::from("pkg/src/lib.ql"),
            }],
        },
        emit_interface: false,
        require_targets,
    }
}

#[test]
fn selected_plan_target_preserves_explicit_outputs() {
    let member = plan_member(true);
    let target = member.member.targets.first().expect("target");
    let options = BuildOptions {
        emit: BuildEmit::Executable,
        profile: BuildProfile::Debug,
        output: Some(PathBuf::from("custom.exe")),
        c_header: Some(BuildCHeaderOptions {
            output: Some(PathBuf::from("custom.h")),
            surface: CHeaderSurface::Both,
        }),
        ..BuildOptions::default()
    };

    let resolved = build_options_for_plan_target(&member, target, &options, true, false);

    assert_eq!(resolved.output, options.output);
    assert_eq!(resolved.c_header, options.c_header);
    assert_eq!(resolved.profile, BuildProfile::Release);
}

#[test]
fn dependency_plan_target_uses_dependency_outputs() {
    let member = plan_member(false);
    let target = member.member.targets.first().expect("target");
    let options = BuildOptions {
        emit: BuildEmit::Executable,
        profile: BuildProfile::Debug,
        output: Some(PathBuf::from("custom.exe")),
        c_header: Some(BuildCHeaderOptions {
            output: Some(PathBuf::from("custom.h")),
            surface: CHeaderSurface::Exports,
        }),
        ..BuildOptions::default()
    };

    let resolved = build_options_for_plan_target(&member, target, &options, false, false);

    assert_eq!(resolved.emit, BuildEmit::StaticLibrary);
    assert_ne!(resolved.output, options.output);
    assert_eq!(resolved.c_header, None);
    assert_eq!(resolved.profile, BuildProfile::Release);
}
