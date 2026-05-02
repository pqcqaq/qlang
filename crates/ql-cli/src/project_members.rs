use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use ql_project::{package_name, render_manifest_with_added_binary_target};
use toml::Value as TomlValue;

use super::{
    detach_workspace_member_dependents, find_workspace_member_with_package_name, normalize_path,
    project_init, relative_path_from, resolve_project_package_manifest,
    resolve_project_selected_package_manifest, resolve_project_workspace_manifest,
    validate_project_package_name,
};

use crate::project_dependencies::find_workspace_member_dependents;

pub(crate) fn project_add_path(
    path: &Path,
    package_name: &str,
    dependencies: &[String],
) -> Result<(), u8> {
    if let Err(message) = validate_project_package_name(package_name) {
        eprintln!("error: `ql project add` {message}");
        return Err(1);
    }

    let workspace_manifest = resolve_project_workspace_manifest(path).map_err(|message| {
        eprintln!("error: `ql project add` {message}");
        1
    })?;
    let dependency_entries =
        resolve_project_add_dependency_entries(&workspace_manifest, package_name, dependencies)
            .map_err(|message| {
                eprintln!("error: `ql project add` {message}");
                1
            })?;
    let (updated_manifest_path, created_paths) =
        add_workspace_project_member(&workspace_manifest, package_name, &dependency_entries)
            .map_err(|message| {
                eprintln!("error: `ql project add` {message}");
                1
            })?;

    println!("updated: {}", normalize_path(&updated_manifest_path));
    for path in created_paths {
        println!("created: {}", normalize_path(&path));
    }

    Ok(())
}

pub(crate) fn project_add_existing_path(path: &Path, existing_path: &Path) -> Result<(), u8> {
    let workspace_manifest = resolve_project_workspace_manifest(path).map_err(|message| {
        eprintln!("error: `ql project add` {message}");
        1
    })?;
    let package_manifest = resolve_project_package_manifest(existing_path).map_err(|message| {
        eprintln!("error: `ql project add` {message}");
        1
    })?;
    let (updated_manifest_path, added_member_root) =
        add_existing_workspace_project_member(&workspace_manifest, &package_manifest).map_err(
            |message| {
                eprintln!("error: `ql project add` {message}");
                1
            },
        )?;

    println!("updated: {}", normalize_path(&updated_manifest_path));
    println!("added: {}", normalize_path(&added_member_root));
    Ok(())
}

pub(crate) fn project_remove_path(
    path: &Path,
    package_name: &str,
    cascade: bool,
) -> Result<(), u8> {
    if let Err(message) = validate_project_package_name(package_name) {
        eprintln!("error: `ql project remove` {message}");
        return Err(1);
    }

    let workspace_manifest = resolve_project_workspace_manifest(path).map_err(|message| {
        eprintln!("error: `ql project remove` {message}");
        1
    })?;
    let (updated_manifest_path, updated_dependency_manifests, removed_member_root) =
        remove_workspace_project_member(&workspace_manifest, package_name, cascade).map_err(
            |message| {
                eprintln!("error: `ql project remove` {message}");
                1
            },
        )?;

    println!("updated: {}", normalize_path(&updated_manifest_path));
    for manifest_path in updated_dependency_manifests {
        println!("updated: {}", normalize_path(&manifest_path));
    }
    println!("removed: {}", normalize_path(&removed_member_root));
    Ok(())
}

