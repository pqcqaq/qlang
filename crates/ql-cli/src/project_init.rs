use std::fs;
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

pub(crate) fn ensure_target_root(target_root: &Path) -> Result<(), String> {
    if target_root.exists() && !target_root.is_dir() {
        return Err(format!(
            "target path `{}` already exists and is not a directory",
            normalize_path(target_root)
        ));
    }
    Ok(())
}

pub(crate) fn create_package_scaffold(
    target_root: &Path,
    package_name: &str,
    dependencies: &[(String, String)],
    stdlib_template: bool,
) -> Result<Vec<PathBuf>, String> {
    let manifest_path = target_root.join("qlang.toml");
    let source_path = target_root.join("src").join("lib.ql");
    let main_path = target_root.join("src").join("main.ql");
    let test_path = target_root.join("tests").join("smoke.ql");
    let manifest = render_package_manifest(package_name, dependencies);
    let package_source = if stdlib_template {
        stdlib_package_source()
    } else {
        default_package_source()
    };
    let main_source = if stdlib_template {
        stdlib_package_main_source()
    } else {
        default_package_main_source()
    };
    let test_source = if stdlib_template {
        super::stdlib_package_test_source()
    } else {
        default_package_test_source()
    };

    write_new_file(&manifest_path, &manifest)?;
    write_new_file(&source_path, package_source)?;
    write_new_file(&main_path, main_source)?;
    write_new_file(&test_path, test_source)?;

    Ok(vec![manifest_path, source_path, main_path, test_path])
}

pub(crate) fn write_new_file(path: &Path, contents: &str) -> Result<(), String> {
    if path.exists() {
        return Err(format!(
            "would overwrite existing path `{}`",
            normalize_path(path)
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create directory `{}`: {error}",
                normalize_path(parent)
            )
        })?;
    }
    fs::write(path, contents)
        .map_err(|error| format!("failed to write `{}`: {error}", normalize_path(path)))
}

pub(crate) fn render_workspace_manifest(package_name: &str) -> String {
    format!("[workspace]\nmembers = [\"packages/{package_name}\"]\n")
}

pub(crate) fn default_package_main_source() -> &'static str {
    "fn main() -> Int {\n    return 0\n}\n"
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

fn render_package_manifest(package_name: &str, dependencies: &[(String, String)]) -> String {
    let mut manifest = format!("[package]\nname = {}\n", toml_string_literal(package_name));
    if !dependencies.is_empty() {
        manifest.push_str("\n[dependencies]\n");
        for (dependency_name, dependency_path) in dependencies {
            manifest.push_str(&format!(
                "{} = {}\n",
                toml_key(dependency_name),
                toml_string_literal(dependency_path)
            ));
        }
    }
    manifest
}

fn toml_key(key: &str) -> String {
    if key
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        key.to_owned()
    } else {
        toml_string_literal(key)
    }
}

fn toml_string_literal(value: &str) -> String {
    let mut rendered = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => rendered.push_str("\\\\"),
            '"' => rendered.push_str("\\\""),
            '\n' => rendered.push_str("\\n"),
            '\r' => rendered.push_str("\\r"),
            '\t' => rendered.push_str("\\t"),
            _ => rendered.push(ch),
        }
    }
    rendered.push('"');
    rendered
}

fn default_package_source() -> &'static str {
    "pub fn run() -> Int {\n    return 0\n}\n"
}

fn default_package_test_source() -> &'static str {
    "fn main() -> Int {\n    return 0\n}\n"
}

fn stdlib_package_source() -> &'static str {
    r#"use std.array.sum3_int_array as sum3_int_array
use std.core.clamp_int as clamp_int
use std.option.some_int as some_int
use std.option.unwrap_or_int as unwrap_or_int
use std.result.ok_int as result_ok_int
use std.result.unwrap_result_or_int as result_unwrap_or_int

pub fn run() -> Int {
    return clamp_int(result_unwrap_or_int(result_ok_int(unwrap_or_int(some_int(42), 0)), 0) + sum3_int_array([1, 2, 3]), 0, 100)
}
"#
}

fn stdlib_package_main_source() -> &'static str {
    r#"use std.array.all3_bool_array as all3_bool_array
use std.core.bool_to_int as bool_to_int
use std.option.some_bool as some_bool
use std.option.unwrap_or_bool as unwrap_or_bool
use std.result.ok_bool as result_ok_bool
use std.result.unwrap_result_or_bool as result_unwrap_or_bool

fn main() -> Int {
    return 1 - bool_to_int(result_unwrap_or_bool(result_ok_bool(all3_bool_array([true, unwrap_or_bool(some_bool(true), false), true])), false))
}
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_package_manifest_quotes_dotted_dependency_keys() {
        let manifest = render_package_manifest(
            "demo-app",
            &[("std.core".to_owned(), "../stdlib/packages/core".to_owned())],
        );

        assert_eq!(
            manifest,
            "[package]\nname = \"demo-app\"\n\n[dependencies]\n\"std.core\" = \"../stdlib/packages/core\"\n"
        );
    }

    #[test]
    fn render_package_manifest_escapes_string_literals() {
        let manifest = render_package_manifest(
            "demo\"app",
            &[("local_dep".to_owned(), "..\\deps\tcore\nnext".to_owned())],
        );

        assert_eq!(
            manifest,
            "[package]\nname = \"demo\\\"app\"\n\n[dependencies]\nlocal_dep = \"..\\\\deps\\tcore\\nnext\"\n"
        );
    }

    #[test]
    fn render_workspace_manifest_uses_conventional_packages_member() {
        assert_eq!(
            render_workspace_manifest("app"),
            "[workspace]\nmembers = [\"packages/app\"]\n"
        );
    }
}
