use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use ql_project::{load_project_manifest, package_name};
use toml::Value as TomlValue;

use crate::cli_utils::{normalize_path, relative_path_from, validate_project_package_name};
use crate::project_dependencies::find_workspace_member_dependents;
use crate::project_dependency_edit::detach_workspace_member_dependents;
use crate::project_init;
use crate::project_manifest_edit::{
    acquire_locked_project_manifest_edits, write_locked_project_manifest,
};
use crate::project_workspace::{
    WorkspaceMemberLookupError, render_workspace_member_lookup_error,
    resolve_project_package_manifest, resolve_project_workspace_manifest,
    resolve_workspace_member_entry_by_package_name,
};

mod target_add;

pub(crate) use target_add::project_add_binary_target_path;

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
    let (updated_manifest_path, created_paths) =
        add_workspace_project_member(&workspace_manifest, package_name, dependencies).map_err(
            |message| {
                eprintln!("error: `ql project add` {message}");
                1
            },
        )?;

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

fn add_workspace_project_member(
    workspace_manifest: &ql_project::ProjectManifest,
    package_name: &str,
    dependencies: &[String],
) -> Result<(PathBuf, Vec<PathBuf>), String> {
    let _locks =
        acquire_locked_project_manifest_edits(vec![workspace_manifest.manifest_path.clone()])?;
    let workspace_manifest = reload_project_manifest_for_edit(&workspace_manifest.manifest_path)?;
    let workspace = require_workspace_manifest(&workspace_manifest)?;

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
    match resolve_workspace_member_entry_by_package_name(&workspace_manifest, package_name) {
        Ok((_, existing_package_manifest)) => {
            return Err(format!(
                "workspace manifest `{}` already contains package `{package_name}` at `{}`",
                normalize_path(&workspace_manifest.manifest_path),
                normalize_path(&existing_package_manifest)
            ));
        }
        Err(WorkspaceMemberLookupError::Missing) => {}
        Err(WorkspaceMemberLookupError::Ambiguous { matches }) => {
            return Err(format!(
                "workspace manifest `{}` contains multiple members for package `{package_name}`: {}",
                normalize_path(&workspace_manifest.manifest_path),
                matches.join(", ")
            ));
        }
        Err(WorkspaceMemberLookupError::InspectionFailure { member, message }) => {
            return Err(format!(
                "failed to inspect workspace member `{member}`: {message}"
            ));
        }
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
    let dependency_entries =
        resolve_project_add_dependency_entries(&workspace_manifest, package_name, dependencies)?;

    let workspace_manifest_source =
        fs::read_to_string(&workspace_manifest.manifest_path).map_err(|error| {
            format!(
                "failed to read `{}`: {error}",
                normalize_path(&workspace_manifest.manifest_path)
            )
        })?;
    let updated_workspace_manifest =
        append_workspace_manifest_member(&workspace_manifest_source, &member_relative_path)?;

    write_locked_project_manifest(
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
        project_init::create_package_scaffold(&member_root, package_name, &dependency_entries)?;

    Ok((workspace_manifest.manifest_path.clone(), created_paths))
}

fn add_existing_workspace_project_member(
    workspace_manifest: &ql_project::ProjectManifest,
    package_manifest: &ql_project::ProjectManifest,
) -> Result<(PathBuf, PathBuf), String> {
    let _locks =
        acquire_locked_project_manifest_edits(vec![workspace_manifest.manifest_path.clone()])?;
    let workspace_manifest = reload_project_manifest_for_edit(&workspace_manifest.manifest_path)?;
    let workspace = require_workspace_manifest(&workspace_manifest)?;

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
    match resolve_workspace_member_entry_by_package_name(&workspace_manifest, package_name) {
        Ok((_, existing_package_manifest)) => {
            return Err(format!(
                "workspace manifest `{}` already contains package `{package_name}` at `{}`",
                normalize_path(&workspace_manifest.manifest_path),
                normalize_path(&existing_package_manifest)
            ));
        }
        Err(WorkspaceMemberLookupError::Missing) => {}
        Err(WorkspaceMemberLookupError::Ambiguous { matches }) => {
            return Err(format!(
                "workspace manifest `{}` contains multiple members for package `{package_name}`: {}",
                normalize_path(&workspace_manifest.manifest_path),
                matches.join(", ")
            ));
        }
        Err(WorkspaceMemberLookupError::InspectionFailure { member, message }) => {
            return Err(format!(
                "failed to inspect workspace member `{member}`: {message}"
            ));
        }
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
    write_locked_project_manifest(
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
    let _locks =
        acquire_locked_project_manifest_edits(vec![workspace_manifest.manifest_path.clone()])?;
    let workspace_manifest = reload_project_manifest_for_edit(&workspace_manifest.manifest_path)?;
    require_workspace_manifest(&workspace_manifest)?;

    let (member_entry, member_manifest_path) = resolve_workspace_member_entry_by_package_name(
        &workspace_manifest,
        package_name,
    )
    .map_err(|error| match error {
        WorkspaceMemberLookupError::Missing => format!(
            "workspace manifest `{}` does not contain member package `{package_name}`",
            normalize_path(&workspace_manifest.manifest_path)
        ),
        WorkspaceMemberLookupError::Ambiguous { matches } => format!(
            "workspace manifest `{}` contains multiple members for package `{package_name}`: {}",
            normalize_path(&workspace_manifest.manifest_path),
            matches.join(", ")
        ),
        WorkspaceMemberLookupError::InspectionFailure { member, message } => {
            format!("failed to inspect workspace member `{member}`: {message}")
        }
    })?;
    let dependent_members =
        find_workspace_member_dependents(&workspace_manifest, &member_manifest_path)?;
    if !dependent_members.is_empty() {
        if cascade {
            let updated_dependency_manifests = detach_workspace_member_dependents(
                package_name,
                &member_manifest_path,
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
                remove_workspace_manifest_member(&workspace_manifest_source, &member_entry)?;
            write_locked_project_manifest(
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
        remove_workspace_manifest_member(&workspace_manifest_source, &member_entry)?;
    write_locked_project_manifest(
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

fn reload_project_manifest_for_edit(
    manifest_path: &Path,
) -> Result<ql_project::ProjectManifest, String> {
    load_project_manifest(manifest_path).map_err(|error| {
        format!(
            "failed to read `{}`: {error}",
            normalize_path(manifest_path)
        )
    })
}

fn require_workspace_manifest(
    manifest: &ql_project::ProjectManifest,
) -> Result<&ql_project::WorkspaceManifest, String> {
    manifest.workspace.as_ref().ok_or_else(|| {
        format!(
            "manifest `{}` is not a workspace",
            normalize_path(&manifest.manifest_path)
        )
    })
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

        let (_, dependency_manifest_path) =
            resolve_workspace_member_entry_by_package_name(workspace_manifest, dependency_name)
                .map_err(|error| {
                    render_workspace_member_lookup_error(
                        workspace_manifest,
                        dependency_name,
                        &error,
                    )
                })?;
        let dependency_root = dependency_manifest_path.parent().unwrap_or(Path::new("."));
        dependencies.push((
            dependency_name.clone(),
            relative_path_from(&member_root, dependency_root),
        ));
    }

    Ok(dependencies)
}

#[cfg(test)]
#[path = "project_members_tests.rs"]
mod tests;
