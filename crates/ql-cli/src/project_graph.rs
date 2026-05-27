use std::path::Path;

use ql_project::{
    load_project_manifest, render_project_graph_resolved, render_project_graph_resolved_json,
};

use crate::project_targets::resolve_project_workspace_member_command_request_root;

mod json;
mod reporting;
mod selection;

use json::{
    render_project_graph_preflight_failure_json, render_project_graph_selection_failure_json,
};
use reporting::report_project_graph_load_error;
use selection::{ProjectGraphPackageSelectionError, resolve_project_graph_package_manifest};

#[cfg(test)]
use reporting::format_project_graph_command;

pub(crate) fn project_graph_path(
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
                    render_project_graph_preflight_failure_json(path, &error)
                );
            } else {
                report_project_graph_load_error(path, &error);
            }
            return Err(1);
        }
    };
    let manifest = if let Some(package_name) = package_name {
        match resolve_project_graph_package_manifest(path, &manifest, package_name, json) {
            Ok(manifest) => manifest,
            Err(ProjectGraphPackageSelectionError::Json(failure)) => {
                print!(
                    "{}",
                    render_project_graph_selection_failure_json(path, &manifest, failure)
                );
                return Err(1);
            }
            Err(ProjectGraphPackageSelectionError::Exit(code)) => return Err(code),
        }
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

#[cfg(test)]
#[path = "project_graph_tests.rs"]
mod tests;
