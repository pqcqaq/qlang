use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ql_project::{
    InterfaceArtifactStatus, default_interface_path, interface_artifact_stale_reasons,
    interface_artifact_status, interface_artifact_status_detail, load_project_manifest,
    package_name,
};

use crate::check_reporting::{format_check_command_label, report_package_check_manifest_failure};
use crate::cli_utils::{
    normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error,
};
use crate::project_interface_reporting::{
    report_package_interface_failure, report_package_interface_manifest_failure,
    report_package_interface_no_sources_failure, report_package_interface_output_failure,
    report_package_interface_source_failure, report_package_interface_source_root_failure,
};
use crate::project_interfaces::{
    EmitPackageInterfaceError, EmitPackageInterfaceResult, emit_package_interface_path,
};
use crate::project_manifest_paths::{record_reference_failure_manifest, reference_manifest_path};
use crate::project_reporting::report_interface_artifact_failure;

#[derive(Default)]
struct ReferenceInterfaceSyncResult {
    written: Vec<PathBuf>,
    failure_count: usize,
    first_failure_manifest: Option<PathBuf>,
}

#[derive(Default)]
struct ReferenceInterfaceCheckResult {
    failure_count: usize,
    first_failure_manifest: Option<PathBuf>,
}

pub(crate) fn sync_reference_interfaces(
    path: &Path,
    visited: &mut BTreeSet<String>,
) -> Result<Vec<PathBuf>, u8> {
    let check_command_label = format_check_command_label(true);
    let manifest = load_project_manifest(path).map_err(|error| {
        if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(&error) {
            eprintln!(
                "error: {} manifest `{}` does not declare `[package].name`",
                check_command_label,
                normalize_path(manifest_path)
            );
            report_package_check_manifest_failure(manifest_path, true);
        } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(&error) {
            eprintln!("error: {check_command_label} {error}");
            report_package_check_manifest_failure(manifest_path, true);
        } else {
            eprintln!("error: {error}");
        }
        1
    })?;
    let mut result = ReferenceInterfaceSyncResult::default();
    sync_reference_interfaces_recursive(&manifest, visited, &mut result, &check_command_label);
    if result.failure_count > 0 {
        for path in &result.written {
            println!("wrote interface: {}", path.display());
        }
        eprintln!(
            "error: {check_command_label} found {} failing referenced package(s)",
            result.failure_count
        );
        if result.failure_count > 1 {
            if let Some(path) = &result.first_failure_manifest {
                eprintln!(
                    "note: first failing reference manifest: {}",
                    normalize_path(path)
                );
            }
        }
        return Err(1);
    }
    Ok(result.written)
}

