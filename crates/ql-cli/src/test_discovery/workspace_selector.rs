use std::path::Path;

use ql_project::{ProjectManifest, load_project_manifest};

use crate::build_reporting::build_json_project_error;
use crate::project_workspace::{
    WorkspaceMemberLookupError, render_workspace_member_lookup_error,
    resolve_workspace_member_entry_by_package_name,
};
use crate::test_command::TestCommandOptions;
use crate::test_reporting::{
    render_test_json_preflight_failure_report, render_test_json_preflight_message_report,
};

pub(super) fn load_workspace_selected_member_manifest_for_json(
    request_path: &Path,
    command_options: &TestCommandOptions,
    manifest: &ProjectManifest,
    selected_package_name: &str,
) -> Result<ProjectManifest, u8> {
    let (_, member_manifest_path) =
        match resolve_workspace_member_entry_by_package_name(manifest, selected_package_name) {
            Ok(member) => member,
            Err(error) => {
                report_workspace_selector_lookup_error(
                    request_path,
                    command_options,
                    manifest,
                    selected_package_name,
                    &error,
                );
                return Err(1);
            }
        };
    match load_project_manifest(&member_manifest_path) {
        Ok(member_manifest) => Ok(member_manifest),
        Err(error) => {
            print!(
                "{}",
                render_test_json_preflight_failure_report(
                    request_path,
                    command_options,
                    build_json_project_error(request_path, &error, "package-selection"),
                )
            );
            Err(1)
        }
    }
}

fn report_workspace_selector_lookup_error(
    request_path: &Path,
    command_options: &TestCommandOptions,
    manifest: &ProjectManifest,
    selected_package_name: &str,
    error: &WorkspaceMemberLookupError,
) {
    let (error_kind, target_count) = workspace_selector_lookup_error_json_fields(error);
    print!(
        "{}",
        render_test_json_preflight_message_report(
            request_path,
            command_options,
            error_kind,
            "package-selection",
            render_workspace_member_lookup_error(manifest, selected_package_name, error),
            Some(format!("package `{selected_package_name}`")),
            target_count,
        )
    );
}

pub(super) fn workspace_selector_lookup_error_json_fields(
    error: &WorkspaceMemberLookupError,
) -> (&'static str, Option<usize>) {
    match error {
        WorkspaceMemberLookupError::Missing => ("selector", Some(0)),
        WorkspaceMemberLookupError::Ambiguous { matches } => ("selector", Some(matches.len())),
        WorkspaceMemberLookupError::InspectionFailure { .. } => ("manifest", None),
    }
}
