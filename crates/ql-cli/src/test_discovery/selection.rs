use std::path::Path;

use ql_project::WorkspaceBuildTargets;

use crate::test_command::TestCommandOptions;

use super::members::load_all_project_test_members;
use super::selected_member::load_selected_project_test_member;

pub(super) fn load_project_test_members(
    request_path: &Path,
    project_path: &Path,
    command_options: &TestCommandOptions,
) -> Result<Vec<WorkspaceBuildTargets>, u8> {
    let Some(selected_package_name) = command_options.package_name.as_deref() else {
        return load_all_project_test_members(request_path, project_path, command_options);
    };

    load_selected_project_test_member(
        request_path,
        project_path,
        command_options,
        selected_package_name,
    )
}