fn sync_reference_interfaces_recursive(
    manifest: &ql_project::ProjectManifest,
    visited: &mut BTreeSet<String>,
    result: &mut ReferenceInterfaceSyncResult,
    command_label: &str,
) {
    let manifest_path = manifest.manifest_path.clone();
    let manifest_key = normalize_path(&manifest_path);
    if !visited.insert(manifest_key) {
        return;
    }

    for reference in &manifest.references.packages {
        let (dependency_manifest, reference_manifest_path) =
            match load_reference_manifest_for_sync(manifest, reference, command_label) {
                Ok(result) => result,
                Err(_) => {
                    result.failure_count += 1;
                    record_reference_failure_manifest(
                        &mut result.first_failure_manifest,
                        reference_manifest_path(manifest, reference),
                    );
                    continue;
                }
            };
        let interface_path = reference_interface_path_for_sync(
            manifest,
            reference,
            &reference_manifest_path,
            &dependency_manifest,
            command_label,
        );
        let interface_path = match interface_path {
            Ok(path) => path,
            Err(_) => {
                result.failure_count += 1;
                record_reference_failure_manifest(
                    &mut result.first_failure_manifest,
                    reference_manifest_path.clone(),
                );
                continue;
            }
        };
        if interface_artifact_status(&dependency_manifest, &interface_path)
            != InterfaceArtifactStatus::Valid
        {
            let emit_result = emit_package_interface_path(
                &dependency_manifest.manifest_path,
                None,
                command_label,
                false,
            );
            match emit_result {
                Ok(EmitPackageInterfaceResult::Wrote(path)) => result.written.push(path),
                Ok(EmitPackageInterfaceResult::UpToDate(_)) => {}
                Err(EmitPackageInterfaceError::ManifestNotFound { .. }) => {
                    let owner_note =
                        format_reference_interface_sync_note(&manifest.manifest_path, reference);
                    report_package_interface_failure(
                        &dependency_manifest.manifest_path,
                        None,
                        None,
                        false,
                        Some(owner_note.as_str()),
                    );
                    result.failure_count += 1;
                    record_reference_failure_manifest(
                        &mut result.first_failure_manifest,
                        dependency_manifest.manifest_path.clone(),
                    );
                }
                Err(EmitPackageInterfaceError::SourceFailure { .. }) => {
                    let owner_note =
                        format_reference_interface_sync_note(&manifest.manifest_path, reference);
                    report_package_interface_source_failure(
                        &dependency_manifest.manifest_path,
                        None,
                        None,
                        false,
                        Some(owner_note.as_str()),
                    );
                    result.failure_count += 1;
                    record_reference_failure_manifest(
                        &mut result.first_failure_manifest,
                        dependency_manifest.manifest_path.clone(),
                    );
                }
                Err(EmitPackageInterfaceError::Code { .. }) => {
                    let owner_note =
                        format_reference_interface_sync_note(&manifest.manifest_path, reference);
                    report_package_interface_failure(
                        &dependency_manifest.manifest_path,
                        None,
                        None,
                        false,
                        Some(owner_note.as_str()),
                    );
                    result.failure_count += 1;
                    record_reference_failure_manifest(
                        &mut result.first_failure_manifest,
                        dependency_manifest.manifest_path.clone(),
                    );
                }
                Err(EmitPackageInterfaceError::NoSourceFilesFailure { source_root, .. }) => {
                    let owner_note =
                        format_reference_interface_sync_note(&manifest.manifest_path, reference);
                    report_package_interface_no_sources_failure(
                        &dependency_manifest.manifest_path,
                        None,
                        &source_root,
                        None,
                        false,
                        Some(owner_note.as_str()),
                    );
                    result.failure_count += 1;
                    record_reference_failure_manifest(
                        &mut result.first_failure_manifest,
                        dependency_manifest.manifest_path.clone(),
                    );
                }
                Err(EmitPackageInterfaceError::ManifestFailure { manifest_path, .. }) => {
                    let owner_note =
                        format_reference_interface_sync_note(&manifest.manifest_path, reference);
                    report_package_interface_manifest_failure(
                        &manifest_path,
                        None,
                        None,
                        false,
                        Some(owner_note.as_str()),
                    );
                    result.failure_count += 1;
                    record_reference_failure_manifest(
                        &mut result.first_failure_manifest,
                        dependency_manifest.manifest_path.clone(),
                    );
                }
                Err(EmitPackageInterfaceError::SourceRootFailure { source_root, .. }) => {
                    let owner_note =
                        format_reference_interface_sync_note(&manifest.manifest_path, reference);
                    report_package_interface_source_root_failure(
                        &dependency_manifest.manifest_path,
                        None,
                        &source_root,
                        None,
                        false,
                        Some(owner_note.as_str()),
                    );
                    result.failure_count += 1;
                    record_reference_failure_manifest(
                        &mut result.first_failure_manifest,
                        dependency_manifest.manifest_path.clone(),
                    );
                }
                Err(EmitPackageInterfaceError::OutputPathFailure { output_path, .. }) => {
                    let owner_note =
                        format_reference_interface_sync_note(&manifest.manifest_path, reference);
                    report_package_interface_output_failure(
                        &dependency_manifest.manifest_path,
                        None,
                        &output_path,
                        None,
                        false,
                        Some(owner_note.as_str()),
                    );
                    result.failure_count += 1;
                    record_reference_failure_manifest(
                        &mut result.first_failure_manifest,
                        dependency_manifest.manifest_path.clone(),
                    );
                }
            }
        }
        sync_reference_interfaces_recursive(&dependency_manifest, visited, result, command_label);
    }
}

