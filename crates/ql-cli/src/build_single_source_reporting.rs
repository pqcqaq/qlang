use std::path::Path;

use ql_driver::{BuildArtifact, BuildError, BuildOptions};

use crate::build_failure_reporting::{
    missing_build_header_import_surface, missing_build_input_path, missing_dylib_exports,
    report_build_export_configuration_failure, report_build_header_configuration_failure,
    report_build_header_import_surface_failure, report_build_header_output_path_failure,
    report_build_input_path_failure, report_build_output_path_failure,
    report_build_source_diagnostics_failure, report_build_toolchain_failure,
    unsupported_build_header_emit,
};
use crate::build_outputs::{
    build_header_output_path, build_output_path, colliding_build_header_output_path,
    io_targets_build_header_output_path, io_targets_build_output_path,
    toolchain_targets_build_output_path,
};
use crate::cli_diagnostics::print_diagnostics;
use crate::cli_utils::normalize_path;

pub(crate) fn report_single_source_build_success(artifact: &BuildArtifact) {
    println!(
        "wrote {}: {}",
        artifact.emit.as_str(),
        artifact.path.display()
    );
    if let Some(header) = artifact.c_header.as_ref() {
        println!("wrote c-header: {}", header.path.display());
    }
}

pub(crate) fn report_single_source_build_error(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    error: BuildError,
) -> u8 {
    match error {
        BuildError::InvalidInput(message) => {
            eprintln!("error: {message}");
            if emit_interface {
                report_invalid_input_emit_interface_hint(path, options, &message);
            }
            1
        }
        BuildError::Io {
            path: io_path,
            error,
        } => {
            eprintln!("error: failed to access `{}`: {error}", io_path.display());
            if emit_interface {
                report_io_emit_interface_hint(path, options, emit_interface, &io_path);
            }
            1
        }
        BuildError::Toolchain {
            error,
            preserved_artifacts,
        } => {
            eprintln!("error: {error}");
            for path in preserved_artifacts {
                eprintln!(
                    "note: preserved intermediate artifact at `{}`",
                    path.display()
                );
            }
            if emit_interface {
                report_toolchain_emit_interface_hint(path, options, emit_interface, &error);
            }
            1
        }
        BuildError::Diagnostics {
            path: diagnostic_path,
            source,
            diagnostics,
        } => {
            print_diagnostics(&diagnostic_path, &source, &diagnostics);
            if emit_interface {
                report_build_source_diagnostics_failure(path, options, emit_interface);
            }
            1
        }
    }
}

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

fn report_invalid_input_emit_interface_hint(path: &Path, options: &BuildOptions, message: &str) {
    if missing_build_input_path(path, message) {
        report_build_input_path_failure(path, options, true);
    } else if missing_dylib_exports(message, options) {
        report_build_export_configuration_failure(path, options, true);
    } else if missing_build_header_import_surface(message, options) {
        report_build_header_import_surface_failure(path, options, true);
    } else if unsupported_build_header_emit(options) {
        report_build_header_configuration_failure(path, options, true);
    } else if let Some(header_output_path) = colliding_build_header_output_path(path, options) {
        report_build_header_output_path_failure(path, options, true, &header_output_path);
    }
}

fn report_io_emit_interface_hint(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    io_path: &Path,
) {
    if io_path == path {
        report_build_input_path_failure(path, options, emit_interface);
        return;
    }
    let Some(output_path) = build_output_path(path, options) else {
        return;
    };
    if io_targets_build_output_path(io_path, &output_path) {
        report_build_output_path_failure(path, options, emit_interface, &output_path);
    } else if let Some(header_output_path) = build_header_output_path(path, options)
        && io_targets_build_header_output_path(io_path, &header_output_path)
    {
        report_build_header_output_path_failure(path, options, emit_interface, &header_output_path);
    }
}

fn report_toolchain_emit_interface_hint(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    error: &ql_driver::ToolchainError,
) {
    if let Some(output_path) = build_output_path(path, options) {
        if toolchain_targets_build_output_path(error, &output_path) {
            report_build_output_path_failure(path, options, emit_interface, &output_path);
        } else {
            report_build_toolchain_failure(path, options, emit_interface);
        }
    } else {
        report_build_toolchain_failure(path, options, emit_interface);
    }
}

#[cfg(test)]
#[path = "build_single_source_reporting_tests.rs"]
mod tests;
