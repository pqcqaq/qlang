use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::path::Path;
use std::process::ExitCode;

use ql_ast::ItemKind;
use ql_parser::parse_source;
use ql_project::{WorkspaceBuildTargets, load_project_manifest, package_name};
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
mod dependency_bridge_externs;
mod dependency_bridge_imports;
mod dependency_bridge_modules;
mod dependency_bridge_names;
mod dependency_bridge_public_functions;
mod dependency_bridge_public_globals;
mod dependency_bridge_public_methods;
mod dependency_bridge_public_type_declarations;
mod dependency_bridge_public_types;
mod dependency_bridge_public_values;
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
mod project_target_build;
mod project_targets;
mod project_workspace;
mod run_command;
mod run_pipeline;
mod test_command;
mod test_pipeline;
mod test_reporting;

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
use build_source_rewrites::{RenderedDependencyBridgeItems, join_dependency_bridge_sections};
use cli_usage::print_usage;
use cli_utils::normalize_path;
use cli_version::{CLI_NAME, is_version_command, version_text};
use dependency_bridge_externs::{
    render_direct_dependency_extern_declarations,
    render_direct_dependency_extern_declarations_quiet,
};
use dependency_bridge_imports::{
    collect_imported_dependency_externs, collect_top_level_definition_names,
    extend_dependency_bridge_name_requirements,
};
use dependency_bridge_modules::{
    dependency_generic_specialization_module_refs, dependency_generic_specialization_modules,
    dependency_interface_module_import_path, package_under_test_bridge_modules,
};
use dependency_bridge_names::{
    DependencyExternOwner, render_dependency_public_function_export_wrapper,
    render_dependency_public_method_export_wrapper,
};
use dependency_bridge_public_functions::{
    DependencyPublicFunctionForwarderError, collect_dependency_module_public_function_forwarders,
    render_direct_dependency_public_function_forwarders,
    render_direct_dependency_public_function_forwarders_quiet,
};
use dependency_bridge_public_methods::{
    render_direct_dependency_public_method_forwarders,
    render_direct_dependency_public_method_forwarders_quiet,
};
use dependency_bridge_public_type_declarations::{
    DependencyPublicTypeBridgeError, collect_dependency_module_public_type_declarations,
    render_direct_dependency_public_type_declarations,
    render_direct_dependency_public_type_declarations_quiet,
};
#[cfg(test)]
pub(crate) use dependency_bridge_public_types::dependency_public_type_bridge_order;
pub(crate) use dependency_bridge_public_types::{
    dependency_public_struct_method_bridge_candidates, dependency_public_type_bridge_candidates,
};
use dependency_bridge_public_values::{
    render_direct_dependency_public_value_declarations,
    render_direct_dependency_public_value_declarations_quiet,
};
pub(crate) use project_emit_interface::project_emit_interface_path;
pub(crate) use project_target_build::{
    build_project_source_target, build_project_source_target_result,
    build_project_source_target_silent, build_project_test_source_target_quiet,
    build_project_test_source_target_silent,
};
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

pub(crate) fn render_direct_dependency_bridge_items(
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

pub(crate) fn render_direct_dependency_bridge_items_quiet(
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

pub(crate) fn render_package_under_test_bridge_items(
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

pub(crate) fn render_public_dependency_function_export_wrappers(
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

pub(crate) fn render_public_dependency_function_export_wrappers_quiet(
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

#[cfg(test)]
mod tests;
