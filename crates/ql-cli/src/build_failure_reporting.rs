use std::path::{Path, PathBuf};

use ql_driver::{BuildEmit, BuildOptions, BuildProfile, CHeaderSurface};
use ql_project::load_project_manifest;

use crate::build_reporting::build_emit_cli_value;
use crate::cli_utils::normalize_path;

pub(crate) fn report_remaining_build_artifacts(paths: &[PathBuf]) {
    for path in paths {
        report_line(build_artifact_remaining_note(path));
    }
}

pub(crate) fn unsupported_build_header_emit(options: &BuildOptions) -> bool {
    options.c_header.is_some()
        && !matches!(
            options.emit,
            BuildEmit::DynamicLibrary | BuildEmit::StaticLibrary
        )
}

pub(crate) fn missing_build_input_path(path: &Path, message: &str) -> bool {
    !path.is_file() && message.contains("is not a file")
}

pub(crate) fn missing_dylib_exports(message: &str, options: &BuildOptions) -> bool {
    options.emit == BuildEmit::DynamicLibrary
        && message
            .contains("requires at least one public top-level `extern \"c\"` function definition")
}

pub(crate) fn missing_build_header_import_surface(message: &str, options: &BuildOptions) -> bool {
    options.c_header.is_some()
        && message.contains("does not define any imported `extern \"c\"` function declarations")
}

fn format_build_command(path: &Path, options: &BuildOptions, emit_interface: bool) -> String {
    let mut command = format!("ql build {}", normalize_path(path));
    command.push_str(&format!(" --emit {}", build_emit_cli_value(options.emit)));
    if options.profile == BuildProfile::Release {
        command.push_str(" --release");
    }
    if let Some(output) = &options.output {
        command.push_str(&format!(" --output {}", normalize_path(output)));
    }
    if let Some(header) = &options.c_header {
        if header.surface != CHeaderSurface::Exports {
            command.push_str(&format!(" --header-surface {}", header.surface.as_str()));
        } else if header.output.is_none() {
            command.push_str(" --header");
        }
        if let Some(output) = &header.output {
            command.push_str(&format!(" --header-output {}", normalize_path(output)));
        }
    }
    if emit_interface {
        command.push_str(" --emit-interface");
    }
    command
}

fn report_line(line: String) {
    eprintln!("{line}");
}

fn report_lines(lines: Vec<String>) {
    for line in lines {
        report_line(line);
    }
}

fn build_path_note(label: &str, path: &Path) -> String {
    format!("note: {label}: {}", normalize_path(path))
}

fn build_artifact_remaining_note(path: &Path) -> String {
    format!("note: build artifact remains at `{}`", normalize_path(path))
}

fn build_rerun_hint(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    reason: &str,
) -> String {
    let rerun_command = format_build_command(path, options, emit_interface);
    format!("hint: rerun `{rerun_command}` {reason}")
}

fn build_manifest_rerun_lines(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    manifest_path: &Path,
    extra_path_note: Option<(&str, &Path)>,
    reason: &str,
) -> Vec<String> {
    let mut lines = vec![build_path_note("failing package manifest", manifest_path)];
    if let Some((label, path)) = extra_path_note {
        lines.push(build_path_note(label, path));
    }
    lines.push(build_rerun_hint(path, options, emit_interface, reason));
    lines
}

fn report_loaded_manifest_rerun_hint(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    extra_path_note: Option<(&str, &Path)>,
    reason: &str,
) {
    if let Ok(manifest) = load_project_manifest(path) {
        report_lines(build_manifest_rerun_lines(
            path,
            options,
            emit_interface,
            &manifest.manifest_path,
            extra_path_note,
            reason,
        ));
    }
}

fn report_lines_with_remaining_artifact(mut lines: Vec<String>, artifact_path: &Path) {
    lines.push(build_artifact_remaining_note(artifact_path));
    report_lines(lines);
}

fn report_build_package_rerun_hint(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    reason: &str,
) {
    report_loaded_manifest_rerun_hint(path, options, emit_interface, None, reason);
}

pub(crate) fn report_build_source_diagnostics_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
) {
    report_build_package_rerun_hint(
        path,
        options,
        emit_interface,
        "after fixing the package sources",
    );
}

