use std::path::{Path, PathBuf};

use crate::cli_utils::normalize_path;
use crate::project_interface_reporting::{
    report_package_interface_failure, report_package_interface_manifest_failure,
    report_package_interface_no_sources_failure, report_package_interface_output_failure,
    report_package_interface_source_failure, report_package_interface_source_root_failure,
};
use crate::project_interfaces::EmitPackageInterfaceError;

pub(super) fn report_reference_interface_sync_emit_error(
    error: EmitPackageInterfaceError,
    dependency_manifest: &ql_project::ProjectManifest,
    owner_manifest_path: &Path,
    reference: &str,
) -> PathBuf {
    let owner_note = format_reference_interface_sync_note(owner_manifest_path, reference);
    match error {
        EmitPackageInterfaceError::ManifestNotFound { .. } => report_package_interface_failure(
            &dependency_manifest.manifest_path,
            None,
            None,
            false,
            Some(owner_note.as_str()),
        ),
        EmitPackageInterfaceError::SourceFailure { .. } => report_package_interface_source_failure(
            &dependency_manifest.manifest_path,
            None,
            None,
            false,
            Some(owner_note.as_str()),
        ),
        EmitPackageInterfaceError::Code { .. } => report_package_interface_failure(
            &dependency_manifest.manifest_path,
            None,
            None,
            false,
            Some(owner_note.as_str()),
        ),
        EmitPackageInterfaceError::NoSourceFilesFailure { source_root, .. } => {
            report_package_interface_no_sources_failure(
                &dependency_manifest.manifest_path,
                None,
                &source_root,
                None,
                false,
                Some(owner_note.as_str()),
            )
        }
        EmitPackageInterfaceError::ManifestFailure { manifest_path, .. } => {
            report_package_interface_manifest_failure(
                &manifest_path,
                None,
                None,
                false,
                Some(owner_note.as_str()),
            )
        }
        EmitPackageInterfaceError::SourceRootFailure { source_root, .. } => {
            report_package_interface_source_root_failure(
                &dependency_manifest.manifest_path,
                None,
                &source_root,
                None,
                false,
                Some(owner_note.as_str()),
            )
        }
        EmitPackageInterfaceError::OutputPathFailure { output_path, .. } => {
            report_package_interface_output_failure(
                &dependency_manifest.manifest_path,
                None,
                &output_path,
                None,
                false,
                Some(owner_note.as_str()),
            )
        }
    }
    dependency_manifest.manifest_path.clone()
}

pub(super) fn format_reference_interface_sync_note(
    owner_manifest_path: &Path,
    reference: &str,
) -> String {
    let owner_manifest_path = normalize_path(owner_manifest_path);
    format!("note: while syncing referenced package `{reference}` from `{owner_manifest_path}`")
}