pub(crate) fn project_add_binary_target_path(
    path: &Path,
    target_package_name: Option<&str>,
    binary_name: &str,
) -> Result<(), u8> {
    if let Err(message) = validate_project_package_name(binary_name) {
        eprintln!("error: `ql project target add` {message}");
        return Err(1);
    }

    let (_, package_manifest) = resolve_project_selected_package_manifest(
        path,
        target_package_name,
        "`ql project target add`",
    )?;
    let package_root = package_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    let binary_relative_path = format!("src/bin/{binary_name}.ql");
    let binary_path = package_root
        .join("src")
        .join("bin")
        .join(format!("{binary_name}.ql"));
    let preserved_binary_paths = if package_manifest.bins.is_empty() {
        collect_conventional_binary_target_relative_paths(&package_manifest)?
    } else {
        Vec::new()
    };
    let package_manifest_source =
        fs::read_to_string(&package_manifest.manifest_path).map_err(|error| {
            eprintln!(
                "error: `ql project target add` failed to read `{}`: {error}",
                normalize_path(&package_manifest.manifest_path)
            );
            1
        })?;
    let updated_package_manifest = render_manifest_with_added_binary_target(
        &package_manifest_source,
        &preserved_binary_paths,
        &binary_relative_path,
    )
    .map_err(|message| {
        eprintln!("error: `ql project target add` {message}");
        1
    })?;

    project_init::write_new_file(&binary_path, project_init::default_package_main_source())
        .map_err(|message| {
            eprintln!("error: `ql project target add` {message}");
            1
        })?;
    fs::write(&package_manifest.manifest_path, updated_package_manifest).map_err(|error| {
        eprintln!(
            "error: `ql project target add` failed to write `{}`: {error}",
            normalize_path(&package_manifest.manifest_path)
        );
        1
    })?;

    println!(
        "updated: {}",
        normalize_path(&package_manifest.manifest_path)
    );
    println!("created: {}", normalize_path(&binary_path));
    Ok(())
}

fn add_workspace_project_member(
    workspace_manifest: &ql_project::ProjectManifest,
    package_name: &str,
    dependencies: &[(String, String)],
) -> Result<(PathBuf, Vec<PathBuf>), String> {
    let Some(workspace) = workspace_manifest.workspace.as_ref() else {
        return Err(format!(
            "manifest `{}` is not a workspace",
            normalize_path(&workspace_manifest.manifest_path)
        ));
    };

    let workspace_root = workspace_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    let packages_dir = workspace_root.join("packages");
    let member_relative_path = format!("packages/{package_name}");
    let member_root = packages_dir.join(package_name);

    if workspace
        .members
        .iter()
        .any(|member| normalize_path(Path::new(member)) == member_relative_path)
    {
        return Err(format!(
            "workspace manifest `{}` already declares member `{member_relative_path}`",
            normalize_path(&workspace_manifest.manifest_path)
        ));
    }
    if let Some(existing_package_manifest) =
        find_workspace_member_with_package_name(workspace_manifest, package_name)
    {
        return Err(format!(
            "workspace manifest `{}` already contains package `{package_name}` at `{}`",
            normalize_path(&workspace_manifest.manifest_path),
            normalize_path(&existing_package_manifest)
        ));
    }
    if packages_dir.exists() && !packages_dir.is_dir() {
        return Err(format!(
            "would overwrite existing path `{}`",
            normalize_path(&packages_dir)
        ));
    }
    if member_root.exists() {
        return Err(format!(
            "would overwrite existing path `{}`",
            normalize_path(&member_root)
        ));
    }

    let workspace_manifest_source =
        fs::read_to_string(&workspace_manifest.manifest_path).map_err(|error| {
            format!(
                "failed to read `{}`: {error}",
                normalize_path(&workspace_manifest.manifest_path)
            )
        })?;
    let updated_workspace_manifest =
        append_workspace_manifest_member(&workspace_manifest_source, &member_relative_path)?;

    fs::write(
        &workspace_manifest.manifest_path,
        updated_workspace_manifest,
    )
    .map_err(|error| {
        format!(
            "failed to write `{}`: {error}",
            normalize_path(&workspace_manifest.manifest_path)
        )
    })?;
    let created_paths =
        project_init::create_package_scaffold(&member_root, package_name, dependencies, false)?;

    Ok((workspace_manifest.manifest_path.clone(), created_paths))
}

