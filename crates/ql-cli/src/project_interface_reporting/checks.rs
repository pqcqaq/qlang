use std::path::{Path, PathBuf};

use ql_project::{
    InterfaceArtifactStaleReason, InterfaceArtifactStatus, default_interface_path,
    interface_artifact_stale_reasons, interface_artifact_status, interface_artifact_status_detail,
};

use super::commands::format_emit_interface_regenerate_command;
use crate::cli_utils::normalize_path;
use crate::project_reporting::report_interface_artifact_failure;

pub(crate) enum CheckPackageInterfaceResult {
    Ok(PathBuf),
    UpToDate(PathBuf),
    Invalid {
        path: PathBuf,
        status: InterfaceArtifactStatus,
        manifest_path: PathBuf,
        detail: Option<String>,
        stale_reasons: Vec<InterfaceArtifactStaleReason>,
    },
}

pub(crate) fn check_package_interface_artifact(
    manifest: &ql_project::ProjectManifest,
    command_label: &str,
    changed_only: bool,
) -> Result<CheckPackageInterfaceResult, u8> {
    let output_path = default_interface_path(manifest).map_err(|error| {
        eprintln!("error: {command_label} {error}");
        1
    })?;
    let status = interface_artifact_status(manifest, &output_path);
    if status == InterfaceArtifactStatus::Valid {
        return Ok(if changed_only {
            CheckPackageInterfaceResult::UpToDate(output_path)
        } else {
            CheckPackageInterfaceResult::Ok(output_path)
        });
    }
    if status != InterfaceArtifactStatus::Valid {
        let detail = interface_artifact_status_detail(&output_path, status);
        let stale_reasons = if status == InterfaceArtifactStatus::Stale {
            interface_artifact_stale_reasons(manifest, &output_path)
        } else {
            Vec::new()
        };
        return Ok(CheckPackageInterfaceResult::Invalid {
            path: output_path,
            status,
            manifest_path: manifest.manifest_path.clone(),
            detail,
            stale_reasons,
        });
    }
    Ok(CheckPackageInterfaceResult::Ok(output_path))
}

pub(crate) fn report_package_interface_check(
    result: CheckPackageInterfaceResult,
    workspace_member_manifest_path: Option<&Path>,
    command_label: &str,
    changed_only: bool,
) -> Result<(), u8> {
    match result {
        CheckPackageInterfaceResult::Ok(path) => {
            println!("ok interface: {}", path.display());
            Ok(())
        }
        CheckPackageInterfaceResult::UpToDate(path) => {
            println!("up-to-date interface: {}", path.display());
            Ok(())
        }
        CheckPackageInterfaceResult::Invalid {
            path,
            status,
            manifest_path,
            detail,
            stale_reasons,
        } => {
            let manifest_path = normalize_path(&manifest_path);
            let error_line = format!(
                "error: {command_label} interface artifact `{}` is {}",
                normalize_path(&path),
                status.label()
            );
            let package_note = format!("note: failing package manifest: {manifest_path}");
            let workspace_member_note = workspace_member_manifest_path.map(|path| {
                format!(
                    "note: failing workspace member manifest: {}",
                    normalize_path(path)
                )
            });
            let mut notes = vec![package_note.as_str()];
            if let Some(workspace_member_note) = workspace_member_note.as_deref() {
                notes.push(workspace_member_note);
            }
            let regenerate_command =
                format_emit_interface_regenerate_command(&manifest_path, changed_only);
            let hint_line = format!("hint: rerun `{regenerate_command}` to regenerate it");
            report_interface_artifact_failure(
                &error_line,
                detail.as_deref(),
                &stale_reasons,
                &notes,
                &hint_line,
            );
            Err(1)
        }
    }
}
