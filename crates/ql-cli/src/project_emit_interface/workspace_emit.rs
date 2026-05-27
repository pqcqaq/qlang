use std::path::{Path, PathBuf};

use ql_project::load_project_manifest;

use crate::cli_utils::{
    normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error,
};
use crate::project_interface_reporting::{
    format_workspace_member_emit_rerun_command, report_package_interface_manifest_failure,
};
use crate::project_manifest_paths::record_reference_failure_manifest;
use crate::project_reporting::report_workspace_member_failure;

use super::package_emit::emit_single_package_interface;

pub(super) fn emit_workspace_member_interface(
    member_manifest_path: &Path,
    changed_only: bool,
    emit_command_label: &str,
    first_failing_member_manifest: &mut Option<PathBuf>,
) -> bool {
    let member_manifest = match load_project_manifest(member_manifest_path) {
        Ok(manifest) => manifest,
        Err(error) => {
            report_workspace_member_emit_manifest_load_error(
                member_manifest_path,
                &error,
                changed_only,
                emit_command_label,
            );
            record_reference_failure_manifest(
                first_failing_member_manifest,
                member_manifest_path.to_path_buf(),
            );
            return false;
        }
    };

    match emit_single_package_interface(
        &member_manifest.manifest_path,
        &member_manifest.manifest_path,
        Some(&member_manifest.manifest_path),
        None,
        emit_command_label,
        changed_only,
    ) {
        Ok(()) => true,
        Err(_) => {
            record_reference_failure_manifest(
                first_failing_member_manifest,
                member_manifest.manifest_path.clone(),
            );
            false
        }
    }
}

fn report_workspace_member_emit_manifest_load_error(
    member_manifest_path: &Path,
    error: &ql_project::ProjectError,
    changed_only: bool,
    emit_command_label: &str,
) {
    if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error) {
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
    } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(error) {
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
            &normalize_path(member_manifest_path),
            changed_only,
            false,
        );
        let rerun_hint =
            format!("hint: rerun `{rerun_command}` after fixing the workspace member manifest");
        report_workspace_member_failure(member_manifest_path, Some(rerun_hint.as_str()));
    }
}
