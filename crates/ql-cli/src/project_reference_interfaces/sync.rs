use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ql_project::{
    InterfaceArtifactStatus, default_interface_path, interface_artifact_status,
    load_project_manifest,
};

use crate::check_reporting::{format_check_command_label, report_package_check_manifest_failure};
use crate::cli_utils::{
    normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error,
};
use crate::project_interfaces::{EmitPackageInterfaceResult, emit_package_interface_path};
use crate::project_manifest_paths::{record_reference_failure_manifest, reference_manifest_path};

use super::emit::report_reference_interface_sync_emit_error;
use super::reporting::report_reference_manifest_issue_for_command;

#[derive(Default)]
struct ReferenceInterfaceSyncResult {
    written: Vec<PathBuf>,
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
                Err(error) => {
                    let failure_manifest = report_reference_interface_sync_emit_error(
                        error,
                        &dependency_manifest,
                        &manifest.manifest_path,
                        reference,
                    );
                    result.failure_count += 1;
                    record_reference_failure_manifest(
                        &mut result.first_failure_manifest,
                        failure_manifest,
                    );
                }
            }
        }
        sync_reference_interfaces_recursive(&dependency_manifest, visited, result, command_label);
    }
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
