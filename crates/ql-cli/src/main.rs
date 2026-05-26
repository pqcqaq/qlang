use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ql_ast::{
    CallArg, Expr, ExprKind, FunctionDecl, GlobalDecl, ItemKind, Module, Param, Visibility,
};
use ql_driver::{BuildArtifact, BuildOptions};
use ql_parser::parse_source;
use ql_project::{
    BuildTargetKind, WorkspaceBuildTargets, default_interface_path, load_interface_artifact,
    load_project_manifest, load_reference_manifests, package_name,
};
mod analysis_commands;
mod build_command;
mod build_failure_reporting;
mod build_outputs;
mod build_pipeline;
mod build_plan;
mod build_reporting;
mod build_single_source;
mod build_source_rewrites;
mod check_command;
mod check_reporting;
mod cli_analysis;
mod cli_build_profile;
mod cli_diagnostics;
mod cli_json_diagnostics;
mod cli_scan;
mod cli_usage;
mod cli_utils;
mod cli_version;
mod dependency_bridge_modules;
mod dependency_bridge_names;
mod dependency_generic_bridge;
mod ffi_command;
mod fmt_command;
mod project_dependencies;
mod project_dependency_commands;
mod project_dependency_edit;
mod project_emit_interface;
mod project_graph;
mod project_init;
mod project_interface_reporting;
mod project_interfaces;
mod project_lifecycle_commands;
mod project_lock;
mod project_maintenance_commands;
mod project_manifest_edit;
mod project_manifest_paths;
mod project_members;
mod project_query_commands;
mod project_reference_interfaces;
mod project_reporting;
mod project_status;
mod project_targets;
mod project_workspace;
mod run_command;
mod run_pipeline;
mod test_command;
mod test_pipeline;
mod test_reporting;

use build_failure_reporting::report_build_input_path_failure;
pub(crate) use build_outputs::{
    apply_manifest_default_profile, first_colliding_project_build_header_output_path,
    first_colliding_project_build_output_path, project_dependency_target_build_options,
    project_target_build_options, project_target_output_path,
};
pub(crate) use build_plan::{
    BuildPlanResolveError, BuildPlanResolveFailureKind, BuildTargetJsonError,
    PrepareProjectTargetBuildError, PrepareProjectTargetBuildFailureKind, ProjectBuildPlanMember,
    prepare_project_dependency_builds, prepare_project_test_package_builds,
    report_project_build_dependency_error, resolve_project_build_plan_members,
    resolve_project_build_plan_members_quiet, select_project_build_plan_root_members,
    target_prep_dependency_manifest_failure,
};
pub(crate) use build_reporting::{
    BuildJsonReport, build_emit_cli_value, build_json_build_plan_failure,
    build_json_dependency_interface_prep_failure, build_json_emit_interface_failure,
    build_json_failure, build_json_preflight_failure, build_json_project_error, build_json_target,
    build_json_target_prep_failure, load_workspace_build_targets_for_build_json_from_request_root,
    select_workspace_build_targets_for_build_json,
};
pub(crate) use build_single_source::{
    build_output_lock_error_message, build_single_source_target, build_single_source_target_quiet,
    build_single_source_target_result, build_single_source_target_silent,
    build_single_source_target_with_inputs_impl, build_single_source_target_with_inputs_result,
    emit_built_package_interface, emit_built_package_interface_quiet,
};
use build_source_rewrites::{
    RenderedDependencyBridgeItems, join_dependency_bridge_sections,
    render_local_generic_function_specializations,
};
use cli_usage::print_usage;
use cli_utils::normalize_path;
use cli_version::{CLI_NAME, is_version_command, version_text};
use dependency_bridge_modules::{
    dependency_generic_specialization_module_refs, dependency_generic_specialization_modules,
    dependency_generic_specialization_modules_quiet, dependency_interface_module_import_path,
    dependency_interface_module_import_paths, dependency_module_source_path,
    package_under_test_bridge_modules,
};
use dependency_bridge_names::{
    DependencyExternOwner, record_dependency_extern_declaration,
    render_dependency_public_function_export_wrapper,
    render_dependency_public_method_export_wrapper,
    render_imported_dependency_public_function_forwarder,
    render_imported_dependency_public_method_forwarder, span_text,
    supports_dependency_public_function_export_bridge,
    supports_dependency_public_function_import_bridge,
    supports_dependency_public_method_import_bridge,
};
pub(crate) use project_emit_interface::project_emit_interface_path;
use project_manifest_paths::reference_manifest_path;
pub(crate) use test_pipeline::{
    discover_test_targets, execute_test_targets, filter_test_targets, list_test_targets,
    report_no_matching_test_target, report_no_matching_tests, report_no_tests_discovered,
    select_test_targets_by_path, test_build_options, test_no_matching_filter_message,
    test_no_matching_target_message, test_no_tests_message,
};
pub(crate) use test_reporting::{
    TestExecutionReport, TestFailure, TestTarget, TestTargetKind,
    render_test_json_preflight_failure_report, render_test_json_preflight_message_report,
    render_test_json_report, render_test_json_selection_failure_report,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => ExitCode::from(code),
    }
}

fn run() -> Result<(), u8> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        return Err(1);
    };

    if is_version_command(&command) {
        if let Some(extra) = args.next() {
            eprintln!("error: `ql {command}` does not accept additional argument `{extra}`");
            return Err(1);
        }
        println!("{}", version_text(CLI_NAME));
        return Ok(());
    }

    match command.as_str() {
        "check" => check_command::check_cli_path(args),
        "fmt" => fmt_command::fmt_path(args),
        "mir" => analysis_commands::mir_path(&mut args),
        "ownership" => analysis_commands::ownership_path(&mut args),
        "runtime" => analysis_commands::runtime_path(&mut args),
        "build" => build_command::build_cli_path(&mut args),
        "run" => run_command::run_cli_path(&mut args),
        "test" => test_command::test_cli_path(&mut args),
        "project" => {
            let Some(subcommand) = args.next() else {
                eprintln!("error: `ql project` expects a subcommand");
                return Err(1);
            };

            match subcommand.as_str() {
                "status" => project_query_commands::project_status_cli_path(&mut args),
                "targets" => project_query_commands::project_targets_cli_path(&mut args),
                "target" => project_maintenance_commands::project_target_cli_path(&mut args),
                "graph" => project_query_commands::project_graph_cli_path(&mut args),
                "dependents" => project_query_commands::project_dependents_cli_path(&mut args),
                "dependencies" => project_query_commands::project_dependencies_cli_path(&mut args),
                "lock" => project_maintenance_commands::project_lock_cli_path(&mut args),
                "emit-interface" => {
                    project_maintenance_commands::project_emit_interface_cli_path(&mut args)
                }
                "init" => project_lifecycle_commands::project_init_cli_path(&mut args),
                "add" => project_lifecycle_commands::project_add_cli_path(&mut args),
                "remove" => project_lifecycle_commands::project_remove_cli_path(&mut args),
                "add-dependency" => {
                    project_dependency_commands::project_add_dependency_cli_path(&mut args)
                }
                "remove-dependency" => {
                    project_dependency_commands::project_remove_dependency_cli_path(&mut args)
                }
                other => {
                    eprintln!("error: unknown `ql project` subcommand `{other}`");
                    print_usage();
                    Err(1)
                }
            }
        }
        "ffi" => ffi_command::ffi_path(&mut args),
        _ => {
            eprintln!("error: unknown command `{command}`");
            print_usage();
            Err(1)
        }
    }
}

#[derive(Clone, Debug, Default)]
struct PreparedProjectTargetBuild {
    source_override: Option<String>,
    additional_link_inputs: Vec<PathBuf>,
}

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

#[derive(Clone, Debug, Default)]
struct ImportedDependencyExterns {
    whole_paths: BTreeSet<Vec<String>>,
    symbols_by_module_path: BTreeMap<Vec<String>, BTreeSet<String>>,
}

fn collect_top_level_definition_names(module: &Module) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for item in &module.items {
        match &item.kind {
            ItemKind::Function(function) => {
                names.insert(function.name.clone());
            }
            ItemKind::Const(global) | ItemKind::Static(global) => {
                names.insert(global.name.clone());
            }
            ItemKind::Struct(struct_decl) => {
                names.insert(struct_decl.name.clone());
            }
            ItemKind::Enum(enum_decl) => {
                names.insert(enum_decl.name.clone());
            }
            ItemKind::Trait(trait_decl) => {
                names.insert(trait_decl.name.clone());
            }
            ItemKind::TypeAlias(alias) => {
                names.insert(alias.name.clone());
            }
            ItemKind::ExternBlock(extern_block) => {
                for function in &extern_block.functions {
                    names.insert(function.name.clone());
                }
            }
            ItemKind::Impl(_) | ItemKind::Extend(_) => {}
        }
    }
    names
}

fn collect_imported_dependency_externs(
    root_module: &Module,
    module_paths: &BTreeSet<Vec<String>>,
) -> ImportedDependencyExterns {
    let mut imported = ImportedDependencyExterns::default();

    for use_decl in &root_module.uses {
        if let Some(group) = &use_decl.group {
            if !module_paths.contains(&use_decl.prefix.segments) {
                continue;
            }
            let symbols = imported
                .symbols_by_module_path
                .entry(use_decl.prefix.segments.clone())
                .or_default();
            for item in group {
                symbols.insert(item.name.clone());
            }
            continue;
        }

        if module_paths.contains(&use_decl.prefix.segments)
            || module_paths
                .iter()
                .any(|module_path| module_path.starts_with(&use_decl.prefix.segments))
        {
            imported
                .whole_paths
                .insert(use_decl.prefix.segments.clone());
            continue;
        }

        if use_decl.prefix.segments.len() < 2 {
            continue;
        }

        let module_path = use_decl.prefix.segments[..use_decl.prefix.segments.len() - 1].to_vec();
        if !module_paths.contains(&module_path) {
            continue;
        }

        if let Some(symbol_name) = use_decl.prefix.segments.last() {
            imported
                .symbols_by_module_path
                .entry(module_path)
                .or_default()
                .insert(symbol_name.clone());
        }
    }

    imported
}

fn dependency_extern_is_imported(
    imported_externs: &ImportedDependencyExterns,
    module_path: &[String],
    symbol_name: &str,
) -> bool {
    imported_externs
        .whole_paths
        .iter()
        .any(|whole_path| module_path.starts_with(whole_path))
        || imported_externs
            .symbols_by_module_path
            .get(module_path)
            .is_some_and(|symbols| symbols.contains(symbol_name))
}

fn extend_dependency_bridge_name_requirements(
    destination: &mut BTreeMap<Vec<String>, BTreeSet<String>>,
    source: &BTreeMap<Vec<String>, BTreeSet<String>>,
) {
    for (module_path, symbols) in source {
        destination
            .entry(module_path.clone())
            .or_default()
            .extend(symbols.iter().cloned());
    }
}

fn render_direct_dependency_bridge_items(
    command_label: &str,
    manifest_path: &Path,
    source: &str,
    report_failure: bool,
) -> Result<RenderedDependencyBridgeItems, u8> {
    let value_declarations = render_direct_dependency_public_value_declarations(
        command_label,
        manifest_path,
        source,
        report_failure,
    )?;
    let function_forwarders = render_direct_dependency_public_function_forwarders(
        command_label,
        manifest_path,
        source,
        &value_declarations.required_functions_by_module_path,
        report_failure,
    )?;
    let mut required_types_by_module_path = value_declarations.required_types_by_module_path;
    extend_dependency_bridge_name_requirements(
        &mut required_types_by_module_path,
        &function_forwarders.required_types_by_module_path,
    );
    let method_forwarders = render_direct_dependency_public_method_forwarders(
        command_label,
        manifest_path,
        &required_types_by_module_path,
        report_failure,
    )?;
    extend_dependency_bridge_name_requirements(
        &mut required_types_by_module_path,
        &method_forwarders.required_types_by_module_path,
    );
    let type_declarations = render_direct_dependency_public_type_declarations(
        command_label,
        manifest_path,
        source,
        &required_types_by_module_path,
        report_failure,
    )?;
    let extern_declarations = render_direct_dependency_extern_declarations(
        command_label,
        manifest_path,
        source,
        report_failure,
    )?;
    let declarations =
        join_dependency_bridge_sections(&type_declarations, &value_declarations.declarations);
    let declarations = join_dependency_bridge_sections(&declarations, &extern_declarations);
    let declarations =
        join_dependency_bridge_sections(&declarations, &function_forwarders.forwarders);
    let declarations =
        join_dependency_bridge_sections(&declarations, &method_forwarders.forwarders);
    Ok(RenderedDependencyBridgeItems {
        declarations,
        source_rewrites: function_forwarders.source_rewrites,
    })
}

