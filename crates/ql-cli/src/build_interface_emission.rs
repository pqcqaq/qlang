use std::path::{Path, PathBuf};

use ql_driver::BuildOptions;

use crate::build_failure_reporting::{
    report_build_interface_failure, report_build_interface_manifest_failure,
    report_build_interface_no_sources_failure, report_build_interface_output_failure,
    report_build_interface_package_context_failure, report_build_interface_source_failure,
    report_build_interface_source_root_failure, report_remaining_build_artifacts,
};
use crate::project_interface_reporting::report_emit_interface_result;
use crate::project_interfaces::{
    EmitPackageInterfaceError, EmitPackageInterfaceResult, emit_package_interface_path,
    emit_package_interface_path_quiet,
};

pub(crate) fn emit_built_package_interface(
    request_path: &Path,
    package_context_path: &Path,
    options: &BuildOptions,
    artifact_path: &Path,
    additional_artifacts: &[PathBuf],
) -> Result<(), u8> {
    let result = emit_built_package_interface_impl(
        request_path,
        package_context_path,
        options,
        artifact_path,
        additional_artifacts,
    )?;
    report_emit_interface_result(result);
    Ok(())
}

pub(crate) fn emit_built_package_interface_quiet(
    request_path: &Path,
    package_context_path: &Path,
    options: &BuildOptions,
    artifact_path: &Path,
    additional_artifacts: &[PathBuf],
) -> Result<EmitPackageInterfaceResult, EmitPackageInterfaceError> {
    let _ = (request_path, options, artifact_path, additional_artifacts);
    emit_package_interface_path_quiet(package_context_path, None, false)
}

fn emit_built_package_interface_impl(
    request_path: &Path,
    package_context_path: &Path,
    options: &BuildOptions,
    artifact_path: &Path,
    additional_artifacts: &[PathBuf],
) -> Result<EmitPackageInterfaceResult, u8> {
    match emit_package_interface_path(
        package_context_path,
        None,
        "`ql build --emit-interface`",
        false,
    ) {
        Ok(result) => Ok(result),
        Err(EmitPackageInterfaceError::ManifestNotFound { .. }) => {
            report_build_interface_package_context_failure(
                request_path,
                options,
                true,
                artifact_path,
            );
            report_remaining_build_artifacts(additional_artifacts);
            Err(1)
        }
        Err(EmitPackageInterfaceError::SourceFailure { code, .. }) => {
            report_build_interface_source_failure(request_path, options, true, artifact_path);
            report_remaining_build_artifacts(additional_artifacts);
            Err(code)
        }
        Err(EmitPackageInterfaceError::Code { code, .. }) => {
            report_build_interface_failure(request_path, options, true, artifact_path);
            report_remaining_build_artifacts(additional_artifacts);
            Err(code)
        }
        Err(EmitPackageInterfaceError::ManifestFailure { manifest_path, .. }) => {
            report_build_interface_manifest_failure(
                request_path,
                options,
                true,
                artifact_path,
                &manifest_path,
            );
            report_remaining_build_artifacts(additional_artifacts);
            Err(1)
        }
        Err(EmitPackageInterfaceError::NoSourceFilesFailure {
            manifest_path,
            source_root,
        }) => {
            report_build_interface_no_sources_failure(
                request_path,
                options,
                true,
                artifact_path,
                &manifest_path,
                &source_root,
            );
            report_remaining_build_artifacts(additional_artifacts);
            Err(1)
        }
        Err(EmitPackageInterfaceError::SourceRootFailure {
            manifest_path,
            source_root,
        }) => {
            report_build_interface_source_root_failure(
                request_path,
                options,
                true,
                artifact_path,
                &manifest_path,
                &source_root,
            );
            report_remaining_build_artifacts(additional_artifacts);
            Err(1)
        }
        Err(EmitPackageInterfaceError::OutputPathFailure { output_path, .. }) => {
            report_build_interface_output_failure(
                request_path,
                options,
                true,
                artifact_path,
                &output_path,
            );
            report_remaining_build_artifacts(additional_artifacts);
            Err(1)
        }
    }
}
