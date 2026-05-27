use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ql_project::{
    InterfaceArtifactStatus, default_interface_path, interface_artifact_status,
    load_project_manifest,
};

use crate::cli_utils::{
    normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error,
};
use crate::project_manifest_paths::{record_reference_failure_manifest, reference_manifest_path};

use super::{EmitPackageInterfaceError, emit_package_interface_path_quiet};

#[derive(Debug)]
pub(crate) struct ReferenceInterfacePrepError {
    pub(crate) failure_count: usize,
    pub(crate) first_failure_manifest: Option<PathBuf>,
    pub(crate) first_failure: ReferenceInterfacePrepFailure,
}

#[derive(Debug)]
pub(crate) struct ReferenceInterfacePrepFailure {
    pub(crate) owner_manifest_path: Option<PathBuf>,
    pub(crate) reference: Option<String>,
    pub(crate) reference_manifest_path: PathBuf,
    pub(crate) manifest_path: Option<PathBuf>,
    pub(crate) failure_kind: ReferenceInterfacePrepFailureKind,
}

#[derive(Debug)]
pub(crate) enum ReferenceInterfacePrepFailureKind {
    Project {
        error_kind: &'static str,
        message: String,
        source_root: Option<PathBuf>,
    },
    InterfaceEmit(EmitPackageInterfaceError),
}

#[derive(Default)]
struct ReferenceInterfaceSyncQuietResult {
    failure_count: usize,
    first_failure_manifest: Option<PathBuf>,
    first_failure: Option<ReferenceInterfacePrepFailure>,
}

pub(crate) fn prepare_reference_interfaces_for_manifests_quiet(
    manifest_paths: &[PathBuf],
) -> Result<(), ReferenceInterfacePrepError> {
    if manifest_paths.is_empty() {
        return Ok(());
    }

    let mut visited = BTreeSet::new();
    let mut result = ReferenceInterfaceSyncQuietResult::default();
    for manifest_path in manifest_paths {
        let manifest = load_project_manifest(manifest_path).map_err(|error| {
            let failure =
                reference_interface_prep_project_failure(None, None, manifest_path, &error);
            ReferenceInterfacePrepError {
                failure_count: 1,
                first_failure_manifest: failure.manifest_path.clone(),
                first_failure: failure,
            }
        })?;
        sync_reference_interfaces_recursive_quiet(&manifest, &mut visited, &mut result);
    }

    if result.failure_count > 0 {
        return Err(ReferenceInterfacePrepError {
            failure_count: result.failure_count,
            first_failure_manifest: result.first_failure_manifest,
            first_failure: result
                .first_failure
                .expect("quiet dependency interface prep failure should record the first failure"),
        });
    }

    Ok(())
}

fn sync_reference_interfaces_recursive_quiet(
    manifest: &ql_project::ProjectManifest,
    visited: &mut BTreeSet<String>,
    result: &mut ReferenceInterfaceSyncQuietResult,
) {
    let manifest_path = manifest.manifest_path.clone();
    let manifest_key = normalize_path(&manifest_path);
    if !visited.insert(manifest_key) {
        return;
    }

    for reference in &manifest.references.packages {
        let reference_manifest_path = reference_manifest_path(manifest, reference);
        let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
        let dependency_manifest = match load_project_manifest(&manifest_dir.join(reference)) {
            Ok(manifest) => manifest,
            Err(error) => {
                record_reference_interface_prep_failure(
                    result,
                    reference_interface_prep_project_failure(
                        Some(&manifest.manifest_path),
                        Some(reference.as_str()),
                        &reference_manifest_path,
                        &error,
                    ),
                );
                continue;
            }
        };
        let interface_path = match default_interface_path(&dependency_manifest) {
            Ok(path) => path,
            Err(error) => {
                record_reference_interface_prep_failure(
                    result,
                    reference_interface_prep_project_failure(
                        Some(&manifest.manifest_path),
                        Some(reference.as_str()),
                        &reference_manifest_path,
                        &error,
                    ),
                );
                continue;
            }
        };
        if interface_artifact_status(&dependency_manifest, &interface_path)
            != InterfaceArtifactStatus::Valid
        {
            if let Err(error) =
                emit_package_interface_path_quiet(&dependency_manifest.manifest_path, None, false)
            {
                record_reference_interface_prep_failure(
                    result,
                    reference_interface_prep_emit_failure(
                        Some(&manifest.manifest_path),
                        Some(reference.as_str()),
                        &reference_manifest_path,
                        &dependency_manifest.manifest_path,
                        error,
                    ),
                );
            }
        }
        sync_reference_interfaces_recursive_quiet(&dependency_manifest, visited, result);
    }
}