fn render_direct_dependency_bridge_items_quiet(
    manifest_path: &Path,
    source: &str,
) -> Result<RenderedDependencyBridgeItems, PrepareProjectTargetBuildError> {
    let value_declarations =
        render_direct_dependency_public_value_declarations_quiet(manifest_path, source)?;
    let function_forwarders = render_direct_dependency_public_function_forwarders_quiet(
        manifest_path,
        source,
        &value_declarations.required_functions_by_module_path,
    )?;
    let mut required_types_by_module_path = value_declarations.required_types_by_module_path;
    extend_dependency_bridge_name_requirements(
        &mut required_types_by_module_path,
        &function_forwarders.required_types_by_module_path,
    );
    let method_forwarders = render_direct_dependency_public_method_forwarders_quiet(
        manifest_path,
        &required_types_by_module_path,
    )?;
    extend_dependency_bridge_name_requirements(
        &mut required_types_by_module_path,
        &method_forwarders.required_types_by_module_path,
    );
    let type_declarations = render_direct_dependency_public_type_declarations_quiet(
        manifest_path,
        source,
        &required_types_by_module_path,
    )?;
    let extern_declarations =
        render_direct_dependency_extern_declarations_quiet(manifest_path, source)?;
    let declarations =
        join_dependency_bridge_sections(&type_declarations, &value_declarations.declarations);
    let declarations = join_dependency_bridge_sections(&declarations, &extern_declarations);
    let declarations =
        join_dependency_bridge_sections(&declarations, &function_forwarders.forwarders);
    let declarations =
        join_dependency_bridge_sections(&declarations, &method_forwarders.forwarders);
    Ok(RenderedDependencyBridgeItems {
        declarations,
        source_rewrites: function_forwarders.source_rewrites,
    })
}

fn render_package_under_test_bridge_items(
    command_label: &str,
    workspace_members: &[WorkspaceBuildTargets],
    manifest_path: &Path,
    source: &str,
    report_failure: bool,
) -> Result<RenderedDependencyBridgeItems, u8> {
    let Some(member) = workspace_members.iter().find(|member| {
        normalize_path(&member.member_manifest_path) == normalize_path(manifest_path)
    }) else {
        return Ok(RenderedDependencyBridgeItems::default());
    };
    let root_source_module = match parse_source(source) {
        Ok(module) => module,
        Err(_) => return Ok(RenderedDependencyBridgeItems::default()),
    };
    let bridge_modules = package_under_test_bridge_modules(command_label, member, report_failure)?;
    if bridge_modules.is_empty() {
        return Ok(RenderedDependencyBridgeItems::default());
    }

    let package_name = member.package_name.as_str();
    let owner_manifest = load_project_manifest(manifest_path).map_err(|error| {
        if report_failure {
            report_project_build_dependency_error(command_label, Some(manifest_path), &error);
        }
        1
    })?;
    let specialization_modules =
        dependency_generic_specialization_modules(command_label, &owner_manifest, report_failure)?;
    let specialization_modules =
        dependency_generic_specialization_module_refs(&specialization_modules);
    let module_import_paths = bridge_modules
        .iter()
        .map(|module| dependency_interface_module_import_path(package_name, &module.module))
        .collect::<BTreeSet<_>>();
    let imported_externs =
        collect_imported_dependency_externs(&root_source_module, &module_import_paths);
    let occupied_root_names = collect_top_level_definition_names(&root_source_module);

    let mut forwarders = Vec::new();
    let mut source_rewrites = Vec::new();
    let mut required_types_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
    let mut function_owners = BTreeMap::<String, DependencyExternOwner>::new();
    let mut rendered_specializations = BTreeSet::new();
    for module in &bridge_modules {
        collect_dependency_module_public_function_forwarders(
            package_name,
            manifest_path,
            &module.module,
            &module.source,
            &root_source_module,
            Some(&imported_externs),
            None,
            &occupied_root_names,
            &specialization_modules,
            &mut required_types_by_module_path,
            &mut function_owners,
            &mut forwarders,
            &mut source_rewrites,
            &mut rendered_specializations,
        )
        .map_err(|error| {
            if !report_failure {
                return 1;
            }
            match error {
                DependencyPublicFunctionForwarderError::DependencyConflict { symbol, owner } => {
                    eprintln!(
                        "error: {command_label} found conflicting package-under-test public function imports for `{symbol}`"
                    );
                    eprintln!("note: first package: `{}`", owner.package_name);
                    eprintln!("note: package under test: `{package_name}`");
                }
                DependencyPublicFunctionForwarderError::LocalConflict { symbol } => {
                    eprintln!(
                        "error: {command_label} cannot synthesize package-under-test public function bridge for `{symbol}` because the test source already defines the same top-level name"
                    );
                    eprintln!(
                        "hint: rename the local top-level item or avoid importing the package-under-test public function with the same original symbol name"
                    );
                }
                DependencyPublicFunctionForwarderError::UnsupportedGeneric { symbol } => {
                    eprintln!(
                        "error: {command_label} cannot synthesize package-under-test public function bridge for generic function `{symbol}` yet"
                    );
                    eprintln!(
                        "hint: generic function monomorphization is not implemented yet; use a non-generic wrapper with concrete parameter and return types"
                    );
                }
            }
            1
        })?;
    }

    let mut type_declarations = Vec::new();
    let mut type_owners = BTreeMap::<String, DependencyExternOwner>::new();
    for module in &bridge_modules {
        let module_import_path =
            dependency_interface_module_import_path(package_name, &module.module);
        collect_dependency_module_public_type_declarations(
            package_name,
            manifest_path,
            &module.module,
            &module.source,
            Some(&imported_externs),
            required_types_by_module_path.get(&module_import_path),
            &occupied_root_names,
            &mut type_owners,
            &mut type_declarations,
        )
        .map_err(|error| {
            if report_failure {
                match error {
                    DependencyPublicTypeBridgeError::DependencyConflict { symbol, owner } => {
                        eprintln!(
                            "error: {command_label} found conflicting package-under-test public type imports for `{symbol}`"
                        );
                        eprintln!("note: first package: `{}`", owner.package_name);
                        eprintln!("note: package under test: `{package_name}`");
                    }
                    DependencyPublicTypeBridgeError::LocalConflict { symbol } => {
                        eprintln!(
                            "error: {command_label} cannot synthesize package-under-test public type bridge for `{symbol}` because the test source already defines the same top-level name"
                        );
                        eprintln!(
                            "hint: rename the local top-level item or avoid importing a package-under-test public type with the same original symbol name"
                        );
                    }
                }
            }
            1
        })?;
    }

    let declarations =
        join_dependency_bridge_sections(&type_declarations.join("\n\n"), &forwarders.join("\n\n"));
    Ok(RenderedDependencyBridgeItems {
        declarations,
        source_rewrites,
    })
}

fn render_direct_dependency_extern_declarations(
    command_label: &str,
    manifest_path: &Path,
    source: &str,
    report_failure: bool,
) -> Result<String, u8> {
    let manifest = load_project_manifest(manifest_path).map_err(|error| {
        if report_failure {
            report_project_build_dependency_error(command_label, None, &error);
        }
        1
    })?;
    let direct_dependencies = load_reference_manifests(&manifest).map_err(|error| {
        if report_failure {
            report_project_build_dependency_error(command_label, Some(manifest_path), &error);
        }
        1
    })?;
    let root_source_module = match parse_source(source) {
        Ok(module) => module,
        Err(_) => return Ok(String::new()),
    };

    let mut declarations = Vec::new();
    let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();

    for dependency in direct_dependencies {
        let dependency_package = match package_name(&dependency) {
            Ok(name) => name.to_owned(),
            Err(error) => {
                if report_failure {
                    report_project_build_dependency_error(
                        command_label,
                        Some(manifest_path),
                        &error,
                    );
                }
                return Err(1);
            }
        };
        let interface_path = default_interface_path(&dependency).map_err(|error| {
            if report_failure {
                report_project_build_dependency_error(command_label, Some(manifest_path), &error);
            }
            1
        })?;
        let artifact = load_interface_artifact(&interface_path).map_err(|error| {
            if report_failure {
                eprintln!(
                    "error: {command_label} failed to load referenced package interface `{}`: {error}",
                    normalize_path(&interface_path)
                );
                eprintln!(
                    "note: while preparing dependency extern declarations for `{}`",
                    normalize_path(manifest_path)
                );
            }
            1
        })?;
        let module_import_paths =
            dependency_interface_module_import_paths(&dependency_package, &artifact.modules);
        let imported_externs =
            collect_imported_dependency_externs(&root_source_module, &module_import_paths);

        for module in &artifact.modules {
            collect_dependency_module_extern_declarations(
                &dependency_package,
                &dependency.manifest_path,
                module.source_path.as_str(),
                &module.syntax,
                &module.contents,
                Some(&imported_externs),
                &mut owners_by_symbol,
                &mut declarations,
            )
            .map_err(|(symbol, owner)| {
                if report_failure {
                    eprintln!(
                        "error: {command_label} found conflicting direct dependency extern imports for `{symbol}`"
                    );
                    eprintln!("note: first package: `{}`", owner.package_name);
                    eprintln!("note: conflicting package: `{dependency_package}`");
                    eprintln!(
                        "hint: keep direct dependency `extern \"c\"` names unique until package-qualified extern resolution lands"
                    );
                }
                1
            })?;
        }
    }

    Ok(declarations.join("\n\n"))
}

