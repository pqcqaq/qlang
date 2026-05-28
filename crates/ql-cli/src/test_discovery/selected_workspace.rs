use std::path::Path;

use ql_project::{ProjectManifest, WorkspaceBuildTargets};

use crate::project_workspace::resolve_selected_workspace_member_manifest;
use crate::test_command::TestCommandOptions;

use super::member_targets::project_test_build_targets_from_manifest;
use super::package_selector::validate_test_package_selector;
use super::workspace_selector::load_workspace_selected_member_manifest_for_json;

pub(super) fn load_workspace_selected_test_member(
    request_path: &Path,
    command_options: &TestCommandOptions,
    manifest: &ProjectManifest,
    selected_package_name: &str,
) -> Result<Vec<WorkspaceBuildTargets>, u8> {
    validate_test_package_selector(request_path, command_options, selected_package_name)?;

    let member_manifest = if command_options.json {
        load_workspace_selected_member_manifest_for_json(
            request_path,
            command_options,
            manifest,
            selected_package_name,
        )?
    } else {
        let (_, member_manifest) = resolve_selected_workspace_member_manifest(
            manifest,
            request_path,
            selected_package_name,
            "`ql test`",
            "--package",
        )?;
        member_manifest
    };

    Ok(vec![project_test_build_targets_from_manifest(
        request_path,
        command_options,
        &member_manifest,
        Some(manifest),
    )?])
}
