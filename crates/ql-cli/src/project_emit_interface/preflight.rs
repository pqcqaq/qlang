use std::path::Path;

use ql_project::{ProjectManifest, load_project_manifest};

use crate::cli_utils::{
    normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error, validate_project_package_name,
};
use crate::project_interface_reporting::{
    format_project_emit_interface_command_label, format_workspace_member_emit_rerun_command,
    report_package_interface_manifest_failure,
    report_project_emit_interface_package_context_failure,
};
use crate::project_targets::resolve_project_workspace_member_command_request_root;

pub(super) struct ProjectEmitInterfaceLabels {
    pub(super) emit: String,
    pub(super) check: String,
    pub(super) active: String,
}

pub(super) fn project_emit_interface_labels(
    output: Option<&Path>,
    changed_only: bool,
    check_only: bool,
) -> ProjectEmitInterfaceLabels {
    let emit = format_project_emit_interface_command_label(output, changed_only, false);
    let check = format_project_emit_interface_command_label(None, changed_only, true);
    let active = if check_only {
        check.clone()
    } else {
        emit.clone()
    };
    ProjectEmitInterfaceLabels {
        emit,
        check,
        active,
    }
}

pub(super) fn validate_project_emit_interface_package_selector(
    selected_package_name: Option<&str>,
    command_label: &str,
) -> Result<(), u8> {
    if let Some(package_name) = selected_package_name
        && let Err(message) = validate_project_package_name(package_name)
    {
        eprintln!("error: {command_label} {message}");
        return Err(1);
    }
    Ok(())
}

pub(super) fn load_project_emit_interface_manifest(
    path: &Path,
    output: Option<&Path>,
    changed_only: bool,
    check_only: bool,
    labels: &ProjectEmitInterfaceLabels,
) -> Result<ProjectManifest, u8> {
    let request_root = if output.is_none() {
        resolve_project_workspace_member_command_request_root(path)
    } else {
        None
    };
    load_project_manifest(request_root.as_deref().unwrap_or(path)).map_err(|error| {
        report_project_emit_interface_manifest_load_error(
            path,
            output,
            changed_only,
            check_only,
            labels,
            &error,
        )
    })
}

pub(super) fn report_package_interface_check_manifest_failure(
    manifest_path: &Path,
    changed_only: bool,
) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command =
        format_workspace_member_emit_rerun_command(&manifest_path, changed_only, true);
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!("hint: rerun `{rerun_command}` after fixing the package manifest");
}

fn report_project_emit_interface_manifest_load_error(
    path: &Path,
    output: Option<&Path>,
    changed_only: bool,
    check_only: bool,
    labels: &ProjectEmitInterfaceLabels,
    error: &ql_project::ProjectError,
) -> u8 {
    if let ql_project::ProjectError::ManifestNotFound { start } = error {
        eprintln!(
            "error: {} requires a package or workspace manifest; could not find `qlang.toml` starting from `{}`",
            labels.active,
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
        if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error) {
            eprintln!(
                "error: {} manifest `{}` does not declare `[package].name`",
                labels.check,
                normalize_path(manifest_path)
            );
            report_package_interface_check_manifest_failure(manifest_path, changed_only);
            return 1;
        }
        if let Some(manifest_path) = package_check_manifest_path_from_project_error(error) {
            eprintln!("error: {} {error}", labels.check);
            report_package_interface_check_manifest_failure(manifest_path, changed_only);
            return 1;
        }
    } else {
        if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error) {
            eprintln!(
                "error: {} manifest `{}` does not declare `[package].name`",
                labels.emit,
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
        if let Some(manifest_path) = package_check_manifest_path_from_project_error(error) {
            eprintln!("error: {} {error}", labels.emit);
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
}
