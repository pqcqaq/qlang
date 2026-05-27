use std::path::Path;

use crate::project_interface_reporting::{
    report_emit_interface_result, report_package_interface_failure,
    report_package_interface_manifest_failure, report_package_interface_no_sources_failure,
    report_package_interface_output_failure, report_package_interface_source_failure,
    report_package_interface_source_root_failure,
};
use crate::project_interfaces::{EmitPackageInterfaceError, emit_package_interface_path};

pub(super) fn emit_single_package_interface(
    emit_path: &Path,
    manifest_path: &Path,
    member_manifest_path: Option<&Path>,
    output: Option<&Path>,
    command_label: &str,
    changed_only: bool,
) -> Result<(), u8> {
    match emit_package_interface_path(emit_path, output, command_label, changed_only) {
        Ok(result) => {
            report_emit_interface_result(result);
            Ok(())
        }
        Err(error) => Err(report_package_emit_interface_error(
            error,
            manifest_path,
            member_manifest_path,
            output,
            changed_only,
        )),
    }
}

fn report_package_emit_interface_error(
    error: EmitPackageInterfaceError,
    manifest_path: &Path,
    member_manifest_path: Option<&Path>,
    output: Option<&Path>,
    changed_only: bool,
) -> u8 {
    match error {
        EmitPackageInterfaceError::ManifestNotFound { .. } => {
            report_package_interface_failure(
                manifest_path,
                member_manifest_path,
                output,
                changed_only,
                None,
            );
            1
        }
        EmitPackageInterfaceError::SourceFailure { code, .. } => {
            report_package_interface_source_failure(
                manifest_path,
                member_manifest_path,
                output,
                changed_only,
                None,
            );
            code
        }
        EmitPackageInterfaceError::Code { code, .. } => {
            report_package_interface_failure(
                manifest_path,
                member_manifest_path,
                output,
                changed_only,
                None,
            );
            code
        }
        EmitPackageInterfaceError::ManifestFailure { .. } => {
            report_package_interface_manifest_failure(
                manifest_path,
                member_manifest_path,
                output,
                changed_only,
                None,
            );
            1
        }
        EmitPackageInterfaceError::NoSourceFilesFailure { source_root, .. } => {
            report_package_interface_no_sources_failure(
                manifest_path,
                member_manifest_path,
                &source_root,
                output,
                changed_only,
                None,
            );
            1
        }
        EmitPackageInterfaceError::SourceRootFailure { source_root, .. } => {
            report_package_interface_source_root_failure(
                manifest_path,
                member_manifest_path,
                &source_root,
                output,
                changed_only,
                None,
            );
            1
        }
        EmitPackageInterfaceError::OutputPathFailure { output_path, .. } => {
            report_package_interface_output_failure(
                manifest_path,
                member_manifest_path,
                &output_path,
                output,
                changed_only,
                None,
            );
            1
        }
    }
}
