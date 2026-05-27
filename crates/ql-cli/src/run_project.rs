use std::path::Path;

use ql_driver::BuildOptions;
use ql_project::WorkspaceBuildTargets;

use crate::build_outputs::{apply_manifest_default_profile, project_target_output_path};
use crate::build_plan::{
    BuildTargetJsonError, prepare_project_dependency_builds, resolve_project_build_plan_members,
    select_project_build_plan_root_members,
};
use crate::build_reporting::{
    load_workspace_build_targets_for_build_json_from_request_root,
    select_workspace_build_targets_for_build_json,
};
use crate::project_reference_interfaces::prepare_reference_interfaces_for_manifests;
use crate::project_target_build::{
    build_project_source_target_result, build_project_source_target_silent,
};
use crate::project_targets::{
    ProjectTargetSelector, load_workspace_build_targets_for_command_from_request_root,
    select_workspace_build_targets,
};
use crate::run_execution::{emit_run_json_execution, run_built_executable};
use crate::run_reporting::RunJsonReport;
use crate::run_targets::{
    RunnableProjectTarget, select_runnable_project_target,
    select_runnable_project_target_for_run_json,
};

pub(crate) fn run_project_path_json(
    path: &Path,
    project_request_root: &Path,
    options: &BuildOptions,
    profile_overridden: bool,
    selector: &ProjectTargetSelector,
    program_args: &[String],
) -> Result<(), u8> {
    let mut report = RunJsonReport::new(
        path,
        Some(project_request_root),
        options,
        profile_overridden,
        program_args,
    );
    let all_members = match load_workspace_build_targets_for_build_json_from_request_root(
        path,
        project_request_root,
    ) {
        Ok(members) => members,
        Err(failure) => {
            report.record_preflight_failure(failure);
            print!("{}", report.into_json());
            return Err(1);
        }
    };
    let members = match select_workspace_build_targets_for_build_json(
        path,
        &all_members,
        selector,
        "build targets",
    ) {
        Ok(members) => members,
        Err(failure) => {
            report.record_preflight_failure(failure);
            print!("{}", report.into_json());
            return Err(1);
        }
    };
    let runnable = match select_runnable_project_target_for_run_json(path, &members, selector) {
        Ok(runnable) => runnable,
        Err(failure) => {
            report.record_preflight_failure(failure);
            print!("{}", report.into_json());
            return Err(1);
        }
    };
    prepare_reference_interfaces_for_manifests(
        std::slice::from_ref(&runnable.member_manifest_path),
        "`ql run`",
        false,
    )?;
    let runnable_members = select_project_build_plan_root_members(
        &all_members,
        std::slice::from_ref(&runnable.member_manifest_path),
    );
    prepare_project_dependency_builds(
        &all_members,
        &runnable_members,
        "`ql run`",
        options,
        profile_overridden,
    )?;
    let build_plan =
        resolve_project_build_plan_members(&all_members, &runnable_members, "`ql run`")?;

    let target_options = run_target_build_options(options, profile_overridden, &runnable);

    let report_member = WorkspaceBuildTargets {
        member_manifest_path: runnable.member_manifest_path.clone(),
        package_name: runnable.package_name.clone(),
        default_profile: runnable.default_profile,
        targets: vec![runnable.target.clone()],
    };

    match build_project_source_target_result(
        &build_plan,
        &runnable.member_manifest_path,
        &runnable.target.path,
        &target_options,
        options,
        profile_overridden,
        false,
    ) {
        Ok(artifact) => {
            report.record_project_target(&report_member, &runnable.target, &artifact);
            emit_run_json_execution(report, &artifact.path, program_args)
        }
        Err(BuildTargetJsonError::Early(error)) => {
            report.record_project_target_prep_failure(&report_member, &runnable.target, &error);
            print!("{}", report.into_json());
            Err(1)
        }
        Err(BuildTargetJsonError::Build(error)) => {
            report.record_project_build_failure(&report_member, &runnable.target, &error);
            print!("{}", report.into_json());
            Err(1)
        }
    }
}

pub(crate) fn run_project_path(
    path: &Path,
    options: &BuildOptions,
    profile_overridden: bool,
    selector: &ProjectTargetSelector,
    program_args: &[String],
    project_request_root: Option<&Path>,
) -> Result<(), u8> {
    let request_root = project_request_root.unwrap_or(path);
    let all_members =
        load_workspace_build_targets_for_command_from_request_root(path, request_root, "`ql run`")?;
    let members =
        select_workspace_build_targets(path, &all_members, selector, "`ql run`", "build targets")?;
    let runnable = select_runnable_project_target(path, &members, selector)?;
    prepare_reference_interfaces_for_manifests(
        std::slice::from_ref(&runnable.member_manifest_path),
        "`ql run`",
        false,
    )?;
    let runnable_members = select_project_build_plan_root_members(
        &all_members,
        std::slice::from_ref(&runnable.member_manifest_path),
    );
    prepare_project_dependency_builds(
        &all_members,
        &runnable_members,
        "`ql run`",
        options,
        profile_overridden,
    )?;
    let target_options = run_target_build_options(options, profile_overridden, &runnable);
    let artifact = build_project_source_target_silent(
        &all_members,
        "`ql run`",
        &runnable.member_manifest_path,
        &runnable.target.path,
        &target_options,
        options,
        profile_overridden,
        false,
        false,
    )?;
    run_built_executable(&artifact.path, program_args)
}

fn run_target_build_options(
    options: &BuildOptions,
    profile_overridden: bool,
    runnable: &RunnableProjectTarget,
) -> BuildOptions {
    let mut target_options =
        apply_manifest_default_profile(options, runnable.default_profile, profile_overridden);
    if target_options.output.is_none() {
        target_options.output = Some(project_target_output_path(
            &runnable.member_manifest_path,
            runnable.target.path.as_path(),
            target_options.profile,
            target_options.emit,
        ));
    }
    target_options
}

#[cfg(test)]
mod tests {
    use ql_driver::{BuildEmit, BuildProfile};

    use super::*;

    #[test]
    fn run_target_build_options_applies_default_output_path() {
        let options = BuildOptions {
            emit: BuildEmit::Executable,
            profile: BuildProfile::Debug,
            ..BuildOptions::default()
        };
        let runnable = RunnableProjectTarget {
            member_manifest_path: Path::new("workspace/app/qlang.toml").to_path_buf(),
            package_name: "app".to_owned(),
            default_profile: None,
            target: ql_project::BuildTarget {
                kind: ql_project::BuildTargetKind::Binary,
                path: Path::new("workspace/app/src/bin/tool.ql").to_path_buf(),
            },
        };

        let target_options = run_target_build_options(&options, false, &runnable);

        assert_eq!(
            target_options.output.as_deref(),
            Some(Path::new("workspace/app/target/ql/debug/bin/tool.exe"))
        );
    }
}
