use std::path::{Path, PathBuf};

use ql_project::{
    InterfaceArtifactStaleReason, InterfaceArtifactStatus, default_interface_path,
    interface_artifact_stale_reasons, interface_artifact_status, interface_artifact_status_detail,
};

use crate::cli_utils::normalize_path;
use crate::project_interfaces::EmitPackageInterfaceResult;
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

pub(crate) fn report_emit_interface_result(result: EmitPackageInterfaceResult) {
    match result {
        EmitPackageInterfaceResult::Wrote(path) => {
            println!("wrote interface: {}", path.display());
        }
        EmitPackageInterfaceResult::UpToDate(path) => {
            println!("up-to-date interface: {}", path.display());
        }
    }
}

pub(crate) fn report_project_emit_interface_package_context_failure(
    path: &Path,
    requested_output_path: Option<&Path>,
    changed_only: bool,
    check_only: bool,
) {
    let command = if check_only {
        "ql project emit-interface --check"
    } else {
        "ql project emit-interface"
    };
    let action = if check_only { "checks" } else { "emits" };
    eprintln!(
        "note: `{command}` only {action} package interfaces for packages or workspace members discoverable from `qlang.toml`"
    );
    let normalized_path = normalize_path(path);
    let rerun_command = format_project_emit_interface_command(
        Some(normalized_path.as_str()),
        requested_output_path,
        changed_only,
        check_only,
    );
    eprintln!("hint: rerun `{rerun_command}` after adding `qlang.toml` for this path");
}

pub(crate) fn report_package_interface_failure(
    manifest_path: &Path,
    workspace_member_manifest_path: Option<&Path>,
    requested_output_path: Option<&Path>,
    changed_only: bool,
    additional_context_note: Option<&str>,
) {
    let manifest_path = normalize_path(manifest_path);
    eprintln!("note: failing package manifest: {manifest_path}");
    if let Some(workspace_member_manifest_path) = workspace_member_manifest_path {
        eprintln!(
            "note: failing workspace member manifest: {}",
            normalize_path(workspace_member_manifest_path)
        );
    }
    if let Some(additional_context_note) = additional_context_note {
        eprintln!("{additional_context_note}");
    }
    let rerun_command =
        format_emit_interface_rerun_command(&manifest_path, requested_output_path, changed_only);
    eprintln!(
        "hint: rerun `{}` after fixing the package interface error",
        rerun_command
    );
}

pub(crate) fn report_package_interface_source_failure(
    manifest_path: &Path,
    workspace_member_manifest_path: Option<&Path>,
    requested_output_path: Option<&Path>,
    changed_only: bool,
    additional_context_note: Option<&str>,
) {
    let manifest_path = normalize_path(manifest_path);
    eprintln!("note: failing package manifest: {manifest_path}");
    if let Some(workspace_member_manifest_path) = workspace_member_manifest_path {
        eprintln!(
            "note: failing workspace member manifest: {}",
            normalize_path(workspace_member_manifest_path)
        );
    }
    if let Some(additional_context_note) = additional_context_note {
        eprintln!("{additional_context_note}");
    }
    let rerun_command =
        format_emit_interface_rerun_command(&manifest_path, requested_output_path, changed_only);
    eprintln!(
        "hint: rerun `{}` after fixing the package sources",
        rerun_command
    );
}

pub(crate) fn report_package_interface_manifest_failure(
    manifest_path: &Path,
    workspace_member_manifest_path: Option<&Path>,
    requested_output_path: Option<&Path>,
    changed_only: bool,
    additional_context_note: Option<&str>,
) {
    let manifest_path = normalize_path(manifest_path);
    eprintln!("note: failing package manifest: {manifest_path}");
    if let Some(workspace_member_manifest_path) = workspace_member_manifest_path {
        eprintln!(
            "note: failing workspace member manifest: {}",
            normalize_path(workspace_member_manifest_path)
        );
    }
    if let Some(additional_context_note) = additional_context_note {
        eprintln!("{additional_context_note}");
    }
    let rerun_command =
        format_emit_interface_rerun_command(&manifest_path, requested_output_path, changed_only);
    eprintln!(
        "hint: rerun `{}` after fixing the package manifest",
        rerun_command
    );
}

