use std::path::Path;

use ql_driver::BuildOptions;

use crate::project_targets::{
    ProjectCommandScope, resolve_project_workspace_member_command_request_root,
};
use crate::test_command::TestCommandOptions;
use crate::test_reporting::TestTarget;

mod direct;
mod filters;
mod listing;
mod no_match;
mod package_selector;
mod paths;
mod project;
mod project_errors;
mod selection;
mod targets;
mod ui;

use direct::discover_direct_test_targets;
pub(crate) use filters::{filter_test_targets, select_test_targets_by_path};
pub(crate) use listing::list_test_targets;
pub(crate) use no_match::{
    report_no_matching_test_target, report_no_matching_tests, report_no_tests_discovered,
    test_no_matching_filter_message, test_no_matching_target_message, test_no_tests_message,
};
use project::discover_project_test_targets;

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
            discover_direct_test_targets(path, options, command_options)
        }
    }
}

#[cfg(test)]
#[path = "test_discovery_tests.rs"]
mod tests;
