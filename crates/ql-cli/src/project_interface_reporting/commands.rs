use std::path::Path;

use crate::cli_utils::normalize_path;

pub(crate) fn format_emit_interface_rerun_command(
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

pub(super) fn format_emit_interface_regenerate_command(
    manifest_path: &str,
    changed_only: bool,
) -> String {
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