pub(crate) fn report_package_interface_source_root_failure(
    manifest_path: &Path,
    workspace_member_manifest_path: Option<&Path>,
    source_root: &Path,
    requested_output_path: Option<&Path>,
    changed_only: bool,
    additional_context_note: Option<&str>,
) {
    let manifest_path = normalize_path(manifest_path);
    eprintln!("note: failing package manifest: {manifest_path}");
    if let Some(workspace_member_manifest_path) = workspace_member_manifest_path {
        eprintln!(
            "note: failing workspace member manifest: {}",
            normalize_path(workspace_member_manifest_path)
        );
    }
    eprintln!(
        "note: failing package source root: {}",
        normalize_path(source_root)
    );
    if let Some(additional_context_note) = additional_context_note {
        eprintln!("{additional_context_note}");
    }
    let rerun_command =
        format_emit_interface_rerun_command(&manifest_path, requested_output_path, changed_only);
    eprintln!(
        "hint: rerun `{}` after fixing the package source root",
        rerun_command
    );
}

pub(crate) fn report_package_interface_no_sources_failure(
    manifest_path: &Path,
    workspace_member_manifest_path: Option<&Path>,
    source_root: &Path,
    requested_output_path: Option<&Path>,
    changed_only: bool,
    additional_context_note: Option<&str>,
) {
    let manifest_path = normalize_path(manifest_path);
    eprintln!("note: failing package manifest: {manifest_path}");
    if let Some(workspace_member_manifest_path) = workspace_member_manifest_path {
        eprintln!(
            "note: failing workspace member manifest: {}",
            normalize_path(workspace_member_manifest_path)
        );
    }
    eprintln!(
        "note: failing package source root: {}",
        normalize_path(source_root)
    );
    if let Some(additional_context_note) = additional_context_note {
        eprintln!("{additional_context_note}");
    }
    let rerun_command =
        format_emit_interface_rerun_command(&manifest_path, requested_output_path, changed_only);
    eprintln!(
        "hint: rerun `{}` after adding package source files",
        rerun_command
    );
}

pub(crate) fn report_package_interface_output_failure(
    manifest_path: &Path,
    workspace_member_manifest_path: Option<&Path>,
    output_path: &Path,
    requested_output_path: Option<&Path>,
    changed_only: bool,
    additional_context_note: Option<&str>,
) {
    let manifest_path = normalize_path(manifest_path);
    eprintln!("note: failing package manifest: {manifest_path}");
    if let Some(workspace_member_manifest_path) = workspace_member_manifest_path {
        eprintln!(
            "note: failing workspace member manifest: {}",
            normalize_path(workspace_member_manifest_path)
        );
    }
    eprintln!(
        "note: failing interface output path: {}",
        normalize_path(output_path)
    );
    if let Some(additional_context_note) = additional_context_note {
        eprintln!("{additional_context_note}");
    }
    let rerun_command =
        format_emit_interface_rerun_command(&manifest_path, requested_output_path, changed_only);
    eprintln!(
        "hint: rerun `{}` after fixing the interface output path",
        rerun_command
    );
}

fn format_emit_interface_rerun_command(
    manifest_path: &str,
    requested_output_path: Option<&Path>,
    changed_only: bool,
) -> String {
    format_project_emit_interface_command(
        Some(manifest_path),
        requested_output_path,
        changed_only,
        false,
    )
}

pub(crate) fn format_project_emit_interface_command(
    manifest_path: Option<&str>,
    requested_output_path: Option<&Path>,
    changed_only: bool,
    check_only: bool,
) -> String {
    let mut command = String::from("ql project emit-interface");
    if let Some(manifest_path) = manifest_path {
        command.push(' ');
        command.push_str(manifest_path);
    }
    if changed_only {
        command.push_str(" --changed-only");
    }
    if check_only {
        command.push_str(" --check");
    }
    if let Some(output_path) = requested_output_path {
        command.push_str(&format!(" --output {}", normalize_path(output_path)));
    }
    command
}

pub(crate) fn format_project_emit_interface_command_label(
    requested_output_path: Option<&Path>,
    changed_only: bool,
    check_only: bool,
) -> String {
    format!(
        "`{}`",
        format_project_emit_interface_command(
            None,
            requested_output_path,
            changed_only,
            check_only,
        )
    )
}

fn format_emit_interface_regenerate_command(manifest_path: &str, changed_only: bool) -> String {
    if changed_only {
        format!("ql project emit-interface {manifest_path} --changed-only")
    } else {
        format!("ql project emit-interface {manifest_path}")
    }
}

pub(crate) fn format_workspace_member_emit_rerun_command(
    manifest_path: &str,
    changed_only: bool,
    check_only: bool,
) -> String {
    format_project_emit_interface_command(Some(manifest_path), None, changed_only, check_only)
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

#[cfg(test)]
#[path = "project_interface_reporting_tests.rs"]
mod tests;
