use std::path::Path;

use crate::build_reporting::build_json_project_error;
use crate::test_command::TestCommandOptions;
use crate::test_reporting::render_test_json_preflight_failure_report;

use super::project_error_messages::ql_test_project_error_message;

pub(super) fn report_ql_test_project_preflight_error(
    request_path: &Path,
    command_options: &TestCommandOptions,
    error: &ql_project::ProjectError,
    stage: &str,
) -> u8 {
    if command_options.json {
        print!(
            "{}",
            render_test_json_preflight_failure_report(
                request_path,
                command_options,
                build_json_project_error(request_path, error, stage),
            )
        );
        return 1;
    }

    report_ql_test_project_error(error)
}

pub(super) fn report_ql_test_project_error(error: &ql_project::ProjectError) -> u8 {
    for line in ql_test_project_error_message(error) {
        eprintln!("{line}");
    }
    1
}
