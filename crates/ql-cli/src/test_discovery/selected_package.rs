use std::path::Path;

use ql_project::{ProjectManifest, WorkspaceBuildTargets, package_name};

use crate::test_command::TestCommandOptions;

use super::member_targets::project_test_build_targets_from_manifest;
use super::package_selector::{report_package_selector_mismatch, validate_test_package_selector};
use super::project_errors::report_ql_test_project_preflight_error;

pub(super) fn load_package_selected_test_member(
    request_path: &Path,
    command_options: &TestCommandOptions,
    manifest: &ProjectManifest,
    selected_package_name: &str,
) -> Result<Vec<WorkspaceBuildTargets>, u8> {
    validate_test_package_selector(request_path, command_options, selected_package_name)?;
    let current_package_name = match package_name(manifest) {
        Ok(package_name) => package_name,
        Err(error) => {
            return Err(report_ql_test_project_preflight_error(
                request_path,
                command_options,
                &error,
                "package-selection",
            ));
        }
    };
    if current_package_name != selected_package_name {
        report_package_selector_mismatch(request_path, command_options, selected_package_name);
        return Err(1);
    }
    Ok(vec![project_test_build_targets_from_manifest(
        request_path,
        command_options,
        manifest,
        None,
    )?])
}
