use std::path::Path;

use ql_driver::BuildOptions;

use crate::cli_scan::collect_ql_files;
use crate::cli_utils::normalize_path;
use crate::project_targets::project_request_root;
use crate::test_command::TestCommandOptions;
use crate::test_reporting::TestTarget;

use super::selection::load_project_test_members;
use super::targets::project_test_target;

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
