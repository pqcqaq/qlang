use std::path::Path;

#[cfg(test)]
use ql_project::{BuildTarget, BuildTargetKind, WorkspaceBuildTargets};

use crate::cli_utils::normalize_path;

mod loader;
mod paths;
mod rendering;
mod selection;
mod selector;

use loader::load_project_target_members_for_workspace_member_path;
pub(crate) use loader::load_workspace_build_targets_for_command_from_request_root;
#[cfg(test)]
use paths::is_ql_source_file;
pub(crate) use paths::{
    ProjectCheckCommandScope, ProjectCommandPathError, ProjectCommandScope,
    ResolvedProjectCommandPath, display_relative_to_root, project_request_root,
    project_target_display_path, resolve_project_check_command_scope, resolve_project_command_path,
    resolve_project_command_scope, resolve_project_member_request_root,
    resolve_project_workspace_member_command_request_root,
};
#[cfg(test)]
pub(crate) use rendering::render_project_targets_json;
use rendering::{render_project_target_members, render_project_targets_selection_failure_json};
use selection::{
    ProjectTargetSelectionFailure, report_project_target_selection_failure,
    select_workspace_build_targets_with_failure,
};
pub(crate) use selection::{
    filter_workspace_build_targets, is_runnable_project_target, select_workspace_build_targets,
};
pub(crate) use selector::{
    ProjectTargetSelector, ProjectTargetSelectorKind, parse_project_target_selector_option,
};

pub(crate) fn report_project_target_selector_requires_project_context(
    command_label: &str,
    selector: &ProjectTargetSelector,
) {
    eprintln!("error: {command_label} target selectors require a package or workspace path");
    eprintln!("note: selector: {}", selector.describe());
}

pub(crate) fn report_project_source_path_rejects_target_selector(
    command_label: &str,
    path: &Path,
    selector: &ProjectTargetSelector,
) {
    eprintln!(
        "error: {command_label} does not support combining a direct project source path with target selectors"
    );
    eprintln!("note: source path: {}", normalize_path(path));
    eprintln!("note: selector: {}", selector.describe());
}

pub(crate) fn project_targets_path(
    path: &Path,
    selector: &ProjectTargetSelector,
    json: bool,
) -> Result<(), u8> {
    let members =
        load_project_target_members_for_workspace_member_path(path, "`ql project targets`", json)?;
    let members = match select_workspace_build_targets_with_failure(
        path,
        &members,
        selector,
        "build targets",
    ) {
        Ok(members) => members,
        Err(failure) => {
            if json {
                print!(
                    "{}",
                    render_project_targets_selection_failure_json(&failure)
                );
            } else {
                report_project_target_selection_failure("`ql project targets`", &failure);
            }
            return Err(1);
        }
    };
    render_project_target_members(&members, json);
    Ok(())
}

pub(crate) fn list_build_targets_path(
    path: &Path,
    selector: &ProjectTargetSelector,
    json: bool,
) -> Result<(), u8> {
    let members =
        load_project_target_members_for_workspace_member_path(path, "`ql build --list`", json)?;
    let members = match select_workspace_build_targets_with_failure(
        path,
        &members,
        selector,
        "build targets",
    ) {
        Ok(members) => members,
        Err(failure) => {
            if json {
                print!(
                    "{}",
                    render_project_targets_selection_failure_json(&failure)
                );
            } else {
                report_project_target_selection_failure("`ql build --list`", &failure);
            }
            return Err(1);
        }
    };
    render_project_target_members(&members, json);
    Ok(())
}

pub(crate) fn list_runnable_targets_path(
    path: &Path,
    selector: &ProjectTargetSelector,
    json: bool,
) -> Result<(), u8> {
    let members =
        load_project_target_members_for_workspace_member_path(path, "`ql run --list`", json)?;
    let selected = if selector.is_active() {
        match select_workspace_build_targets_with_failure(path, &members, selector, "build targets")
        {
            Ok(members) => members,
            Err(failure) => {
                if json {
                    print!(
                        "{}",
                        render_project_targets_selection_failure_json(&failure)
                    );
                } else {
                    report_project_target_selection_failure("`ql run --list`", &failure);
                }
                return Err(1);
            }
        }
    } else {
        members
    };
    let runnable_members =
        filter_workspace_build_targets(&selected, !selector.is_active(), |target| {
            is_runnable_project_target(target.kind)
        });
    if selector.is_active()
        && runnable_members
            .iter()
            .map(|member| member.targets.len())
            .sum::<usize>()
            == 0
    {
        let failure = ProjectTargetSelectionFailure {
            stage: "runnable-selection",
            path: normalize_path(path),
            message: format!(
                "target selector matched no runnable build targets under `{}`",
                normalize_path(path)
            ),
            selector: selector.describe(),
            target_count: selected
                .iter()
                .map(|member| member.targets.len())
                .sum::<usize>(),
        };
        if json {
            print!(
                "{}",
                render_project_targets_selection_failure_json(&failure)
            );
        } else {
            report_project_target_selection_failure("`ql run --list`", &failure);
        }
        return Err(1);
    }
    render_project_target_members(&runnable_members, json);
    Ok(())
}

#[cfg(test)]
#[path = "project_targets_tests.rs"]
mod tests;
