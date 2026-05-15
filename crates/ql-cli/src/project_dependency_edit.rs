use std::fs;
use std::path::{Path, PathBuf};

use ql_project::{
    load_project_manifest, package_name, render_manifest_with_added_local_dependency,
    render_manifest_with_removed_local_dependency,
};

use crate::project_dependencies::{ProjectDependentMember, find_workspace_member_dependents};
use crate::project_workspace::{
    render_workspace_member_lookup_error, resolve_project_selected_package_manifest,
    resolve_project_workspace_manifest, resolve_workspace_member_entry_by_package_name,
};

use super::{
    absolute_user_path, normalize_path, relative_path_from, validate_project_package_name,
};

pub(crate) fn project_add_dependency_path(
    path: &Path,
    target_package_name: Option<&str>,
    package_name: Option<&str>,
    dependency_path: Option<&Path>,
) -> Result<(), u8> {
    let (workspace_manifest, package_manifest) = resolve_project_selected_package_manifest(
        path,
        target_package_name,
        "`ql project add-dependency`",
    )?;
    let dependency_entry = match (package_name, dependency_path) {
        (Some(package_name), None) => {
            if let Err(message) = validate_project_package_name(package_name) {
                eprintln!("error: `ql project add-dependency` {message}");
                return Err(1);
            }
            resolve_project_existing_dependency_entry(
                &workspace_manifest,
                &package_manifest,
                package_name,
            )
        }
        (None, Some(dependency_path)) => {
            resolve_project_path_dependency_entry(&package_manifest, dependency_path)
        }
        _ => Err("requires exactly one dependency selector".to_owned()),
    }
    .map_err(|message| {
        eprintln!("error: `ql project add-dependency` {message}");
        1
    })?;
    let package_manifest_source =
        fs::read_to_string(&package_manifest.manifest_path).map_err(|error| {
            eprintln!(
                "error: `ql project add-dependency` failed to read `{}`: {error}",
                normalize_path(&package_manifest.manifest_path)
            );
            1
        })?;
    let updated_package_manifest = render_manifest_with_added_local_dependency(
        &package_manifest_source,
        &dependency_entry.0,
        &dependency_entry.1,
    )
    .map_err(|message| {
        eprintln!("error: `ql project add-dependency` {message}");
        1
    })?;
    fs::write(&package_manifest.manifest_path, updated_package_manifest).map_err(|error| {
        eprintln!(
            "error: `ql project add-dependency` failed to write `{}`: {error}",
            normalize_path(&package_manifest.manifest_path)
        );
        1
    })?;

    println!(
        "updated: {}",
        normalize_path(&package_manifest.manifest_path)
    );
    Ok(())
}

pub(crate) fn project_remove_dependency_path(
    path: &Path,
    target_package_name: Option<&str>,
    package_name: &str,
    remove_all: bool,
) -> Result<(), u8> {
    if let Err(message) = validate_project_package_name(package_name) {
        eprintln!("error: `ql project remove-dependency` {message}");
        return Err(1);
    }

    if remove_all {
        if target_package_name.is_some() {
            eprintln!(
                "error: `ql project remove-dependency --all` does not accept `--package`; bulk cleanup already targets all dependents of `--name`"
            );
            return Err(1);
        }
        return project_remove_dependency_from_all_workspace_members(path, package_name);
    }

    let (workspace_manifest, package_manifest) = resolve_project_selected_package_manifest(
        path,
        target_package_name,
        "`ql project remove-dependency`",
    )?;
    let dependency_entry = resolve_project_existing_dependency_entry(
        &workspace_manifest,
        &package_manifest,
        package_name,
    )
    .map_err(|message| {
        eprintln!("error: `ql project remove-dependency` {message}");
        1
    })?;
    let package_manifest_source =
        fs::read_to_string(&package_manifest.manifest_path).map_err(|error| {
            eprintln!(
                "error: `ql project remove-dependency` failed to read `{}`: {error}",
                normalize_path(&package_manifest.manifest_path)
            );
            1
        })?;
    let updated_package_manifest = render_manifest_with_removed_local_dependency(
        &package_manifest_source,
        &dependency_entry.0,
        &dependency_entry.1,
    )
    .map_err(|message| {
        eprintln!("error: `ql project remove-dependency` {message}");
        1
    })?;
    fs::write(&package_manifest.manifest_path, updated_package_manifest).map_err(|error| {
        eprintln!(
            "error: `ql project remove-dependency` failed to write `{}`: {error}",
            normalize_path(&package_manifest.manifest_path)
        );
        1
    })?;

    println!(
        "updated: {}",
        normalize_path(&package_manifest.manifest_path)
    );
    Ok(())
}

