use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use ql_analysis::analyze_source as analyze_semantics;
use ql_driver::{BuildError, acquire_build_output_locks, write_file_atomically};
use ql_project::{
    InterfaceArtifactStatus, collect_package_sources, default_interface_path,
    interface_artifact_status, load_project_manifest, package_name, package_source_root,
    render_module_interface,
};

use crate::cli_diagnostics::print_diagnostics;
use crate::cli_utils::{
    normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error,
};
use crate::project_manifest_paths::{record_reference_failure_manifest, reference_manifest_path};

#[derive(Debug)]
pub(crate) enum EmitPackageInterfaceResult {
    Wrote(PathBuf),
    UpToDate(PathBuf),
}

#[derive(Debug)]
pub(crate) enum EmitPackageInterfaceError {
    Code {
        code: u8,
        message: Option<String>,
    },
    SourceFailure {
        code: u8,
        failure_count: usize,
        first_failing_source: Option<PathBuf>,
    },
    ManifestNotFound {
        start: PathBuf,
    },
    ManifestFailure {
        manifest_path: PathBuf,
        message: String,
    },
    NoSourceFilesFailure {
        manifest_path: PathBuf,
        source_root: PathBuf,
    },
    SourceRootFailure {
        manifest_path: PathBuf,
        source_root: PathBuf,
    },
    OutputPathFailure {
        manifest_path: Option<PathBuf>,
        output_path: PathBuf,
        message: String,
    },
}

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
            if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(&error)
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

fn record_first_failing_path(slot: &mut Option<PathBuf>, path: &Path) {
    if slot.is_none() {
        *slot = Some(path.to_path_buf());
    }
}

fn render_interface_artifact(package_name: &str, modules: &[(String, String)]) -> String {
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

#[cfg(test)]
#[path = "project_interfaces_tests.rs"]
mod tests;
