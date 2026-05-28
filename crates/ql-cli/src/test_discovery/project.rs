use std::path::Path;

use ql_driver::BuildOptions;

use crate::project_targets::project_request_root;
use crate::test_command::TestCommandOptions;
use crate::test_reporting::TestTarget;

use super::selection::load_project_test_members;
use super::targets::project_test_target;
use super::test_files::collect_project_test_files;

pub(super) fn discover_project_test_targets(
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
        for file in collect_project_test_files(&package_root)? {
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
