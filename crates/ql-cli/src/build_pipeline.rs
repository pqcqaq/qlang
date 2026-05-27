use std::path::Path;

use ql_driver::BuildOptions;

use crate::build_interface_emission::{
    emit_built_package_interface, emit_built_package_interface_quiet,
};
use crate::build_json_report::{BuildJsonReport, emit_build_json_failure};
use crate::build_outputs::{
    first_colliding_project_build_header_output_path, first_colliding_project_build_output_path,
};
use crate::build_plan::{
    resolve_project_build_plan_members, resolve_project_build_plan_members_quiet,
};
use crate::build_project_execution::execute_project_build_plan;
use crate::build_reporting::{
    build_json_build_plan_failure, build_json_dependency_interface_prep_failure,
    build_json_emit_interface_failure, build_json_preflight_failure,
    load_workspace_build_targets_for_build_json_from_request_root,
    select_workspace_build_targets_for_build_json,
};
use crate::build_single_source::{build_single_source_target, build_single_source_target_result};
use crate::cli_utils::normalize_path;
use crate::project_interfaces::prepare_reference_interfaces_for_manifests_quiet;
use crate::project_reference_interfaces::prepare_reference_interfaces_for_manifests;
use crate::project_targets::{
    ProjectCommandPathError, ProjectTargetSelector, ResolvedProjectCommandPath,
    load_workspace_build_targets_for_command_from_request_root,
    report_project_source_path_rejects_target_selector,
    report_project_target_selector_requires_project_context, resolve_project_command_path,
    select_workspace_build_targets,
};

pub(crate) fn build_path(
    path: &Path,
    options: &BuildOptions,
    selector: &ProjectTargetSelector,
    emit_interface: bool,
    emit_overridden: bool,
    profile_overridden: bool,
    json: bool,
) -> Result<(), u8> {
    match resolve_project_command_path(path, selector) {
        Ok(ResolvedProjectCommandPath::Project {
            request_root_manifest_path,
            selector,
        }) => {
            return build_project_path(
                path,
                options,
                &selector,
                emit_interface,
                emit_overridden,
                profile_overridden,
                json,
                request_root_manifest_path.as_deref(),
            );
        }
        Ok(ResolvedProjectCommandPath::DirectSource) => {}
        Err(ProjectCommandPathError::SourcePathRejectsSelector) => {
            if json {
                let mut report =
                    BuildJsonReport::new(path, None, options, profile_overridden, emit_interface);
                report.record_preflight_failure(build_json_preflight_failure(
                    path,
                    None,
                    None,
                    None,
                    "selector",
                    "project-context",
                    "direct project source paths do not support target selectors".to_owned(),
                    Some(selector.describe()),
                    None,
                    None,
                ));
                print!("{}", report.into_json());
            } else {
                report_project_source_path_rejects_target_selector("`ql build`", path, selector);
            }
            return Err(1);
        }
        Err(ProjectCommandPathError::SelectorRequiresProjectContext) => {
            if json {
                let mut report =
                    BuildJsonReport::new(path, None, options, profile_overridden, emit_interface);
                report.record_preflight_failure(build_json_preflight_failure(
                    path,
                    None,
                    None,
                    None,
                    "selector",
                    "project-context",
                    "target selectors require a package or workspace path".to_owned(),
                    Some(selector.describe()),
                    None,
                    None,
                ));
                print!("{}", report.into_json());
            } else {
                report_project_target_selector_requires_project_context("`ql build`", selector);
            }
            return Err(1);
        }
    }

    if json {
        let mut report =
            BuildJsonReport::new(path, None, options, profile_overridden, emit_interface);
        let artifact = match build_single_source_target_result(path, options) {
            Ok(artifact) => artifact,
            Err(error) => {
                report.record_source_failure(path, &error);
                print!("{}", report.into_json());
                return Err(1);
            }
        };
        report.record_source_target(path, &artifact);
        if emit_interface {
            match emit_built_package_interface_quiet(path, path, options, &artifact.path, &[]) {
                Ok(interface_result) => {
                    report.record_interface_result(None, None, true, interface_result);
                }
                Err(error) => {
                    report.record_preflight_failure(build_json_emit_interface_failure(
                        path, None, None, &error,
                    ));
                    print!("{}", report.into_json());
                    return Err(1);
                }
            }
        }
        print!("{}", report.into_json());
        return Ok(());
    }

    let artifact = build_single_source_target(path, options, emit_interface)?;
    if emit_interface {
        emit_built_package_interface(path, path, options, &artifact.path, &[])?;
    }
    Ok(())
}