fn render_direct_dependency_extern_declarations_quiet(
    manifest_path: &Path,
    source: &str,
) -> Result<String, PrepareProjectTargetBuildError> {
    let manifest = load_project_manifest(manifest_path)
        .map_err(|error| target_prep_dependency_manifest_failure(None, &error))?;
    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let root_source_module = match parse_source(source) {
        Ok(module) => module,
        Err(_) => return Ok(String::new()),
    };
    let direct_dependencies = manifest
        .references
        .packages
        .iter()
        .map(|reference| {
            let reference_manifest_path = reference_manifest_path(&manifest, reference);
            let dependency_manifest = load_project_manifest(&manifest_dir.join(reference))
                .map_err(|error| {
                    target_prep_dependency_manifest_failure(Some(&reference_manifest_path), &error)
                })?;
            Ok::<_, PrepareProjectTargetBuildError>(dependency_manifest)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut declarations = Vec::new();
    let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();

    for dependency_manifest in direct_dependencies {
        let dependency_package = package_name(&dependency_manifest)
            .map(str::to_owned)
            .map_err(|error| {
                target_prep_dependency_manifest_failure(
                    Some(&dependency_manifest.manifest_path),
                    &error,
                )
            })?;
        let interface_path = default_interface_path(&dependency_manifest).map_err(|error| {
            target_prep_dependency_manifest_failure(
                Some(&dependency_manifest.manifest_path),
                &error,
            )
        })?;
        let artifact = load_interface_artifact(&interface_path).map_err(|error| {
            PrepareProjectTargetBuildError {
                failure_kind: PrepareProjectTargetBuildFailureKind::DependencyInterface {
                    dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                    dependency_package: dependency_package.clone(),
                    interface_path: interface_path.clone(),
                    message: format!(
                        "failed to load referenced package interface `{}`: {error}",
                        normalize_path(&interface_path)
                    ),
                },
            }
        })?;
        let module_import_paths =
            dependency_interface_module_import_paths(&dependency_package, &artifact.modules);
        let imported_externs =
            collect_imported_dependency_externs(&root_source_module, &module_import_paths);

        for module in &artifact.modules {
            collect_dependency_module_extern_declarations(
                &dependency_package,
                &dependency_manifest.manifest_path,
                module.source_path.as_str(),
                &module.syntax,
                &module.contents,
                Some(&imported_externs),
                &mut owners_by_symbol,
                &mut declarations,
            )
            .map_err(|(symbol, owner)| PrepareProjectTargetBuildError {
                failure_kind: PrepareProjectTargetBuildFailureKind::DependencyExternConflict {
                    symbol,
                    first_package: owner.package_name,
                    first_manifest_path: owner.manifest_path,
                    conflicting_package: dependency_package.clone(),
                    conflicting_manifest_path: dependency_manifest.manifest_path.clone(),
                },
            })?;
        }
    }

    Ok(declarations.join("\n\n"))
}

#[derive(Default)]
struct RenderedDependencyPublicValueDeclarations {
    declarations: String,
    required_functions_by_module_path: BTreeMap<Vec<String>, BTreeSet<String>>,
    required_types_by_module_path: BTreeMap<Vec<String>, BTreeSet<String>>,
}

#[derive(Default)]
struct RenderedDependencyPublicFunctionForwarders {
    forwarders: String,
    source_rewrites: Vec<dependency_generic_bridge::SourceRewrite>,
    required_types_by_module_path: BTreeMap<Vec<String>, BTreeSet<String>>,
}

#[derive(Default)]
struct RenderedDependencyPublicMethodForwarders {
    forwarders: String,
    required_types_by_module_path: BTreeMap<Vec<String>, BTreeSet<String>>,
}

fn render_direct_dependency_public_value_declarations(
    command_label: &str,
    manifest_path: &Path,
    source: &str,
    report_failure: bool,
) -> Result<RenderedDependencyPublicValueDeclarations, u8> {
    let manifest = load_project_manifest(manifest_path).map_err(|error| {
        if report_failure {
            report_project_build_dependency_error(command_label, None, &error);
        }
        1
    })?;
    let direct_dependencies = load_reference_manifests(&manifest).map_err(|error| {
        if report_failure {
            report_project_build_dependency_error(command_label, Some(manifest_path), &error);
        }
        1
    })?;
    let root_source_module = match parse_source(source) {
        Ok(module) => module,
        Err(_) => return Ok(RenderedDependencyPublicValueDeclarations::default()),
    };
    let occupied_root_names = collect_top_level_definition_names(&root_source_module);

    let mut declarations = Vec::new();
    let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();
    let mut required_functions_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
    let mut required_types_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();

    for dependency in direct_dependencies {
        let dependency_package = match package_name(&dependency) {
            Ok(name) => name.to_owned(),
            Err(error) => {
                if report_failure {
                    report_project_build_dependency_error(
                        command_label,
                        Some(manifest_path),
                        &error,
                    );
                }
                return Err(1);
            }
        };
        let interface_path = default_interface_path(&dependency).map_err(|error| {
            if report_failure {
                report_project_build_dependency_error(command_label, Some(manifest_path), &error);
            }
            1
        })?;
        let artifact = load_interface_artifact(&interface_path).map_err(|error| {
            if report_failure {
                eprintln!(
                    "error: {command_label} failed to load referenced package interface `{}`: {error}",
                    normalize_path(&interface_path)
                );
                eprintln!(
                    "note: while preparing dependency public value bridges for `{}`",
                    normalize_path(manifest_path)
                );
            }
            1
        })?;
        let module_import_paths =
            dependency_interface_module_import_paths(&dependency_package, &artifact.modules);
        let imported_externs =
            collect_imported_dependency_externs(&root_source_module, &module_import_paths);

        for module in &artifact.modules {
            let dependency_source_path =
                dependency_module_source_path(&dependency.manifest_path, &module.source_path);
            let dependency_source = fs::read_to_string(&dependency_source_path).map_err(|error| {
                if report_failure {
                    eprintln!(
                        "error: {command_label} failed to access dependency source `{}`: {error}",
                        normalize_path(&dependency_source_path)
                    );
                    eprintln!(
                        "note: while preparing dependency public value bridges for `{}`",
                        normalize_path(manifest_path)
                    );
                }
                1
            })?;
            let source_module = match parse_source(&dependency_source) {
                Ok(module) => module,
                Err(_) => {
                    if report_failure {
                        eprintln!(
                            "error: {command_label} failed to parse dependency source `{}` while preparing public value bridges",
                            normalize_path(&dependency_source_path)
                        );
                        eprintln!("note: dependency package: `{dependency_package}`");
                    }
                    return Err(1);
                }
            };
            collect_dependency_module_public_value_declarations(
                &dependency_package,
                &dependency.manifest_path,
                &source_module,
                &dependency_source,
                Some(&imported_externs),
                &occupied_root_names,
                &mut required_functions_by_module_path,
                &mut required_types_by_module_path,
                &mut owners_by_symbol,
                &mut declarations,
            )
            .map_err(|error| {
                if report_failure {
                    match error {
                        DependencyPublicValueBridgeError::DependencyConflict { symbol, owner } => {
                            eprintln!(
                                "error: {command_label} found conflicting direct dependency public value imports for `{symbol}`"
                            );
                            eprintln!("note: first package: `{}`", owner.package_name);
                            eprintln!("note: conflicting package: `{dependency_package}`");
                            eprintln!(
                                "hint: keep direct dependency public value names unique until package-qualified dependency value lowering lands"
                            );
                        }
                        DependencyPublicValueBridgeError::LocalConflict { symbol } => {
                            eprintln!(
                                "error: {command_label} cannot synthesize direct dependency public value bridge for `{symbol}` because the root source already defines the same top-level name"
                            );
                            eprintln!("note: conflicting direct dependency package: `{dependency_package}`");
                            eprintln!(
                                "hint: rename the local top-level item or avoid importing a direct dependency public value with the same original symbol name"
                            );
                        }
                    }
                }
                1
            })?;
        }
    }

    Ok(RenderedDependencyPublicValueDeclarations {
        declarations: declarations.join("\n\n"),
        required_functions_by_module_path,
        required_types_by_module_path,
    })
}

fn render_direct_dependency_public_value_declarations_quiet(
    manifest_path: &Path,
    source: &str,
) -> Result<RenderedDependencyPublicValueDeclarations, PrepareProjectTargetBuildError> {
    let manifest = load_project_manifest(manifest_path)
        .map_err(|error| target_prep_dependency_manifest_failure(None, &error))?;
    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let root_source_module = match parse_source(source) {
        Ok(module) => module,
        Err(_) => return Ok(RenderedDependencyPublicValueDeclarations::default()),
    };
    let occupied_root_names = collect_top_level_definition_names(&root_source_module);
    let direct_dependencies = manifest
        .references
        .packages
        .iter()
        .map(|reference| {
            let reference_manifest_path = reference_manifest_path(&manifest, reference);
            let dependency_manifest = load_project_manifest(&manifest_dir.join(reference))
                .map_err(|error| {
                    target_prep_dependency_manifest_failure(Some(&reference_manifest_path), &error)
                })?;
            Ok::<_, PrepareProjectTargetBuildError>(dependency_manifest)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut declarations = Vec::new();
    let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();
    let mut required_functions_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
    let mut required_types_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();

    for dependency_manifest in direct_dependencies {
        let dependency_package = package_name(&dependency_manifest)
            .map(str::to_owned)
            .map_err(|error| {
                target_prep_dependency_manifest_failure(
                    Some(&dependency_manifest.manifest_path),
                    &error,
                )
            })?;
        let interface_path = default_interface_path(&dependency_manifest).map_err(|error| {
            target_prep_dependency_manifest_failure(
                Some(&dependency_manifest.manifest_path),
                &error,
            )
        })?;
        let artifact = load_interface_artifact(&interface_path).map_err(|error| {
            PrepareProjectTargetBuildError {
                failure_kind: PrepareProjectTargetBuildFailureKind::DependencyInterface {
                    dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                    dependency_package: dependency_package.clone(),
                    interface_path: interface_path.clone(),
                    message: format!(
                        "failed to load referenced package interface `{}`: {error}",
                        normalize_path(&interface_path)
                    ),
                },
            }
        })?;
        let module_import_paths =
            dependency_interface_module_import_paths(&dependency_package, &artifact.modules);
        let imported_externs =
            collect_imported_dependency_externs(&root_source_module, &module_import_paths);

        for module in &artifact.modules {
            let dependency_source_path = dependency_module_source_path(
                &dependency_manifest.manifest_path,
                &module.source_path,
            );
            let dependency_source =
                fs::read_to_string(&dependency_source_path).map_err(|error| {
                    PrepareProjectTargetBuildError {
                        failure_kind: PrepareProjectTargetBuildFailureKind::DependencySource {
                            dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                            dependency_package: dependency_package.clone(),
                            source_path: dependency_source_path.clone(),
                            message: format!(
                                "failed to access dependency source `{}`: {error}",
                                normalize_path(&dependency_source_path)
                            ),
                        },
                    }
                })?;
            let source_module = parse_source(&dependency_source).map_err(|_| {
                PrepareProjectTargetBuildError {
                    failure_kind: PrepareProjectTargetBuildFailureKind::DependencySource {
                        dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                        dependency_package: dependency_package.clone(),
                        source_path: dependency_source_path.clone(),
                        message: format!(
                            "failed to parse dependency source `{}` while preparing public value bridges",
                            normalize_path(&dependency_source_path)
                        ),
                    },
                }
            })?;
            collect_dependency_module_public_value_declarations(
                &dependency_package,
                &dependency_manifest.manifest_path,
                &source_module,
                &dependency_source,
                Some(&imported_externs),
                &occupied_root_names,
                &mut required_functions_by_module_path,
                &mut required_types_by_module_path,
                &mut owners_by_symbol,
                &mut declarations,
            )
            .map_err(|error| match error {
                DependencyPublicValueBridgeError::DependencyConflict { symbol, owner } => {
                    PrepareProjectTargetBuildError {
                        failure_kind:
                            PrepareProjectTargetBuildFailureKind::DependencyValueConflict {
                                symbol,
                                first_package: owner.package_name,
                                first_manifest_path: owner.manifest_path,
                                conflicting_package: dependency_package.clone(),
                                conflicting_manifest_path: dependency_manifest
                                    .manifest_path
                                    .clone(),
                            },
                    }
                }
                DependencyPublicValueBridgeError::LocalConflict { symbol } => {
                    PrepareProjectTargetBuildError {
                        failure_kind:
                            PrepareProjectTargetBuildFailureKind::DependencyValueLocalConflict {
                                symbol,
                                dependency_package: dependency_package.clone(),
                                dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                            },
                    }
                }
            })?;
        }
    }

    Ok(RenderedDependencyPublicValueDeclarations {
        declarations: declarations.join("\n\n"),
        required_functions_by_module_path,
        required_types_by_module_path,
    })
}

fn render_direct_dependency_public_function_forwarders(
    command_label: &str,
    manifest_path: &Path,
    source: &str,
    required_functions_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
    report_failure: bool,
) -> Result<RenderedDependencyPublicFunctionForwarders, u8> {
    let manifest = load_project_manifest(manifest_path).map_err(|error| {
        if report_failure {
            report_project_build_dependency_error(command_label, None, &error);
        }
        1
    })?;
    let direct_dependencies = load_reference_manifests(&manifest).map_err(|error| {
        if report_failure {
            report_project_build_dependency_error(command_label, Some(manifest_path), &error);
        }
        1
    })?;
    let root_source_module = match parse_source(source) {
        Ok(module) => module,
        Err(_) => return Ok(RenderedDependencyPublicFunctionForwarders::default()),
    };
    let occupied_root_names = collect_top_level_definition_names(&root_source_module);

    let mut forwarders = Vec::new();
    let mut source_rewrites = Vec::new();
    let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();
    let mut required_types_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
    let mut rendered_specializations = BTreeSet::new();

    for dependency in direct_dependencies {
        let dependency_package = match package_name(&dependency) {
            Ok(name) => name.to_owned(),
            Err(error) => {
                if report_failure {
                    report_project_build_dependency_error(
                        command_label,
                        Some(manifest_path),
                        &error,
                    );
                }
                return Err(1);
            }
        };
        let interface_path = default_interface_path(&dependency).map_err(|error| {
            if report_failure {
                report_project_build_dependency_error(command_label, Some(manifest_path), &error);
            }
            1
        })?;
        let artifact = load_interface_artifact(&interface_path).map_err(|error| {
            if report_failure {
                eprintln!(
                    "error: {command_label} failed to load referenced package interface `{}`: {error}",
                    normalize_path(&interface_path)
                );
                eprintln!(
                    "note: while preparing dependency public function wrappers for `{}`",
                    normalize_path(manifest_path)
                );
            }
            1
        })?;
        let specialization_modules =
            dependency_generic_specialization_modules(command_label, &dependency, report_failure)?;
        let specialization_modules =
            dependency_generic_specialization_module_refs(&specialization_modules);
        collect_dependency_public_function_forwarders_from_modules(
            &dependency_package,
            &dependency.manifest_path,
            &artifact.modules,
            &root_source_module,
            required_functions_by_module_path,
            &occupied_root_names,
            &specialization_modules,
            &mut required_types_by_module_path,
            &mut owners_by_symbol,
            &mut forwarders,
            &mut source_rewrites,
            &mut rendered_specializations,
            |dependency_source_path| {
                fs::read_to_string(dependency_source_path).map_err(|error| {
                    if report_failure {
                        eprintln!(
                            "error: {command_label} failed to access dependency source `{}`: {error}",
                            normalize_path(dependency_source_path)
                        );
                        eprintln!(
                            "note: while preparing dependency public function wrappers for `{}`",
                            normalize_path(manifest_path)
                        );
                    }
                    1
                })
            },
            |dependency_source_path, dependency_source| {
                parse_source(dependency_source).map_err(|_| {
                    if report_failure {
                        eprintln!(
                            "error: {command_label} failed to parse dependency source `{}` while preparing public function wrappers",
                            normalize_path(dependency_source_path)
                        );
                        eprintln!("note: dependency package: `{dependency_package}`");
                    }
                    1
                })
            },
            |error| {
                if report_failure {
                    match error {
                        DependencyPublicFunctionForwarderError::DependencyConflict {
                            symbol,
                            owner,
                        } => {
                            eprintln!(
                                "error: {command_label} found conflicting direct dependency public function imports for `{symbol}`"
                            );
                            eprintln!("note: first package: `{}`", owner.package_name);
                            eprintln!("note: conflicting package: `{dependency_package}`");
                            eprintln!(
                                "hint: keep direct dependency public function names unique until package-qualified dependency call lowering lands"
                            );
                        }
                        DependencyPublicFunctionForwarderError::LocalConflict { symbol } => {
                            eprintln!(
                                "error: {command_label} cannot synthesize direct dependency public function bridge for `{symbol}` because the root source already defines the same top-level name"
                            );
                            eprintln!(
                                "note: conflicting direct dependency package: `{dependency_package}`"
                            );
                            eprintln!(
                                "hint: rename the local top-level item or avoid importing a direct dependency public function with the same original symbol name"
                            );
                        }
                        DependencyPublicFunctionForwarderError::UnsupportedGeneric { symbol } => {
                            eprintln!(
                                "error: {command_label} cannot synthesize direct dependency public function bridge for generic function `{symbol}` yet"
                            );
                            eprintln!("note: direct dependency package: `{dependency_package}`");
                            eprintln!(
                                "hint: generic function monomorphization is not implemented yet; use a non-generic wrapper with concrete parameter and return types"
                            );
                        }
                    }
                }
                1
            },
        )?;
    }

    Ok(RenderedDependencyPublicFunctionForwarders {
        forwarders: forwarders.join("\n\n"),
        source_rewrites,
        required_types_by_module_path,
    })
}

fn render_direct_dependency_public_function_forwarders_quiet(
    manifest_path: &Path,
    source: &str,
    required_functions_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
) -> Result<RenderedDependencyPublicFunctionForwarders, PrepareProjectTargetBuildError> {
    let manifest = load_project_manifest(manifest_path)
        .map_err(|error| target_prep_dependency_manifest_failure(None, &error))?;
    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let root_source_module = match parse_source(source) {
        Ok(module) => module,
        Err(_) => return Ok(RenderedDependencyPublicFunctionForwarders::default()),
    };
    let occupied_root_names = collect_top_level_definition_names(&root_source_module);
    let direct_dependencies = manifest
        .references
        .packages
        .iter()
        .map(|reference| {
            let reference_manifest_path = reference_manifest_path(&manifest, reference);
            let dependency_manifest = load_project_manifest(&manifest_dir.join(reference))
                .map_err(|error| {
                    target_prep_dependency_manifest_failure(Some(&reference_manifest_path), &error)
                })?;
            Ok::<_, PrepareProjectTargetBuildError>(dependency_manifest)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut forwarders = Vec::new();
    let mut source_rewrites = Vec::new();
    let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();
    let mut required_types_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
    let mut rendered_specializations = BTreeSet::new();

    for dependency_manifest in direct_dependencies {
        let dependency_package = package_name(&dependency_manifest)
            .map(str::to_owned)
            .map_err(|error| {
                target_prep_dependency_manifest_failure(
                    Some(&dependency_manifest.manifest_path),
                    &error,
                )
            })?;
        let interface_path = default_interface_path(&dependency_manifest).map_err(|error| {
            target_prep_dependency_manifest_failure(
                Some(&dependency_manifest.manifest_path),
                &error,
            )
        })?;
        let artifact = load_interface_artifact(&interface_path).map_err(|error| {
            PrepareProjectTargetBuildError {
                failure_kind: PrepareProjectTargetBuildFailureKind::DependencyInterface {
                    dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                    dependency_package: dependency_package.clone(),
                    interface_path: interface_path.clone(),
                    message: format!(
                        "failed to load referenced package interface `{}`: {error}",
                        normalize_path(&interface_path)
                    ),
                },
            }
        })?;
        let specialization_modules =
            dependency_generic_specialization_modules_quiet(&dependency_manifest)?;
        let specialization_modules =
            dependency_generic_specialization_module_refs(&specialization_modules);
        collect_dependency_public_function_forwarders_from_modules(
            &dependency_package,
            &dependency_manifest.manifest_path,
            &artifact.modules,
            &root_source_module,
            required_functions_by_module_path,
            &occupied_root_names,
            &specialization_modules,
            &mut required_types_by_module_path,
            &mut owners_by_symbol,
            &mut forwarders,
            &mut source_rewrites,
            &mut rendered_specializations,
            |dependency_source_path| {
                fs::read_to_string(dependency_source_path).map_err(|error| {
                    PrepareProjectTargetBuildError {
                        failure_kind: PrepareProjectTargetBuildFailureKind::DependencySource {
                            dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                            dependency_package: dependency_package.clone(),
                            source_path: dependency_source_path.to_path_buf(),
                            message: format!(
                                "failed to access dependency source `{}`: {error}",
                                normalize_path(dependency_source_path)
                            ),
                        },
                    }
                })
            },
            |dependency_source_path, dependency_source| {
                parse_source(dependency_source).map_err(|_| PrepareProjectTargetBuildError {
                    failure_kind: PrepareProjectTargetBuildFailureKind::DependencySource {
                        dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                        dependency_package: dependency_package.clone(),
                        source_path: dependency_source_path.to_path_buf(),
                        message: format!(
                            "failed to parse dependency source `{}` while preparing public function wrappers",
                            normalize_path(dependency_source_path)
                        ),
                    },
                })
            },
            |error| {
                dependency_function_forwarder_target_prep_error(
                    error,
                    &dependency_package,
                    &dependency_manifest.manifest_path,
                )
            },
        )?;
    }

    Ok(RenderedDependencyPublicFunctionForwarders {
        forwarders: forwarders.join("\n\n"),
        source_rewrites,
        required_types_by_module_path,
    })
}

fn collect_dependency_public_function_forwarders_from_modules<E>(
    dependency_package: &str,
    dependency_manifest_path: &Path,
    modules: &[ql_project::InterfaceModule],
    root_source_module: &Module,
    required_functions_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
    occupied_root_names: &BTreeSet<String>,
    specialization_modules: &[dependency_generic_bridge::SpecializationModule<'_>],
    required_types_by_module_path: &mut BTreeMap<Vec<String>, BTreeSet<String>>,
    owners_by_symbol: &mut BTreeMap<String, DependencyExternOwner>,
    forwarders: &mut Vec<String>,
    source_rewrites: &mut Vec<dependency_generic_bridge::SourceRewrite>,
    rendered_specializations: &mut BTreeSet<String>,
    mut read_source: impl FnMut(&Path) -> Result<String, E>,
    mut parse_source_module: impl FnMut(&Path, &str) -> Result<Module, E>,
    mut map_bridge_error: impl FnMut(DependencyPublicFunctionForwarderError) -> E,
) -> Result<(), E> {
    let module_import_paths = dependency_interface_module_import_paths(dependency_package, modules);
    let imported_externs =
        collect_imported_dependency_externs(root_source_module, &module_import_paths);

    for module in modules {
        let dependency_source_path =
            dependency_module_source_path(dependency_manifest_path, &module.source_path);
        let dependency_source = read_source(&dependency_source_path)?;
        let source_module = parse_source_module(&dependency_source_path, &dependency_source)?;
        let module_import_path =
            dependency_interface_module_import_path(dependency_package, &source_module);
        collect_dependency_module_public_function_forwarders(
            dependency_package,
            dependency_manifest_path,
            &source_module,
            &dependency_source,
            root_source_module,
            Some(&imported_externs),
            required_functions_by_module_path.get(&module_import_path),
            occupied_root_names,
            specialization_modules,
            required_types_by_module_path,
            owners_by_symbol,
            forwarders,
            source_rewrites,
            rendered_specializations,
        )
        .map_err(&mut map_bridge_error)?;
    }

    Ok(())
}

fn render_direct_dependency_public_method_forwarders(
    command_label: &str,
    manifest_path: &Path,
    required_types_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
    report_failure: bool,
) -> Result<RenderedDependencyPublicMethodForwarders, u8> {
    let manifest = load_project_manifest(manifest_path).map_err(|error| {
        if report_failure {
            report_project_build_dependency_error(command_label, None, &error);
        }
        1
    })?;
    let direct_dependencies = load_reference_manifests(&manifest).map_err(|error| {
        if report_failure {
            report_project_build_dependency_error(command_label, Some(manifest_path), &error);
        }
        1
    })?;

    let mut forwarders = Vec::new();
    let mut discovered_required_types = BTreeMap::<Vec<String>, BTreeSet<String>>::new();

    for dependency in direct_dependencies {
        let dependency_package = match package_name(&dependency) {
            Ok(name) => name.to_owned(),
            Err(error) => {
                if report_failure {
                    report_project_build_dependency_error(
                        command_label,
                        Some(manifest_path),
                        &error,
                    );
                }
                return Err(1);
            }
        };
        let interface_path = default_interface_path(&dependency).map_err(|error| {
            if report_failure {
                report_project_build_dependency_error(command_label, Some(manifest_path), &error);
            }
            1
        })?;
        let artifact = load_interface_artifact(&interface_path).map_err(|error| {
            if report_failure {
                eprintln!(
                    "error: {command_label} failed to load referenced package interface `{}`: {error}",
                    normalize_path(&interface_path)
                );
                eprintln!(
                    "note: while preparing dependency public method bridges for `{}`",
                    normalize_path(manifest_path)
                );
            }
            1
        })?;

        collect_dependency_public_method_forwarders_from_modules(
            &dependency_package,
            &dependency.manifest_path,
            &artifact.modules,
            required_types_by_module_path,
            &mut discovered_required_types,
            &mut forwarders,
            |dependency_source_path| {
                fs::read_to_string(dependency_source_path).map_err(|error| {
                    if report_failure {
                        eprintln!(
                            "error: {command_label} failed to access dependency source `{}`: {error}",
                            normalize_path(dependency_source_path)
                        );
                        eprintln!(
                            "note: while preparing dependency public method bridges for `{}`",
                            normalize_path(manifest_path)
                        );
                    }
                    1
                })
            },
            |dependency_source_path, dependency_source| {
                parse_source(dependency_source).map_err(|_| {
                    if report_failure {
                        eprintln!(
                            "error: {command_label} failed to parse dependency source `{}` while preparing public method bridges",
                            normalize_path(dependency_source_path)
                        );
                        eprintln!("note: dependency package: `{dependency_package}`");
                    }
                    1
                })
            },
        )?;
    }

    Ok(RenderedDependencyPublicMethodForwarders {
        forwarders: forwarders.join("\n\n"),
        required_types_by_module_path: discovered_required_types,
    })
}

fn render_direct_dependency_public_method_forwarders_quiet(
    manifest_path: &Path,
    required_types_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
) -> Result<RenderedDependencyPublicMethodForwarders, PrepareProjectTargetBuildError> {
    let manifest = load_project_manifest(manifest_path)
        .map_err(|error| target_prep_dependency_manifest_failure(None, &error))?;
    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let direct_dependencies = manifest
        .references
        .packages
        .iter()
        .map(|reference| {
            let reference_manifest_path = reference_manifest_path(&manifest, reference);
            let dependency_manifest = load_project_manifest(&manifest_dir.join(reference))
                .map_err(|error| {
                    target_prep_dependency_manifest_failure(Some(&reference_manifest_path), &error)
                })?;
            Ok::<_, PrepareProjectTargetBuildError>(dependency_manifest)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut forwarders = Vec::new();
    let mut discovered_required_types = BTreeMap::<Vec<String>, BTreeSet<String>>::new();

    for dependency_manifest in direct_dependencies {
        let dependency_package = package_name(&dependency_manifest)
            .map(str::to_owned)
            .map_err(|error| {
                target_prep_dependency_manifest_failure(
                    Some(&dependency_manifest.manifest_path),
                    &error,
                )
            })?;
        let interface_path = default_interface_path(&dependency_manifest).map_err(|error| {
            target_prep_dependency_manifest_failure(
                Some(&dependency_manifest.manifest_path),
                &error,
            )
        })?;
        let artifact = load_interface_artifact(&interface_path).map_err(|error| {
            PrepareProjectTargetBuildError {
                failure_kind: PrepareProjectTargetBuildFailureKind::DependencyInterface {
                    dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                    dependency_package: dependency_package.clone(),
                    interface_path: interface_path.clone(),
                    message: format!(
                        "failed to load referenced package interface `{}`: {error}",
                        normalize_path(&interface_path)
                    ),
                },
            }
        })?;

        collect_dependency_public_method_forwarders_from_modules(
            &dependency_package,
            &dependency_manifest.manifest_path,
            &artifact.modules,
            required_types_by_module_path,
            &mut discovered_required_types,
            &mut forwarders,
            |dependency_source_path| {
                fs::read_to_string(dependency_source_path).map_err(|error| {
                    PrepareProjectTargetBuildError {
                        failure_kind: PrepareProjectTargetBuildFailureKind::DependencySource {
                            dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                            dependency_package: dependency_package.clone(),
                            source_path: dependency_source_path.to_path_buf(),
                            message: format!(
                                "failed to access dependency source `{}`: {error}",
                                normalize_path(dependency_source_path)
                            ),
                        },
                    }
                })
            },
            |dependency_source_path, dependency_source| {
                parse_source(dependency_source).map_err(|_| PrepareProjectTargetBuildError {
                    failure_kind: PrepareProjectTargetBuildFailureKind::DependencySource {
                        dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                        dependency_package: dependency_package.clone(),
                        source_path: dependency_source_path.to_path_buf(),
                        message: format!(
                            "failed to parse dependency source `{}` while preparing public method bridges",
                            normalize_path(dependency_source_path)
                        ),
                    },
                })
            },
        )?;
    }

    Ok(RenderedDependencyPublicMethodForwarders {
        forwarders: forwarders.join("\n\n"),
        required_types_by_module_path: discovered_required_types,
    })
}

fn collect_dependency_public_method_forwarders_from_modules<E>(
    dependency_package: &str,
    dependency_manifest_path: &Path,
    modules: &[ql_project::InterfaceModule],
    required_types_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
    discovered_required_types: &mut BTreeMap<Vec<String>, BTreeSet<String>>,
    forwarders: &mut Vec<String>,
    mut read_source: impl FnMut(&Path) -> Result<String, E>,
    mut parse_source_module: impl FnMut(&Path, &str) -> Result<Module, E>,
) -> Result<(), E> {
    for module in modules {
        let dependency_source_path =
            dependency_module_source_path(dependency_manifest_path, &module.source_path);
        let dependency_source = read_source(&dependency_source_path)?;
        let source_module = parse_source_module(&dependency_source_path, &dependency_source)?;
        collect_dependency_module_public_method_forwarders(
            dependency_package,
            &source_module,
            &dependency_source,
            required_types_by_module_path,
            discovered_required_types,
            forwarders,
        );
    }

    Ok(())
}

fn collect_dependency_module_public_method_forwarders(
    dependency_package: &str,
    module: &Module,
    contents: &str,
    required_types_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
    discovered_required_types: &mut BTreeMap<Vec<String>, BTreeSet<String>>,
    forwarders: &mut Vec<String>,
) {
    let module_import_path = dependency_interface_module_import_path(dependency_package, module);
    let Some(initial_required_types) = required_types_by_module_path.get(&module_import_path)
    else {
        return;
    };
    let type_candidates = dependency_public_type_bridge_candidates(module);
    let mut pending_types = initial_required_types.clone();
    let mut processed_types = BTreeSet::new();
    let mut emitted_methods = BTreeSet::<(String, String)>::new();

    while let Some(struct_name) = pending_types.iter().next().cloned() {
        pending_types.remove(&struct_name);
        if !processed_types.insert(struct_name.clone())
            || !type_candidates.contains_key(&struct_name)
        {
            continue;
        }
        if type_candidates
            .get(&struct_name)
            .is_some_and(|candidate| candidate.decl.is_generic())
        {
            continue;
        }

        for (method_name, method) in
            dependency_public_struct_method_bridge_candidates(module, &struct_name)
        {
            let type_dependencies =
                collect_dependency_public_function_type_dependencies(method, &type_candidates);
            if !type_dependencies.is_empty() {
                let required_types = discovered_required_types
                    .entry(module_import_path.clone())
                    .or_default();
                for dependency in type_dependencies {
                    if required_types.insert(dependency.clone()) {
                        pending_types.insert(dependency);
                    }
                }
            }

            if !emitted_methods.insert((struct_name.clone(), method_name)) {
                continue;
            }

            if let Some(forwarder) = render_imported_dependency_public_method_forwarder(
                &module_import_path,
                &struct_name,
                method,
                contents,
            ) {
                forwarders.push(forwarder);
            }
        }
    }
}

fn render_public_dependency_function_export_wrappers(
    command_label: &str,
    manifest_path: &Path,
    source: &str,
    report_failure: bool,
) -> Result<String, u8> {
    let manifest = load_project_manifest(manifest_path).map_err(|error| {
        if report_failure {
            eprintln!("error: {command_label} {error}");
        }
        1
    })?;
    let package_name = package_name(&manifest).map_err(|error| {
        if report_failure {
            eprintln!("error: {command_label} {error}");
        }
        1
    })?;
    Ok(render_public_dependency_function_export_wrappers_for_package(package_name, source))
}

fn render_public_dependency_function_export_wrappers_quiet(
    manifest_path: &Path,
    source: &str,
) -> Result<String, PrepareProjectTargetBuildError> {
    let manifest = load_project_manifest(manifest_path)
        .map_err(|error| target_prep_dependency_manifest_failure(None, &error))?;
    let package_name = package_name(&manifest)
        .map_err(|error| target_prep_dependency_manifest_failure(Some(manifest_path), &error))?;
    Ok(render_public_dependency_function_export_wrappers_for_package(package_name, source))
}

fn render_public_dependency_function_export_wrappers_for_package(
    package_name: &str,
    source: &str,
) -> String {
    let Ok(module) = parse_source(source) else {
        return String::new();
    };

    let mut wrappers = Vec::new();
    let module_import_path = dependency_interface_module_import_path(package_name, &module);
    let type_candidates = dependency_public_type_bridge_candidates(&module);
    for item in &module.items {
        let ItemKind::Function(function) = &item.kind else {
            continue;
        };
        if let Some(wrapper) =
            render_dependency_public_function_export_wrapper(&module_import_path, function, source)
        {
            wrappers.push(wrapper);
        }
    }
    for struct_name in type_candidates.keys() {
        if type_candidates
            .get(struct_name)
            .is_some_and(|candidate| candidate.decl.is_generic())
        {
            continue;
        }
        for method in
            dependency_public_struct_method_bridge_candidates(&module, struct_name).values()
        {
            if let Some(wrapper) = render_dependency_public_method_export_wrapper(
                &module_import_path,
                struct_name,
                method,
                source,
            ) {
                wrappers.push(wrapper);
            }
        }
    }

    wrappers.join("\n\n")
}

fn collect_dependency_module_extern_declarations(
    dependency_package: &str,
    dependency_manifest_path: &Path,
    _module_source_path: &str,
    module: &Module,
    contents: &str,
    imported_externs: Option<&ImportedDependencyExterns>,
    owners_by_symbol: &mut BTreeMap<String, DependencyExternOwner>,
    declarations: &mut Vec<String>,
) -> Result<(), (String, DependencyExternOwner)> {
    let module_import_path = dependency_interface_module_import_path(dependency_package, module);

    for item in &module.items {
        match &item.kind {
            ItemKind::Function(function)
                if function.visibility == Visibility::Public
                    && function.abi.as_deref() == Some("c")
                    && imported_externs.is_none_or(|imports| {
                        dependency_extern_is_imported(imports, &module_import_path, &function.name)
                    }) =>
            {
                record_dependency_extern_declaration(
                    dependency_package,
                    dependency_manifest_path,
                    &function.name,
                    span_text(contents, item.span),
                    owners_by_symbol,
                    declarations,
                )?;
            }
            ItemKind::ExternBlock(extern_block)
                if extern_block.visibility == Visibility::Public && extern_block.abi == "c" =>
            {
                for function in &extern_block.functions {
                    if imported_externs.is_some_and(|imports| {
                        !dependency_extern_is_imported(imports, &module_import_path, &function.name)
                    }) {
                        continue;
                    }
                    let mut declaration = String::from("extern \"c\" pub ");
                    declaration.push_str(span_text(contents, function.span).trim());
                    record_dependency_extern_declaration(
                        dependency_package,
                        dependency_manifest_path,
                        &function.name,
                        declaration,
                        owners_by_symbol,
                        declarations,
                    )?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct DependencyPublicGlobalBridgeCandidate<'a> {
    item: &'a ql_ast::Item,
    global: &'a GlobalDecl,
}

#[derive(Clone, Copy)]
struct DependencyPublicTypeBridgeCandidate<'a> {
    item: &'a ql_ast::Item,
    decl: DependencyPublicTypeDecl<'a>,
}

#[derive(Clone, Copy)]
enum DependencyPublicTypeDecl<'a> {
    Struct(&'a ql_ast::StructDecl),
    Enum(&'a ql_ast::EnumDecl),
    TypeAlias(&'a ql_ast::TypeAliasDecl),
}

impl<'a> DependencyPublicTypeDecl<'a> {
    fn name(self) -> &'a str {
        match self {
            Self::Struct(struct_decl) => struct_decl.name.as_str(),
            Self::Enum(enum_decl) => enum_decl.name.as_str(),
            Self::TypeAlias(alias) => alias.name.as_str(),
        }
    }

    fn is_generic(self) -> bool {
        match self {
            Self::Struct(struct_decl) => !struct_decl.generics.is_empty(),
            Self::Enum(enum_decl) => !enum_decl.generics.is_empty(),
            Self::TypeAlias(alias) => !alias.generics.is_empty(),
        }
    }
}

#[derive(Default)]
struct DependencyPublicGlobalExprDependencies {
    globals: BTreeSet<String>,
    functions: BTreeSet<String>,
}

fn dependency_public_global_bridge_candidates<'a>(
    module: &'a Module,
) -> BTreeMap<String, DependencyPublicGlobalBridgeCandidate<'a>> {
    let mut candidates = BTreeMap::new();
    for item in &module.items {
        let global = match &item.kind {
            ItemKind::Const(global) | ItemKind::Static(global)
                if global.visibility == Visibility::Public =>
            {
                global
            }
            _ => continue,
        };
        candidates.insert(
            global.name.clone(),
            DependencyPublicGlobalBridgeCandidate { item, global },
        );
    }
    candidates
}

fn dependency_public_type_bridge_candidates<'a>(
    module: &'a Module,
) -> BTreeMap<String, DependencyPublicTypeBridgeCandidate<'a>> {
    let mut candidates = BTreeMap::new();
    for item in &module.items {
        let decl = match &item.kind {
            ItemKind::Struct(struct_decl) if struct_decl.visibility == Visibility::Public => {
                DependencyPublicTypeDecl::Struct(struct_decl)
            }
            ItemKind::Enum(enum_decl) if enum_decl.visibility == Visibility::Public => {
                DependencyPublicTypeDecl::Enum(enum_decl)
            }
            ItemKind::TypeAlias(alias)
                if alias.visibility == Visibility::Public
                    && !alias.is_opaque
                    && alias.generics.is_empty() =>
            {
                DependencyPublicTypeDecl::TypeAlias(alias)
            }
            _ => continue,
        };
        candidates.insert(
            decl.name().to_owned(),
            DependencyPublicTypeBridgeCandidate { item, decl },
        );
    }
    candidates
}

fn dependency_public_function_bridge_candidates(module: &Module) -> BTreeSet<String> {
    module
        .items
        .iter()
        .filter_map(|item| match &item.kind {
            ItemKind::Function(function)
                if supports_dependency_public_function_export_bridge(function) =>
            {
                Some(function.name.clone())
            }
            _ => None,
        })
        .collect()
}

fn dependency_type_expr_targets_struct(ty: &ql_ast::TypeExpr, struct_name: &str) -> bool {
    let ql_ast::TypeExprKind::Named { path, .. } = &ty.kind else {
        return false;
    };
    path.segments
        .last()
        .is_some_and(|segment| segment == struct_name)
}

fn dependency_public_struct_method_bridge_candidates<'a>(
    module: &'a Module,
    struct_name: &str,
) -> BTreeMap<String, &'a FunctionDecl> {
    let mut impl_candidates = BTreeMap::<String, Vec<&'a FunctionDecl>>::new();
    let mut extend_candidates = BTreeMap::<String, Vec<&'a FunctionDecl>>::new();
    let mut trait_impl_candidates = BTreeMap::<String, Vec<&'a FunctionDecl>>::new();

    for item in &module.items {
        match &item.kind {
            ItemKind::Impl(impl_block)
                if impl_block.trait_ty.is_none()
                    && dependency_type_expr_targets_struct(&impl_block.target, struct_name) =>
            {
                for method in impl_block
                    .methods
                    .iter()
                    .filter(|method| supports_dependency_public_method_import_bridge(method))
                {
                    impl_candidates
                        .entry(method.name.clone())
                        .or_default()
                        .push(method);
                }
            }
            ItemKind::Impl(impl_block)
                if impl_block.trait_ty.is_some()
                    && dependency_type_expr_targets_struct(&impl_block.target, struct_name) =>
            {
                for method in impl_block
                    .methods
                    .iter()
                    .filter(|method| supports_dependency_public_method_import_bridge(method))
                {
                    trait_impl_candidates
                        .entry(method.name.clone())
                        .or_default()
                        .push(method);
                }
            }
            ItemKind::Extend(extend_block)
                if dependency_type_expr_targets_struct(&extend_block.target, struct_name) =>
            {
                for method in extend_block
                    .methods
                    .iter()
                    .filter(|method| supports_dependency_public_method_import_bridge(method))
                {
                    extend_candidates
                        .entry(method.name.clone())
                        .or_default()
                        .push(method);
                }
            }
            _ => {}
        }
    }

    let mut methods = BTreeMap::new();
    for (name, candidates) in impl_candidates {
        if candidates.len() == 1 {
            methods.insert(name, candidates.into_iter().next().unwrap());
        }
    }
    for (name, candidates) in extend_candidates {
        if methods.contains_key(&name) || candidates.len() != 1 {
            continue;
        }
        methods.insert(name, candidates.into_iter().next().unwrap());
    }
    for (name, candidates) in trait_impl_candidates {
        if methods.contains_key(&name) || candidates.len() != 1 {
            continue;
        }
        methods.insert(name, candidates.into_iter().next().unwrap());
    }
    methods
}

fn collect_dependency_public_type_expr_dependencies<'a>(
    ty: &ql_ast::TypeExpr,
    type_candidates: &BTreeMap<String, DependencyPublicTypeBridgeCandidate<'a>>,
    dependencies: &mut BTreeSet<String>,
) {
    match &ty.kind {
        ql_ast::TypeExprKind::Pointer { inner, .. } => {
            collect_dependency_public_type_expr_dependencies(inner, type_candidates, dependencies);
        }
        ql_ast::TypeExprKind::Array { element, .. } => {
            collect_dependency_public_type_expr_dependencies(
                element,
                type_candidates,
                dependencies,
            );
        }
        ql_ast::TypeExprKind::Named { path, args } => {
            if let [name] = path.segments.as_slice() {
                if type_candidates.contains_key(name) {
                    dependencies.insert(name.clone());
                }
            }
            for arg in args {
                collect_dependency_public_type_expr_dependencies(
                    arg,
                    type_candidates,
                    dependencies,
                );
            }
        }
        ql_ast::TypeExprKind::Tuple(items) => {
            for item in items {
                collect_dependency_public_type_expr_dependencies(
                    item,
                    type_candidates,
                    dependencies,
                );
            }
        }
        ql_ast::TypeExprKind::Callable { params, ret } => {
            for param in params {
                collect_dependency_public_type_expr_dependencies(
                    param,
                    type_candidates,
                    dependencies,
                );
            }
            collect_dependency_public_type_expr_dependencies(ret, type_candidates, dependencies);
        }
    }
}

fn dependency_public_type_dependencies<'a>(
    symbol_name: &str,
    type_candidates: &BTreeMap<String, DependencyPublicTypeBridgeCandidate<'a>>,
) -> Option<BTreeSet<String>> {
    let candidate = type_candidates.get(symbol_name)?;
    let mut dependencies = BTreeSet::new();
    match candidate.decl {
        DependencyPublicTypeDecl::Struct(struct_decl) => {
            for field in &struct_decl.fields {
                collect_dependency_public_type_expr_dependencies(
                    &field.ty,
                    type_candidates,
                    &mut dependencies,
                );
            }
        }
        DependencyPublicTypeDecl::Enum(enum_decl) => {
            for variant in &enum_decl.variants {
                match &variant.fields {
                    ql_ast::VariantFields::Unit => {}
                    ql_ast::VariantFields::Tuple(items) => {
                        for item in items {
                            collect_dependency_public_type_expr_dependencies(
                                item,
                                type_candidates,
                                &mut dependencies,
                            );
                        }
                    }
                    ql_ast::VariantFields::Struct(fields) => {
                        for field in fields {
                            collect_dependency_public_type_expr_dependencies(
                                &field.ty,
                                type_candidates,
                                &mut dependencies,
                            );
                        }
                    }
                }
            }
        }
        DependencyPublicTypeDecl::TypeAlias(alias) => {
            collect_dependency_public_type_expr_dependencies(
                &alias.ty,
                type_candidates,
                &mut dependencies,
            );
        }
    }
    Some(dependencies)
}

fn dependency_public_type_bridge_order<'a>(
    symbol_name: &str,
    type_candidates: &BTreeMap<String, DependencyPublicTypeBridgeCandidate<'a>>,
) -> Option<Vec<String>> {
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut ordered = Vec::new();
    if collect_dependency_public_type_bridge_order(
        symbol_name,
        type_candidates,
        &mut visiting,
        &mut visited,
        &mut ordered,
    ) {
        Some(ordered)
    } else {
        None
    }
}

fn collect_dependency_public_type_bridge_order<'a>(
    symbol_name: &str,
    type_candidates: &BTreeMap<String, DependencyPublicTypeBridgeCandidate<'a>>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    ordered: &mut Vec<String>,
) -> bool {
    if visited.contains(symbol_name) {
        return true;
    }
    let Some(_) = type_candidates.get(symbol_name) else {
        return false;
    };
    if !visiting.insert(symbol_name.to_owned()) {
        return false;
    }

    let Some(dependencies) = dependency_public_type_dependencies(symbol_name, type_candidates)
    else {
        visiting.remove(symbol_name);
        return false;
    };

    for dependency in dependencies {
        if !collect_dependency_public_type_bridge_order(
            &dependency,
            type_candidates,
            visiting,
            visited,
            ordered,
        ) {
            visiting.remove(symbol_name);
            return false;
        }
    }

    visiting.remove(symbol_name);
    visited.insert(symbol_name.to_owned());
    ordered.push(symbol_name.to_owned());
    true
}

fn collect_dependency_public_function_type_dependencies<'a>(
    function: &FunctionDecl,
    type_candidates: &BTreeMap<String, DependencyPublicTypeBridgeCandidate<'a>>,
) -> BTreeSet<String> {
    let mut dependencies = BTreeSet::new();
    for param in &function.params {
        if let Param::Regular { ty, .. } = param {
            collect_dependency_public_type_expr_dependencies(
                ty,
                type_candidates,
                &mut dependencies,
            );
        }
    }
    if let Some(return_type) = &function.return_type {
        collect_dependency_public_type_expr_dependencies(
            return_type,
            type_candidates,
            &mut dependencies,
        );
    }
    dependencies
}

fn collect_dependency_public_global_expr_dependencies<'a>(
    expr: &Expr,
    global_candidates: &BTreeMap<String, DependencyPublicGlobalBridgeCandidate<'a>>,
    function_candidates: &BTreeSet<String>,
    dependencies: &mut DependencyPublicGlobalExprDependencies,
) -> bool {
    match &expr.kind {
        ExprKind::Integer(_) | ExprKind::String { .. } | ExprKind::Bool(_) => true,
        ExprKind::Name(name) => {
            if global_candidates.contains_key(name) {
                dependencies.globals.insert(name.clone());
                true
            } else if function_candidates.contains(name) {
                dependencies.functions.insert(name.clone());
                true
            } else {
                false
            }
        }
        ExprKind::Tuple(items) | ExprKind::Array(items) => items.iter().all(|item| {
            collect_dependency_public_global_expr_dependencies(
                item,
                global_candidates,
                function_candidates,
                dependencies,
            )
        }),
        ExprKind::RepeatArray { value, .. } => collect_dependency_public_global_expr_dependencies(
            value,
            global_candidates,
            function_candidates,
            dependencies,
        ),
        ExprKind::StructLiteral { fields, .. } => fields.iter().all(|field| {
            field.value.as_ref().is_some_and(|value| {
                collect_dependency_public_global_expr_dependencies(
                    value,
                    global_candidates,
                    function_candidates,
                    dependencies,
                )
            })
        }),
        ExprKind::Binary { left, right, .. } => {
            collect_dependency_public_global_expr_dependencies(
                left,
                global_candidates,
                function_candidates,
                dependencies,
            ) && collect_dependency_public_global_expr_dependencies(
                right,
                global_candidates,
                function_candidates,
                dependencies,
            )
        }
        ExprKind::Unary { expr, .. } | ExprKind::Question(expr) => {
            collect_dependency_public_global_expr_dependencies(
                expr,
                global_candidates,
                function_candidates,
                dependencies,
            )
        }
        ExprKind::Call { callee, args } => {
            collect_dependency_public_global_expr_dependencies(
                callee,
                global_candidates,
                function_candidates,
                dependencies,
            ) && args.iter().all(|arg| match arg {
                CallArg::Positional(value) => collect_dependency_public_global_expr_dependencies(
                    value,
                    global_candidates,
                    function_candidates,
                    dependencies,
                ),
                CallArg::Named { value, .. } => collect_dependency_public_global_expr_dependencies(
                    value,
                    global_candidates,
                    function_candidates,
                    dependencies,
                ),
            })
        }
        ExprKind::Member { object, .. } => collect_dependency_public_global_expr_dependencies(
            object,
            global_candidates,
            function_candidates,
            dependencies,
        ),
        ExprKind::Bracket { target, items } => {
            collect_dependency_public_global_expr_dependencies(
                target,
                global_candidates,
                function_candidates,
                dependencies,
            ) && items.iter().all(|item| {
                collect_dependency_public_global_expr_dependencies(
                    item,
                    global_candidates,
                    function_candidates,
                    dependencies,
                )
            })
        }
        ExprKind::NoneLiteral
        | ExprKind::Block(_)
        | ExprKind::Unsafe(_)
        | ExprKind::If { .. }
        | ExprKind::Match { .. }
        | ExprKind::Closure { .. } => false,
    }
}

fn dependency_public_global_dependencies<'a>(
    symbol_name: &str,
    global_candidates: &BTreeMap<String, DependencyPublicGlobalBridgeCandidate<'a>>,
    function_candidates: &BTreeSet<String>,
) -> Option<DependencyPublicGlobalExprDependencies> {
    let candidate = global_candidates.get(symbol_name)?;
    let mut dependencies = DependencyPublicGlobalExprDependencies::default();
    collect_dependency_public_global_expr_dependencies(
        &candidate.global.value,
        global_candidates,
        function_candidates,
        &mut dependencies,
    )
    .then_some(dependencies)
}

