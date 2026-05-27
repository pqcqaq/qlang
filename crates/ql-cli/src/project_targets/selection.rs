use std::path::Path;

use ql_project::{BuildTarget, BuildTargetKind, WorkspaceBuildTargets};

use crate::cli_utils::normalize_path;

use super::ProjectTargetSelector;

pub(crate) fn select_workspace_build_targets(
    path: &Path,
    members: &[WorkspaceBuildTargets],
    selector: &ProjectTargetSelector,
    command_label: &str,
    target_label: &str,
) -> Result<Vec<WorkspaceBuildTargets>, u8> {
    select_workspace_build_targets_with_failure(path, members, selector, target_label).map_err(
        |failure| {
            report_project_target_selection_failure(command_label, &failure);
            1
        },
    )
}

pub(super) fn select_workspace_build_targets_with_failure(
    path: &Path,
    members: &[WorkspaceBuildTargets],
    selector: &ProjectTargetSelector,
    target_label: &str,
) -> Result<Vec<WorkspaceBuildTargets>, ProjectTargetSelectionFailure> {
    if !selector.is_active() {
        return Ok(members.to_vec());
    }

    let mut selected = Vec::new();
    for member in members {
        let targets = member
            .targets
            .iter()
            .filter(|target| {
                selector.matches(
                    member.member_manifest_path.as_path(),
                    &member.package_name,
                    target,
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        if !targets.is_empty() {
            selected.push(WorkspaceBuildTargets {
                member_manifest_path: member.member_manifest_path.clone(),
                package_name: member.package_name.clone(),
                default_profile: member.default_profile,
                targets,
            });
        }
    }

    if selected
        .iter()
        .map(|member| member.targets.len())
        .sum::<usize>()
        == 0
    {
        return Err(ProjectTargetSelectionFailure {
            stage: "target-selection",
            path: normalize_path(path),
            message: format!(
                "target selector matched no {target_label} under `{}`",
                normalize_path(path)
            ),
            selector: selector.describe(),
            target_count: members
                .iter()
                .map(|member| member.targets.len())
                .sum::<usize>(),
        });
    }

    Ok(selected)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProjectTargetSelectionFailure {
    pub(crate) stage: &'static str,
    pub(crate) path: String,
    pub(crate) message: String,
    pub(crate) selector: String,
    pub(crate) target_count: usize,
}

pub(super) fn report_project_target_selection_failure(
    command_label: &str,
    failure: &ProjectTargetSelectionFailure,
) {
    eprintln!("error: {command_label} {}", failure.message);
    eprintln!("note: selector: {}", failure.selector);
    eprintln!(
        "hint: rerun `ql project targets {}` to inspect the discovered build targets",
        failure.path
    );
}

pub(crate) fn filter_workspace_build_targets(
    members: &[WorkspaceBuildTargets],
    keep_empty_members: bool,
    predicate: impl Fn(&BuildTarget) -> bool,
) -> Vec<WorkspaceBuildTargets> {
    members
        .iter()
        .filter_map(|member| {
            let targets = member
                .targets
                .iter()
                .filter(|target| predicate(target))
                .cloned()
                .collect::<Vec<_>>();
            if targets.is_empty() && !keep_empty_members {
                return None;
            }
            Some(WorkspaceBuildTargets {
                member_manifest_path: member.member_manifest_path.clone(),
                package_name: member.package_name.clone(),
                default_profile: member.default_profile,
                targets,
            })
        })
        .collect()
}

pub(crate) fn is_runnable_project_target(kind: BuildTargetKind) -> bool {
    matches!(kind, BuildTargetKind::Binary | BuildTargetKind::Source)
}
