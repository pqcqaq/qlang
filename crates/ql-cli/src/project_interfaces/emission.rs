use std::fs;
use std::path::{Path, PathBuf};

use ql_analysis::analyze_source as analyze_semantics;
use ql_driver::{BuildError, acquire_build_output_locks, write_file_atomically};
use ql_project::{
    InterfaceArtifactStatus, collect_package_sources, default_interface_path,
    interface_artifact_status, load_project_manifest, package_name, package_source_root,
    render_module_interface,
};

use super::{EmitPackageInterfaceError, EmitPackageInterfaceResult};
use crate::cli_diagnostics::print_diagnostics;
use crate::cli_utils::{
    normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error,
};

pub(crate) fn emit_package_interface_path(
    path: &Path,
    output: Option<&Path>,
    command_label: &str,
    changed_only: bool,
) -> Result<EmitPackageInterfaceResult, EmitPackageInterfaceError> {
    emit_package_interface_path_impl(path, output, command_label, changed_only, true)
}

pub(crate) fn emit_package_interface_path_quiet(
    path: &Path,
    output: Option<&Path>,
    changed_only: bool,
) -> Result<EmitPackageInterfaceResult, EmitPackageInterfaceError> {
    emit_package_interface_path_impl(
        path,
        output,
        "`ql build --emit-interface`",
        changed_only,
        false,
    )
}