fn build_project_path(
    path: &Path,
    options: &BuildOptions,
    selector: &ProjectTargetSelector,
    emit_interface: bool,
    emit_overridden: bool,
    profile_overridden: bool,
    json: bool,
    project_request_root: Option<&Path>,
) -> Result<(), u8> {
    let mut json_report = json.then(|| {
        BuildJsonReport::new(
            path,
            project_request_root,
            options,
            profile_overridden,
            emit_interface,
        )
    });
    let request_root = project_request_root.unwrap_or(path);

    let members = if json {
        match load_workspace_build_targets_for_build_json_from_request_root(path, request_root) {
            Ok(members) => members,
            Err(failure) => return emit_build_json_failure(&mut json_report, failure),
        }
    } else {
        load_workspace_build_targets_for_command_from_request_root(
            path,
            request_root,
            "`ql build`",
        )?
    };
    let selected_members = if json {
        match select_workspace_build_targets_for_build_json(
            path,
            &members,
            selector,
            "build targets",
        ) {
            Ok(selected) => selected,
            Err(failure) => return emit_build_json_failure(&mut json_report, failure),
        }
    } else {
        select_workspace_build_targets(path, &members, selector, "`ql build`", "build targets")?
    };
    let total_targets = selected_members
        .iter()
        .map(|member| member.targets.len())
        .sum::<usize>();
    if total_targets == 0 {
        let normalized_path = normalize_path(path);
        if json {
            return emit_build_json_failure(
                &mut json_report,
                build_json_preflight_failure(
                    path,
                    None,
                    None,
                    None,
                    "project",
                    "target-discovery",
                    format!("found no discovered build targets under `{normalized_path}`"),
                    None,
                    None,
                    Some(0),
                ),
            );
        }
        eprintln!("error: `ql build` found no discovered build targets under `{normalized_path}`");
        eprintln!(
            "hint: rerun `ql project targets {normalized_path}` to inspect the discovered build targets"
        );
        return Err(1);
    }

    if total_targets > 1 {
        let normalized_path = normalize_path(path);
        if options.output.is_some() {
            if json {
                return emit_build_json_failure(
                    &mut json_report,
                    build_json_preflight_failure(
                        path,
                        None,
                        None,
                        None,
                        "output-conflict",
                        "output-planning",
                        "`ql build --output` only supports a single discovered build target"
                            .to_owned(),
                        None,
                        None,
                        Some(total_targets),
                    ),
                );
            }
            eprintln!("error: `ql build --output` only supports a single discovered build target");
            eprintln!("note: `{normalized_path}` resolved to {total_targets} build targets");
            return Err(1);
        }
        if options
            .c_header
            .as_ref()
            .and_then(|header| header.output.as_ref())
            .is_some()
        {
            if json {
                return emit_build_json_failure(
                    &mut json_report,
                    build_json_preflight_failure(
                        path,
                        None,
                        None,
                        None,
                        "output-conflict",
                        "output-planning",
                        "`ql build --header-output` only supports a single discovered build target"
                            .to_owned(),
                        None,
                        None,
                        Some(total_targets),
                    ),
                );
            }
            eprintln!(
                "error: `ql build --header-output` only supports a single discovered build target"
            );
            eprintln!("note: `{normalized_path}` resolved to {total_targets} build targets");
            return Err(1);
        }
    }

    if let Some(output_path) = first_colliding_project_build_output_path(
        &selected_members,
        options,
        emit_overridden,
        profile_overridden,
    ) {
        if json {
            return emit_build_json_failure(
                &mut json_report,
                build_json_preflight_failure(
                    path,
                    None,
                    None,
                    None,
                    "output-conflict",
                    "output-planning",
                    format!(
                        "resolved multiple build targets to the same output path `{}`",
                        normalize_path(&output_path)
                    ),
                    None,
                    Some(normalize_path(&output_path)),
                    None,
                ),
            );
        }
        eprintln!(
            "error: `ql build` resolved multiple build targets to the same output path `{}`",
            normalize_path(&output_path)
        );
        eprintln!(
            "hint: rerun a single package/target path or rename the conflicting build target stems"
        );
        return Err(1);
    }
    if let Some(output_path) = first_colliding_project_build_header_output_path(
        &selected_members,
        options,
        emit_overridden,
        profile_overridden,
    ) {
        if json {
            return emit_build_json_failure(
                &mut json_report,
                build_json_preflight_failure(
                    path,
                    None,
                    None,
                    None,
                    "output-conflict",
                    "output-planning",
                    format!(
                        "resolved multiple build targets to the same header output path `{}`",
                        normalize_path(&output_path)
                    ),
                    None,
                    Some(normalize_path(&output_path)),
                    None,
                ),
            );
        }
        eprintln!(
            "error: `ql build` resolved multiple build targets to the same header output path `{}`",
            normalize_path(&output_path)
        );
        eprintln!(
            "hint: rerun a single package/target path or rename the conflicting build target stems"
        );
        return Err(1);
    }

    let member_manifest_paths = selected_members
        .iter()
        .map(|member| member.member_manifest_path.clone())
        .collect::<Vec<_>>();
    if json {
        if let Err(failure) =
            prepare_reference_interfaces_for_manifests_quiet(&member_manifest_paths)
        {
            return emit_build_json_failure(
                &mut json_report,
                build_json_dependency_interface_prep_failure(path, &failure),
            );
        }
    } else {
        prepare_reference_interfaces_for_manifests(&member_manifest_paths, "`ql build`", true)?;
    }

    let build_plan = if json {
        match resolve_project_build_plan_members_quiet(&members, &selected_members) {
            Ok(build_plan) => build_plan,
            Err(failure) => {
                return emit_build_json_failure(
                    &mut json_report,
                    build_json_build_plan_failure(path, &failure),
                );
            }
        }
    } else {
        resolve_project_build_plan_members(&members, &selected_members, "`ql build`")?
    };

    execute_project_build_plan(
        path,
        &members,
        &build_plan,
        options,
        emit_interface,
        emit_overridden,
        profile_overridden,
        &mut json_report,
    )?;

    if let Some(report) = json_report {
        print!("{}", report.into_json());
    }

    Ok(())
}
