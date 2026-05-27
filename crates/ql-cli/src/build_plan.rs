use std::path::{Path, PathBuf};

use ql_driver::{BuildError, BuildOptions};
use ql_project::{BuildTargetKind, WorkspaceBuildTargets};

use crate::build_outputs::project_dependency_target_build_options;
use crate::cli_utils::{
    normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error,
};
use crate::dependency_bridge_reporting::{
    dependency_interface_load_message, dependency_source_parse_message,
    dependency_source_read_message,
};
use crate::project_target_build::build_project_source_target_silent;

mod resolution;

use resolution::dependency_manifest_paths_for_selected_roots;
pub(crate) use resolution::{
    BuildPlanResolveError, BuildPlanResolveFailureKind, ProjectBuildPlanMember,
    resolve_project_build_plan_members, resolve_project_build_plan_members_quiet,
    select_project_build_plan_root_members,
};

pub(crate) enum BuildTargetJsonError {
    Early(PrepareProjectTargetBuildError),
    Build(BuildError),
}

pub(crate) struct PrepareProjectTargetBuildError {
    pub(crate) failure_kind: PrepareProjectTargetBuildFailureKind,
}

pub(crate) enum PrepareProjectTargetBuildFailureKind {
    DependencyManifest {
        dependency_manifest_path: Option<PathBuf>,
        error_kind: &'static str,
        message: String,
    },
    DependencyInterface {
        dependency_manifest_path: PathBuf,
        dependency_package: String,
        interface_path: PathBuf,
        message: String,
    },
    DependencySource {
        dependency_manifest_path: PathBuf,
        dependency_package: String,
        source_path: PathBuf,
        message: String,
    },
    DependencyExternConflict {
        symbol: String,
        first_package: String,
        first_manifest_path: PathBuf,
        conflicting_package: String,
        conflicting_manifest_path: PathBuf,
    },
    DependencyFunctionConflict {
        symbol: String,
        first_package: String,
        first_manifest_path: PathBuf,
        conflicting_package: String,
        conflicting_manifest_path: PathBuf,
    },
    DependencyFunctionLocalConflict {
        symbol: String,
        dependency_package: String,
        dependency_manifest_path: PathBuf,
    },
    DependencyFunctionUnsupportedGeneric {
        symbol: String,
        dependency_package: String,
        dependency_manifest_path: PathBuf,
    },
    DependencyTypeConflict {
        symbol: String,
        first_package: String,
        first_manifest_path: PathBuf,
        conflicting_package: String,
        conflicting_manifest_path: PathBuf,
    },
    DependencyTypeLocalConflict {
        symbol: String,
        dependency_package: String,
        dependency_manifest_path: PathBuf,
    },
    DependencyValueConflict {
        symbol: String,
        first_package: String,
        first_manifest_path: PathBuf,
        conflicting_package: String,
        conflicting_manifest_path: PathBuf,
    },
    DependencyValueLocalConflict {
        symbol: String,
        dependency_package: String,
        dependency_manifest_path: PathBuf,
    },
    SourceRead {
        path: PathBuf,
        message: String,
    },
}

pub(crate) fn prepare_project_dependency_builds(
    workspace_members: &[WorkspaceBuildTargets],
    selected_members: &[WorkspaceBuildTargets],
    command_label: &str,
    options: &BuildOptions,
    profile_overridden: bool,
) -> Result<(), u8> {
    let build_plan =
        resolve_project_build_plan_members(workspace_members, selected_members, command_label)?;
    let dependency_manifest_paths = dependency_manifest_paths_for_selected_roots(
        workspace_members,
        selected_members,
        command_label,
    )?;
    for plan_member in &build_plan {
        let manifest_path = normalize_path(&plan_member.member.member_manifest_path);
        if !dependency_manifest_paths.contains(&manifest_path)
            || plan_member.member.targets.is_empty()
        {
            continue;
        }
        for target in &plan_member.member.targets {
            let target_options = project_dependency_target_build_options(
                &plan_member.member,
                target,
                options,
                false,
                profile_overridden,
            );
            build_project_source_target_silent(
                workspace_members,
                command_label,
                &plan_member.member.member_manifest_path,
                &target.path,
                &target_options,
                options,
                profile_overridden,
                false,
                target.kind == BuildTargetKind::Library,
            )?;
        }
    }
    Ok(())
}

