use std::path::Path;

use crate::cli_utils::normalize_path;

pub(crate) fn format_check_command(sync_interfaces: bool, manifest_path: Option<&str>) -> String {
    let mut command = String::from("ql check");
    if sync_interfaces {
        command.push_str(" --sync-interfaces");
    }
    if let Some(manifest_path) = manifest_path {
        command.push(' ');
        command.push_str(manifest_path);
    }
    command
}

pub(crate) fn format_check_command_label(sync_interfaces: bool) -> String {
    format!("`{}`", format_check_command(sync_interfaces, None))
}

pub(crate) fn format_workspace_member_check_rerun_command(
    manifest_path: &str,
    sync_interfaces: bool,
) -> String {
    format_check_command(sync_interfaces, Some(manifest_path))
}

fn format_workspace_member_reference_failure_rerun_hint(
    manifest_path: &Path,
    sync_interfaces: bool,
) -> String {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command =
        format_workspace_member_check_rerun_command(&manifest_path, sync_interfaces);
    format!(
        "hint: rerun `{rerun_command}` after fixing the referenced package or reference manifest"
    )
}

pub(crate) fn report_workspace_member_package_check_source_root_failure(
    manifest_path: &Path,
    source_root: &Path,
    sync_interfaces: bool,
) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command =
        format_workspace_member_check_rerun_command(&manifest_path, sync_interfaces);
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!("note: failing workspace member manifest: {manifest_path}");
    eprintln!(
        "note: failing package source root: {}",
        normalize_path(source_root)
    );
    eprintln!("hint: rerun `{rerun_command}` after fixing the package source root");
}

pub(crate) fn report_workspace_member_package_check_no_sources_failure(
    manifest_path: &Path,
    source_root: &Path,
    sync_interfaces: bool,
) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command =
        format_workspace_member_check_rerun_command(&manifest_path, sync_interfaces);
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!("note: failing workspace member manifest: {manifest_path}");
    eprintln!(
        "note: failing package source root: {}",
        normalize_path(source_root)
    );
    eprintln!("hint: rerun `{rerun_command}` after adding package source files");
}

pub(crate) fn report_workspace_member_package_check_source_diagnostics_failure(
    manifest_path: &Path,
    sync_interfaces: bool,
) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command =
        format_workspace_member_check_rerun_command(&manifest_path, sync_interfaces);
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!("note: failing workspace member manifest: {manifest_path}");
    eprintln!("hint: rerun `{rerun_command}` after fixing the package sources");
}

pub(crate) fn report_workspace_member_package_check_reference_failure(
    manifest_path: &Path,
    sync_interfaces: bool,
) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_hint = format_workspace_member_reference_failure_rerun_hint(
        Path::new(&manifest_path),
        sync_interfaces,
    );
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!("note: failing workspace member manifest: {manifest_path}");
    eprintln!("{rerun_hint}");
}

pub(crate) fn report_workspace_member_package_check_manifest_failure(
    manifest_path: &Path,
    sync_interfaces: bool,
) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command =
        format_workspace_member_check_rerun_command(&manifest_path, sync_interfaces);
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!("note: failing workspace member manifest: {manifest_path}");
    eprintln!("hint: rerun `{rerun_command}` after fixing the package manifest");
}

pub(crate) fn report_package_check_manifest_failure(manifest_path: &Path, sync_interfaces: bool) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command = format_check_command(sync_interfaces, Some(&manifest_path));
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!("hint: rerun `{rerun_command}` after fixing the package manifest");
}

pub(crate) fn report_package_check_source_root_failure(
    manifest_path: &Path,
    source_root: &Path,
    sync_interfaces: bool,
) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command = format_check_command(sync_interfaces, Some(&manifest_path));
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!(
        "note: failing package source root: {}",
        normalize_path(source_root)
    );
    eprintln!("hint: rerun `{rerun_command}` after fixing the package source root");
}

pub(crate) fn report_package_check_no_sources_failure(
    manifest_path: &Path,
    source_root: &Path,
    sync_interfaces: bool,
) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command = format_check_command(sync_interfaces, Some(&manifest_path));
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!(
        "note: failing package source root: {}",
        normalize_path(source_root)
    );
    eprintln!("hint: rerun `{rerun_command}` after adding package source files");
}

pub(crate) fn report_package_check_source_diagnostics_failure(
    manifest_path: &Path,
    sync_interfaces: bool,
) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command = format_check_command(sync_interfaces, Some(&manifest_path));
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!("hint: rerun `{rerun_command}` after fixing the package sources");
}

pub(crate) fn report_package_check_reference_failure(manifest_path: &Path, sync_interfaces: bool) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command = format_check_command(sync_interfaces, Some(&manifest_path));
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!(
        "hint: rerun `{rerun_command}` after fixing the referenced package or reference manifest"
    );
}

pub(crate) fn report_check_package_selector_requires_workspace_context(package_name: &str) {
    eprintln!("error: `ql check` package selectors require a workspace path");
    eprintln!("note: selector: package `{package_name}`");
}
