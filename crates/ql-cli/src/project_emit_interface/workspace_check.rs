use std::path::{Path, PathBuf};

use ql_project::{load_project_manifest, package_name};

use crate::cli_utils::{
    normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error,
};
use crate::project_interface_reporting::{
    check_package_interface_artifact, format_workspace_member_emit_rerun_command,
    report_package_interface_check,
};
use crate::project_manifest_paths::record_reference_failure_manifest;
use crate::project_reporting::report_workspace_member_failure;

pub(super) fn check_workspace_member_interface(
    member_manifest_path: &Path,
    changed_only: bool,
    check_command_label: &str,
    first_failing_member_manifest: &mut Option<PathBuf>,
) -> bool {
    let member_manifest = match load_project_manifest(member_manifest_path) {
        Ok(manifest) => manifest,
        Err(error) => {
            report_workspace_member_check_manifest_load_error(
                member_manifest_path,
                &error,
                changed_only,
                check_command_label,
            );
            record_reference_failure_manifest(
                first_failing_member_manifest,
                member_manifest_path.to_path_buf(),
            );
            return false;
        }
    };

    if let Err(error) = package_name(&member_manifest) {
        eprintln!("error: {check_command_label} {error}");
        report_workspace_member_package_interface_check_manifest_failure(
            &member_manifest.manifest_path,
            changed_only,
        );
        record_reference_failure_manifest(
            first_failing_member_manifest,
            member_manifest.manifest_path.clone(),
        );
        return false;
    }

    let result =
        match check_package_interface_artifact(&member_manifest, check_command_label, changed_only)
        {
            Ok(result) => result,
            Err(_) => {
                let rerun_command = format_workspace_member_emit_rerun_command(
                    &normalize_path(&member_manifest.manifest_path),
                    changed_only,
                    true,
                );
                let rerun_hint = format!(
                    "hint: rerun `{rerun_command}` after fixing the workspace member manifest"
                );
                report_workspace_member_failure(
                    &member_manifest.manifest_path,
                    Some(rerun_hint.as_str()),
                );
                record_reference_failure_manifest(
                    first_failing_member_manifest,
                    member_manifest.manifest_path.clone(),
                );
                return false;
            }
        };

    if report_package_interface_check(
        result,
        Some(&member_manifest.manifest_path),
        check_command_label,
        changed_only,
    )
    .is_err()
    {
        record_reference_failure_manifest(
            first_failing_member_manifest,
            member_manifest.manifest_path.clone(),
        );
        return false;
    }

    true
}

fn report_workspace_member_check_manifest_load_error(
    member_manifest_path: &Path,
    error: &ql_project::ProjectError,
    changed_only: bool,
    check_command_label: &str,
) {
    if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error) {
        eprintln!(
            "error: {} manifest `{}` does not declare `[package].name`",
            check_command_label,
            normalize_path(manifest_path)
        );
        report_workspace_member_package_interface_check_manifest_failure(
            manifest_path,
            changed_only,
        );
    } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(error) {
        eprintln!("error: {check_command_label} {error}");
        report_workspace_member_package_interface_check_manifest_failure(
            manifest_path,
            changed_only,
        );
    } else {
        eprintln!("error: {error}");
        let rerun_command = format_workspace_member_emit_rerun_command(
            &normalize_path(member_manifest_path),
            changed_only,
            true,
        );
        let rerun_hint =
            format!("hint: rerun `{rerun_command}` after fixing the workspace member manifest");
        report_workspace_member_failure(member_manifest_path, Some(rerun_hint.as_str()));
    }
}

fn report_workspace_member_package_interface_check_manifest_failure(
    manifest_path: &Path,
    changed_only: bool,
) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command =
        format_workspace_member_emit_rerun_command(&manifest_path, changed_only, true);
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!("note: failing workspace member manifest: {manifest_path}");
    eprintln!("hint: rerun `{rerun_command}` after fixing the package manifest");
}
