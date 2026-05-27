use std::fs;
use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use crate::cli_utils::{absolute_user_path, normalize_path};

use super::templates;

struct ScaffoldFile {
    path: PathBuf,
    contents: String,
}

pub(crate) fn create_package_scaffold(
    target_root: &Path,
    package_name: &str,
    dependencies: &[(String, String)],
) -> Result<Vec<PathBuf>, String> {
    let sources = templates::default_package_sources();
    create_package_scaffold_with_sources(target_root, package_name, dependencies, &sources)
}

pub(super) fn create_stdlib_package_scaffold(
    target_root: &Path,
    package_name: &str,
    dependencies: &[(String, String)],
    stdlib_path: &Path,
) -> Result<Vec<PathBuf>, String> {
    let sources = load_stdlib_package_sources(stdlib_path)?;
    create_package_scaffold_with_sources(target_root, package_name, dependencies, &sources)
}

pub(super) fn create_workspace_scaffold(
    target_root: &Path,
    package_name: &str,
    dependencies: &[(String, String)],
    sources: &templates::PackageSources,
) -> Result<Vec<PathBuf>, String> {
    let workspace_manifest_path = target_root.join("qlang.toml");
    let member_dir = target_root.join("packages").join(package_name);
    let mut files = vec![ScaffoldFile {
        path: workspace_manifest_path,
        contents: render_workspace_manifest(package_name),
    }];
    files.extend(package_scaffold_files(
        &member_dir,
        package_name,
        dependencies,
        sources,
    ));
    write_scaffold_files(&files)
}

pub(super) fn load_stdlib_package_sources(
    stdlib_path: &Path,
) -> Result<templates::PackageSources, String> {
    templates::stdlib_package_sources(&absolute_user_path(stdlib_path))
}

fn create_package_scaffold_with_sources(
    target_root: &Path,
    package_name: &str,
    dependencies: &[(String, String)],
    sources: &templates::PackageSources,
) -> Result<Vec<PathBuf>, String> {
    let files = package_scaffold_files(target_root, package_name, dependencies, sources);
    write_scaffold_files(&files)
}

fn package_scaffold_files(
    target_root: &Path,
    package_name: &str,
    dependencies: &[(String, String)],
    sources: &templates::PackageSources,
) -> Vec<ScaffoldFile> {
    let manifest_path = target_root.join("qlang.toml");
    let source_path = target_root.join("src").join("lib.ql");
    let main_path = target_root.join("src").join("main.ql");
    let test_path = target_root.join("tests").join("smoke.ql");
    vec![
        ScaffoldFile {
            path: manifest_path,
            contents: render_package_manifest(package_name, dependencies),
        },
        ScaffoldFile {
            path: source_path,
            contents: sources.package_source.clone(),
        },
        ScaffoldFile {
            path: main_path,
            contents: sources.main_source.clone(),
        },
        ScaffoldFile {
            path: test_path,
            contents: sources.test_source.clone(),
        },
    ]
}

fn write_scaffold_files(files: &[ScaffoldFile]) -> Result<Vec<PathBuf>, String> {
    ensure_new_file_paths(files.iter().map(|file| file.path.as_path()))?;
    for file in files {
        write_new_file(&file.path, &file.contents)?;
    }
    Ok(files.iter().map(|file| file.path.clone()).collect())
}

fn ensure_new_file_paths<'a>(paths: impl IntoIterator<Item = &'a Path>) -> Result<(), String> {
    for path in paths {
        if path.exists() {
            return Err(format!(
                "would overwrite existing path `{}`",
                normalize_path(path)
            ));
        }
    }
    Ok(())
}

pub(crate) fn write_new_file(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create directory `{}`: {error}",
                normalize_path(parent)
            )
        })?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| match error.kind() {
            ErrorKind::AlreadyExists => {
                format!("would overwrite existing path `{}`", normalize_path(path))
            }
            _ => format!("failed to write `{}`: {error}", normalize_path(path)),
        })?;
    file.write_all(contents.as_bytes())
        .map_err(|error| format!("failed to write `{}`: {error}", normalize_path(path)))
}

pub(crate) fn render_workspace_manifest(package_name: &str) -> String {
    format!("[workspace]\nmembers = [\"packages/{package_name}\"]\n")
}

pub(crate) fn render_package_manifest(
    package_name: &str,
    dependencies: &[(String, String)],
) -> String {
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
