use std::path::Path;

use ql_driver::{BuildArtifact, BuildOptions};
use ql_project::WorkspaceBuildTargets;

use crate::build_plan::{BuildTargetJsonError, ProjectBuildPlanMember};
use crate::build_single_source::{
    build_single_source_target_with_inputs_impl, build_single_source_target_with_inputs_result,
};
use crate::project_target_build_prep::{
    prepare_project_target_build, prepare_project_target_build_quiet,
    prepare_project_test_target_build,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_project_source_target(
    workspace_members: &[WorkspaceBuildTargets],
    command_label: &str,
    manifest_path: &Path,
    path: &Path,
    options: &BuildOptions,
    dependency_options: &BuildOptions,
    profile_overridden: bool,
    emit_interface: bool,
    include_public_function_exports: bool,
) -> Result<BuildArtifact, u8> {
    build_project_source_target_impl(
        workspace_members,
        command_label,
        manifest_path,
        path,
        options,
        dependency_options,
        profile_overridden,
        emit_interface,
        include_public_function_exports,
        true,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_project_source_target_silent(
    workspace_members: &[WorkspaceBuildTargets],
    command_label: &str,
    manifest_path: &Path,
    path: &Path,
    options: &BuildOptions,
    dependency_options: &BuildOptions,
    profile_overridden: bool,
    emit_interface: bool,
    include_public_function_exports: bool,
) -> Result<BuildArtifact, u8> {
    build_project_source_target_impl(
        workspace_members,
        command_label,
        manifest_path,
        path,
        options,
        dependency_options,
        profile_overridden,
        emit_interface,
        include_public_function_exports,
        false,
        true,
    )
}

pub(crate) fn build_project_source_target_result(
    build_plan: &[ProjectBuildPlanMember],
    manifest_path: &Path,
    path: &Path,
    options: &BuildOptions,
    dependency_options: &BuildOptions,
    profile_overridden: bool,
    include_public_function_exports: bool,
) -> Result<BuildArtifact, BuildTargetJsonError> {
    let prepared = prepare_project_target_build_quiet(
        build_plan,
        manifest_path,
        path,
        dependency_options,
        profile_overridden,
        include_public_function_exports,
    )
    .map_err(BuildTargetJsonError::Early)?;
    build_single_source_target_with_inputs_result(
        path,
        options,
        prepared.source_override.as_deref(),
        &prepared.additional_link_inputs,
    )
    .map_err(BuildTargetJsonError::Build)
}

pub(crate) fn build_project_test_source_target_silent(
    workspace_members: &[WorkspaceBuildTargets],
    command_label: &str,
    manifest_path: &Path,
    path: &Path,
    options: &BuildOptions,
    dependency_options: &BuildOptions,
    profile_overridden: bool,
) -> Result<BuildArtifact, u8> {
    build_project_test_source_target_impl(
        workspace_members,
        command_label,
        manifest_path,
        path,
        options,
        dependency_options,
        profile_overridden,
        true,
    )
}

pub(crate) fn build_project_test_source_target_quiet(
    workspace_members: &[WorkspaceBuildTargets],
    command_label: &str,
    manifest_path: &Path,
    path: &Path,
    options: &BuildOptions,
    dependency_options: &BuildOptions,
    profile_overridden: bool,
) -> Result<BuildArtifact, u8> {
    build_project_test_source_target_impl(
        workspace_members,
        command_label,
        manifest_path,
        path,
        options,
        dependency_options,
        profile_overridden,
        false,
    )
}

fn build_project_test_source_target_impl(
    workspace_members: &[WorkspaceBuildTargets],
    command_label: &str,
    manifest_path: &Path,
    path: &Path,
    options: &BuildOptions,
    dependency_options: &BuildOptions,
    profile_overridden: bool,
    report_failure: bool,
) -> Result<BuildArtifact, u8> {
    let prepared = prepare_project_test_target_build(
        workspace_members,
        command_label,
        manifest_path,
        path,
        dependency_options,
        profile_overridden,
        report_failure,
    )?;
    build_single_source_target_with_inputs_impl(
        path,
        options,
        false,
        false,
        report_failure,
        prepared.source_override.as_deref(),
        &prepared.additional_link_inputs,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_project_source_target_impl(
    workspace_members: &[WorkspaceBuildTargets],
    command_label: &str,
    manifest_path: &Path,
    path: &Path,
    options: &BuildOptions,
    dependency_options: &BuildOptions,
    profile_overridden: bool,
    emit_interface: bool,
    include_public_function_exports: bool,
    report_success: bool,
    report_failure: bool,
) -> Result<BuildArtifact, u8> {
    let prepared = prepare_project_target_build(
        workspace_members,
        command_label,
        manifest_path,
        path,
        options,
        dependency_options,
        profile_overridden,
        emit_interface,
        include_public_function_exports,
        report_failure,
    )?;
    build_single_source_target_with_inputs_impl(
        path,
        options,
        emit_interface,
        report_success,
        report_failure,
        prepared.source_override.as_deref(),
        &prepared.additional_link_inputs,
    )
}