fn dependency_public_global_bridge_order<'a>(
    symbol_name: &str,
    global_candidates: &BTreeMap<String, DependencyPublicGlobalBridgeCandidate<'a>>,
    function_candidates: &BTreeSet<String>,
) -> Option<Vec<String>> {
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut ordered = Vec::new();
    if collect_dependency_public_global_bridge_order(
        symbol_name,
        global_candidates,
        function_candidates,
        &mut visiting,
        &mut visited,
        &mut ordered,
    ) {
        Some(ordered)
    } else {
        None
    }
}

fn collect_dependency_public_global_bridge_order<'a>(
    symbol_name: &str,
    global_candidates: &BTreeMap<String, DependencyPublicGlobalBridgeCandidate<'a>>,
    function_candidates: &BTreeSet<String>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    ordered: &mut Vec<String>,
) -> bool {
    if visited.contains(symbol_name) {
        return true;
    }
    let Some(_) = global_candidates.get(symbol_name) else {
        return false;
    };
    if !visiting.insert(symbol_name.to_owned()) {
        return false;
    }

    let Some(dependencies) =
        dependency_public_global_dependencies(symbol_name, global_candidates, function_candidates)
    else {
        visiting.remove(symbol_name);
        return false;
    };

    for dependency in dependencies.globals {
        if !collect_dependency_public_global_bridge_order(
            &dependency,
            global_candidates,
            function_candidates,
            visiting,
            visited,
            ordered,
        ) {
            visiting.remove(symbol_name);
            return false;
        }
    }

    visiting.remove(symbol_name);
    visited.insert(symbol_name.to_owned());
    ordered.push(symbol_name.to_owned());
    true
}

