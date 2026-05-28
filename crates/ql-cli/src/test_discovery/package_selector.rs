use std::path::Path;

use crate::cli_utils::{normalize_path, validate_project_package_name};
use crate::test_command::TestCommandOptions;
use crate::test_reporting::render_test_json_preflight_message_report;

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
