use std::fs;
use std::path::{Path, PathBuf};

use ql_driver::{BuildArtifact, BuildError, BuildOptions, build_source_with_link_inputs};

use crate::build_failure_reporting::{
    missing_build_header_import_surface, missing_build_input_path, missing_dylib_exports,
    report_build_export_configuration_failure, report_build_header_configuration_failure,
    report_build_header_import_surface_failure, report_build_header_output_path_failure,
    report_build_input_path_failure, report_build_interface_failure,
    report_build_interface_manifest_failure, report_build_interface_no_sources_failure,
    report_build_interface_output_failure, report_build_interface_package_context_failure,
    report_build_interface_source_failure, report_build_interface_source_root_failure,
    report_build_output_path_failure, report_build_source_diagnostics_failure,
    report_build_toolchain_failure, report_remaining_build_artifacts,
    unsupported_build_header_emit,
};
use crate::build_outputs::{
    build_header_output_path, build_output_path, colliding_build_header_output_path,
    io_targets_build_header_output_path, io_targets_build_output_path,
    toolchain_targets_build_output_path,
};
use crate::build_source_rewrites::local_generic_source_override;
use crate::cli_diagnostics::print_diagnostics;
use crate::cli_utils::normalize_path;
use crate::project_interface_reporting::report_emit_interface_result;
use crate::project_interfaces::{
    EmitPackageInterfaceError, EmitPackageInterfaceResult, emit_package_interface_path,
    emit_package_interface_path_quiet,
};

pub(crate) fn build_output_lock_error_message(error: BuildError) -> String {
    match error {
        BuildError::Io { path, error } => format!(
            "failed to acquire build output lock `{}`: {error}",
            normalize_path(&path)
        ),
        BuildError::InvalidInput(message) => message,
        BuildError::Diagnostics { path, .. } => format!(
            "failed to acquire build output lock while diagnostics were reported for `{}`",
            normalize_path(&path)
        ),
        BuildError::Toolchain { error, .. } => format!("{error}"),
    }
}

pub(crate) fn build_single_source_target(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
) -> Result<BuildArtifact, u8> {
    build_single_source_target_impl(path, options, emit_interface, true, true)
}

pub(crate) fn build_single_source_target_silent(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
) -> Result<BuildArtifact, u8> {
    build_single_source_target_impl(path, options, emit_interface, false, true)
}

pub(crate) fn build_single_source_target_result(
    path: &Path,
    options: &BuildOptions,
) -> Result<BuildArtifact, BuildError> {
    build_single_source_target_with_inputs_result(path, options, None, &[])
}

pub(crate) fn build_single_source_target_quiet(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
) -> Result<BuildArtifact, u8> {
    build_single_source_target_impl(path, options, emit_interface, false, false)
}

fn build_single_source_target_impl(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    report_success: bool,
    report_failure: bool,
) -> Result<BuildArtifact, u8> {
    build_single_source_target_with_inputs_impl(
        path,
        options,
        emit_interface,
        report_success,
        report_failure,
        None,
        &[],
    )
}

pub(crate) fn build_single_source_target_with_inputs_impl(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    report_success: bool,
    report_failure: bool,
    source_override: Option<&str>,
    additional_link_inputs: &[PathBuf],
) -> Result<BuildArtifact, u8> {
    match build_single_source_target_with_inputs_result(
        path,
        options,
        source_override,
        additional_link_inputs,
    ) {
        Ok(artifact) => {
            if report_success {
                println!(
                    "wrote {}: {}",
                    artifact.emit.as_str(),
                    artifact.path.display()
                );
                if let Some(header) = artifact.c_header.as_ref() {
                    println!("wrote c-header: {}", header.path.display());
                }
            }
            Ok(artifact)
        }
        Err(BuildError::InvalidInput(message)) => {
            if report_failure {
                eprintln!("error: {message}");
            }
            if emit_interface && report_failure {
                if missing_build_input_path(path, &message) {
                    report_build_input_path_failure(path, options, emit_interface);
                } else if missing_dylib_exports(&message, options) {
                    report_build_export_configuration_failure(path, options, emit_interface);
                } else if missing_build_header_import_surface(&message, options) {
                    report_build_header_import_surface_failure(path, options, emit_interface);
                } else if unsupported_build_header_emit(options) {
                    report_build_header_configuration_failure(path, options, emit_interface);
                } else if let Some(header_output_path) =
                    colliding_build_header_output_path(path, options)
                {
                    report_build_header_output_path_failure(
                        path,
                        options,
                        emit_interface,
                        &header_output_path,
                    );
                }
            }
            Err(1)
        }
        Err(BuildError::Io {
            path: io_path,
            error,
        }) => {
            if report_failure {
                eprintln!("error: failed to access `{}`: {error}", io_path.display());
            }
            if emit_interface && report_failure {
                if io_path == path {
                    report_build_input_path_failure(path, options, emit_interface);
                } else if let Some(output_path) = build_output_path(path, options) {
                    if io_targets_build_output_path(&io_path, &output_path) {
                        report_build_output_path_failure(
                            path,
                            options,
                            emit_interface,
                            &output_path,
                        );
                    } else if let Some(header_output_path) = build_header_output_path(path, options)
                    {
                        if io_targets_build_header_output_path(&io_path, &header_output_path) {
                            report_build_header_output_path_failure(
                                path,
                                options,
                                emit_interface,
                                &header_output_path,
                            );
                        }
                    }
                }
            }
            Err(1)
        }
        Err(BuildError::Toolchain {
            error,
            preserved_artifacts,
        }) => {
            if report_failure {
                eprintln!("error: {error}");
                for path in preserved_artifacts {
                    eprintln!(
                        "note: preserved intermediate artifact at `{}`",
                        path.display()
                    );
                }
            }
            if emit_interface && report_failure {
                if let Some(output_path) = build_output_path(path, options) {
                    if toolchain_targets_build_output_path(&error, &output_path) {
                        report_build_output_path_failure(
                            path,
                            options,
                            emit_interface,
                            &output_path,
                        );
                    } else {
                        report_build_toolchain_failure(path, options, emit_interface);
                    }
                } else {
                    report_build_toolchain_failure(path, options, emit_interface);
                }
            }
            Err(1)
        }
        Err(BuildError::Diagnostics {
            path: diagnostic_path,
            source,
            diagnostics,
        }) => {
            if report_failure {
                print_diagnostics(&diagnostic_path, &source, &diagnostics);
            }
            if emit_interface && report_failure {
                report_build_source_diagnostics_failure(path, options, emit_interface);
            }
            Err(1)
        }
    }
}

pub(crate) fn build_single_source_target_with_inputs_result(
    path: &Path,
    options: &BuildOptions,
    source_override: Option<&str>,
    additional_link_inputs: &[PathBuf],
) -> Result<BuildArtifact, BuildError> {
    match source_override {
        Some(source) => {
            build_source_with_link_inputs(path, source, options, additional_link_inputs)
        }
        None => {
            if !path.is_file() {
                return Err(BuildError::InvalidInput(format!(
                    "`{}` is not a file",
                    path.display()
                )));
            }
            let source = fs::read_to_string(path).map_err(|error| BuildError::Io {
                path: path.to_path_buf(),
                error,
            })?;
            let local_source_override = local_generic_source_override(&source);
            build_source_with_link_inputs(
                path,
                local_source_override.as_deref().unwrap_or(&source),
                options,
                additional_link_inputs,
            )
        }
    }
}

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
