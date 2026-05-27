use std::fs;
use std::path::Path;

use ql_project::{load_project_manifest, render_manifest_with_added_binary_target};

use crate::cli_utils::{normalize_path, validate_project_package_name};
use crate::project_init;
use crate::project_manifest_edit::{
    acquire_locked_project_manifest_edits, write_locked_project_manifest,
};
use crate::project_workspace::resolve_project_selected_package_manifest;

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
    let _locks =
        acquire_locked_project_manifest_edits(vec![package_manifest.manifest_path.clone()])
            .map_err(|message| {
                eprintln!(
                    "error: `ql project target add` failed to lock manifest `{}`: {message}",
                    normalize_path(&package_manifest.manifest_path)
                );
                1
            })?;
    let package_manifest =
        load_project_manifest(&package_manifest.manifest_path).map_err(|error| {
            eprintln!(
                "error: `ql project target add` failed to read `{}`: {error}",
                normalize_path(&package_manifest.manifest_path)
            );
            1
        })?;
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
    write_locked_project_manifest(&package_manifest.manifest_path, updated_package_manifest)
        .map_err(|error| {
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
