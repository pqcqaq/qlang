use std::path::{Path, PathBuf};

use ql_project::{load_project_manifest, package_name};

use super::{
    EmitPackageInterfaceError, ReferenceInterfacePrepError, ReferenceInterfacePrepFailureKind,
    absolute_user_path, normalize_path, prepare_reference_interfaces_for_manifests_quiet,
    relative_path_from,
};

const STDLIB_PACKAGES: [(&str, &str); 5] = [
    ("std.core", "core"),
    ("std.option", "option"),
    ("std.result", "result"),
    ("std.array", "array"),
    ("std.test", "test"),
];

pub(crate) fn resolve_stdlib_dependencies(
    package_root: &Path,
    stdlib_path: Option<&Path>,
) -> Result<Vec<(String, String)>, String> {
    let Some(stdlib_path) = stdlib_path else {
        return Ok(Vec::new());
    };

    let stdlib_root = absolute_user_path(stdlib_path);
    let package_root = absolute_user_path(package_root);
    STDLIB_PACKAGES
        .iter()
        .map(|(package_name, directory)| {
            let package_dir = stdlib_root.join("packages").join(directory);
            validate_stdlib_package(&package_dir, package_name)?;
            Ok((
                (*package_name).to_owned(),
                relative_path_from(&package_root, &package_dir),
            ))
        })
        .collect()
}

pub(crate) fn sync_stdlib_interfaces(
    manifest_paths: &[PathBuf],
    stdlib_path: Option<&Path>,
) -> Result<(), String> {
    if stdlib_path.is_none() {
        return Ok(());
    }

    prepare_reference_interfaces_for_manifests_quiet(manifest_paths).map_err(|error| {
        format!(
            "failed to prepare stdlib interface artifacts for initialized project: {}",
            reference_prep_error_message(&error)
        )
    })
}

fn validate_stdlib_package(package_root: &Path, expected_name: &str) -> Result<(), String> {
    let manifest = load_project_manifest(package_root).map_err(|error| {
        format!(
            "stdlib package `{expected_name}` is not available at `{}`: {error}",
            normalize_path(package_root)
        )
    })?;
    let actual_name = package_name(&manifest).map_err(|error| {
        format!(
            "stdlib package manifest `{}` is invalid: {error}",
            normalize_path(&manifest.manifest_path)
        )
    })?;
    if actual_name != expected_name {
        return Err(format!(
            "stdlib package manifest `{}` must declare `[package].name = \"{expected_name}\"`, found `{actual_name}`",
            normalize_path(&manifest.manifest_path)
        ));
    }
    Ok(())
}

fn reference_prep_error_message(error: &ReferenceInterfacePrepError) -> String {
    let reference = error
        .first_failure
        .reference
        .as_deref()
        .unwrap_or("<unknown>");
    let reference_manifest = normalize_path(&error.first_failure.reference_manifest_path);
    let detail = match &error.first_failure.failure_kind {
        ReferenceInterfacePrepFailureKind::Project { message, .. } => message.clone(),
        ReferenceInterfacePrepFailureKind::InterfaceEmit(emit_error) => {
            interface_emit_error_message(emit_error)
        }
    };
    let mut message =
        format!("referenced package `{reference}` at `{reference_manifest}` could not be prepared");
    if error.failure_count > 1 {
        message.push_str(&format!(
            "; {} referenced packages failed",
            error.failure_count
        ));
    }
    message.push_str(&format!("; first failure: {detail}"));
    message
}

fn interface_emit_error_message(error: &EmitPackageInterfaceError) -> String {
    match error {
        EmitPackageInterfaceError::Code { message, .. } => message
            .clone()
            .unwrap_or_else(|| "interface emission failed".to_owned()),
        EmitPackageInterfaceError::SourceFailure {
            failure_count,
            first_failing_source,
            ..
        } => {
            let source = first_failing_source
                .as_deref()
                .map(normalize_path)
                .unwrap_or_else(|| "<unknown>".to_owned());
            format!(
                "interface emission found {failure_count} failing source file(s); first failure: {source}"
            )
        }
        EmitPackageInterfaceError::ManifestNotFound { start } => format!(
            "could not find `qlang.toml` starting from `{}`",
            normalize_path(start)
        ),
        EmitPackageInterfaceError::ManifestFailure { message, .. } => message.clone(),
        EmitPackageInterfaceError::NoSourceFilesFailure { source_root, .. } => format!(
            "no `.ql` files found under `{}`",
            normalize_path(source_root)
        ),
        EmitPackageInterfaceError::SourceRootFailure { source_root, .. } => format!(
            "package source directory `{}` does not exist",
            normalize_path(source_root)
        ),
        EmitPackageInterfaceError::OutputPathFailure { message, .. } => message.clone(),
    }
}
