use std::path::Path;

use ql_project::{
    WorkspaceBuildTargets, discover_package_build_targets, discover_workspace_build_targets,
    load_project_manifest, package_name,
};

use crate::build_reporting::build_json_project_error;
use crate::project_targets::load_workspace_build_targets_for_command_from_request_root;
use crate::project_workspace::resolve_selected_workspace_member_manifest;
use crate::test_command::TestCommandOptions;
use crate::test_reporting::render_test_json_preflight_failure_report;

use super::package_selector::{
    load_workspace_selected_member_manifest_for_json, report_package_selector_mismatch,
    validate_test_package_selector,
};
use super::project_errors::{report_ql_test_project_error, report_ql_test_project_preflight_error};

pub(super) fn load_project_test_members(
    request_path: &Path,
    project_path: &Path,
    command_options: &TestCommandOptions,
) -> Result<Vec<WorkspaceBuildTargets>, u8> {
    let Some(selected_package_name) = command_options.package_name.as_deref() else {
        return load_all_project_test_members(request_path, project_path, command_options);
    };

    let manifest = match load_project_manifest(project_path) {
        Ok(manifest) => manifest,
        Err(error) => {
            if command_options.json {
                print!(
                    "{}",
                    render_test_json_preflight_failure_report(
                        request_path,
                        command_options,
                        build_json_project_error(request_path, &error, "manifest-load"),
                    )
                );
            } else {
                report_ql_test_project_error(&error);
            }
            return Err(1);
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

fn load_all_project_test_members(
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

fn load_workspace_selected_test_member(
    request_path: &Path,
    command_options: &TestCommandOptions,
    manifest: &ql_project::ProjectManifest,
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
    manifest: &ql_project::ProjectManifest,
    selected_package_name: &str,
) -> Result<Vec<WorkspaceBuildTargets>, u8> {
    validate_test_package_selector(request_path, command_options, selected_package_name)?;
    let current_package_name = match package_name(manifest) {
        Ok(package_name) => package_name,
        Err(error) => {
            if command_options.json {
                print!(
                    "{}",
                    render_test_json_preflight_failure_report(
                        request_path,
                        command_options,
                        build_json_project_error(request_path, &error, "package-selection"),
                    )
                );
            } else {
                eprintln!("error: `ql test` {error}");
            }
            return Err(1);
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

fn project_test_build_targets_from_manifest(
    request_path: &Path,
    command_options: &TestCommandOptions,
    manifest: &ql_project::ProjectManifest,
    workspace_manifest: Option<&ql_project::ProjectManifest>,
) -> Result<WorkspaceBuildTargets, u8> {
    let workspace_default_profile = workspace_manifest
        .and_then(|manifest| manifest.profile.as_ref().map(|profile| profile.default));
    Ok(WorkspaceBuildTargets {
        member_manifest_path: manifest.manifest_path.clone(),
        package_name: match package_name(manifest) {
            Ok(package_name) => package_name.to_owned(),
            Err(error) => {
                return Err(report_ql_test_project_preflight_error(
                    request_path,
                    command_options,
                    &error,
                    "target-discovery",
                ));
            }
        },
        default_profile: manifest
            .profile
            .as_ref()
            .map(|profile| profile.default)
            .or(workspace_default_profile),
        targets: match discover_package_build_targets(manifest) {
            Ok(targets) => targets,
            Err(error) => {
                return Err(report_ql_test_project_preflight_error(
                    request_path,
                    command_options,
                    &error,
                    "target-discovery",
                ));
            }
        },
    })
}
