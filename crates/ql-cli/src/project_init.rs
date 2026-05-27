use std::path::{Path, PathBuf};

use ql_project::{load_project_manifest, package_name};

use crate::cli_utils::{
    absolute_user_path, normalize_path, relative_path_from, validate_project_package_name,
};

use crate::project_interfaces::{
    EmitPackageInterfaceError, ReferenceInterfacePrepError, ReferenceInterfacePrepFailureKind,
    prepare_reference_interfaces_for_manifests_quiet,
};

mod scaffold;
mod templates;

pub(crate) use scaffold::{create_package_scaffold, write_new_file};
use scaffold::{create_stdlib_package_scaffold, create_workspace_scaffold};
#[cfg(test)]
use scaffold::{render_package_manifest, render_workspace_manifest};
pub(crate) use templates::default_package_main_source;

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

pub(crate) fn project_init_path(
    path: &Path,
    workspace: bool,
    package_name: Option<&str>,
    stdlib_path: Option<&Path>,
) -> Result<(), u8> {
    let target_root = path.to_path_buf();
    let package_name = match project_init_package_name(&target_root, workspace, package_name) {
        Ok(package_name) => package_name,
        Err(message) => {
            eprintln!("error: `ql project init` {message}");
            return Err(1);
        }
    };

    let created_paths = if workspace {
        init_workspace_project(&target_root, &package_name, stdlib_path)
    } else {
        init_package_project(&target_root, &package_name, stdlib_path)
    }
    .map_err(|message| {
        eprintln!("error: `ql project init` {message}");
        1
    })?;

    for path in created_paths {
        println!("created: {}", normalize_path(&path));
    }

    Ok(())
}

pub(crate) fn ensure_target_root(target_root: &Path) -> Result<(), String> {
    if target_root.exists() && !target_root.is_dir() {
        return Err(format!(
            "target path `{}` already exists and is not a directory",
            normalize_path(target_root)
        ));
    }
    Ok(())
}

fn project_init_package_name(
    target_root: &Path,
    workspace: bool,
    package_name: Option<&str>,
) -> Result<String, String> {
    let package_name = match package_name {
        Some(package_name) => package_name.to_owned(),
        None if workspace => "app".to_owned(),
        None => target_root
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| {
                format!(
                    "could not derive a package name from `{}`; rerun with `--name <package>`",
                    normalize_path(target_root)
                )
            })?,
    };

    validate_project_package_name(&package_name)?;
    Ok(package_name)
}

fn init_package_project(
    target_root: &Path,
    package_name: &str,
    stdlib_path: Option<&Path>,
) -> Result<Vec<PathBuf>, String> {
    ensure_target_root(target_root)?;
    let dependencies = resolve_stdlib_dependencies(target_root, stdlib_path)?;
    let created_paths = if let Some(stdlib_path) = stdlib_path {
        create_stdlib_package_scaffold(target_root, package_name, &dependencies, stdlib_path)?
    } else {
        create_package_scaffold(target_root, package_name, &dependencies)?
    };
    sync_stdlib_interfaces(&[target_root.join("qlang.toml")], stdlib_path)?;
    Ok(created_paths)
}

fn init_workspace_project(
    target_root: &Path,
    package_name: &str,
    stdlib_path: Option<&Path>,
) -> Result<Vec<PathBuf>, String> {
    ensure_target_root(target_root)?;

    let member_dir = target_root.join("packages").join(package_name);
    let dependencies = resolve_stdlib_dependencies(&member_dir, stdlib_path)?;
    let stdlib_sources = stdlib_path
        .map(scaffold::load_stdlib_package_sources)
        .transpose()?;
    let package_sources;
    let sources = if let Some(sources) = &stdlib_sources {
        sources
    } else {
        package_sources = templates::default_package_sources();
        &package_sources
    };
    let created_paths =
        create_workspace_scaffold(target_root, package_name, &dependencies, sources)?;
    sync_stdlib_interfaces(&[member_dir.join("qlang.toml")], stdlib_path)?;
    Ok(created_paths)
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

#[cfg(test)]
#[path = "project_init_tests.rs"]
mod tests;
