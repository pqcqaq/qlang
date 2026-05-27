use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ql_project::{
    InterfaceArtifactStatus, default_interface_path, interface_artifact_status,
    load_project_manifest, package_name,
};

use crate::check_reporting::format_check_command_label;
use crate::cli_utils::normalize_path;
use crate::project_manifest_paths::{record_reference_failure_manifest, reference_manifest_path};

mod emit;
mod reporting;
mod sync;

#[cfg(test)]
use emit::format_reference_interface_sync_note;
use reporting::{report_reference_interface_artifact_issue, report_reference_manifest_issue};
pub(crate) use sync::{prepare_reference_interfaces_for_manifests, sync_reference_interfaces};

#[derive(Default)]
struct ReferenceInterfaceCheckResult {
    failure_count: usize,
    first_failure_manifest: Option<PathBuf>,
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

#[cfg(test)]
#[path = "project_reference_interfaces_tests.rs"]
mod tests;