pub(crate) fn prepare_project_test_package_builds(
    workspace_members: &[WorkspaceBuildTargets],
    selected_members: &[WorkspaceBuildTargets],
    command_label: &str,
    options: &BuildOptions,
    profile_overridden: bool,
) -> Result<(), u8> {
    let build_plan =
        resolve_project_build_plan_members(workspace_members, selected_members, command_label)?;
    for plan_member in &build_plan {
        if !plan_member.require_targets {
            continue;
        }
        for target in &plan_member.member.targets {
            if target.kind != BuildTargetKind::Library {
                continue;
            }
            let target_options = project_dependency_target_build_options(
                &plan_member.member,
                target,
                options,
                false,
                profile_overridden,
            );
            build_project_source_target_silent(
                workspace_members,
                command_label,
                &plan_member.member.member_manifest_path,
                &target.path,
                &target_options,
                options,
                profile_overridden,
                false,
                true,
            )?;
        }
    }
    Ok(())
}

pub(crate) fn report_project_build_dependency_error(
    command_label: &str,
    owner_manifest_path: Option<&Path>,
    error: &ql_project::ProjectError,
) {
    if let ql_project::ProjectError::ManifestNotFound { start } = error {
        eprintln!(
            "error: {command_label} could not find `qlang.toml` for a local package dependency starting from `{}`",
            normalize_path(start)
        );
    } else if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error)
    {
        eprintln!(
            "error: {command_label} manifest `{}` does not declare `[package].name`",
            normalize_path(manifest_path)
        );
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(manifest_path)
        );
    } else if let ql_project::ProjectError::PackageSourceRootNotFound { path } = error {
        eprintln!(
            "error: {command_label} package source directory `{}` does not exist",
            normalize_path(path)
        );
    } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(error) {
        eprintln!("error: {command_label} {error}");
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(manifest_path)
        );
    } else {
        eprintln!("error: {command_label} {error}");
    }

    if let Some(owner_manifest_path) = owner_manifest_path {
        eprintln!(
            "note: while resolving local build dependency from `{}`",
            normalize_path(owner_manifest_path)
        );
    }
}

pub(crate) fn target_prep_dependency_manifest_failure(
    dependency_manifest_path: Option<&Path>,
    error: &ql_project::ProjectError,
) -> PrepareProjectTargetBuildError {
    let dependency_manifest_path =
        if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error) {
            Some(manifest_path.to_path_buf())
        } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(error) {
            Some(manifest_path.to_path_buf())
        } else {
            dependency_manifest_path.map(Path::to_path_buf)
        };
    let error_kind = match error {
        ql_project::ProjectError::PackageSourceRootNotFound { .. } => "package-source-root",
        _ => "manifest",
    };

    PrepareProjectTargetBuildError {
        failure_kind: PrepareProjectTargetBuildFailureKind::DependencyManifest {
            dependency_manifest_path,
            error_kind,
            message: error.to_string(),
        },
    }
}

pub(crate) fn target_prep_dependency_interface_failure(
    dependency_manifest_path: &Path,
    dependency_package: &str,
    interface_path: &Path,
    error: impl std::fmt::Display,
) -> PrepareProjectTargetBuildError {
    PrepareProjectTargetBuildError {
        failure_kind: PrepareProjectTargetBuildFailureKind::DependencyInterface {
            dependency_manifest_path: dependency_manifest_path.to_path_buf(),
            dependency_package: dependency_package.to_owned(),
            interface_path: interface_path.to_path_buf(),
            message: dependency_interface_load_message(interface_path, error),
        },
    }
}

pub(crate) fn target_prep_dependency_source_read_failure(
    dependency_manifest_path: &Path,
    dependency_package: &str,
    source_path: &Path,
    error: impl std::fmt::Display,
) -> PrepareProjectTargetBuildError {
    PrepareProjectTargetBuildError {
        failure_kind: PrepareProjectTargetBuildFailureKind::DependencySource {
            dependency_manifest_path: dependency_manifest_path.to_path_buf(),
            dependency_package: dependency_package.to_owned(),
            source_path: source_path.to_path_buf(),
            message: dependency_source_read_message(source_path, error),
        },
    }
}

pub(crate) fn target_prep_dependency_source_parse_failure(
    dependency_manifest_path: &Path,
    dependency_package: &str,
    source_path: &Path,
    bridge_context: &str,
) -> PrepareProjectTargetBuildError {
    PrepareProjectTargetBuildError {
        failure_kind: PrepareProjectTargetBuildFailureKind::DependencySource {
            dependency_manifest_path: dependency_manifest_path.to_path_buf(),
            dependency_package: dependency_package.to_owned(),
            source_path: source_path.to_path_buf(),
            message: dependency_source_parse_message(source_path, bridge_context),
        },
    }
}

#[cfg(test)]
#[path = "build_plan_tests.rs"]
mod tests;