fn collect_dependency_module_public_value_declarations(
    dependency_package: &str,
    dependency_manifest_path: &Path,
    module: &Module,
    contents: &str,
    imported_externs: Option<&ImportedDependencyExterns>,
    occupied_root_names: &BTreeSet<String>,
    required_functions_by_module_path: &mut BTreeMap<Vec<String>, BTreeSet<String>>,
    required_types_by_module_path: &mut BTreeMap<Vec<String>, BTreeSet<String>>,
    owners_by_symbol: &mut BTreeMap<String, DependencyExternOwner>,
    declarations: &mut Vec<String>,
) -> Result<(), DependencyPublicValueBridgeError> {
    let module_import_path = dependency_interface_module_import_path(dependency_package, module);
    let global_candidates = dependency_public_global_bridge_candidates(module);
    let function_candidates = dependency_public_function_bridge_candidates(module);
    let type_candidates = dependency_public_type_bridge_candidates(module);
    let mut emitted = BTreeSet::new();

    for item in &module.items {
        let global = match &item.kind {
            ItemKind::Const(global) | ItemKind::Static(global)
                if global.visibility == Visibility::Public =>
            {
                global
            }
            _ => continue,
        };
        if imported_externs.is_some_and(|imports| {
            !dependency_extern_is_imported(imports, &module_import_path, &global.name)
        }) {
            continue;
        }

        let Some(ordered_symbols) = dependency_public_global_bridge_order(
            &global.name,
            &global_candidates,
            &function_candidates,
        ) else {
            continue;
        };

        for ordered_symbol in ordered_symbols {
            if emitted.contains(&ordered_symbol) {
                continue;
            }
            if occupied_root_names.contains(&ordered_symbol) {
                return Err(DependencyPublicValueBridgeError::LocalConflict {
                    symbol: ordered_symbol,
                });
            }
            let candidate = global_candidates
                .get(&ordered_symbol)
                .expect("ordered dependency public globals should resolve to candidates");
            let dependencies = dependency_public_global_dependencies(
                &ordered_symbol,
                &global_candidates,
                &function_candidates,
            )
            .expect("emitted dependency public globals should remain bridgeable");
            record_dependency_extern_declaration(
                dependency_package,
                dependency_manifest_path,
                &ordered_symbol,
                span_text(contents, candidate.item.span),
                owners_by_symbol,
                declarations,
            )
            .map_err(|(symbol, owner)| {
                DependencyPublicValueBridgeError::DependencyConflict { symbol, owner }
            })?;
            if !dependencies.functions.is_empty() {
                required_functions_by_module_path
                    .entry(module_import_path.clone())
                    .or_default()
                    .extend(dependencies.functions);
            }
            let mut type_dependencies = BTreeSet::new();
            collect_dependency_public_type_expr_dependencies(
                &candidate.global.ty,
                &type_candidates,
                &mut type_dependencies,
            );
            if !type_dependencies.is_empty() {
                required_types_by_module_path
                    .entry(module_import_path.clone())
                    .or_default()
                    .extend(type_dependencies);
            }
            emitted.insert(ordered_symbol);
        }
    }

    Ok(())
}