fn add_existing_workspace_project_member(
    workspace_manifest: &ql_project::ProjectManifest,
    package_manifest: &ql_project::ProjectManifest,
) -> Result<(PathBuf, PathBuf), String> {
    let Some(workspace) = workspace_manifest.workspace.as_ref() else {
        return Err(format!(
            "manifest `{}` is not a workspace",
            normalize_path(&workspace_manifest.manifest_path)
        ));
    };

    if normalize_path(&workspace_manifest.manifest_path)
        == normalize_path(&package_manifest.manifest_path)
    {
        return Err(format!(
            "does not accept workspace manifest `{}` as an existing member package",
            normalize_path(&workspace_manifest.manifest_path)
        ));
    }

    let package_name = package_name(package_manifest)
        .map_err(|error| format!("failed to resolve existing package name: {error}"))?;
    let workspace_root = workspace_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    let member_root = package_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();
    let member_relative_path = relative_path_from(workspace_root, &member_root);

    if workspace
        .members
        .iter()
        .any(|member| normalize_path(Path::new(member)) == member_relative_path)
    {
        return Err(format!(
            "workspace manifest `{}` already declares member `{member_relative_path}`",
            normalize_path(&workspace_manifest.manifest_path)
        ));
    }
    if let Some(existing_package_manifest) =
        find_workspace_member_with_package_name(workspace_manifest, package_name)
    {
        return Err(format!(
            "workspace manifest `{}` already contains package `{package_name}` at `{}`",
            normalize_path(&workspace_manifest.manifest_path),
            normalize_path(&existing_package_manifest)
        ));
    }

    let workspace_manifest_source =
        fs::read_to_string(&workspace_manifest.manifest_path).map_err(|error| {
            format!(
                "failed to read `{}`: {error}",
                normalize_path(&workspace_manifest.manifest_path)
            )
        })?;
    let updated_workspace_manifest =
        append_workspace_manifest_member(&workspace_manifest_source, &member_relative_path)?;
    fs::write(
        &workspace_manifest.manifest_path,
        updated_workspace_manifest,
    )
    .map_err(|error| {
        format!(
            "failed to write `{}`: {error}",
            normalize_path(&workspace_manifest.manifest_path)
        )
    })?;

    Ok((workspace_manifest.manifest_path.clone(), member_root))
}

