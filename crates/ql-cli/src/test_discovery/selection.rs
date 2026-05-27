use std::path::Path;

use ql_project::{
    WorkspaceBuildTargets, discover_package_build_targets, discover_workspace_build_targets,
    load_project_manifest, package_name,
};

use crate::build_reporting::build_json_project_error;
use crate::cli_utils::{
    normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error, validate_project_package_name,
};
use crate::project_targets::load_workspace_build_targets_for_command_from_request_root;
use crate::project_workspace::{
    WorkspaceMemberLookupError, render_workspace_member_lookup_error,
    resolve_selected_workspace_member_manifest, resolve_workspace_member_entry_by_package_name,
};
use crate::test_command::TestCommandOptions;
use crate::test_reporting::{
    render_test_json_preflight_failure_report, render_test_json_preflight_message_report,
};

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

fn load_workspace_selected_member_manifest_for_json(
    request_path: &Path,
    command_options: &TestCommandOptions,
    manifest: &ql_project::ProjectManifest,
    selected_package_name: &str,
) -> Result<ql_project::ProjectManifest, u8> {
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

fn validate_test_package_selector(
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

fn report_package_selector_mismatch(
    request_path: &Path,
    command_options: &TestCommandOptions,
    selected_package_name: &str,
) {
    let normalized_path = normalize_path(request_path);
    if command_options.json {
        print!(
            "{}",
            render_test_json_preflight_message_report(
                request_path,
                command_options,
                "selector",
                "package-selection",
                format!("package selector matched no workspace members under `{normalized_path}`"),
                Some(format!("package `{selected_package_name}`")),
                Some(0),
            )
        );
    } else {
        eprintln!(
            "error: `ql test` package selector matched no workspace members under `{normalized_path}`"
        );
        eprintln!("note: selector: package `{selected_package_name}`");
        eprintln!(
            "hint: rerun `ql test {normalized_path}` to inspect all workspace members, or adjust `--package`"
        );
    }
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

fn report_ql_test_project_preflight_error(
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

fn report_ql_test_project_error(error: &ql_project::ProjectError) -> u8 {
    if let ql_project::ProjectError::ManifestNotFound { start } = error {
        eprintln!(
            "error: `ql test` requires a package or workspace manifest; could not find `qlang.toml` starting from `{}`",
            normalize_path(start)
        );
    } else if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error)
    {
        eprintln!(
            "error: `ql test` manifest `{}` does not declare `[package].name`",
            normalize_path(manifest_path)
        );
    } else if let ql_project::ProjectError::PackageSourceRootNotFound { path } = error {
        eprintln!(
            "error: `ql test` package source directory `{}` does not exist",
            normalize_path(path)
        );
    } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(error) {
        eprintln!("error: `ql test` {error}");
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(manifest_path)
        );
    } else {
        eprintln!("error: `ql test` {error}");
    }
    1
}