fn emit_package_interface_path_impl(
    path: &Path,
    output: Option<&Path>,
    command_label: &str,
    changed_only: bool,
    report_failure: bool,
) -> Result<EmitPackageInterfaceResult, EmitPackageInterfaceError> {
    let manifest = load_project_manifest(path).map_err(|error| match error {
        ql_project::ProjectError::ManifestNotFound { start } => {
            if report_failure {
                eprintln!(
                    "error: {command_label} requires a package manifest; could not find `qlang.toml` starting from `{}`",
                    normalize_path(&start)
                );
            }
            EmitPackageInterfaceError::ManifestNotFound { start }
        }
        error => {
            if let Some(manifest_path) =
                package_missing_name_manifest_path_from_project_error(&error)
            {
                if report_failure {
                    eprintln!(
                        "error: {} manifest `{}` does not declare `[package].name`",
                        command_label,
                        normalize_path(manifest_path)
                    );
                }
                EmitPackageInterfaceError::ManifestFailure {
                    manifest_path: manifest_path.to_path_buf(),
                    message: format!(
                        "manifest `{}` does not declare `[package].name`",
                        normalize_path(manifest_path)
                    ),
                }
            } else if let Some(manifest_path) =
                package_check_manifest_path_from_project_error(&error)
            {
                if report_failure {
                    eprintln!("error: {command_label} {error}");
                }
                EmitPackageInterfaceError::ManifestFailure {
                    manifest_path: manifest_path.to_path_buf(),
                    message: error.to_string(),
                }
            } else {
                if report_failure {
                    eprintln!("error: {error}");
                }
                EmitPackageInterfaceError::Code {
                    code: 1,
                    message: Some(error.to_string()),
                }
            }
        }
    })?;
    let package_name = package_name(&manifest).map_err(|error| {
        if report_failure {
            eprintln!("error: {command_label} {error}");
        }
        EmitPackageInterfaceError::ManifestFailure {
            manifest_path: manifest.manifest_path.clone(),
            message: error.to_string(),
        }
    })?;
    let output_path = output.map(Path::to_path_buf).unwrap_or_else(|| {
        default_interface_path(&manifest).expect("package emit should have a default qi path")
    });
    if changed_only
        && interface_artifact_status(&manifest, &output_path) == InterfaceArtifactStatus::Valid
    {
        return Ok(EmitPackageInterfaceResult::UpToDate(output_path));
    }

    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let source_root =
        package_source_root(&manifest).expect("package interface emission requires a package");
    let files = collect_package_sources(&manifest).map_err(|error| match error {
        ql_project::ProjectError::PackageSourceRootNotFound { path } => {
            if report_failure {
                eprintln!(
                    "error: {command_label} package source directory `{}` does not exist",
                    normalize_path(&path)
                );
            }
            EmitPackageInterfaceError::SourceRootFailure {
                manifest_path: manifest.manifest_path.clone(),
                source_root: path,
            }
        }
        error => {
            if report_failure {
                eprintln!("error: {error}");
            }
            EmitPackageInterfaceError::Code {
                code: 1,
                message: Some(error.to_string()),
            }
        }
    })?;
    if files.is_empty() {
        if report_failure {
            eprintln!(
                "error: {command_label} no `.ql` files found under `{}`",
                normalize_path(&source_root)
            );
        }
        return Err(EmitPackageInterfaceError::NoSourceFilesFailure {
            manifest_path: manifest.manifest_path.clone(),
            source_root,
        });
    }

    let mut rendered_modules = Vec::new();
    let mut failing_source_count = 0usize;
    let mut first_failing_source = None;
    for file in files {
        let source = fs::read_to_string(&file).map_err(|error| {
            if report_failure {
                eprintln!("error: failed to read `{}`: {error}", file.display());
            }
            error
        });
        let source = match source {
            Ok(source) => source,
            Err(_) => {
                failing_source_count += 1;
                record_first_failing_path(&mut first_failing_source, &file);
                continue;
            }
        };
        let analysis = match analyze_semantics(&source) {
            Ok(analysis) => analysis,
            Err(diagnostics) => {
                if report_failure {
                    print_diagnostics(&file, &source, &diagnostics);
                }
                failing_source_count += 1;
                record_first_failing_path(&mut first_failing_source, &file);
                continue;
            }
        };
        if analysis.has_errors() {
            if report_failure {
                print_diagnostics(&file, &source, analysis.diagnostics());
            }
            failing_source_count += 1;
            record_first_failing_path(&mut first_failing_source, &file);
            continue;
        }
        if let Some(rendered) = render_module_interface(analysis.ast()) {
            let relative = file.strip_prefix(manifest_dir).unwrap_or(&file);
            rendered_modules.push((normalize_path(relative), rendered));
        }
    }

    if failing_source_count > 0 {
        if report_failure {
            eprintln!("error: {command_label} found {failing_source_count} failing source file(s)");
            if failing_source_count > 1 {
                if let Some(path) = &first_failing_source {
                    eprintln!("note: first failing source file: {}", normalize_path(path));
                }
            }
        }
        return Err(EmitPackageInterfaceError::SourceFailure {
            code: 1,
            failure_count: failing_source_count,
            first_failing_source,
        });
    }

    if let Some(parent) = output_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            if report_failure {
                eprintln!(
                    "error: failed to create interface output directory `{}`: {error}",
                    parent.display()
                );
            }
            EmitPackageInterfaceError::OutputPathFailure {
                manifest_path: Some(manifest.manifest_path.clone()),
                output_path: output_path.clone(),
                message: format!(
                    "failed to create interface output directory `{}`: {error}",
                    normalize_path(parent)
                ),
            }
        })?;
    }

    let rendered = render_interface_artifact(package_name, &rendered_modules);
    let _output_locks = acquire_build_output_locks(vec![output_path.clone()]).map_err(|error| {
        let message = match error {
            BuildError::Io { path, error } => format!(
                "failed to acquire interface output lock `{}`: {error}",
                normalize_path(&path)
            ),
            BuildError::InvalidInput(message) => message,
            BuildError::Diagnostics { .. } => {
                "failed to acquire interface output lock due to diagnostics".to_owned()
            }
            BuildError::Toolchain { error, .. } => format!("{error}"),
        };
        if report_failure {
            eprintln!("error: {message}");
        }
        EmitPackageInterfaceError::OutputPathFailure {
            manifest_path: Some(manifest.manifest_path.clone()),
            output_path: output_path.clone(),
            message,
        }
    })?;
    write_file_atomically(&output_path, &rendered).map_err(|error| {
        if report_failure {
            eprintln!(
                "error: failed to write interface `{}`: {error}",
                output_path.display()
            );
        }
        EmitPackageInterfaceError::OutputPathFailure {
            manifest_path: Some(manifest.manifest_path.clone()),
            output_path: output_path.clone(),
            message: format!(
                "failed to write interface `{}`: {error}",
                normalize_path(&output_path)
            ),
        }
    })?;
    Ok(EmitPackageInterfaceResult::Wrote(output_path))
}

fn record_first_failing_path(slot: &mut Option<PathBuf>, path: &Path) {
    if slot.is_none() {
        *slot = Some(path.to_path_buf());
    }
}

pub(crate) fn render_interface_artifact(
    package_name: &str,
    modules: &[(String, String)],
) -> String {
    let mut rendered = String::new();
    rendered.push_str("// qlang interface v1\n");
    rendered.push_str(&format!("// package: {package_name}\n"));
    if modules.is_empty() {
        rendered.push('\n');
        return rendered;
    }

    for (path, module) in modules {
        rendered.push('\n');
        rendered.push_str(&format!("// source: {path}\n"));
        rendered.push_str(module);
    }

    rendered
}
