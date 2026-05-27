use std::path::Path;

pub(crate) use self::checks::{check_package_interface_artifact, report_package_interface_check};
pub(crate) use self::commands::{
    format_emit_interface_rerun_command, format_project_emit_interface_command,
    format_project_emit_interface_command_label, format_workspace_member_emit_rerun_command,
};
use crate::cli_utils::normalize_path;
use crate::project_interfaces::EmitPackageInterfaceResult;

mod checks;
mod commands;

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

#[cfg(test)]
#[path = "project_interface_reporting_tests.rs"]
mod tests;
