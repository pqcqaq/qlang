use std::path::Path;

use crate::cli_utils::{normalize_path, validate_project_package_name};
use crate::project_workspace::{
    resolve_project_package_manifest, resolve_project_workspace_manifest,
};

mod target_add;
mod workspace_members;

pub(crate) use target_add::project_add_binary_target_path;
use workspace_members::{
    add_existing_workspace_project_member, add_workspace_project_member,
    remove_workspace_project_member,
};

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
