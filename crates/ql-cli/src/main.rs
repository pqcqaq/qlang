use std::env;
use std::process::ExitCode;

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
mod dependency_bridge_direct;
mod dependency_bridge_externs;
mod dependency_bridge_imports;
mod dependency_bridge_modules;
mod dependency_bridge_names;
mod dependency_bridge_package_under_test;
mod dependency_bridge_public_export_wrappers;
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
use cli_usage::print_usage;
use cli_version::{CLI_NAME, is_version_command, version_text};
#[cfg(test)]
pub(crate) use dependency_bridge_public_types::dependency_public_type_bridge_order;
#[cfg(test)]
pub(crate) use dependency_bridge_public_types::{
    dependency_public_struct_method_bridge_candidates, dependency_public_type_bridge_candidates,
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

#[cfg(test)]
mod tests;
