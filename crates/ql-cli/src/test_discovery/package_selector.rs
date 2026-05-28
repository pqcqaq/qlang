use std::path::Path;

use ql_project::{ProjectManifest, load_project_manifest};

use crate::build_reporting::build_json_project_error;
use crate::cli_utils::{normalize_path, validate_project_package_name};
use crate::project_workspace::{
    WorkspaceMemberLookupError, render_workspace_member_lookup_error,
    resolve_workspace_member_entry_by_package_name,
};
use crate::test_command::TestCommandOptions;
use crate::test_reporting::{
    render_test_json_preflight_failure_report, render_test_json_preflight_message_report,
};

pub(super) fn validate_test_package_selector(
    request_path: &Path,
    command_options: &TestCommandOptions,
    selected_package_name: &str,
) -> Result<(), u8> {
    if let Err(message) = validate_project_package_name(selected_package_name) {
        if command_options.json {
            print!(
                "{}",
                render_test_json_preflight_message_report(
                    request_path,
                    command_options,
                    "selector",
                    "package-selection",
                    format!("`ql test` {message}"),
                    Some(format!("package `{selected_package_name}`")),
                    None,
                )
            );
        } else {
            eprintln!("error: `ql test` {message}");
        }
        return Err(1);
    }
    Ok(())
}

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
                let (error_kind, target_count) = match &error {
                    WorkspaceMemberLookupError::Missing => ("selector", Some(0)),
                    WorkspaceMemberLookupError::Ambiguous { matches } => {
                        ("selector", Some(matches.len()))
                    }
                    WorkspaceMemberLookupError::InspectionFailure { .. } => ("manifest", None),
                };
                print!(
                    "{}",
                    render_test_json_preflight_message_report(
                        request_path,
                        command_options,
                        error_kind,
                        "package-selection",
                        render_workspace_member_lookup_error(
                            manifest,
                            selected_package_name,
                            &error,
                        ),
                        Some(format!("package `{selected_package_name}`")),
                        target_count,
                    )
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

pub(super) fn report_package_selector_mismatch(
    request_path: &Path,
    command_options: &TestCommandOptions,
    selected_package_name: &str,
) {
    let message = package_selector_mismatch_message(request_path);
    if command_options.json {
        print!(
            "{}",
            render_test_json_preflight_message_report(
                request_path,
                command_options,
                "selector",
                "package-selection",
                message,
                Some(format!("package `{selected_package_name}`")),
                Some(0),
            )
        );
    } else {
        eprintln!("error: `ql test` {message}");
        eprintln!("note: selector: package `{selected_package_name}`");
        eprintln!(
            "hint: rerun `ql test {}` to inspect all workspace members, or adjust `--package`",
            normalize_path(request_path)
        );
    }
}

pub(super) fn package_selector_mismatch_message(request_path: &Path) -> String {
    format!(
        "package selector matched no workspace members under `{}`",
        normalize_path(request_path)
    )
}
