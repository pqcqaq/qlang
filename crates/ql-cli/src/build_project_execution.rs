use std::path::{Path, PathBuf};

use ql_driver::{BuildArtifact, BuildOptions};
use ql_project::{BuildTarget, BuildTargetKind, WorkspaceBuildTargets};

use crate::build_json_report::{BuildJsonReport, emit_build_json_failure};
use crate::build_outputs::{project_dependency_target_build_options, project_target_build_options};
use crate::build_plan::{BuildTargetJsonError, ProjectBuildPlanMember};
use crate::build_reporting::{
    build_json_emit_interface_failure, build_json_preflight_failure, build_json_target_prep_failure,
};
use crate::build_single_source::{
    emit_built_package_interface, emit_built_package_interface_quiet,
};
use crate::cli_utils::normalize_path;
use crate::project_target_build::{
    build_project_source_target, build_project_source_target_result,
};

pub(crate) fn execute_project_build_plan(
    path: &Path,
    members: &[WorkspaceBuildTargets],
    build_plan: &[ProjectBuildPlanMember],
    options: &BuildOptions,
    emit_interface: bool,
    emit_overridden: bool,
    profile_overridden: bool,
    json_report: &mut Option<BuildJsonReport>,
) -> Result<(), u8> {
    for plan_member in build_plan {
        ensure_required_targets(path, plan_member, json_report)?;
        if plan_member.member.targets.is_empty() {
            continue;
        }

        let mut first_artifact_path = None;
        let mut additional_artifacts = Vec::new();
        for (index, target) in plan_member.member.targets.iter().enumerate() {
            let artifact = build_plan_target(
                members,
                build_plan,
                plan_member,
                target,
                options,
                emit_interface,
                emit_overridden,
                profile_overridden,
                json_report,
            )?;
            if let Some(report) = json_report.as_mut() {
                report.record_project_target(
                    &plan_member.member,
                    target,
                    &artifact,
                    plan_member.require_targets,
                );
            }
            if index == 0 {
                first_artifact_path = Some(artifact.path);
            } else {
                additional_artifacts.push(artifact.path);
            }
        }

        if plan_member.emit_interface {
            emit_plan_member_interface(
                path,
                plan_member,
                options,
                &first_artifact_path.expect("non-empty plan member should have first artifact"),
                &additional_artifacts,
                json_report,
            )?;
        }
    }

    Ok(())
}

fn ensure_required_targets(
    path: &Path,
    plan_member: &ProjectBuildPlanMember,
    json_report: &mut Option<BuildJsonReport>,
) -> Result<(), u8> {
    if !plan_member.require_targets || !plan_member.member.targets.is_empty() {
        return Ok(());
    }

    if json_report.is_some() {
        return emit_build_json_failure(
            json_report,
            build_json_preflight_failure(
                path,
                Some(&plan_member.member.member_manifest_path),
                Some(plan_member.member.package_name.as_str()),
                Some(true),
                "project",
                "build-plan",
                format!(
                    "package `{}` has no discovered build targets",
                    plan_member.member.package_name
                ),
                None,
                None,
                Some(0),
            ),
        );
    }

    eprintln!(
        "error: `ql build` package `{}` has no discovered build targets",
        plan_member.member.package_name
    );
    eprintln!(
        "note: failing package manifest: {}",
        normalize_path(&plan_member.member.member_manifest_path)
    );
    eprintln!(
        "hint: rerun `ql project targets {}` to inspect the discovered build targets",
        normalize_path(&plan_member.member.member_manifest_path)
    );
    Err(1)
}

fn build_plan_target(
    members: &[WorkspaceBuildTargets],
    build_plan: &[ProjectBuildPlanMember],
    plan_member: &ProjectBuildPlanMember,
    target: &BuildTarget,
    options: &BuildOptions,
    emit_interface: bool,
    emit_overridden: bool,
    profile_overridden: bool,
    json_report: &mut Option<BuildJsonReport>,
) -> Result<BuildArtifact, u8> {
    let target_options = build_options_for_plan_target(
        plan_member,
        target,
        options,
        emit_overridden,
        profile_overridden,
    );
    let library_dependency =
        !plan_member.require_targets && target.kind == BuildTargetKind::Library;

    if json_report.is_some() {
        return match build_project_source_target_result(
            build_plan,
            &plan_member.member.member_manifest_path,
            &target.path,
            &target_options,
            options,
            profile_overridden,
            library_dependency,
        ) {
            Ok(artifact) => Ok(artifact),
            Err(BuildTargetJsonError::Early(error)) => {
                emit_build_json_failure(
                    json_report,
                    build_json_target_prep_failure(
                        &plan_member.member,
                        target,
                        plan_member.require_targets,
                        &error,
                    ),
                )?;
                Err(1)
            }
            Err(BuildTargetJsonError::Build(error)) => {
                let mut report = json_report
                    .take()
                    .expect("json report should exist for `ql build --json`");
                report.record_project_failure(
                    &plan_member.member,
                    target,
                    &error,
                    plan_member.require_targets,
                );
                print!("{}", report.into_json());
                Err(1)
            }
        };
    }

    build_project_source_target(
        members,
        "`ql build`",
        &plan_member.member.member_manifest_path,
        &target.path,
        &target_options,
        options,
        profile_overridden,
        emit_interface,
        library_dependency,
    )
}

fn build_options_for_plan_target(
    plan_member: &ProjectBuildPlanMember,
    target: &BuildTarget,
    options: &BuildOptions,
    emit_overridden: bool,
    profile_overridden: bool,
) -> BuildOptions {
    if plan_member.require_targets {
        project_target_build_options(
            &plan_member.member,
            target,
            options,
            emit_overridden,
            profile_overridden,
        )
    } else {
        project_dependency_target_build_options(
            &plan_member.member,
            target,
            options,
            emit_overridden,
            profile_overridden,
        )
    }
}

fn emit_plan_member_interface(
    path: &Path,
    plan_member: &ProjectBuildPlanMember,
    options: &BuildOptions,
    first_artifact_path: &Path,
    additional_artifacts: &[PathBuf],
    json_report: &mut Option<BuildJsonReport>,
) -> Result<(), u8> {
    if let Some(report) = json_report.as_mut() {
        let interface_result = match emit_built_package_interface_quiet(
            path,
            &plan_member.member.member_manifest_path,
            options,
            first_artifact_path,
            additional_artifacts,
        ) {
            Ok(result) => result,
            Err(error) => {
                return emit_build_json_failure(
                    json_report,
                    build_json_emit_interface_failure(
                        path,
                        Some(&plan_member.member.member_manifest_path),
                        Some(plan_member.member.package_name.as_str()),
                        &error,
                    ),
                );
            }
        };
        report.record_interface_result(
            Some(&plan_member.member.member_manifest_path),
            Some(plan_member.member.package_name.as_str()),
            true,
            interface_result,
        );
        return Ok(());
    }

    emit_built_package_interface(
        path,
        &plan_member.member.member_manifest_path,
        options,
        first_artifact_path,
        additional_artifacts,
    )
}

#[cfg(test)]
mod tests {
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
}