pub(crate) fn report_build_toolchain_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
) {
    report_build_package_rerun_hint(
        path,
        options,
        emit_interface,
        "after fixing the build toolchain",
    );
}

pub(crate) fn report_build_export_configuration_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
) {
    report_build_package_rerun_hint(
        path,
        options,
        emit_interface,
        "after fixing the dylib export surface",
    );
}

pub(crate) fn report_build_header_import_surface_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
) {
    report_build_package_rerun_hint(
        path,
        options,
        emit_interface,
        "after fixing the build header import surface",
    );
}

pub(crate) fn report_build_input_path_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
) {
    report_loaded_manifest_rerun_hint(
        path,
        options,
        emit_interface,
        Some(("failing build input path", path)),
        "after fixing the build input path",
    );
}

pub(crate) fn report_build_output_path_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    output_path: &Path,
) {
    report_loaded_manifest_rerun_hint(
        path,
        options,
        emit_interface,
        Some(("failing build output path", output_path)),
        "after fixing the build output path",
    );
}

pub(crate) fn report_build_header_output_path_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    output_path: &Path,
) {
    report_loaded_manifest_rerun_hint(
        path,
        options,
        emit_interface,
        Some(("failing build header output path", output_path)),
        "after fixing the build header output path",
    );
}

pub(crate) fn report_build_header_configuration_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
) {
    report_loaded_manifest_rerun_hint(
        path,
        options,
        emit_interface,
        None,
        "after fixing the build header configuration",
    );
}

pub(crate) fn report_build_interface_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    artifact_path: &Path,
) {
    report_build_package_rerun_hint(
        path,
        options,
        emit_interface,
        "after fixing the package interface error",
    );
    report_line(build_artifact_remaining_note(artifact_path));
}

pub(crate) fn report_build_interface_package_context_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    artifact_path: &Path,
) {
    report_lines_with_remaining_artifact(vec![
        "note: `ql build --emit-interface` only emits package interfaces for sources inside a package"
            .to_owned(),
        build_rerun_hint(
            path,
            options,
            emit_interface,
            "after adding `qlang.toml` for this source",
        ),
    ], artifact_path);
}

pub(crate) fn report_build_interface_source_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    artifact_path: &Path,
) {
    report_build_package_rerun_hint(
        path,
        options,
        emit_interface,
        "after fixing the package sources",
    );
    report_line(build_artifact_remaining_note(artifact_path));
}

pub(crate) fn report_build_interface_manifest_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    artifact_path: &Path,
    manifest_path: &Path,
) {
    report_lines_with_remaining_artifact(
        build_manifest_rerun_lines(
            path,
            options,
            emit_interface,
            manifest_path,
            None,
            "after fixing the package manifest",
        ),
        artifact_path,
    );
}

pub(crate) fn report_build_interface_source_root_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    artifact_path: &Path,
    manifest_path: &Path,
    source_root: &Path,
) {
    report_lines_with_remaining_artifact(
        build_manifest_rerun_lines(
            path,
            options,
            emit_interface,
            manifest_path,
            Some(("failing package source root", source_root)),
            "after fixing the package source root",
        ),
        artifact_path,
    );
}

pub(crate) fn report_build_interface_no_sources_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    artifact_path: &Path,
    manifest_path: &Path,
    source_root: &Path,
) {
    report_lines_with_remaining_artifact(
        build_manifest_rerun_lines(
            path,
            options,
            emit_interface,
            manifest_path,
            Some(("failing package source root", source_root)),
            "after adding package source files",
        ),
        artifact_path,
    );
}

pub(crate) fn report_build_interface_output_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    artifact_path: &Path,
    output_path: &Path,
) {
    if let Ok(manifest) = load_project_manifest(path) {
        report_lines(build_manifest_rerun_lines(
            path,
            options,
            emit_interface,
            &manifest.manifest_path,
            Some(("failing interface output path", output_path)),
            "after fixing the interface output path",
        ));
    }
    report_line(build_artifact_remaining_note(artifact_path));
}

#[cfg(test)]
#[path = "build_failure_reporting_tests.rs"]
mod tests;
