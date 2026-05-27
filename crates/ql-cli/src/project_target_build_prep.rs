use std::fs;
use std::path::{Path, PathBuf};

use ql_driver::BuildOptions;
use ql_project::{BuildTargetKind, WorkspaceBuildTargets};

use crate::build_failure_reporting::report_build_input_path_failure;
use crate::build_outputs::project_dependency_target_build_options;
use crate::build_plan::{
    PrepareProjectTargetBuildError, PrepareProjectTargetBuildFailureKind, ProjectBuildPlanMember,
    resolve_project_build_plan_members, select_project_build_plan_root_members,
};
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

#[derive(Clone, Debug, Default)]
pub(crate) struct PreparedProjectTargetBuild {
    pub(crate) source_override: Option<String>,
    pub(crate) additional_link_inputs: Vec<PathBuf>,
}

pub(crate) fn prepare_project_target_build_quiet(
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
    let source_override = render_project_source_override_quiet(
        manifest_path,
        &source,
        include_public_function_exports,
    )?;

    Ok(PreparedProjectTargetBuild {
        source_override,
        additional_link_inputs,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_project_target_build(
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
    let source_override = render_project_source_override(
        command_label,
        manifest_path,
        &source,
        include_public_function_exports,
        report_failure,
    )?;

    Ok(PreparedProjectTargetBuild {
        source_override,
        additional_link_inputs,
    })
}

pub(crate) fn prepare_project_test_target_build(
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
    let mut bridge_items = render_direct_dependency_bridge_items(
        command_label,
        manifest_path,
        &source,
        report_failure,
    )?;
    bridge_items.append_items(render_package_under_test_bridge_items(
        command_label,
        workspace_members,
        manifest_path,
        &source,
        report_failure,
    )?);
    bridge_items.append_items(render_local_generic_function_specializations(&source));

    Ok(PreparedProjectTargetBuild {
        source_override: bridge_items.into_source_override(&source),
        additional_link_inputs,
    })
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

fn render_project_source_override_quiet(
    manifest_path: &Path,
    source: &str,
    include_public_function_exports: bool,
) -> Result<Option<String>, PrepareProjectTargetBuildError> {
    let mut bridge_items = render_direct_dependency_bridge_items_quiet(manifest_path, source)?;
    bridge_items.append_items(render_local_generic_function_specializations(source));
    let public_function_exports = if include_public_function_exports {
        render_public_dependency_function_export_wrappers_quiet(manifest_path, source)?
    } else {
        String::new()
    };
    bridge_items.append_declarations(&public_function_exports);
    Ok(bridge_items.into_source_override(source))
}

fn render_project_source_override(
    command_label: &str,
    manifest_path: &Path,
    source: &str,
    include_public_function_exports: bool,
    report_failure: bool,
) -> Result<Option<String>, u8> {
    let mut bridge_items = render_direct_dependency_bridge_items(
        command_label,
        manifest_path,
        source,
        report_failure,
    )?;
    bridge_items.append_items(render_local_generic_function_specializations(source));
    let public_function_exports = if include_public_function_exports {
        render_public_dependency_function_export_wrappers(
            command_label,
            manifest_path,
            source,
            report_failure,
        )?
    } else {
        String::new()
    };
    bridge_items.append_declarations(&public_function_exports);
    Ok(bridge_items.into_source_override(source))
}

pub(crate) fn project_dependency_link_inputs(
    build_plan: &[ProjectBuildPlanMember],
    dependency_options: &BuildOptions,
    profile_overridden: bool,
) -> Vec<PathBuf> {
    library_link_inputs(build_plan, dependency_options, profile_overridden, false)
}

pub(crate) fn project_selected_library_link_inputs(
    build_plan: &[ProjectBuildPlanMember],
    dependency_options: &BuildOptions,
    profile_overridden: bool,
) -> Vec<PathBuf> {
    library_link_inputs(build_plan, dependency_options, profile_overridden, true)
}

fn library_link_inputs(
    build_plan: &[ProjectBuildPlanMember],
    dependency_options: &BuildOptions,
    profile_overridden: bool,
    selected_members: bool,
) -> Vec<PathBuf> {
    let mut outputs = Vec::new();
    for plan_member in build_plan.iter().rev() {
        if plan_member.require_targets != selected_members {
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

#[cfg(test)]
mod tests {
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
}