fn collect_dependency_module_public_function_forwarders(
    dependency_package: &str,
    dependency_manifest_path: &Path,
    module: &Module,
    contents: &str,
    root_module: &Module,
    imported_externs: Option<&ImportedDependencyExterns>,
    required_function_names: Option<&BTreeSet<String>>,
    occupied_root_names: &BTreeSet<String>,
    specialization_modules: &[dependency_generic_bridge::SpecializationModule<'_>],
    required_types_by_module_path: &mut BTreeMap<Vec<String>, BTreeSet<String>>,
    owners_by_symbol: &mut BTreeMap<String, DependencyExternOwner>,
    forwarders: &mut Vec<String>,
    source_rewrites: &mut Vec<dependency_generic_bridge::SourceRewrite>,
    rendered_specializations: &mut BTreeSet<String>,
) -> Result<(), DependencyPublicFunctionForwarderError> {
    let module_import_path = dependency_interface_module_import_path(dependency_package, module);
    let type_candidates = dependency_public_type_bridge_candidates(module);

    for item in &module.items {
        let ItemKind::Function(function) = &item.kind else {
            continue;
        };
        let imported = imported_externs.is_none_or(|imports| {
            dependency_extern_is_imported(imports, &module_import_path, &function.name)
        });
        let required_by_value = required_function_names.is_some_and(|required_function_names| {
            required_function_names.contains(&function.name)
        });
        if !imported && !required_by_value {
            continue;
        }
        if dependency_generic_bridge::supports_public_function_specialization(function) {
            let rendered =
                match dependency_generic_bridge::render_public_function_specialization_status_with_context(
                    &module_import_path,
                    function,
                    contents,
                    root_module,
                    module,
                    specialization_modules,
                    rendered_specializations,
                ) {
                    dependency_generic_bridge::PublicFunctionSpecializationRender::Rendered(
                        rendered,
                    ) => rendered,
                    dependency_generic_bridge::PublicFunctionSpecializationRender::NotCalled
                        if !required_by_value =>
                    {
                        continue;
                    }
                    dependency_generic_bridge::PublicFunctionSpecializationRender::NotCalled
                    | dependency_generic_bridge::PublicFunctionSpecializationRender::Unsupported => {
                        return Err(DependencyPublicFunctionForwarderError::UnsupportedGeneric {
                            symbol: function.name.clone(),
                        });
                    }
                };
            record_dependency_extern_declaration(
                dependency_package,
                dependency_manifest_path,
                &function.name,
                rendered.declarations,
                owners_by_symbol,
                forwarders,
            )
            .map_err(|(symbol, owner)| {
                DependencyPublicFunctionForwarderError::DependencyConflict { symbol, owner }
            })?;
            let type_dependencies =
                collect_dependency_public_function_type_dependencies(function, &type_candidates);
            if !type_dependencies.is_empty() {
                required_types_by_module_path
                    .entry(module_import_path.clone())
                    .or_default()
                    .extend(type_dependencies);
            }
            source_rewrites.extend(rendered.call_rewrites);
            continue;
        }
        if !supports_dependency_public_function_import_bridge(function) {
            continue;
        }
        let type_dependencies =
            collect_dependency_public_function_type_dependencies(function, &type_candidates);
        if occupied_root_names.contains(&function.name) {
            return Err(DependencyPublicFunctionForwarderError::LocalConflict {
                symbol: function.name.clone(),
            });
        }
        let Some(forwarder) = render_imported_dependency_public_function_forwarder(
            &module_import_path,
            function,
            contents,
        ) else {
            continue;
        };
        record_dependency_extern_declaration(
            dependency_package,
            dependency_manifest_path,
            &function.name,
            forwarder,
            owners_by_symbol,
            forwarders,
        )
        .map_err(|(symbol, owner)| {
            DependencyPublicFunctionForwarderError::DependencyConflict { symbol, owner }
        })?;
        if !type_dependencies.is_empty() {
            required_types_by_module_path
                .entry(module_import_path.clone())
                .or_default()
                .extend(type_dependencies);
        }
    }

    Ok(())
}

fn render_direct_dependency_public_type_declarations(
    command_label: &str,
    manifest_path: &Path,
    source: &str,
    required_types_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
    report_failure: bool,
) -> Result<String, u8> {
    let manifest = load_project_manifest(manifest_path).map_err(|error| {
        if report_failure {
            report_project_build_dependency_error(command_label, None, &error);
        }
        1
    })?;
    let direct_dependencies = load_reference_manifests(&manifest).map_err(|error| {
        if report_failure {
            report_project_build_dependency_error(command_label, Some(manifest_path), &error);
        }
        1
    })?;
    let root_source_module = match parse_source(source) {
        Ok(module) => module,
        Err(_) => return Ok(String::new()),
    };
    let occupied_root_names = collect_top_level_definition_names(&root_source_module);

    let mut declarations = Vec::new();
    let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();

    for dependency in direct_dependencies {
        let dependency_package = match package_name(&dependency) {
            Ok(name) => name.to_owned(),
            Err(error) => {
                if report_failure {
                    report_project_build_dependency_error(
                        command_label,
                        Some(manifest_path),
                        &error,
                    );
                }
                return Err(1);
            }
        };
        let interface_path = default_interface_path(&dependency).map_err(|error| {
            if report_failure {
                report_project_build_dependency_error(command_label, Some(manifest_path), &error);
            }
            1
        })?;
        let artifact = load_interface_artifact(&interface_path).map_err(|error| {
            if report_failure {
                eprintln!(
                    "error: {command_label} failed to load referenced package interface `{}`: {error}",
                    normalize_path(&interface_path)
                );
                eprintln!(
                    "note: while preparing dependency public type bridges for `{}`",
                    normalize_path(manifest_path)
                );
            }
            1
        })?;
        let module_import_paths =
            dependency_interface_module_import_paths(&dependency_package, &artifact.modules);
        let imported_externs =
            collect_imported_dependency_externs(&root_source_module, &module_import_paths);

        for module in &artifact.modules {
            let dependency_source_path =
                dependency_module_source_path(&dependency.manifest_path, &module.source_path);
            let dependency_source = fs::read_to_string(&dependency_source_path).map_err(|error| {
                if report_failure {
                    eprintln!(
                        "error: {command_label} failed to access dependency source `{}`: {error}",
                        normalize_path(&dependency_source_path)
                    );
                    eprintln!(
                        "note: while preparing dependency public type bridges for `{}`",
                        normalize_path(manifest_path)
                    );
                }
                1
            })?;
            let source_module = match parse_source(&dependency_source) {
                Ok(module) => module,
                Err(_) => {
                    if report_failure {
                        eprintln!(
                            "error: {command_label} failed to parse dependency source `{}` while preparing public type bridges",
                            normalize_path(&dependency_source_path)
                        );
                        eprintln!("note: dependency package: `{dependency_package}`");
                    }
                    return Err(1);
                }
            };
            let module_import_path =
                dependency_interface_module_import_path(&dependency_package, &source_module);
            collect_dependency_module_public_type_declarations(
                &dependency_package,
                &dependency.manifest_path,
                &source_module,
                &dependency_source,
                Some(&imported_externs),
                required_types_by_module_path.get(&module_import_path),
                &occupied_root_names,
                &mut owners_by_symbol,
                &mut declarations,
            )
            .map_err(|error| {
                if report_failure {
                    match error {
                        DependencyPublicTypeBridgeError::DependencyConflict { symbol, owner } => {
                            eprintln!(
                                "error: {command_label} found conflicting direct dependency public type imports for `{symbol}`"
                            );
                            eprintln!("note: first package: `{}`", owner.package_name);
                            eprintln!("note: conflicting package: `{dependency_package}`");
                            eprintln!(
                                "hint: keep direct dependency public type names unique until package-qualified dependency type lowering lands"
                            );
                        }
                        DependencyPublicTypeBridgeError::LocalConflict { symbol } => {
                            eprintln!(
                                "error: {command_label} cannot synthesize direct dependency public type bridge for `{symbol}` because the root source already defines the same top-level name"
                            );
                            eprintln!("note: conflicting direct dependency package: `{dependency_package}`");
                            eprintln!(
                                "hint: rename the local top-level item or avoid importing a direct dependency public type with the same original symbol name"
                            );
                        }
                    }
                }
                1
            })?;
        }
    }

    Ok(declarations.join("\n\n"))
}

fn render_direct_dependency_public_type_declarations_quiet(
    manifest_path: &Path,
    source: &str,
    required_types_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
) -> Result<String, PrepareProjectTargetBuildError> {
    let manifest = load_project_manifest(manifest_path)
        .map_err(|error| target_prep_dependency_manifest_failure(None, &error))?;
    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let root_source_module = match parse_source(source) {
        Ok(module) => module,
        Err(_) => return Ok(String::new()),
    };
    let occupied_root_names = collect_top_level_definition_names(&root_source_module);
    let direct_dependencies = manifest
        .references
        .packages
        .iter()
        .map(|reference| {
            let reference_manifest_path = reference_manifest_path(&manifest, reference);
            let dependency_manifest = load_project_manifest(&manifest_dir.join(reference))
                .map_err(|error| {
                    target_prep_dependency_manifest_failure(Some(&reference_manifest_path), &error)
                })?;
            Ok::<_, PrepareProjectTargetBuildError>(dependency_manifest)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut declarations = Vec::new();
    let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();

    for dependency_manifest in direct_dependencies {
        let dependency_package = package_name(&dependency_manifest)
            .map(str::to_owned)
            .map_err(|error| {
                target_prep_dependency_manifest_failure(
                    Some(&dependency_manifest.manifest_path),
                    &error,
                )
            })?;
        let interface_path = default_interface_path(&dependency_manifest).map_err(|error| {
            target_prep_dependency_manifest_failure(
                Some(&dependency_manifest.manifest_path),
                &error,
            )
        })?;
        let artifact = load_interface_artifact(&interface_path).map_err(|error| {
            PrepareProjectTargetBuildError {
                failure_kind: PrepareProjectTargetBuildFailureKind::DependencyInterface {
                    dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                    dependency_package: dependency_package.clone(),
                    interface_path: interface_path.clone(),
                    message: format!(
                        "failed to load referenced package interface `{}`: {error}",
                        normalize_path(&interface_path)
                    ),
                },
            }
        })?;
        let module_import_paths =
            dependency_interface_module_import_paths(&dependency_package, &artifact.modules);
        let imported_externs =
            collect_imported_dependency_externs(&root_source_module, &module_import_paths);

        for module in &artifact.modules {
            let dependency_source_path = dependency_module_source_path(
                &dependency_manifest.manifest_path,
                &module.source_path,
            );
            let dependency_source =
                fs::read_to_string(&dependency_source_path).map_err(|error| {
                    PrepareProjectTargetBuildError {
                        failure_kind: PrepareProjectTargetBuildFailureKind::DependencySource {
                            dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                            dependency_package: dependency_package.clone(),
                            source_path: dependency_source_path.clone(),
                            message: format!(
                                "failed to access dependency source `{}`: {error}",
                                normalize_path(&dependency_source_path)
                            ),
                        },
                    }
                })?;
            let source_module = parse_source(&dependency_source).map_err(|_| {
                PrepareProjectTargetBuildError {
                    failure_kind: PrepareProjectTargetBuildFailureKind::DependencySource {
                        dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                        dependency_package: dependency_package.clone(),
                        source_path: dependency_source_path.clone(),
                        message: format!(
                            "failed to parse dependency source `{}` while preparing public type bridges",
                            normalize_path(&dependency_source_path)
                        ),
                    },
                }
            })?;
            let module_import_path =
                dependency_interface_module_import_path(&dependency_package, &source_module);
            collect_dependency_module_public_type_declarations(
                &dependency_package,
                &dependency_manifest.manifest_path,
                &source_module,
                &dependency_source,
                Some(&imported_externs),
                required_types_by_module_path.get(&module_import_path),
                &occupied_root_names,
                &mut owners_by_symbol,
                &mut declarations,
            )
            .map_err(|error| match error {
                DependencyPublicTypeBridgeError::DependencyConflict { symbol, owner } => {
                    PrepareProjectTargetBuildError {
                        failure_kind:
                            PrepareProjectTargetBuildFailureKind::DependencyTypeConflict {
                                symbol,
                                first_package: owner.package_name,
                                first_manifest_path: owner.manifest_path,
                                conflicting_package: dependency_package.clone(),
                                conflicting_manifest_path: dependency_manifest
                                    .manifest_path
                                    .clone(),
                            },
                    }
                }
                DependencyPublicTypeBridgeError::LocalConflict { symbol } => {
                    PrepareProjectTargetBuildError {
                        failure_kind:
                            PrepareProjectTargetBuildFailureKind::DependencyTypeLocalConflict {
                                symbol,
                                dependency_package: dependency_package.clone(),
                                dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                            },
                    }
                }
            })?;
        }
    }

    Ok(declarations.join("\n\n"))
}

fn collect_dependency_module_public_type_declarations(
    dependency_package: &str,
    dependency_manifest_path: &Path,
    module: &Module,
    contents: &str,
    imported_externs: Option<&ImportedDependencyExterns>,
    required_type_names: Option<&BTreeSet<String>>,
    occupied_root_names: &BTreeSet<String>,
    owners_by_symbol: &mut BTreeMap<String, DependencyExternOwner>,
    declarations: &mut Vec<String>,
) -> Result<(), DependencyPublicTypeBridgeError> {
    let module_import_path = dependency_interface_module_import_path(dependency_package, module);
    let type_candidates = dependency_public_type_bridge_candidates(module);
    let mut emitted = BTreeSet::new();

    for item in &module.items {
        let type_name = match &item.kind {
            ItemKind::Struct(struct_decl) if struct_decl.visibility == Visibility::Public => {
                struct_decl.name.as_str()
            }
            ItemKind::Enum(enum_decl) if enum_decl.visibility == Visibility::Public => {
                enum_decl.name.as_str()
            }
            ItemKind::TypeAlias(alias)
                if alias.visibility == Visibility::Public
                    && !alias.is_opaque
                    && alias.generics.is_empty() =>
            {
                alias.name.as_str()
            }
            _ => continue,
        };
        let imported = imported_externs.is_none_or(|imports| {
            dependency_extern_is_imported(imports, &module_import_path, type_name)
        });
        let required_by_bridge = required_type_names
            .is_some_and(|required_type_names| required_type_names.contains(type_name));
        if !imported && !required_by_bridge {
            continue;
        }

        let Some(ordered_symbols) =
            dependency_public_type_bridge_order(type_name, &type_candidates)
        else {
            continue;
        };

        for ordered_symbol in ordered_symbols {
            if emitted.contains(&ordered_symbol) {
                continue;
            }
            if occupied_root_names.contains(&ordered_symbol) {
                return Err(DependencyPublicTypeBridgeError::LocalConflict {
                    symbol: ordered_symbol,
                });
            }
            let candidate = type_candidates
                .get(&ordered_symbol)
                .expect("ordered dependency public types should resolve to candidates");
            record_dependency_extern_declaration(
                dependency_package,
                dependency_manifest_path,
                &ordered_symbol,
                span_text(contents, candidate.item.span),
                owners_by_symbol,
                declarations,
            )
            .map_err(|(symbol, owner)| {
                DependencyPublicTypeBridgeError::DependencyConflict { symbol, owner }
            })?;
            emitted.insert(ordered_symbol);
        }
    }

    Ok(())
}

enum DependencyPublicTypeBridgeError {
    DependencyConflict {
        symbol: String,
        owner: DependencyExternOwner,
    },
    LocalConflict {
        symbol: String,
    },
}

enum DependencyPublicFunctionForwarderError {
    DependencyConflict {
        symbol: String,
        owner: DependencyExternOwner,
    },
    LocalConflict {
        symbol: String,
    },
    UnsupportedGeneric {
        symbol: String,
    },
}

fn dependency_function_forwarder_target_prep_error(
    error: DependencyPublicFunctionForwarderError,
    dependency_package: &str,
    dependency_manifest_path: &Path,
) -> PrepareProjectTargetBuildError {
    let failure_kind = match error {
        DependencyPublicFunctionForwarderError::DependencyConflict { symbol, owner } => {
            PrepareProjectTargetBuildFailureKind::DependencyFunctionConflict {
                symbol,
                first_package: owner.package_name,
                first_manifest_path: owner.manifest_path,
                conflicting_package: dependency_package.to_owned(),
                conflicting_manifest_path: dependency_manifest_path.to_path_buf(),
            }
        }
        DependencyPublicFunctionForwarderError::LocalConflict { symbol } => {
            PrepareProjectTargetBuildFailureKind::DependencyFunctionLocalConflict {
                symbol,
                dependency_package: dependency_package.to_owned(),
                dependency_manifest_path: dependency_manifest_path.to_path_buf(),
            }
        }
        DependencyPublicFunctionForwarderError::UnsupportedGeneric { symbol } => {
            PrepareProjectTargetBuildFailureKind::DependencyFunctionUnsupportedGeneric {
                symbol,
                dependency_package: dependency_package.to_owned(),
                dependency_manifest_path: dependency_manifest_path.to_path_buf(),
            }
        }
    };
    PrepareProjectTargetBuildError { failure_kind }
}

enum DependencyPublicValueBridgeError {
    DependencyConflict {
        symbol: String,
        owner: DependencyExternOwner,
    },
    LocalConflict {
        symbol: String,
    },
}

#[cfg(test)]
mod tests;
