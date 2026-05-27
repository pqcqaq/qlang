use std::fs;
use std::path::{Path, PathBuf};

use ql_driver::{BuildArtifact, BuildOptions};
use ql_project::{BuildTargetKind, WorkspaceBuildTargets};

use crate::build_failure_reporting::report_build_input_path_failure;
use crate::build_source_rewrites::render_local_generic_function_specializations;
use crate::cli_utils::normalize_path;
use crate::dependency_bridge_direct::{
    render_direct_dependency_bridge_items, render_direct_dependency_bridge_items_quiet,
};
use crate::dependency_bridge_package_under_test::render_package_under_test_bridge_items;
use crate::dependency_bridge_public_export_wrappers::{
    render_public_dependency_function_export_wrappers,
    render_public_dependency_function_export_wrappers_quiet,
};
use crate::{
    BuildTargetJsonError, PrepareProjectTargetBuildError, PrepareProjectTargetBuildFailureKind,
    ProjectBuildPlanMember, build_single_source_target_with_inputs_impl,
    build_single_source_target_with_inputs_result, project_dependency_target_build_options,
    resolve_project_build_plan_members, select_project_build_plan_root_members,
};

#[derive(Clone, Debug, Default)]
struct PreparedProjectTargetBuild {
    source_override: Option<String>,
    additional_link_inputs: Vec<PathBuf>,
}

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