pub(crate) fn prepare_reference_interfaces_for_manifests(
    manifest_paths: &[PathBuf],
    command_label: &str,
    report_writes: bool,
) -> Result<(), u8> {
    if manifest_paths.is_empty() {
        return Ok(());
    }

    let mut visited = BTreeSet::new();
    let mut result = ReferenceInterfaceSyncResult::default();
    for manifest_path in manifest_paths {
        let manifest = load_project_manifest(manifest_path).map_err(|error| {
            if let Some(manifest_path) =
                package_missing_name_manifest_path_from_project_error(&error)
            {
                eprintln!(
                    "error: {} manifest `{}` does not declare `[package].name`",
                    command_label,
                    normalize_path(manifest_path)
                );
            } else if let Some(manifest_path) =
                package_check_manifest_path_from_project_error(&error)
            {
                eprintln!("error: {command_label} {error}");
                eprintln!(
                    "note: failing package manifest: {}",
                    normalize_path(manifest_path)
                );
            } else {
                eprintln!("error: {command_label} {error}");
            }
            1
        })?;
        sync_reference_interfaces_recursive(&manifest, &mut visited, &mut result, command_label);
    }

    if result.failure_count > 0 {
        if report_writes {
            for path in &result.written {
                println!("wrote interface: {}", path.display());
            }
        }
        eprintln!(
            "error: {command_label} found {} failing referenced package(s)",
            result.failure_count
        );
        if result.failure_count > 1 {
            if let Some(path) = &result.first_failure_manifest {
                eprintln!(
                    "note: first failing reference manifest: {}",
                    normalize_path(path)
                );
            }
        }
        return Err(1);
    }

    if report_writes {
        for path in result.written {
            println!("wrote interface: {}", path.display());
        }
    }

    Ok(())
}

pub(crate) fn ensure_reference_interfaces_current(
    manifest: &ql_project::ProjectManifest,
) -> Result<(), u8> {
    let check_command_label = format_check_command_label(false);
    let result = ensure_reference_interfaces_current_recursive(manifest, &mut BTreeSet::new());
    if result.failure_count > 0 {
        eprintln!(
            "error: {check_command_label} found {} failing referenced package(s)",
            result.failure_count
        );
        if result.failure_count > 1 {
            if let Some(path) = &result.first_failure_manifest {
                eprintln!(
                    "note: first failing reference manifest: {}",
                    normalize_path(path)
                );
            }
        }
        return Err(1);
    }
    Ok(())
}

fn ensure_reference_interfaces_current_recursive(
    manifest: &ql_project::ProjectManifest,
    visited: &mut BTreeSet<String>,
) -> ReferenceInterfaceCheckResult {
    let manifest_path = manifest.manifest_path.clone();
    let manifest_key = normalize_path(&manifest_path);
    if !visited.insert(manifest_key) {
        return ReferenceInterfaceCheckResult::default();
    }

    let mut result = ReferenceInterfaceCheckResult::default();
    for reference in &manifest.references.packages {
        let (dependency_manifest, reference_manifest_path) =
            match load_reference_manifest_for_interfaces(manifest, reference, false) {
                Ok(result) => result,
                Err(_) => {
                    result.failure_count += 1;
                    record_reference_failure_manifest(
                        &mut result.first_failure_manifest,
                        reference_manifest_path(manifest, reference),
                    );
                    continue;
                }
            };
        let dependency_package = reference_package_name_for_interfaces(
            manifest,
            reference,
            &reference_manifest_path,
            &dependency_manifest,
            false,
        );
        let dependency_package = match dependency_package {
            Ok(name) => name,
            Err(_) => {
                result.failure_count += 1;
                record_reference_failure_manifest(
                    &mut result.first_failure_manifest,
                    reference_manifest_path.clone(),
                );
                continue;
            }
        };
        let interface_path = reference_interface_path_for_interfaces(
            manifest,
            reference,
            &reference_manifest_path,
            &dependency_manifest,
            false,
        );
        let interface_path = match interface_path {
            Ok(path) => path,
            Err(_) => {
                result.failure_count += 1;
                record_reference_failure_manifest(
                    &mut result.first_failure_manifest,
                    reference_manifest_path.clone(),
                );
                continue;
            }
        };
        let status = interface_artifact_status(&dependency_manifest, &interface_path);
        if status != InterfaceArtifactStatus::Valid {
            report_reference_interface_artifact_issue(
                &dependency_manifest,
                reference,
                &dependency_package,
                &manifest_path,
                &interface_path,
                status,
            );
            result.failure_count += 1;
            record_reference_failure_manifest(
                &mut result.first_failure_manifest,
                dependency_manifest.manifest_path.clone(),
            );
        }
        let nested_result =
            ensure_reference_interfaces_current_recursive(&dependency_manifest, visited);
        result.failure_count += nested_result.failure_count;
        if result.first_failure_manifest.is_none() {
            result.first_failure_manifest = nested_result.first_failure_manifest;
        }
    }

    result
}

