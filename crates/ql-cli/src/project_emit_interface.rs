use std::path::Path;

use ql_project::{load_project_manifest, package_name};

use crate::cli_utils::{
    normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error, validate_project_package_name,
};
use crate::project_interface_reporting::{
    check_package_interface_artifact, format_project_emit_interface_command_label,
    format_workspace_member_emit_rerun_command, report_package_interface_check,
    report_package_interface_failure, report_package_interface_manifest_failure,
    report_project_emit_interface_package_context_failure,
};
use crate::project_manifest_paths::{
    record_reference_failure_manifest, workspace_member_manifest_path,
};
use crate::project_reporting::report_workspace_member_failure;
use crate::project_targets::resolve_project_workspace_member_command_request_root;
use crate::project_workspace::{
    resolve_selected_workspace_member_manifest, select_workspace_members,
};

mod package_emit;
mod workspace_check;

use package_emit::emit_single_package_interface;
use workspace_check::check_workspace_member_interface;

fn report_package_interface_check_manifest_failure(manifest_path: &Path, changed_only: bool) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command =
        format_workspace_member_emit_rerun_command(&manifest_path, changed_only, true);
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!("hint: rerun `{rerun_command}` after fixing the package manifest");
}

pub(crate) fn project_emit_interface_path(
    path: &Path,
    output: Option<&Path>,
    selected_package_name: Option<&str>,
    changed_only: bool,
    check_only: bool,
) -> Result<(), u8> {
    if check_only && output.is_some() {
        eprintln!("error: `ql project emit-interface --check` does not support `--output`");
        return Err(1);
    }

    let emit_command_label =
        format_project_emit_interface_command_label(output, changed_only, false);
    let check_command_label = format_project_emit_interface_command_label(None, changed_only, true);
    let command_label = if check_only {
        check_command_label.as_str()
    } else {
        emit_command_label.as_str()
    };
    if let Some(package_name) = selected_package_name
        && let Err(message) = validate_project_package_name(package_name)
    {
        eprintln!("error: {command_label} {message}");
        return Err(1);
    }
    let request_root = if output.is_none() {
        resolve_project_workspace_member_command_request_root(path)
    } else {
        None
    };
    let manifest = load_project_manifest(request_root.as_deref().unwrap_or(path)).map_err(|error| {
        if let ql_project::ProjectError::ManifestNotFound { start } = &error {
            let command_label = if check_only {
                check_command_label.as_str()
            } else {
                emit_command_label.as_str()
            };
            eprintln!(
                "error: {} requires a package or workspace manifest; could not find `qlang.toml` starting from `{}`",
                command_label,
                normalize_path(start)
            );
            report_project_emit_interface_package_context_failure(
                path,
                output,
                changed_only,
                check_only,
            );
            return 1;
        }
        if check_only {
            if let Some(manifest_path) =
                package_missing_name_manifest_path_from_project_error(&error)
            {
                eprintln!(
                    "error: {} manifest `{}` does not declare `[package].name`",
                    check_command_label,
                    normalize_path(manifest_path)
                );
                report_package_interface_check_manifest_failure(manifest_path, changed_only);
                return 1;
            }
            if let Some(manifest_path) = package_check_manifest_path_from_project_error(&error) {
                eprintln!("error: {check_command_label} {error}");
                report_package_interface_check_manifest_failure(manifest_path, changed_only);
                return 1;
            }
        } else {
            if let Some(manifest_path) =
                package_missing_name_manifest_path_from_project_error(&error)
            {
                eprintln!(
                    "error: {} manifest `{}` does not declare `[package].name`",
                    emit_command_label,
                    normalize_path(manifest_path)
                );
                report_package_interface_manifest_failure(
                    manifest_path,
                    None,
                    output,
                    changed_only,
                    None,
                );
                return 1;
            }
            if let Some(manifest_path) = package_check_manifest_path_from_project_error(&error) {
                eprintln!("error: {emit_command_label} {error}");
                report_package_interface_manifest_failure(
                    manifest_path,
                    None,
                    output,
                    changed_only,
                    None,
                );
                return 1;
            }
        }
        eprintln!("error: {error}");
        1
    })?;

    if output.is_some() && manifest.workspace.is_some() {
        if let Some(selected_package_name) = selected_package_name {
            let (_, package_manifest) = resolve_selected_workspace_member_manifest(
                &manifest,
                path,
                selected_package_name,
                emit_command_label.as_str(),
                "--package",
            )?;
            emit_single_package_interface(
                &package_manifest.manifest_path,
                &package_manifest.manifest_path,
                None,
                output,
                emit_command_label.as_str(),
                changed_only,
            )?;
            return Ok(());
        }
    }

    if manifest.package.is_some() {
        if let Some(selected_package_name) = selected_package_name {
            let actual_package_name = package_name(&manifest).map_err(|error| {
                eprintln!("error: {command_label} {error}");
                if check_only {
                    report_package_interface_check_manifest_failure(
                        &manifest.manifest_path,
                        changed_only,
                    );
                } else {
                    report_package_interface_manifest_failure(
                        &manifest.manifest_path,
                        None,
                        output,
                        changed_only,
                        None,
                    );
                }
                1
            })?;
            if actual_package_name != selected_package_name {
                eprintln!(
                    "error: {command_label} package selector expected `{selected_package_name}` but `{}` resolves to package `{actual_package_name}`",
                    normalize_path(path)
                );
                return Err(1);
            }
        }
        if check_only {
            let result = match check_package_interface_artifact(
                &manifest,
                check_command_label.as_str(),
                changed_only,
            ) {
                Ok(result) => result,
                Err(code) => {
                    report_package_interface_failure(
                        &manifest.manifest_path,
                        None,
                        None,
                        changed_only,
                        None,
                    );
                    return Err(code);
                }
            };
            return report_package_interface_check(
                result,
                None,
                check_command_label.as_str(),
                changed_only,
            );
        }
        emit_single_package_interface(
            path,
            &manifest.manifest_path,
            None,
            output,
            emit_command_label.as_str(),
            changed_only,
        )?;
        return Ok(());
    }

    if output.is_some() {
        eprintln!("error: `ql project emit-interface --output` only supports package manifests");
        return Err(1);
    }

    let Some(_) = &manifest.workspace else {
        eprintln!("error: `ql project emit-interface` requires `[package]` or `[workspace]`");
        return Err(1);
    };

    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let selected_members = select_workspace_members(
        &manifest,
        path,
        selected_package_name,
        command_label,
        "--package",
    )?;
    let mut failing_member_count = 0usize;
    let mut emission_failure_count = 0usize;
    let mut first_failing_member_manifest = None;
    for member in &selected_members {
        let member_manifest_path = workspace_member_manifest_path(&manifest_dir.join(member));
        if check_only {
            if !check_workspace_member_interface(
                &member_manifest_path,
                changed_only,
                check_command_label.as_str(),
                &mut first_failing_member_manifest,
            ) {
                failing_member_count += 1;
            }
        } else {
            let member_manifest = match load_project_manifest(&manifest_dir.join(member)) {
                Ok(manifest) => manifest,
                Err(error) => {
                    if let Some(manifest_path) =
                        package_missing_name_manifest_path_from_project_error(&error)
                    {
                        eprintln!(
                            "error: {} manifest `{}` does not declare `[package].name`",
                            emit_command_label,
                            normalize_path(manifest_path)
                        );
                        report_package_interface_manifest_failure(
                            manifest_path,
                            Some(manifest_path),
                            None,
                            changed_only,
                            None,
                        );
                    } else if let Some(manifest_path) =
                        package_check_manifest_path_from_project_error(&error)
                    {
                        eprintln!("error: {emit_command_label} {error}");
                        report_package_interface_manifest_failure(
                            manifest_path,
                            Some(manifest_path),
                            None,
                            changed_only,
                            None,
                        );
                    } else {
                        eprintln!("error: {error}");
                        let rerun_command = format_workspace_member_emit_rerun_command(
                            &normalize_path(&member_manifest_path),
                            changed_only,
                            check_only,
                        );
                        let rerun_hint = format!(
                            "hint: rerun `{rerun_command}` after fixing the workspace member manifest"
                        );
                        report_workspace_member_failure(
                            &member_manifest_path,
                            Some(rerun_hint.as_str()),
                        );
                    }
                    emission_failure_count += 1;
                    record_reference_failure_manifest(
                        &mut first_failing_member_manifest,
                        member_manifest_path.clone(),
                    );
                    continue;
                }
            };
            match emit_single_package_interface(
                &member_manifest.manifest_path,
                &member_manifest.manifest_path,
                Some(&member_manifest.manifest_path),
                None,
                emit_command_label.as_str(),
                changed_only,
            ) {
                Ok(()) => {}
                Err(_) => {
                    emission_failure_count += 1;
                    record_reference_failure_manifest(
                        &mut first_failing_member_manifest,
                        member_manifest.manifest_path.clone(),
                    );
                }
            }
        }
    }

    if check_only && failing_member_count > 0 {
        eprintln!("error: {check_command_label} found {failing_member_count} failing member(s)");
        if failing_member_count > 1 {
            if let Some(path) = &first_failing_member_manifest {
                eprintln!(
                    "note: first failing member manifest: {}",
                    normalize_path(path)
                );
            }
        }
        return Err(1);
    }

    if !check_only && emission_failure_count > 0 {
        eprintln!("error: {emit_command_label} found {emission_failure_count} failing member(s)");
        if emission_failure_count > 1 {
            if let Some(path) = &first_failing_member_manifest {
                eprintln!(
                    "note: first failing member manifest: {}",
                    normalize_path(path)
                );
            }
        }
        return Err(1);
    }

    Ok(())
}
