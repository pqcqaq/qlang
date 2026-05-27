use std::path::{Path, PathBuf};

use ql_project::{discover_package_build_targets, load_project_manifest, package_name};

use crate::cli_utils::normalize_path;

use super::{ProjectTargetSelector, ProjectTargetSelectorKind};

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

fn resolve_project_source_build_target_request(
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

fn resolve_project_file_test_request(path: &Path) -> Option<ProjectFileTestRequest> {
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

pub(crate) fn project_target_display_path(manifest_path: &Path, target_path: &Path) -> String {
    let package_root = manifest_path.parent().unwrap_or(Path::new("."));
    if let Ok(relative) = target_path.strip_prefix(package_root) {
        normalize_path(relative)
    } else {
        normalize_path(target_path)
    }
}

pub(super) fn is_ql_source_file(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("ql"))
}
