use std::path::Path;

use ql_driver::BuildOptions;

use crate::test_command::TestCommandOptions;
use crate::test_reporting::{TestTarget, render_test_json_preflight_message_report};

use super::targets::direct_test_target;

pub(super) fn discover_direct_test_targets(
    path: &Path,
    options: &BuildOptions,
    command_options: &TestCommandOptions,
) -> Result<Vec<TestTarget>, u8> {
    if let Some(package_name) = command_options.package_name.as_deref() {
        if command_options.json {
            print!(
                "{}",
                render_test_json_preflight_message_report(
                    path,
                    command_options,
                    "selector",
                    "target-selection",
                    "`ql test` package selectors require a package or workspace path".to_owned(),
                    Some(format!("package `{package_name}`")),
                    None,
                )
            );
        } else {
            report_test_package_selector_requires_project_context(package_name);
        }
        return Err(1);
    }

    if let Some(target_path) = command_options.target_path.as_deref() {
        if command_options.json {
            print!(
                "{}",
                render_test_json_preflight_message_report(
                    path,
                    command_options,
                    "selector",
                    "target-selection",
                    "`ql test` target selectors require a package or workspace path".to_owned(),
                    Some(format!("target `{target_path}`")),
                    None,
                )
            );
        } else {
            report_test_target_selector_requires_project_context(target_path);
        }
        return Err(1);
    }

    Ok(vec![direct_test_target(path, options)?])
}

fn report_test_package_selector_requires_project_context(package_name: &str) {
    eprintln!("error: `ql test` package selectors require a package or workspace path");
    eprintln!("note: selector: package `{package_name}`");
}

fn report_test_target_selector_requires_project_context(target_path: &str) {
    eprintln!("error: `ql test` target selectors require a package or workspace path");
    eprintln!("note: selector: target `{target_path}`");
}