fn reference_interface_prep_project_failure(
    owner_manifest_path: Option<&Path>,
    reference: Option<&str>,
    reference_manifest_path: &Path,
    error: &ql_project::ProjectError,
) -> ReferenceInterfacePrepFailure {
    let (manifest_path, error_kind, message, source_root) =
        if let ql_project::ProjectError::ManifestNotFound { start } = error {
            (
                Some(reference_manifest_path.to_path_buf()),
                "manifest",
                format!(
                    "could not find `qlang.toml` starting from `{}`",
                    normalize_path(start)
                ),
                None,
            )
        } else if let Some(manifest_path) =
            package_missing_name_manifest_path_from_project_error(error)
        {
            (
                Some(manifest_path.to_path_buf()),
                "manifest",
                format!(
                    "manifest `{}` does not declare `[package].name`",
                    normalize_path(manifest_path)
                ),
                None,
            )
        } else if let ql_project::ProjectError::PackageSourceRootNotFound { path } = error {
            (
                Some(reference_manifest_path.to_path_buf()),
                "package-source-root",
                format!(
                    "package source directory `{}` does not exist",
                    normalize_path(path)
                ),
                Some(path.clone()),
            )
        } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(error) {
            (
                Some(manifest_path.to_path_buf()),
                "manifest",
                error.to_string(),
                None,
            )
        } else {
            (
                Some(reference_manifest_path.to_path_buf()),
                "manifest",
                error.to_string(),
                None,
            )
        };
    ReferenceInterfacePrepFailure {
        owner_manifest_path: owner_manifest_path.map(Path::to_path_buf),
        reference: reference.map(str::to_owned),
        reference_manifest_path: reference_manifest_path.to_path_buf(),
        manifest_path,
        failure_kind: ReferenceInterfacePrepFailureKind::Project {
            error_kind,
            message,
            source_root,
        },
    }
}

fn reference_interface_prep_emit_failure(
    owner_manifest_path: Option<&Path>,
    reference: Option<&str>,
    reference_manifest_path: &Path,
    dependency_manifest_path: &Path,
    error: EmitPackageInterfaceError,
) -> ReferenceInterfacePrepFailure {
    let manifest_path = match &error {
        EmitPackageInterfaceError::OutputPathFailure {
            manifest_path: output_manifest_path,
            ..
        } => output_manifest_path
            .clone()
            .or(Some(dependency_manifest_path.to_path_buf())),
        EmitPackageInterfaceError::ManifestFailure { manifest_path, .. } => {
            Some(manifest_path.clone())
        }
        EmitPackageInterfaceError::NoSourceFilesFailure { manifest_path, .. } => {
            Some(manifest_path.clone())
        }
        EmitPackageInterfaceError::SourceRootFailure { manifest_path, .. } => {
            Some(manifest_path.clone())
        }
        _ => Some(dependency_manifest_path.to_path_buf()),
    };
    ReferenceInterfacePrepFailure {
        owner_manifest_path: owner_manifest_path.map(Path::to_path_buf),
        reference: reference.map(str::to_owned),
        reference_manifest_path: reference_manifest_path.to_path_buf(),
        manifest_path,
        failure_kind: ReferenceInterfacePrepFailureKind::InterfaceEmit(error),
    }
}

fn record_reference_interface_prep_failure(
    result: &mut ReferenceInterfaceSyncQuietResult,
    failure: ReferenceInterfacePrepFailure,
) {
    result.failure_count += 1;
    let manifest_path = failure
        .manifest_path
        .clone()
        .unwrap_or_else(|| failure.reference_manifest_path.clone());
    record_reference_failure_manifest(&mut result.first_failure_manifest, manifest_path);
    if result.first_failure.is_none() {
        result.first_failure = Some(failure);
    }
}

#[cfg(test)]
#[path = "../project_interfaces_reference_prep_tests.rs"]
mod tests;
