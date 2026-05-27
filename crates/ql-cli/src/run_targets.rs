use std::path::{Path, PathBuf};

use ql_project::{BuildTarget, ManifestBuildProfile, WorkspaceBuildTargets};
use serde_json::Value as JsonValue;

use crate::build_reporting::build_json_preflight_failure;
use crate::cli_utils::normalize_path;
use crate::project_targets::{
    ProjectTargetSelector, is_runnable_project_target, project_target_display_path,
};

#[derive(Clone, Debug)]
pub(crate) struct RunnableProjectTarget {
    pub(crate) member_manifest_path: PathBuf,
    pub(crate) package_name: String,
    pub(crate) default_profile: Option<ManifestBuildProfile>,
    pub(crate) target: BuildTarget,
}

fn collect_runnable_project_targets(
    members: &[WorkspaceBuildTargets],
) -> Vec<RunnableProjectTarget> {
    let mut runnable_targets = Vec::new();
    for member in members {
        for target in &member.targets {
            if is_runnable_project_target(target.kind) {
                runnable_targets.push(RunnableProjectTarget {
                    member_manifest_path: member.member_manifest_path.clone(),
                    package_name: member.package_name.clone(),
                    default_profile: member.default_profile,
                    target: target.clone(),
                });
            }
        }
    }
    runnable_targets
}

pub(crate) fn select_runnable_project_target(
    path: &Path,
    members: &[WorkspaceBuildTargets],
    selector: &ProjectTargetSelector,
) -> Result<RunnableProjectTarget, u8> {
    let mut runnable_targets = collect_runnable_project_targets(members);
    match runnable_targets.len() {
        0 => report_no_runnable_targets(path, selector),
        1 => Ok(runnable_targets
            .pop()
            .expect("runnable target count checked above")),
        count => report_multiple_runnable_targets(path, selector, &runnable_targets, count),
    }
}

fn report_no_runnable_targets(
    path: &Path,
    selector: &ProjectTargetSelector,
) -> Result<RunnableProjectTarget, u8> {
    let normalized_path = normalize_path(path);
    if selector.is_active() {
        eprintln!(
            "error: `ql run` target selector matched no runnable build targets under `{normalized_path}`"
        );
        eprintln!("note: selector: {}", selector.describe());
        eprintln!(
            "hint: rerun `ql project targets {normalized_path}` to inspect the discovered build targets"
        );
        return Err(1);
    }
    eprintln!("error: `ql run` found no runnable build targets under `{normalized_path}`");
    eprintln!(
        "hint: add `src/main.ql`, `src/bin/*.ql`, or declare `[[bin]].path`, or rerun `ql project targets {normalized_path}` to inspect the discovered build targets"
    );
    Err(1)
}

fn report_multiple_runnable_targets(
    path: &Path,
    selector: &ProjectTargetSelector,
    runnable_targets: &[RunnableProjectTarget],
    count: usize,
) -> Result<RunnableProjectTarget, u8> {
    let normalized_path = normalize_path(path);
    if selector.is_active() {
        eprintln!(
            "error: `ql run` target selector matched multiple runnable build targets under `{normalized_path}`"
        );
        eprintln!("note: selector: {}", selector.describe());
        eprintln!("note: `{normalized_path}` resolved to {count} runnable build targets");
        for runnable in runnable_targets {
            eprintln!(
                "note: candidate target `{}` from package `{}`",
                project_target_display_path(
                    &runnable.member_manifest_path,
                    runnable.target.path.as_path()
                ),
                runnable.package_name
            );
        }
        eprintln!(
            "hint: refine the selector with `--package`, `--bin`, or `--target`, or rerun `ql project targets {normalized_path}` to inspect the discovered build targets"
        );
        return Err(1);
    }
    eprintln!("error: `ql run` found multiple runnable build targets under `{normalized_path}`");
    eprintln!("note: `{normalized_path}` resolved to {count} runnable build targets");
    for runnable in runnable_targets {
        eprintln!(
            "note: candidate target `{}` from package `{}`",
            project_target_display_path(
                &runnable.member_manifest_path,
                runnable.target.path.as_path()
            ),
            runnable.package_name
        );
    }
    eprintln!(
        "hint: rerun `ql run <source-file>` for a specific target, or `ql project targets {normalized_path}` to inspect the discovered build targets"
    );
    Err(1)
}

pub(crate) fn select_runnable_project_target_for_run_json(
    path: &Path,
    members: &[WorkspaceBuildTargets],
    selector: &ProjectTargetSelector,
) -> Result<RunnableProjectTarget, JsonValue> {
    let mut runnable_targets = collect_runnable_project_targets(members);
    match runnable_targets.len() {
        0 => Err(run_target_selection_json_failure(path, selector, 0)),
        1 => Ok(runnable_targets
            .pop()
            .expect("runnable target count checked above")),
        count => Err(run_target_selection_json_failure(path, selector, count)),
    }
}

fn run_target_selection_json_failure(
    path: &Path,
    selector: &ProjectTargetSelector,
    count: usize,
) -> JsonValue {
    let normalized_path = normalize_path(path);
    let (error_kind, message, selector) = match (selector.is_active(), count) {
        (true, 0) => (
            "selector",
            format!("target selector matched no runnable build targets under `{normalized_path}`"),
            Some(selector.describe()),
        ),
        (false, 0) => (
            "project",
            format!("found no runnable build targets under `{normalized_path}`"),
            None,
        ),
        (true, _) => (
            "selector",
            format!(
                "target selector matched multiple runnable build targets under `{normalized_path}`"
            ),
            Some(selector.describe()),
        ),
        (false, _) => (
            "project",
            format!("found multiple runnable build targets under `{normalized_path}`"),
            None,
        ),
    };
    build_json_preflight_failure(
        path,
        None,
        None,
        None,
        error_kind,
        "target-selection",
        message,
        selector,
        None,
        Some(count),
    )
}

#[cfg(test)]
#[path = "run_targets_tests.rs"]
mod tests;
