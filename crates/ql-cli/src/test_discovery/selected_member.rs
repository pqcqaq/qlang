use std::path::Path;

use ql_project::{WorkspaceBuildTargets, load_project_manifest};

use crate::test_command::TestCommandOptions;

use super::project_errors::report_ql_test_project_preflight_error;
use super::selected_package::load_package_selected_test_member;
use super::selected_workspace::load_workspace_selected_test_member;

pub(super) fn load_selected_project_test_member(
    request_path: &Path,
    project_path: &Path,
    command_options: &TestCommandOptions,
    selected_package_name: &str,
) -> Result<Vec<WorkspaceBuildTargets>, u8> {
    let manifest = match load_project_manifest(project_path) {
        Ok(manifest) => manifest,
        Err(error) => {
            return Err(report_ql_test_project_preflight_error(
                request_path,
                command_options,
                &error,
                "manifest-load",
            ));
        }
    };

    if manifest.workspace.is_some() {
        return load_workspace_selected_test_member(
            request_path,
            command_options,
            &manifest,
            selected_package_name,
        );
    }

    load_package_selected_test_member(
        request_path,
        command_options,
        &manifest,
        selected_package_name,
    )
}
