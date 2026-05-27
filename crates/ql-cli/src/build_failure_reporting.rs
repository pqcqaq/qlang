use std::path::{Path, PathBuf};

use ql_driver::{BuildEmit, BuildOptions, BuildProfile, CHeaderSurface};
use ql_project::load_project_manifest;

use crate::build_reporting::build_emit_cli_value;
use crate::cli_utils::normalize_path;

pub(crate) fn report_remaining_build_artifacts(paths: &[PathBuf]) {
    for path in paths {
        eprintln!("note: build artifact remains at `{}`", normalize_path(path));
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

fn report_build_package_rerun_hint(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    reason: &str,
) {
    if let Ok(manifest) = load_project_manifest(path) {
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(&manifest.manifest_path)
        );
        let rerun_command = format_build_command(path, options, emit_interface);
        eprintln!("hint: rerun `{rerun_command}` {reason}");
    }
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
    if let Ok(manifest) = load_project_manifest(path) {
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(&manifest.manifest_path)
        );
        eprintln!("note: failing build input path: {}", normalize_path(path));
        let rerun_command = format_build_command(path, options, emit_interface);
        eprintln!("hint: rerun `{rerun_command}` after fixing the build input path");
    }
}

pub(crate) fn report_build_output_path_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    output_path: &Path,
) {
    if let Ok(manifest) = load_project_manifest(path) {
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(&manifest.manifest_path)
        );
        eprintln!(
            "note: failing build output path: {}",
            normalize_path(output_path)
        );
        let rerun_command = format_build_command(path, options, emit_interface);
        eprintln!("hint: rerun `{rerun_command}` after fixing the build output path");
    }
}

pub(crate) fn report_build_header_output_path_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    output_path: &Path,
) {
    if let Ok(manifest) = load_project_manifest(path) {
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(&manifest.manifest_path)
        );
        eprintln!(
            "note: failing build header output path: {}",
            normalize_path(output_path)
        );
        let rerun_command = format_build_command(path, options, emit_interface);
        eprintln!("hint: rerun `{rerun_command}` after fixing the build header output path");
    }
}

pub(crate) fn report_build_header_configuration_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
) {
    if let Ok(manifest) = load_project_manifest(path) {
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(&manifest.manifest_path)
        );
        let rerun_command = format_build_command(path, options, emit_interface);
        eprintln!("hint: rerun `{rerun_command}` after fixing the build header configuration");
    }
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
    eprintln!(
        "note: build artifact remains at `{}`",
        normalize_path(artifact_path)
    );
}

pub(crate) fn report_build_interface_package_context_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    artifact_path: &Path,
) {
    eprintln!(
        "note: `ql build --emit-interface` only emits package interfaces for sources inside a package"
    );
    let rerun_command = format_build_command(path, options, emit_interface);
    eprintln!("hint: rerun `{rerun_command}` after adding `qlang.toml` for this source");
    eprintln!(
        "note: build artifact remains at `{}`",
        normalize_path(artifact_path)
    );
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
    eprintln!(
        "note: build artifact remains at `{}`",
        normalize_path(artifact_path)
    );
}

pub(crate) fn report_build_interface_manifest_failure(
    path: &Path,
    options: &BuildOptions,
    emit_interface: bool,
    artifact_path: &Path,
    manifest_path: &Path,
) {
    eprintln!(
        "note: failing package manifest: {}",
        normalize_path(manifest_path)
    );
    let rerun_command = format_build_command(path, options, emit_interface);
    eprintln!("hint: rerun `{rerun_command}` after fixing the package manifest");
    eprintln!(
        "note: build artifact remains at `{}`",
        normalize_path(artifact_path)
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
    eprintln!(
        "note: failing package manifest: {}",
        normalize_path(manifest_path)
    );
    eprintln!(
        "note: failing package source root: {}",
        normalize_path(source_root)
    );
    let rerun_command = format_build_command(path, options, emit_interface);
    eprintln!("hint: rerun `{rerun_command}` after fixing the package source root");
    eprintln!(
        "note: build artifact remains at `{}`",
        normalize_path(artifact_path)
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
    eprintln!(
        "note: failing package manifest: {}",
        normalize_path(manifest_path)
    );
    eprintln!(
        "note: failing package source root: {}",
        normalize_path(source_root)
    );
    let rerun_command = format_build_command(path, options, emit_interface);
    eprintln!("hint: rerun `{rerun_command}` after adding package source files");
    eprintln!(
        "note: build artifact remains at `{}`",
        normalize_path(artifact_path)
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
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(&manifest.manifest_path)
        );
        eprintln!(
            "note: failing interface output path: {}",
            normalize_path(output_path)
        );
        let rerun_command = format_build_command(path, options, emit_interface);
        eprintln!("hint: rerun `{rerun_command}` after fixing the interface output path");
    }
    eprintln!(
        "note: build artifact remains at `{}`",
        normalize_path(artifact_path)
    );
}
