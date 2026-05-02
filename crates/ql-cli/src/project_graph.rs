use std::path::Path;

use ql_project::{
    load_project_manifest, package_name, render_project_graph_resolved,
    render_project_graph_resolved_json,
};

use super::{
    find_workspace_member_entries_by_package_name, normalize_path,
    package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error,
    resolve_project_workspace_member_command_request_root, validate_project_package_name,
};

pub(crate) fn project_graph_path(
    path: &Path,
    package_name: Option<&str>,
    json: bool,
) -> Result<(), u8> {
    let request_root = resolve_project_workspace_member_command_request_root(path);
    let manifest = load_project_manifest(request_root.as_deref().unwrap_or(path))
        .map_err(|error| report_project_graph_load_error(path, &error))?;
    let manifest = if let Some(package_name) = package_name {
        resolve_project_graph_package_manifest(path, &manifest, package_name)?
    } else {
        manifest
    };
    let rendered = if json {
        render_project_graph_resolved_json(&manifest)
    } else {
        render_project_graph_resolved(&manifest)
    }
    .map_err(|error| {
        eprintln!("error: {error}");
        1
    })?;
    print!("{rendered}");
    Ok(())
}

fn report_project_graph_load_error(path: &Path, error: &ql_project::ProjectError) -> u8 {
    if let ql_project::ProjectError::ManifestNotFound { start } = error {
        eprintln!(
            "error: `ql project graph` requires a package or workspace manifest; could not find `qlang.toml` starting from `{}`",
            normalize_path(start)
        );
        report_project_graph_package_context_failure(path);
    } else if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error)
    {
        eprintln!(
            "error: `ql project graph` manifest `{}` does not declare `[package].name`",
            normalize_path(manifest_path)
        );
        report_project_graph_manifest_failure(manifest_path);
    } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(error) {
        eprintln!("error: `ql project graph` {error}");
        report_project_graph_manifest_failure(manifest_path);
    } else {
        eprintln!("error: {error}");
    }
    1
}

fn resolve_project_graph_package_manifest(
    path: &Path,
    manifest: &ql_project::ProjectManifest,
    selected_package_name: &str,
) -> Result<ql_project::ProjectManifest, u8> {
    if let Err(message) = validate_project_package_name(selected_package_name) {
        eprintln!("error: `ql project graph` {message}");
        return Err(1);
    }

    if manifest.workspace.is_some() {
        let member_entries =
            find_workspace_member_entries_by_package_name(manifest, selected_package_name);
        if member_entries.is_empty() {
            eprintln!(
                "error: `ql project graph` workspace manifest `{}` does not contain package `{selected_package_name}`",
                normalize_path(&manifest.manifest_path)
            );
            return Err(1);
        }
        if member_entries.len() > 1 {
            let matching_members = member_entries
                .iter()
                .map(|(member, _)| member.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            eprintln!(
                "error: `ql project graph` workspace manifest `{}` contains multiple members for package `{selected_package_name}`: {matching_members}",
                normalize_path(&manifest.manifest_path)
            );
            return Err(1);
        }

        return load_project_manifest(&member_entries[0].1)
            .map_err(|error| report_project_graph_load_error(path, &error));
    }

    let actual_package_name = package_name(manifest).map_err(|error| {
        eprintln!("error: `ql project graph` {error}");
        report_project_graph_manifest_failure(&manifest.manifest_path);
        1
    })?;
    if actual_package_name != selected_package_name {
        eprintln!(
            "error: `ql project graph` package selector expected `{selected_package_name}` but `{}` resolves to package `{actual_package_name}`",
            normalize_path(path)
        );
        return Err(1);
    }

    Ok(manifest.clone())
}

fn report_project_graph_manifest_failure(manifest_path: &Path) {
    let manifest_path = normalize_path(manifest_path);
    let rerun_command = format_project_graph_command(&manifest_path);
    eprintln!("note: failing package manifest: {manifest_path}");
    eprintln!("hint: rerun `{rerun_command}` after fixing the package manifest");
}

fn report_project_graph_package_context_failure(path: &Path) {
    let normalized_path = normalize_path(path);
    let rerun_command = format_project_graph_command(&normalized_path);
    eprintln!(
        "note: `ql project graph` only renders package/workspace graphs for packages or workspace members discoverable from `qlang.toml`"
    );
    eprintln!("hint: rerun `{rerun_command}` after adding `qlang.toml` for this path");
}

fn format_project_graph_command(normalized_path: &str) -> String {
    format!("ql project graph {normalized_path}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_rerun_command_preserves_normalized_path() {
        assert_eq!(
            format_project_graph_command("packages/app/qlang.toml"),
            "ql project graph packages/app/qlang.toml"
        );
    }
}