fn read_project_source_for_build(
    path: &Path,
    options: Option<&BuildOptions>,
    emit_interface: bool,
    report_failure: bool,
) -> Result<String, u8> {
    fs::read_to_string(path).map_err(|error| {
        if report_failure {
            eprintln!(
                "error: failed to access `{}`: {error}",
                normalize_path(path)
            );
            if emit_interface && let Some(options) = options {
                report_build_input_path_failure(path, options, true);
            }
        }
        1
    })
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

fn prepare_project_target_build_quiet(
    build_plan: &[ProjectBuildPlanMember],
    manifest_path: &Path,
    path: &Path,
    dependency_options: &BuildOptions,
    profile_overridden: bool,
    include_public_function_exports: bool,
) -> Result<PreparedProjectTargetBuild, PrepareProjectTargetBuildError> {
    let additional_link_inputs =
        project_dependency_link_inputs(build_plan, dependency_options, profile_overridden);
    let source = fs::read_to_string(path).map_err(|error| PrepareProjectTargetBuildError {
        failure_kind: PrepareProjectTargetBuildFailureKind::SourceRead {
            path: path.to_path_buf(),
            message: format!("failed to access `{}`: {error}", normalize_path(path)),
        },
    })?;
    let mut bridge_items = render_direct_dependency_bridge_items_quiet(manifest_path, &source)?;
    bridge_items.append_items(render_local_generic_function_specializations(&source));
    let public_function_exports = if include_public_function_exports {
        render_public_dependency_function_export_wrappers_quiet(manifest_path, &source)?
    } else {
        String::new()
    };
    bridge_items.append_declarations(&public_function_exports);

    Ok(PreparedProjectTargetBuild {
        source_override: bridge_items.into_source_override(&source),
        additional_link_inputs,
    })
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

#[allow(clippy::too_many_arguments)]
fn prepare_project_target_build(
    workspace_members: &[WorkspaceBuildTargets],
    command_label: &str,
    manifest_path: &Path,
    path: &Path,
    options: &BuildOptions,
    dependency_options: &BuildOptions,
    profile_overridden: bool,
    emit_interface: bool,
    include_public_function_exports: bool,
    report_failure: bool,
) -> Result<PreparedProjectTargetBuild, u8> {
    let selected_members =
        select_project_build_plan_root_members(workspace_members, &[manifest_path.to_path_buf()]);
    let build_plan =
        resolve_project_build_plan_members(workspace_members, &selected_members, command_label)?;
    let additional_link_inputs =
        project_dependency_link_inputs(&build_plan, dependency_options, profile_overridden);
    let source =
        read_project_source_for_build(path, Some(options), emit_interface, report_failure)?;
    let mut bridge_items = match render_direct_dependency_bridge_items(
        command_label,
        manifest_path,
        &source,
        report_failure,
    ) {
        Ok(items) => items,
        Err(code) => return Err(code),
    };
    bridge_items.append_items(render_local_generic_function_specializations(&source));
    let public_function_exports = match if include_public_function_exports {
        render_public_dependency_function_export_wrappers(
            command_label,
            manifest_path,
            &source,
            report_failure,
        )
    } else {
        Ok(String::new())
    } {
        Ok(items) => items,
        Err(code) => return Err(code),
    };
    bridge_items.append_declarations(&public_function_exports);

    Ok(PreparedProjectTargetBuild {
        source_override: bridge_items.into_source_override(&source),
        additional_link_inputs,
    })
}

fn prepare_project_test_target_build(
    workspace_members: &[WorkspaceBuildTargets],
    command_label: &str,
    manifest_path: &Path,
    path: &Path,
    dependency_options: &BuildOptions,
    profile_overridden: bool,
    report_failure: bool,
) -> Result<PreparedProjectTargetBuild, u8> {
    let selected_members =
        select_project_build_plan_root_members(workspace_members, &[manifest_path.to_path_buf()]);
    let build_plan =
        resolve_project_build_plan_members(workspace_members, &selected_members, command_label)?;
    let mut additional_link_inputs =
        project_dependency_link_inputs(&build_plan, dependency_options, profile_overridden);
    additional_link_inputs.extend(project_selected_library_link_inputs(
        &build_plan,
        dependency_options,
        profile_overridden,
    ));
    let source = read_project_source_for_build(path, None, false, report_failure)?;
    let mut bridge_items = match render_direct_dependency_bridge_items(
        command_label,
        manifest_path,
        &source,
        report_failure,
    ) {
        Ok(items) => items,
        Err(code) => return Err(code),
    };
    bridge_items.append_items(
        match render_package_under_test_bridge_items(
            command_label,
            workspace_members,
            manifest_path,
            &source,
            report_failure,
        ) {
            Ok(items) => items,
            Err(code) => return Err(code),
        },
    );
    bridge_items.append_items(render_local_generic_function_specializations(&source));

    Ok(PreparedProjectTargetBuild {
        source_override: bridge_items.into_source_override(&source),
        additional_link_inputs,
    })
}

fn project_dependency_link_inputs(
    build_plan: &[ProjectBuildPlanMember],
    dependency_options: &BuildOptions,
    profile_overridden: bool,
) -> Vec<PathBuf> {
    let mut outputs = Vec::new();
    for plan_member in build_plan.iter().rev() {
        if plan_member.require_targets {
            continue;
        }
        for target in &plan_member.member.targets {
            if target.kind != BuildTargetKind::Library {
                continue;
            }
            let target_options = project_dependency_target_build_options(
                &plan_member.member,
                target,
                dependency_options,
                false,
                profile_overridden,
            );
            if let Some(path) = target_options.output {
                outputs.push(path);
            }
        }
    }
    outputs
}

fn project_selected_library_link_inputs(
    build_plan: &[ProjectBuildPlanMember],
    dependency_options: &BuildOptions,
    profile_overridden: bool,
) -> Vec<PathBuf> {
    let mut outputs = Vec::new();
    for plan_member in build_plan.iter().rev() {
        if !plan_member.require_targets {
            continue;
        }
        for target in &plan_member.member.targets {
            if target.kind != BuildTargetKind::Library {
                continue;
            }
            let target_options = project_dependency_target_build_options(
                &plan_member.member,
                target,
                dependency_options,
                false,
                profile_overridden,
            );
            if let Some(path) = target_options.output {
                outputs.push(path);
            }
        }
    }
    outputs
}
