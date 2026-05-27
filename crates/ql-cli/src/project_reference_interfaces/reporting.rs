use std::path::Path;

use ql_project::{
    InterfaceArtifactStatus, interface_artifact_stale_reasons, interface_artifact_status_detail,
};

use crate::check_reporting::format_check_command_label;
use crate::cli_utils::normalize_path;
use crate::project_reporting::report_interface_artifact_failure;

pub(super) fn report_reference_manifest_issue(
    sync_interfaces: bool,
    reference: &str,
    owner_manifest_path: &Path,
    reference_manifest_path: &Path,
    error: &ql_project::ProjectError,
) {
    let check_command_label = format_check_command_label(sync_interfaces);
    eprintln!("error: {check_command_label} failed to load referenced package `{reference}`");
    eprintln!("detail: {error}");
    eprintln!(
        "note: failing reference manifest: {}",
        normalize_path(reference_manifest_path)
    );
    eprintln!(
        "hint: fix the reference in `{}` or repair `{}`",
        normalize_path(owner_manifest_path),
        normalize_path(reference_manifest_path)
    );
}

pub(super) fn report_reference_manifest_issue_for_command(
    command_label: &str,
    reference: &str,
    owner_manifest_path: &Path,
    reference_manifest_path: &Path,
    error: &ql_project::ProjectError,
) {
    eprintln!("error: {command_label} failed to load referenced package `{reference}`");
    eprintln!("detail: {error}");
    eprintln!(
        "note: failing reference manifest: {}",
        normalize_path(reference_manifest_path)
    );
    eprintln!(
        "hint: fix the reference in `{}` or repair `{}`",
        normalize_path(owner_manifest_path),
        normalize_path(reference_manifest_path)
    );
}

pub(super) fn report_reference_interface_artifact_issue(
    dependency_manifest: &ql_project::ProjectManifest,
    reference: &str,
    dependency_package: &str,
    owner_manifest_path: &Path,
    interface_path: &Path,
    status: InterfaceArtifactStatus,
) {
    let check_command_label = format_check_command_label(false);
    let error_line = match status {
        InterfaceArtifactStatus::Missing => format!(
            "error: {check_command_label} referenced package `{dependency_package}` is missing interface artifact `{}`",
            normalize_path(interface_path)
        ),
        InterfaceArtifactStatus::Unreadable => format!(
            "error: {check_command_label} referenced package `{dependency_package}` has unreadable interface artifact `{}`",
            normalize_path(interface_path)
        ),
        InterfaceArtifactStatus::Invalid => format!(
            "error: {check_command_label} referenced package `{dependency_package}` has invalid interface artifact `{}`",
            normalize_path(interface_path)
        ),
        InterfaceArtifactStatus::Stale => format!(
            "error: {check_command_label} referenced package `{dependency_package}` has stale interface artifact `{}`",
            normalize_path(interface_path)
        ),
        InterfaceArtifactStatus::Valid => return,
    };
    let detail = match status {
        InterfaceArtifactStatus::Unreadable => {
            interface_artifact_status_detail(interface_path, InterfaceArtifactStatus::Unreadable)
        }
        InterfaceArtifactStatus::Invalid => {
            interface_artifact_status_detail(interface_path, InterfaceArtifactStatus::Invalid)
        }
        _ => None,
    };
    let stale_reasons = if status == InterfaceArtifactStatus::Stale {
        interface_artifact_stale_reasons(dependency_manifest, interface_path)
    } else {
        Vec::new()
    };
    let failing_manifest_note = format!(
        "note: failing referenced package manifest: {}",
        normalize_path(&dependency_manifest.manifest_path)
    );
    let owner_manifest_path = normalize_path(owner_manifest_path);
    let owner_note = format!(
        "note: while checking referenced package `{reference}` from `{owner_manifest_path}`"
    );
    let hint_line = format!(
        "hint: rerun `ql check --sync-interfaces {owner_manifest_path}` or regenerate `{dependency_package}` with `ql project emit-interface {}`",
        normalize_path(&dependency_manifest.manifest_path)
    );
    let notes = [failing_manifest_note.as_str(), owner_note.as_str()];

    report_interface_artifact_failure(
        &error_line,
        detail.as_deref(),
        &stale_reasons,
        &notes,
        &hint_line,
    );
}
