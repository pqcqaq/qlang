use std::path::Path;

use ql_project::load_project_manifest;

use crate::cli_utils::{
    normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error,
};
use crate::project_targets::resolve_project_workspace_member_command_request_root;

mod collection;
mod rendering;

use collection::{ProjectStatusMemberSelectionError, collect_project_status_members};
use rendering::{
    render_project_status, render_project_status_json,
    render_project_status_preflight_failure_json, render_project_status_selection_failure_json,
};

pub(crate) fn project_status_path(
    path: &Path,
    package_name: Option<&str>,
    json: bool,
) -> Result<(), u8> {
    let request_root = resolve_project_workspace_member_command_request_root(path);
    let manifest = match load_project_manifest(request_root.as_deref().unwrap_or(path)) {
        Ok(manifest) => manifest,
        Err(error) => {
            if json {
                print!(
                    "{}",
                    render_project_status_preflight_failure_json(path, &error)
                );
            } else {
                report_project_status_load_error(path, &error);
            }
            return Err(1);
        }
    };
    let members = match collect_project_status_members(&manifest, path, package_name, json) {
        Ok(members) => members,
        Err(ProjectStatusMemberSelectionError::Json(failure)) => {
            print!(
                "{}",
                render_project_status_selection_failure_json(path, &manifest, failure)
            );
            return Err(1);
        }
        Err(ProjectStatusMemberSelectionError::Message(message)) => {
            eprintln!("error: `ql project status` {message}");
            return Err(1);
        }
        Err(ProjectStatusMemberSelectionError::Exit(code)) => return Err(code),
    };
    let rendered = if json {
        render_project_status_json(path, &manifest, &members)
    } else {
        render_project_status(&manifest, &members)
    };
    print!("{rendered}");
    Ok(())
}

fn report_project_status_load_error(path: &Path, error: &ql_project::ProjectError) -> u8 {
    if let ql_project::ProjectError::ManifestNotFound { start } = error {
        eprintln!(
            "error: `ql project status` requires a package or workspace manifest; could not find `qlang.toml` starting from `{}`",
            normalize_path(start)
        );
    } else if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error)
    {
        eprintln!(
            "error: `ql project status` manifest `{}` does not declare `[package].name`",
            normalize_path(manifest_path)
        );
    } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(error) {
        eprintln!("error: `ql project status` {error}");
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(manifest_path)
        );
    } else {
        eprintln!("error: `ql project status` {error}");
    }
    eprintln!("note: requested path: {}", normalize_path(path));
    1
}