fn remove_workspace_project_member(
    workspace_manifest: &ql_project::ProjectManifest,
    package_name: &str,
    cascade: bool,
) -> Result<(PathBuf, Vec<PathBuf>, PathBuf), String> {
    let Some(_workspace) = workspace_manifest.workspace.as_ref() else {
        return Err(format!(
            "manifest `{}` is not a workspace",
            normalize_path(&workspace_manifest.manifest_path)
        ));
    };

    let member_entries =
        super::find_workspace_member_entries_by_package_name(workspace_manifest, package_name);
    if member_entries.is_empty() {
        return Err(format!(
            "workspace manifest `{}` does not contain member package `{package_name}`",
            normalize_path(&workspace_manifest.manifest_path)
        ));
    }
    if member_entries.len() > 1 {
        let matching_members = member_entries
            .iter()
            .map(|(member, _)| member.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "workspace manifest `{}` contains multiple members for package `{package_name}`: {matching_members}",
            normalize_path(&workspace_manifest.manifest_path)
        ));
    }

    let (member_entry, member_manifest_path) = &member_entries[0];
    let dependent_members =
        find_workspace_member_dependents(workspace_manifest, member_manifest_path)?;
    if !dependent_members.is_empty() {
        if cascade {
            let updated_dependency_manifests = detach_workspace_member_dependents(
                package_name,
                member_manifest_path,
                &dependent_members,
            )?;
            let workspace_manifest_source = fs::read_to_string(&workspace_manifest.manifest_path)
                .map_err(|error| {
                format!(
                    "failed to read `{}`: {error}",
                    normalize_path(&workspace_manifest.manifest_path)
                )
            })?;
            let updated_workspace_manifest =
                remove_workspace_manifest_member(&workspace_manifest_source, member_entry)?;
            fs::write(
                &workspace_manifest.manifest_path,
                updated_workspace_manifest,
            )
            .map_err(|error| {
                format!(
                    "failed to write `{}`: {error}",
                    normalize_path(&workspace_manifest.manifest_path)
                )
            })?;

            return Ok((
                workspace_manifest.manifest_path.clone(),
                updated_dependency_manifests,
                member_manifest_path
                    .parent()
                    .unwrap_or(Path::new("."))
                    .to_path_buf(),
            ));
        }

        let dependent_members = dependent_members
            .iter()
            .map(|dependent| format!("{} ({})", dependent.member, dependent.package_name))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "cannot remove member package `{package_name}` from workspace manifest `{}` because other members still depend on it: {dependent_members}; remove those edges first with `ql project remove-dependency <member> --name {package_name}` or rerun with `ql project remove <file-or-dir> --name {package_name} --cascade`",
            normalize_path(&workspace_manifest.manifest_path)
        ));
    }
    let workspace_manifest_source =
        fs::read_to_string(&workspace_manifest.manifest_path).map_err(|error| {
            format!(
                "failed to read `{}`: {error}",
                normalize_path(&workspace_manifest.manifest_path)
            )
        })?;
    let updated_workspace_manifest =
        remove_workspace_manifest_member(&workspace_manifest_source, member_entry)?;
    fs::write(
        &workspace_manifest.manifest_path,
        updated_workspace_manifest,
    )
    .map_err(|error| {
        format!(
            "failed to write `{}`: {error}",
            normalize_path(&workspace_manifest.manifest_path)
        )
    })?;

    Ok((
        workspace_manifest.manifest_path.clone(),
        Vec::new(),
        member_manifest_path
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf(),
    ))
}

fn append_workspace_manifest_member(source: &str, member: &str) -> Result<String, String> {
    let mut value = toml::from_str::<TomlValue>(source)
        .map_err(|error| format!("failed to parse workspace manifest: {error}"))?;
    let Some(root) = value.as_table_mut() else {
        return Err("workspace manifest must be a TOML table".to_owned());
    };
    let Some(workspace) = root.get_mut("workspace").and_then(TomlValue::as_table_mut) else {
        return Err("workspace manifest must declare `[workspace]`".to_owned());
    };
    let members = workspace
        .entry("members")
        .or_insert_with(|| TomlValue::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| "workspace manifest must declare `[workspace].members`".to_owned())?;
    members.push(TomlValue::String(member.to_owned()));

    let mut rendered = toml::to_string(&value)
        .map_err(|error| format!("failed to render workspace manifest: {error}"))?;
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    Ok(rendered)
}

fn remove_workspace_manifest_member(source: &str, member: &str) -> Result<String, String> {
    let mut value = toml::from_str::<TomlValue>(source)
        .map_err(|error| format!("failed to parse workspace manifest: {error}"))?;
    let Some(root) = value.as_table_mut() else {
        return Err("workspace manifest must be a TOML table".to_owned());
    };
    let Some(workspace) = root.get_mut("workspace").and_then(TomlValue::as_table_mut) else {
        return Err("workspace manifest must declare `[workspace]`".to_owned());
    };
    let members = workspace
        .entry("members")
        .or_insert_with(|| TomlValue::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| "workspace manifest must declare `[workspace].members`".to_owned())?;

    let original_len = members.len();
    members.retain(|value| value.as_str().is_none_or(|existing| existing != member));
    if members.len() == original_len {
        return Err(format!(
            "workspace manifest does not declare member `{member}`"
        ));
    }

    let mut rendered = toml::to_string(&value)
        .map_err(|error| format!("failed to render workspace manifest: {error}"))?;
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    Ok(rendered)
}

