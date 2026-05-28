use std::path::Path;

use ql_project::{ProjectManifest, WorkspaceBuildTargets, load_project_manifest, package_name};

use crate::project_workspace::resolve_selected_workspace_member_manifest;
use crate::test_command::TestCommandOptions;

use super::member_targets::project_test_build_targets_from_manifest;
use super::package_selector::{report_package_selector_mismatch, validate_test_package_selector};
use super::project_errors::report_ql_test_project_preflight_error;
use super::workspace_selector::load_workspace_selected_member_manifest_for_json;

pub(super) fn load_selected_project_test_member(
    request_path: &Path,
    project_path: &Path,
    command_options: &TestCommandOptions,
    selected_package_name: &str,
) -> Result<Vec<WorkspaceBuildTargets>, u8> {
    let manifest = match load_project_manifest(project_path) {
        Ok(manifest) => manifest,
        Err(error) => {
            return Err(report_ql_test_project_preflight_error(
                request_path,
                command_options,
                &error,
                "manifest-load",
            ));
        }
    };

    if manifest.workspace.is_some() {
        return load_workspace_selected_test_member(
            request_path,
            command_options,
            &manifest,
            selected_package_name,
        );
    }

    load_package_selected_test_member(
        request_path,
        command_options,
        &manifest,
        selected_package_name,
    )
}

fn load_workspace_selected_test_member(
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

fn load_package_selected_test_member(
    request_path: &Path,
    command_options: &TestCommandOptions,
    manifest: &ProjectManifest,
    selected_package_name: &str,
) -> Result<Vec<WorkspaceBuildTargets>, u8> {
    validate_test_package_selector(request_path, command_options, selected_package_name)?;
    let current_package_name = match package_name(manifest) {
        Ok(package_name) => package_name,
        Err(error) => {
            return Err(report_ql_test_project_preflight_error(
                request_path,
                command_options,
                &error,
                "package-selection",
            ));
        }
    };
    if current_package_name != selected_package_name {
        report_package_selector_mismatch(request_path, command_options, selected_package_name);
        return Err(1);
    }
    Ok(vec![project_test_build_targets_from_manifest(
        request_path,
        command_options,
        manifest,
        None,
    )?])
}