fn project_remove_dependency_from_all_workspace_members(
    path: &Path,
    package_name: &str,
) -> Result<(), u8> {
    let workspace_manifest = resolve_project_workspace_manifest(path).map_err(|message| {
        eprintln!("error: `ql project remove-dependency` {message}");
        1
    })?;
    let (_, member_manifest_path) =
        resolve_workspace_member_entry_by_package_name(&workspace_manifest, package_name).map_err(
            |error| {
                eprintln!(
                    "error: `ql project remove-dependency` {}",
                    render_workspace_member_lookup_error(&workspace_manifest, package_name, &error)
                );
                1
            },
        )?;
    let dependents = find_workspace_member_dependents(&workspace_manifest, &member_manifest_path)
        .map_err(|message| {
        eprintln!("error: `ql project remove-dependency` {message}");
        1
    })?;
    if dependents.is_empty() {
        eprintln!(
            "error: `ql project remove-dependency` workspace package `{package_name}` does not have any dependent members to update in workspace manifest `{}`",
            normalize_path(&workspace_manifest.manifest_path)
        );
        return Err(1);
    }

    let updated_dependency_manifests =
        detach_workspace_member_dependents(package_name, &member_manifest_path, &dependents)
            .map_err(|message| {
                eprintln!("error: `ql project remove-dependency` {message}");
                1
            })?;
    for manifest_path in updated_dependency_manifests {
        println!("updated: {}", normalize_path(&manifest_path));
    }
    Ok(())
}

pub(crate) fn detach_workspace_member_dependents(
    dependency_name: &str,
    dependency_manifest_path: &Path,
    dependents: &[ProjectDependentMember],
) -> Result<Vec<PathBuf>, String> {
    let dependency_root = dependency_manifest_path.parent().unwrap_or(Path::new("."));
    let mut updated_manifests = Vec::with_capacity(dependents.len());

    for dependent in dependents {
        let dependent_root = dependent.manifest_path.parent().unwrap_or(Path::new("."));
        let dependency_path = relative_path_from(dependent_root, dependency_root);
        let manifest_source = fs::read_to_string(&dependent.manifest_path).map_err(|error| {
            format!(
                "failed to read `{}` while detaching dependent `{}`: {error}",
                normalize_path(&dependent.manifest_path),
                dependent.package_name
            )
        })?;
        let updated_manifest = render_manifest_with_removed_local_dependency(
            &manifest_source,
            dependency_name,
            &dependency_path,
        )
        .map_err(|message| {
            format!(
                "failed to detach local dependency from `{}`: {message}",
                normalize_path(&dependent.manifest_path)
            )
        })?;
        fs::write(&dependent.manifest_path, updated_manifest).map_err(|error| {
            format!(
                "failed to write `{}` while detaching dependent `{}`: {error}",
                normalize_path(&dependent.manifest_path),
                dependent.package_name
            )
        })?;
        updated_manifests.push(dependent.manifest_path.clone());
    }

    Ok(updated_manifests)
}

fn resolve_project_existing_dependency_entry(
    workspace_manifest: &ql_project::ProjectManifest,
    package_manifest: &ql_project::ProjectManifest,
    dependency_name: &str,
) -> Result<(String, String), String> {
    let member_package_name = package_name(package_manifest)
        .map_err(|error| format!("failed to resolve current package name: {error}"))?;
    if dependency_name == member_package_name {
        return Err(format!(
            "does not accept self dependency `{dependency_name}` for package `{member_package_name}`"
        ));
    }

    let (_, dependency_manifest_path) =
        resolve_workspace_member_entry_by_package_name(workspace_manifest, dependency_name)
            .map_err(|error| {
                render_workspace_member_lookup_error(workspace_manifest, dependency_name, &error)
            })?;

    let member_root = package_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    let dependency_root = dependency_manifest_path.parent().unwrap_or(Path::new("."));
    Ok((
        dependency_name.to_owned(),
        relative_path_from(member_root, dependency_root),
    ))
}

fn resolve_project_path_dependency_entry(
    package_manifest: &ql_project::ProjectManifest,
    dependency_path: &Path,
) -> Result<(String, String), String> {
    let member_package_name = package_name(package_manifest)
        .map_err(|error| format!("failed to resolve current package name: {error}"))?;
    let dependency_manifest =
        load_project_manifest(&absolute_user_path(dependency_path)).map_err(|error| {
            format!(
                "failed to resolve local dependency path `{}`: {error}",
                normalize_path(dependency_path)
            )
        })?;
    let dependency_name = package_name(&dependency_manifest)
        .map_err(|error| format!("failed to resolve dependency package name: {error}"))?;
    validate_project_package_name(dependency_name)?;

    if normalize_path(&dependency_manifest.manifest_path)
        == normalize_path(&package_manifest.manifest_path)
    {
        return Err(format!(
            "does not accept self dependency `{dependency_name}` for package `{member_package_name}`"
        ));
    }
    if dependency_name == member_package_name {
        return Err(format!(
            "does not accept dependency `{dependency_name}` because the current package has the same name"
        ));
    }

    let member_root = package_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    let dependency_root = dependency_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    Ok((
        dependency_name.to_owned(),
        relative_path_from(member_root, dependency_root),
    ))
}