fn resolve_project_add_dependency_entries(
    workspace_manifest: &ql_project::ProjectManifest,
    package_name: &str,
    dependency_names: &[String],
) -> Result<Vec<(String, String)>, String> {
    let workspace_root = workspace_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    let member_root = workspace_root.join("packages").join(package_name);
    let mut seen = BTreeSet::new();
    let mut dependencies = Vec::with_capacity(dependency_names.len());

    for dependency_name in dependency_names {
        validate_project_package_name(dependency_name)?;
        if dependency_name == package_name {
            return Err(format!(
                "does not accept self dependency `{dependency_name}` for new package `{package_name}`"
            ));
        }
        if !seen.insert(dependency_name.clone()) {
            return Err(format!(
                "received duplicate `--dependency {dependency_name}`"
            ));
        }

        let Some(dependency_manifest_path) =
            find_workspace_member_with_package_name(workspace_manifest, dependency_name)
        else {
            return Err(format!(
                "workspace manifest `{}` does not contain package `{dependency_name}`",
                normalize_path(&workspace_manifest.manifest_path)
            ));
        };
        let dependency_root = dependency_manifest_path.parent().unwrap_or(Path::new("."));
        dependencies.push((
            dependency_name.clone(),
            relative_path_from(&member_root, dependency_root),
        ));
    }

    Ok(dependencies)
}

fn collect_conventional_binary_target_relative_paths(
    package_manifest: &ql_project::ProjectManifest,
) -> Result<Vec<String>, u8> {
    let package_root = package_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    let source_root = package_root.join("src");
    let mut binary_paths = Vec::new();
    let main_path = source_root.join("main.ql");
    if main_path.is_file() {
        binary_paths.push(normalize_path(
            main_path.strip_prefix(package_root).unwrap_or(&main_path),
        ));
    }

    let bin_root = source_root.join("bin");
    if bin_root.is_dir() {
        collect_conventional_binary_target_relative_paths_recursive(
            package_root,
            &bin_root,
            &mut binary_paths,
        )?;
    }

    binary_paths.sort();
    binary_paths.dedup();
    Ok(binary_paths)
}

fn collect_conventional_binary_target_relative_paths_recursive(
    package_root: &Path,
    directory: &Path,
    binary_paths: &mut Vec<String>,
) -> Result<(), u8> {
    let entries = fs::read_dir(directory).map_err(|error| {
        eprintln!(
            "error: `ql project target add` failed to read directory `{}`: {error}",
            normalize_path(directory)
        );
        1
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            eprintln!(
                "error: `ql project target add` failed to read directory entry under `{}`: {error}",
                normalize_path(directory)
            );
            1
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_conventional_binary_target_relative_paths_recursive(
                package_root,
                &path,
                binary_paths,
            )?;
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) == Some("ql") {
            binary_paths.push(normalize_path(
                path.strip_prefix(package_root).unwrap_or(&path),
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_workspace_manifest_member_adds_member_and_trailing_newline() {
        let rendered = append_workspace_manifest_member(
            "[workspace]\nmembers = [\"packages/core\"]\n",
            "packages/app",
        )
        .expect("append workspace member");

        assert!(rendered.contains("\"packages/core\""));
        assert!(rendered.contains("\"packages/app\""));
        assert!(rendered.ends_with('\n'));
    }

    #[test]
    fn remove_workspace_manifest_member_removes_exact_member() {
        let rendered = remove_workspace_manifest_member(
            "[workspace]\nmembers = [\"packages/core\", \"packages/app\"]\n",
            "packages/core",
        )
        .expect("remove workspace member");

        assert!(!rendered.contains("\"packages/core\""));
        assert!(rendered.contains("\"packages/app\""));
    }

    #[test]
    fn remove_workspace_manifest_member_reports_missing_member() {
        let error = remove_workspace_manifest_member(
            "[workspace]\nmembers = [\"packages/app\"]\n",
            "packages/core",
        )
        .expect_err("missing member should fail");

        assert_eq!(
            error,
            "workspace manifest does not declare member `packages/core`"
        );
    }
}
