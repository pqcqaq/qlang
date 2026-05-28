use std::path::Path;

use ql_project::{WorkspaceBuildTargets, discover_workspace_build_targets, load_project_manifest};

use crate::build_reporting::build_json_project_error;
use crate::project_targets::load_workspace_build_targets_for_command_from_request_root;
use crate::test_command::TestCommandOptions;
use crate::test_reporting::render_test_json_preflight_failure_report;

pub(super) fn load_all_project_test_members(
    request_path: &Path,
    project_path: &Path,
    command_options: &TestCommandOptions,
) -> Result<Vec<WorkspaceBuildTargets>, u8> {
    if command_options.json {
        let manifest = match load_project_manifest(project_path) {
            Ok(manifest) => manifest,
            Err(error) => {
                print!(
                    "{}",
                    render_test_json_preflight_failure_report(
                        request_path,
                        command_options,
                        build_json_project_error(request_path, &error, "manifest-load"),
                    )
                );
                return Err(1);
            }
        };
        return match discover_workspace_build_targets(&manifest) {
            Ok(members) => Ok(members),
            Err(error) => {
                print!(
                    "{}",
                    render_test_json_preflight_failure_report(
                        request_path,
                        command_options,
                        build_json_project_error(request_path, &error, "target-discovery"),
                    )
                );
                Err(1)
            }
        };
    }

    load_workspace_build_targets_for_command_from_request_root(
        request_path,
        project_path,
        "`ql test`",
    )
}
