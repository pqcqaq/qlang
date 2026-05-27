use std::path::{Path, PathBuf};

use ql_project::{
    BuildTarget, BuildTargetKind, WorkspaceBuildTargets, discover_package_build_targets,
    discover_workspace_build_targets, load_project_manifest, package_name,
};

use crate::cli_utils::{
    normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error,
};

mod rendering;
mod selector;

#[cfg(test)]
pub(crate) use rendering::render_project_targets_json;
use rendering::{
    render_project_target_members, render_project_targets_preflight_failure_json,
    render_project_targets_selection_failure_json,
};
pub(crate) use selector::{
    ProjectTargetSelector, ProjectTargetSelectorKind, parse_project_target_selector_option,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProjectCommandScope {
    Project,
    ProjectBuildTarget(ProjectSourceBuildTargetRequest),
    ProjectTestFile(ProjectFileTestRequest),
    DirectSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProjectCheckCommandScope {
    Project {
        request_root_manifest_path: Option<PathBuf>,
    },
    DirectSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ResolvedProjectCommandPath {
    Project {
        request_root_manifest_path: Option<PathBuf>,
        selector: ProjectTargetSelector,
    },
    DirectSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProjectCommandPathError {
    SourcePathRejectsSelector,
    SelectorRequiresProjectContext,
}

pub(crate) fn resolve_project_command_path(
    path: &Path,
    selector: &ProjectTargetSelector,
) -> Result<ResolvedProjectCommandPath, ProjectCommandPathError> {
    match resolve_project_command_scope(path) {
        ProjectCommandScope::Project => Ok(ResolvedProjectCommandPath::Project {
            request_root_manifest_path: None,
            selector: selector.clone(),
        }),
        ProjectCommandScope::ProjectBuildTarget(request) => {
            if selector.is_active() {
                return Err(ProjectCommandPathError::SourcePathRejectsSelector);
            }
            Ok(ResolvedProjectCommandPath::Project {
                request_root_manifest_path: Some(request.request_root_manifest_path),
                selector: request.selector,
            })
        }
        ProjectCommandScope::ProjectTestFile(_) | ProjectCommandScope::DirectSource => {
            if selector.is_active() {
                Err(ProjectCommandPathError::SelectorRequiresProjectContext)
            } else {
                Ok(ResolvedProjectCommandPath::DirectSource)
            }
        }
    }
}

pub(crate) fn resolve_project_command_scope(path: &Path) -> ProjectCommandScope {
    if is_project_context_path(path) {
        return ProjectCommandScope::Project;
    }
    if let Some(request) = resolve_project_source_build_target_request(path) {
        return ProjectCommandScope::ProjectBuildTarget(request);
    }
    if let Some(request) = resolve_project_file_test_request(path) {
        return ProjectCommandScope::ProjectTestFile(request);
    }
    ProjectCommandScope::DirectSource
}

pub(crate) fn resolve_project_check_command_scope(path: &Path) -> ProjectCheckCommandScope {
    if is_project_context_path(path) {
        return ProjectCheckCommandScope::Project {
            request_root_manifest_path: resolve_project_workspace_member_command_request_root(path),
        };
    }

    if let Some(request_root_manifest_path) =
        resolve_project_workspace_member_command_request_root(path)
    {
        return ProjectCheckCommandScope::Project {
            request_root_manifest_path: Some(request_root_manifest_path),
        };
    }

    ProjectCheckCommandScope::DirectSource
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProjectSourceBuildTargetRequest {
    pub(crate) request_root_manifest_path: PathBuf,
    pub(crate) selector: ProjectTargetSelector,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProjectFileTestRequest {
    pub(crate) request_root_manifest_path: PathBuf,
    pub(crate) display_path: String,
}

pub(crate) fn project_request_root(path: &Path) -> PathBuf {
    if is_project_manifest_path(path) {
        path.parent().unwrap_or(Path::new(".")).to_path_buf()
    } else {
        path.to_path_buf()
    }
}

pub(crate) fn resolve_project_member_request_root(package_manifest_path: &Path) -> PathBuf {
    find_enclosing_workspace_manifest_for_member(package_manifest_path)
        .unwrap_or_else(|| package_manifest_path.to_path_buf())
}

pub(crate) fn display_relative_to_root(root: &Path, path: &Path) -> String {
    if let Ok(relative) = path.strip_prefix(root) {
        normalize_path(relative)
    } else {
        normalize_path(path)
    }
}

pub(crate) fn resolve_project_workspace_member_command_request_root(
    path: &Path,
) -> Option<PathBuf> {
    if !path.is_dir() && !is_ql_source_file(path) && !is_project_manifest_path(path) {
        return None;
    }

    let manifest = load_project_manifest(path).ok()?;
    Some(resolve_project_member_request_root(&manifest.manifest_path))
}

pub(crate) fn resolve_project_source_build_target_request(
    path: &Path,
) -> Option<ProjectSourceBuildTargetRequest> {
    if !is_ql_source_file(path) {
        return None;
    }

    let manifest = load_project_manifest(path).ok()?;
    let package_name = package_name(&manifest).ok()?.to_owned();
    let display_path = project_target_display_path(&manifest.manifest_path, path);
    let targets = discover_package_build_targets(&manifest).ok()?;
    if !targets.iter().any(|target| {
        project_target_display_path(&manifest.manifest_path, target.path.as_path()) == display_path
    }) {
        return None;
    }

    let request_root_manifest_path = resolve_project_member_request_root(&manifest.manifest_path);

    Some(ProjectSourceBuildTargetRequest {
        request_root_manifest_path,
        selector: ProjectTargetSelector {
            package_name: Some(package_name),
            target: Some(ProjectTargetSelectorKind::DisplayPath(display_path)),
        },
    })
}

pub(crate) fn resolve_project_file_test_request(path: &Path) -> Option<ProjectFileTestRequest> {
    if !is_ql_source_file(path) {
        return None;
    }

    let manifest = load_project_manifest(path).ok()?;
    let _ = package_name(&manifest).ok()?;
    let manifest_path = manifest.manifest_path.clone();
    let package_root = manifest_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();
    let tests_root = package_root.join("tests");
    path.strip_prefix(&tests_root).ok()?;
    let request_root_manifest_path = resolve_project_member_request_root(&manifest_path);
    let request_root = project_request_root(&request_root_manifest_path);

    Some(ProjectFileTestRequest {
        request_root_manifest_path,
        display_path: display_relative_to_root(&request_root, path),
    })
}

fn is_project_context_path(path: &Path) -> bool {
    path.is_dir() || is_project_manifest_path(path)
}

fn is_project_manifest_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("qlang.toml"))
}

fn find_enclosing_workspace_manifest_for_member(package_manifest_path: &Path) -> Option<PathBuf> {
    let package_root = package_manifest_path.parent()?;
    let mut current = package_root.parent().map(Path::to_path_buf);

    while let Some(directory) = current {
        let candidate = directory.join("qlang.toml");
        if candidate.is_file()
            && let Ok(manifest) = load_project_manifest(&candidate)
            && workspace_manifest_contains_member(&manifest, package_manifest_path)
        {
            return Some(manifest.manifest_path);
        }
        current = directory.parent().map(Path::to_path_buf);
    }

    None
}

fn workspace_manifest_contains_member(
    workspace_manifest: &ql_project::ProjectManifest,
    package_manifest_path: &Path,
) -> bool {
    let Some(workspace) = workspace_manifest.workspace.as_ref() else {
        return false;
    };

    let expected_manifest_path = normalize_path(package_manifest_path);
    let workspace_root = workspace_manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."));
    workspace.members.iter().any(|member| {
        load_project_manifest(&workspace_root.join(member))
            .ok()
            .is_some_and(|member_manifest| {
                normalize_path(&member_manifest.manifest_path) == expected_manifest_path
            })
    })
}

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

fn select_workspace_build_targets_with_failure(
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

struct ProjectTargetSelectionFailure {
    stage: &'static str,
    path: String,
    message: String,
    selector: String,
    target_count: usize,
}

fn report_project_target_selection_failure(
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

fn filter_workspace_build_targets(
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

fn load_project_target_members_for_workspace_member_path(
    path: &Path,
    command_label: &str,
    json: bool,
) -> Result<Vec<WorkspaceBuildTargets>, u8> {
    let request_root = resolve_project_workspace_member_command_request_root(path);
    let request_root = request_root.as_deref().unwrap_or(path);
    if json {
        return load_project_target_members_for_json(path, request_root, command_label);
    }

    load_workspace_build_targets_for_command_from_request_root(path, request_root, command_label)
}

pub(crate) fn load_workspace_build_targets_for_command_from_request_root(
    _request_path: &Path,
    request_root: &Path,
    command_label: &str,
) -> Result<Vec<WorkspaceBuildTargets>, u8> {
    let manifest = load_project_manifest(request_root).map_err(|error| {
        if let ql_project::ProjectError::ManifestNotFound { start } = &error {
            eprintln!(
                "error: {command_label} requires a package or workspace manifest; could not find `qlang.toml` starting from `{}`",
                normalize_path(start)
            );
        } else if let Some(manifest_path) =
            package_missing_name_manifest_path_from_project_error(&error)
        {
            eprintln!(
                "error: {command_label} manifest `{}` does not declare `[package].name`",
                normalize_path(manifest_path)
            );
        } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(&error)
        {
            eprintln!("error: {command_label} {error}");
            eprintln!(
                "note: failing package manifest: {}",
                normalize_path(manifest_path)
            );
        } else {
            eprintln!("error: {command_label} {error}");
        }
        1
    })?;

    discover_workspace_build_targets(&manifest).map_err(|error| {
        if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(&error) {
            eprintln!(
                "error: {command_label} manifest `{}` does not declare `[package].name`",
                normalize_path(manifest_path)
            );
        } else if let ql_project::ProjectError::PackageSourceRootNotFound { path } = &error {
            eprintln!(
                "error: {command_label} package source directory `{}` does not exist",
                normalize_path(path)
            );
        } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(&error) {
            eprintln!("error: {command_label} {error}");
            eprintln!(
                "note: failing package manifest: {}",
                normalize_path(manifest_path)
            );
        } else {
            eprintln!("error: {command_label} {error}");
        }
        1
    })
}

fn load_project_target_members_for_json(
    path: &Path,
    request_root: &Path,
    command_label: &str,
) -> Result<Vec<WorkspaceBuildTargets>, u8> {
    let manifest = load_project_manifest(request_root).map_err(|error| {
        print!(
            "{}",
            render_project_targets_preflight_failure_json(
                path,
                "manifest-load",
                project_targets_load_error_message(command_label, &error),
                project_targets_error_manifest_path(&error),
            )
        );
        1
    })?;

    discover_workspace_build_targets(&manifest).map_err(|error| {
        print!(
            "{}",
            render_project_targets_preflight_failure_json(
                path,
                "target-discovery",
                project_targets_discovery_error_message(command_label, &error),
                project_targets_error_manifest_path(&error),
            )
        );
        1
    })
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

pub(crate) fn is_runnable_project_target(kind: BuildTargetKind) -> bool {
    matches!(kind, BuildTargetKind::Binary | BuildTargetKind::Source)
}

pub(crate) fn project_target_display_path(manifest_path: &Path, target_path: &Path) -> String {
    let package_root = manifest_path.parent().unwrap_or(Path::new("."));
    if let Ok(relative) = target_path.strip_prefix(package_root) {
        normalize_path(relative)
    } else {
        normalize_path(target_path)
    }
}

fn project_targets_load_error_message(
    command_label: &str,
    error: &ql_project::ProjectError,
) -> String {
    if let ql_project::ProjectError::ManifestNotFound { start } = error {
        return format!(
            "{command_label} requires a package or workspace manifest; could not find `qlang.toml` starting from `{}`",
            normalize_path(start)
        );
    }
    if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error) {
        return format!(
            "{command_label} manifest `{}` does not declare `[package].name`",
            normalize_path(manifest_path)
        );
    }
    format!("{command_label} {error}")
}

fn project_targets_discovery_error_message(
    command_label: &str,
    error: &ql_project::ProjectError,
) -> String {
    if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error) {
        return format!(
            "{command_label} manifest `{}` does not declare `[package].name`",
            normalize_path(manifest_path)
        );
    }
    if let ql_project::ProjectError::PackageSourceRootNotFound { path } = error {
        return format!(
            "{command_label} package source directory `{}` does not exist",
            normalize_path(path)
        );
    }
    format!("{command_label} {error}")
}

fn project_targets_error_manifest_path(error: &ql_project::ProjectError) -> Option<&Path> {
    package_missing_name_manifest_path_from_project_error(error)
        .or_else(|| package_check_manifest_path_from_project_error(error))
}

fn is_ql_source_file(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("ql"))
}

#[cfg(test)]
#[path = "project_targets_tests.rs"]
mod tests;