fn load_reference_manifest_for_interfaces(
    owner_manifest: &ql_project::ProjectManifest,
    reference: &str,
    sync_interfaces: bool,
) -> Result<(ql_project::ProjectManifest, PathBuf), u8> {
    let reference_manifest_path = reference_manifest_path(owner_manifest, reference);
    let manifest_dir = owner_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    let dependency_manifest =
        load_project_manifest(&manifest_dir.join(reference)).map_err(|error| {
            report_reference_manifest_issue(
                sync_interfaces,
                reference,
                &owner_manifest.manifest_path,
                &reference_manifest_path,
                &error,
            );
            1
        })?;
    Ok((dependency_manifest, reference_manifest_path))
}

fn load_reference_manifest_for_sync(
    owner_manifest: &ql_project::ProjectManifest,
    reference: &str,
    command_label: &str,
) -> Result<(ql_project::ProjectManifest, PathBuf), u8> {
    let reference_manifest_path = reference_manifest_path(owner_manifest, reference);
    let manifest_dir = owner_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    let dependency_manifest =
        load_project_manifest(&manifest_dir.join(reference)).map_err(|error| {
            report_reference_manifest_issue_for_command(
                command_label,
                reference,
                &owner_manifest.manifest_path,
                &reference_manifest_path,
                &error,
            );
            1
        })?;
    Ok((dependency_manifest, reference_manifest_path))
}

fn reference_package_name_for_interfaces(
    owner_manifest: &ql_project::ProjectManifest,
    reference: &str,
    reference_manifest_path: &Path,
    dependency_manifest: &ql_project::ProjectManifest,
    sync_interfaces: bool,
) -> Result<String, u8> {
    package_name(dependency_manifest)
        .map(str::to_owned)
        .map_err(|error| {
            report_reference_manifest_issue(
                sync_interfaces,
                reference,
                &owner_manifest.manifest_path,
                reference_manifest_path,
                &error,
            );
            1
        })
}

fn reference_interface_path_for_interfaces(
    owner_manifest: &ql_project::ProjectManifest,
    reference: &str,
    reference_manifest_path: &Path,
    dependency_manifest: &ql_project::ProjectManifest,
    sync_interfaces: bool,
) -> Result<PathBuf, u8> {
    default_interface_path(dependency_manifest).map_err(|error| {
        report_reference_manifest_issue(
            sync_interfaces,
            reference,
            &owner_manifest.manifest_path,
            reference_manifest_path,
            &error,
        );
        1
    })
}

fn reference_interface_path_for_sync(
    owner_manifest: &ql_project::ProjectManifest,
    reference: &str,
    reference_manifest_path: &Path,
    dependency_manifest: &ql_project::ProjectManifest,
    command_label: &str,
) -> Result<PathBuf, u8> {
    default_interface_path(dependency_manifest).map_err(|error| {
        report_reference_manifest_issue_for_command(
            command_label,
            reference,
            &owner_manifest.manifest_path,
            reference_manifest_path,
            &error,
        );
        1
    })
}

fn report_reference_manifest_issue(
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

fn report_reference_manifest_issue_for_command(
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

fn format_reference_interface_sync_note(owner_manifest_path: &Path, reference: &str) -> String {
    let owner_manifest_path = normalize_path(owner_manifest_path);
    format!("note: while syncing referenced package `{reference}` from `{owner_manifest_path}`")
}

fn report_reference_interface_artifact_issue(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_interface_sync_note_mentions_owner_and_reference() {
        assert_eq!(
            format_reference_interface_sync_note(Path::new("app/qlang.toml"), "std/core"),
            "note: while syncing referenced package `std/core` from `app/qlang.toml`"
        );
    }
}
