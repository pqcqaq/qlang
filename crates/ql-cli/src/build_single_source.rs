use std::path::{Path, PathBuf};

use ql_driver::{BuildArtifact, BuildError, BuildOptions, build_source_with_link_inputs};

use crate::build_single_source_input::prepare_single_source;
use crate::build_single_source_reporting::{
    report_single_source_build_error, report_single_source_build_success,
};

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
                report_single_source_build_success(&artifact);
            }
            Ok(artifact)
        }
        Err(error) if report_failure => Err(report_single_source_build_error(
            path,
            options,
            emit_interface,
            error,
        )),
        Err(_) => Err(1),
    }
}

pub(crate) fn build_single_source_target_with_inputs_result(
    path: &Path,
    options: &BuildOptions,
    source_override: Option<&str>,
    additional_link_inputs: &[PathBuf],
) -> Result<BuildArtifact, BuildError> {
    let prepared_source = prepare_single_source(path, source_override)?;
    build_source_with_link_inputs(
        path,
        prepared_source.source(),
        options,
        additional_link_inputs,
    )
}
