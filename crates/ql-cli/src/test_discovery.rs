use std::path::Path;

use ql_driver::BuildOptions;

use crate::cli_scan::collect_ql_files;
use crate::cli_utils::normalize_path;
use crate::project_targets::{
    ProjectCommandScope, project_request_root,
    resolve_project_workspace_member_command_request_root,
};
use crate::test_command::TestCommandOptions;
use crate::test_reporting::{TestTarget, render_test_json_preflight_message_report};

mod filters;
mod no_match;
mod selection;
mod targets;

pub(crate) use filters::{filter_test_targets, select_test_targets_by_path};
pub(crate) use no_match::{
    report_no_matching_test_target, report_no_matching_tests, report_no_tests_discovered,
    test_no_matching_filter_message, test_no_matching_target_message, test_no_tests_message,
};
use selection::load_project_test_members;
use targets::{direct_test_target, project_test_target};

pub(crate) fn discover_test_targets(
    path: &Path,
    options: &BuildOptions,
    command_options: &TestCommandOptions,
    command_scope: &ProjectCommandScope,
) -> Result<Vec<TestTarget>, u8> {
    match command_scope {
        ProjectCommandScope::Project => {
            let request_root = resolve_project_workspace_member_command_request_root(path);
            discover_project_test_targets(
                path,
                request_root.as_deref().unwrap_or(path),
                options,
                command_options,
            )
        }
        ProjectCommandScope::ProjectTestFile(request) => {
            let discovered = discover_project_test_targets(
                path,
                &request.request_root_manifest_path,
                options,
                command_options,
            )?;
            Ok(select_test_targets_by_path(
                discovered,
                &request.display_path,
                None,
            ))
        }
        ProjectCommandScope::ProjectBuildTarget(_) | ProjectCommandScope::DirectSource => {
            if let Some(package_name) = command_options.package_name.as_deref() {
                if command_options.json {
                    print!(
                        "{}",
                        render_test_json_preflight_message_report(
                            path,
                            command_options,
                            "selector",
                            "target-selection",
                            "`ql test` package selectors require a package or workspace path"
                                .to_owned(),
                            Some(format!("package `{package_name}`")),
                            None,
                        )
                    );
                } else {
                    report_test_package_selector_requires_project_context(package_name);
                }
                return Err(1);
            }
            if let Some(target_path) = command_options.target_path.as_deref() {
                if command_options.json {
                    print!(
                        "{}",
                        render_test_json_preflight_message_report(
                            path,
                            command_options,
                            "selector",
                            "target-selection",
                            "`ql test` target selectors require a package or workspace path"
                                .to_owned(),
                            Some(format!("target `{target_path}`")),
                            None,
                        )
                    );
                } else {
                    report_test_target_selector_requires_project_context(target_path);
                }
                return Err(1);
            }
            Ok(vec![direct_test_target(path, options)?])
        }
    }
}

fn discover_project_test_targets(
    request_path: &Path,
    project_path: &Path,
    options: &BuildOptions,
    command_options: &TestCommandOptions,
) -> Result<Vec<TestTarget>, u8> {
    let members = load_project_test_members(request_path, project_path, command_options)?;
    let request_root = project_request_root(project_path);
    let mut targets = Vec::new();

    for member in members {
        let package_root = member
            .member_manifest_path
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();
        let tests_root = package_root.join("tests");
        if !tests_root.is_dir() {
            continue;
        }

        let files = collect_ql_files(&tests_root).map_err(|error| {
            eprintln!(
                "error: `ql test` failed to read `{}`: {error}",
                normalize_path(&tests_root)
            );
            1
        })?;

        for file in files {
            targets.push(project_test_target(
                &request_root,
                &member,
                &package_root,
                &file,
                options,
                command_options.profile_overridden,
            ));
        }
    }

    Ok(targets)
}

fn report_test_package_selector_requires_project_context(package_name: &str) {
    eprintln!("error: `ql test` package selectors require a package or workspace path");
    eprintln!("note: selector: package `{package_name}`");
}

fn report_test_target_selector_requires_project_context(target_path: &str) {
    eprintln!("error: `ql test` target selectors require a package or workspace path");
    eprintln!("note: selector: target `{target_path}`");
}

pub(crate) fn list_test_targets(targets: &[TestTarget]) {
    for target in targets {
        println!("{}", target.display_path);
    }
    println!();
    println!("test listing: {} discovered", targets.len());
}

#[cfg(test)]
#[path = "test_discovery_tests.rs"]
mod tests;
