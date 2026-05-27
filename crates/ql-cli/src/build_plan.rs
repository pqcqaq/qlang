use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ql_driver::{BuildError, BuildOptions};
use ql_project::{
    BuildTargetKind, WorkspaceBuildTargets, discover_package_build_targets, load_project_manifest,
    package_name,
};

use crate::build_outputs::project_dependency_target_build_options;
use crate::cli_utils::{
    normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error,
};
use crate::dependency_bridge_reporting::{
    dependency_interface_load_message, dependency_source_parse_message,
    dependency_source_read_message,
};
use crate::project_manifest_paths::reference_manifest_path;
use crate::project_target_build::build_project_source_target_silent;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProjectBuildPlanMember {
    pub(crate) member: WorkspaceBuildTargets,
    pub(crate) emit_interface: bool,
    pub(crate) require_targets: bool,
}

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

pub(crate) struct BuildPlanResolveError {
    pub(crate) manifest_path: Option<PathBuf>,
    pub(crate) owner_manifest_path: Option<PathBuf>,
    pub(crate) dependency_manifest_path: Option<PathBuf>,
    pub(crate) failure_kind: BuildPlanResolveFailureKind,
}

pub(crate) enum BuildPlanResolveFailureKind {
    Dependency { message: String },
    Cycle { cycle_manifests: Vec<String> },
}

pub(crate) fn resolve_project_build_plan_members(
    workspace_members: &[WorkspaceBuildTargets],
    selected_members: &[WorkspaceBuildTargets],
    command_label: &str,
) -> Result<Vec<ProjectBuildPlanMember>, u8> {
    let workspace_members_by_manifest = workspace_members
        .iter()
        .map(|member| (normalize_path(&member.member_manifest_path), member.clone()))
        .collect::<BTreeMap<_, _>>();
    let selected_members_by_manifest = selected_members
        .iter()
        .map(|member| (normalize_path(&member.member_manifest_path), member.clone()))
        .collect::<BTreeMap<_, _>>();
    let selected_manifest_paths = selected_members_by_manifest
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();

    let mut ordered = Vec::new();
    let mut visiting = Vec::new();
    let mut in_progress = BTreeSet::new();
    let mut visited = BTreeSet::new();

    for member in selected_members {
        collect_project_build_plan_member(
            &member.member_manifest_path,
            None,
            &workspace_members_by_manifest,
            &selected_members_by_manifest,
            &selected_manifest_paths,
            &mut visiting,
            &mut in_progress,
            &mut visited,
            &mut ordered,
            command_label,
        )?;
    }

    Ok(ordered)
}

pub(crate) fn resolve_project_build_plan_members_quiet(
    workspace_members: &[WorkspaceBuildTargets],
    selected_members: &[WorkspaceBuildTargets],
) -> Result<Vec<ProjectBuildPlanMember>, BuildPlanResolveError> {
    let workspace_members_by_manifest = workspace_members
        .iter()
        .map(|member| (normalize_path(&member.member_manifest_path), member.clone()))
        .collect::<BTreeMap<_, _>>();
    let selected_members_by_manifest = selected_members
        .iter()
        .map(|member| (normalize_path(&member.member_manifest_path), member.clone()))
        .collect::<BTreeMap<_, _>>();
    let selected_manifest_paths = selected_members_by_manifest
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();

    let mut ordered = Vec::new();
    let mut visiting = Vec::new();
    let mut in_progress = BTreeSet::new();
    let mut visited = BTreeSet::new();

    for member in selected_members {
        collect_project_build_plan_member_quiet(
            &member.member_manifest_path,
            Some(&member.member_manifest_path),
            None,
            &workspace_members_by_manifest,
            &selected_members_by_manifest,
            &selected_manifest_paths,
            &mut visiting,
            &mut in_progress,
            &mut visited,
            &mut ordered,
        )?;
    }

    Ok(ordered)
}

pub(crate) fn select_project_build_plan_root_members(
    members: &[WorkspaceBuildTargets],
    manifest_paths: &[PathBuf],
) -> Vec<WorkspaceBuildTargets> {
    let selected_manifest_paths = manifest_paths
        .iter()
        .map(|manifest_path| normalize_path(manifest_path))
        .collect::<BTreeSet<_>>();
    members
        .iter()
        .filter(|member| {
            selected_manifest_paths.contains(&normalize_path(&member.member_manifest_path))
        })
        .cloned()
        .collect()
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

fn dependency_manifest_paths_for_selected_roots(
    workspace_members: &[WorkspaceBuildTargets],
    selected_members: &[WorkspaceBuildTargets],
    command_label: &str,
) -> Result<BTreeSet<String>, u8> {
    let mut dependency_manifest_paths = BTreeSet::new();
    for selected_member in selected_members {
        let selected_plan = resolve_project_build_plan_members(
            workspace_members,
            std::slice::from_ref(selected_member),
            command_label,
        )?;
        for plan_member in selected_plan {
            if !plan_member.require_targets {
                dependency_manifest_paths
                    .insert(normalize_path(&plan_member.member.member_manifest_path));
            }
        }
    }
    Ok(dependency_manifest_paths)
}

fn collect_project_build_plan_member(
    start_path: &Path,
    owner_manifest_path: Option<&Path>,
    workspace_members_by_manifest: &BTreeMap<String, WorkspaceBuildTargets>,
    selected_members_by_manifest: &BTreeMap<String, WorkspaceBuildTargets>,
    selected_manifest_paths: &BTreeSet<String>,
    visiting: &mut Vec<String>,
    in_progress: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    ordered: &mut Vec<ProjectBuildPlanMember>,
    command_label: &str,
) -> Result<(), u8> {
    let manifest = load_project_manifest(start_path).map_err(|error| {
        report_project_build_dependency_error(command_label, owner_manifest_path, &error);
        1
    })?;
    let manifest_key = normalize_path(&manifest.manifest_path);
    if visited.contains(&manifest_key) {
        return Ok(());
    }
    if in_progress.contains(&manifest_key) {
        report_project_build_dependency_cycle(command_label, visiting, &manifest_key);
        return Err(1);
    }

    in_progress.insert(manifest_key.clone());
    visiting.push(manifest_key.clone());

    let manifest_dir = manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();
    for reference in &manifest.references.packages {
        collect_project_build_plan_member(
            &manifest_dir.join(reference),
            Some(&manifest.manifest_path),
            workspace_members_by_manifest,
            selected_members_by_manifest,
            selected_manifest_paths,
            visiting,
            in_progress,
            visited,
            ordered,
            command_label,
        )?;
    }

    visiting.pop();
    in_progress.remove(&manifest_key);
    visited.insert(manifest_key.clone());

    let member = project_build_plan_member_targets(
        &manifest,
        &manifest_key,
        workspace_members_by_manifest,
        selected_members_by_manifest,
        command_label,
    )?;
    let selected = selected_manifest_paths.contains(&manifest_key);
    ordered.push(ProjectBuildPlanMember {
        member,
        emit_interface: selected,
        require_targets: selected,
    });

    Ok(())
}

fn collect_project_build_plan_member_quiet(
    start_path: &Path,
    dependency_manifest_path: Option<&Path>,
    owner_manifest_path: Option<&Path>,
    workspace_members_by_manifest: &BTreeMap<String, WorkspaceBuildTargets>,
    selected_members_by_manifest: &BTreeMap<String, WorkspaceBuildTargets>,
    selected_manifest_paths: &BTreeSet<String>,
    visiting: &mut Vec<String>,
    in_progress: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    ordered: &mut Vec<ProjectBuildPlanMember>,
) -> Result<(), BuildPlanResolveError> {
    let manifest = load_project_manifest(start_path).map_err(|error| {
        build_plan_dependency_failure(
            owner_manifest_path,
            dependency_manifest_path.or(Some(start_path)),
            &error,
        )
    })?;
    let manifest_key = normalize_path(&manifest.manifest_path);
    if visited.contains(&manifest_key) {
        return Ok(());
    }
    if in_progress.contains(&manifest_key) {
        return Err(build_plan_cycle_failure(visiting, &manifest_key));
    }

    in_progress.insert(manifest_key.clone());
    visiting.push(manifest_key.clone());

    let manifest_dir = manifest
        .manifest_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();
    for reference in &manifest.references.packages {
        let reference_manifest_path = reference_manifest_path(&manifest, reference);
        collect_project_build_plan_member_quiet(
            &manifest_dir.join(reference),
            Some(&reference_manifest_path),
            Some(&manifest.manifest_path),
            workspace_members_by_manifest,
            selected_members_by_manifest,
            selected_manifest_paths,
            visiting,
            in_progress,
            visited,
            ordered,
        )?;
    }

    visiting.pop();
    in_progress.remove(&manifest_key);
    visited.insert(manifest_key.clone());

    let member = project_build_plan_member_targets_quiet(
        &manifest,
        &manifest_key,
        owner_manifest_path,
        workspace_members_by_manifest,
        selected_members_by_manifest,
    )?;
    let selected = selected_manifest_paths.contains(&manifest_key);
    ordered.push(ProjectBuildPlanMember {
        member,
        emit_interface: selected,
        require_targets: selected,
    });

    Ok(())
}

fn project_build_plan_member_targets(
    manifest: &ql_project::ProjectManifest,
    manifest_key: &str,
    workspace_members_by_manifest: &BTreeMap<String, WorkspaceBuildTargets>,
    selected_members_by_manifest: &BTreeMap<String, WorkspaceBuildTargets>,
    command_label: &str,
) -> Result<WorkspaceBuildTargets, u8> {
    if let Some(member) = selected_members_by_manifest.get(manifest_key) {
        return Ok(member.clone());
    }
    if let Some(member) = workspace_members_by_manifest.get(manifest_key) {
        return Ok(member.clone());
    }

    let package_name = package_name(manifest).map_err(|error| {
        report_project_build_dependency_error(command_label, None, &error);
        1
    })?;
    let targets = discover_package_build_targets(manifest).map_err(|error| {
        report_project_build_dependency_error(command_label, None, &error);
        1
    })?;
    Ok(WorkspaceBuildTargets {
        member_manifest_path: manifest.manifest_path.clone(),
        package_name: package_name.to_owned(),
        default_profile: manifest.profile.as_ref().map(|profile| profile.default),
        targets,
    })
}

fn project_build_plan_member_targets_quiet(
    manifest: &ql_project::ProjectManifest,
    manifest_key: &str,
    owner_manifest_path: Option<&Path>,
    workspace_members_by_manifest: &BTreeMap<String, WorkspaceBuildTargets>,
    selected_members_by_manifest: &BTreeMap<String, WorkspaceBuildTargets>,
) -> Result<WorkspaceBuildTargets, BuildPlanResolveError> {
    if let Some(member) = selected_members_by_manifest.get(manifest_key) {
        return Ok(member.clone());
    }
    if let Some(member) = workspace_members_by_manifest.get(manifest_key) {
        return Ok(member.clone());
    }

    let package_name = package_name(manifest).map_err(|error| {
        build_plan_dependency_failure(owner_manifest_path, Some(&manifest.manifest_path), &error)
    })?;
    let targets = discover_package_build_targets(manifest).map_err(|error| {
        build_plan_dependency_failure(owner_manifest_path, Some(&manifest.manifest_path), &error)
    })?;
    Ok(WorkspaceBuildTargets {
        member_manifest_path: manifest.manifest_path.clone(),
        package_name: package_name.to_owned(),
        default_profile: manifest.profile.as_ref().map(|profile| profile.default),
        targets,
    })
}

fn build_plan_dependency_failure(
    owner_manifest_path: Option<&Path>,
    dependency_manifest_path: Option<&Path>,
    error: &ql_project::ProjectError,
) -> BuildPlanResolveError {
    let manifest_path =
        if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error) {
            Some(manifest_path.to_path_buf())
        } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(error) {
            Some(manifest_path.to_path_buf())
        } else if let ql_project::ProjectError::ManifestNotFound { .. } = error {
            dependency_manifest_path.map(Path::to_path_buf)
        } else if let ql_project::ProjectError::PackageSourceRootNotFound { .. } = error {
            dependency_manifest_path.map(Path::to_path_buf)
        } else {
            dependency_manifest_path.map(Path::to_path_buf)
        };
    BuildPlanResolveError {
        manifest_path,
        owner_manifest_path: owner_manifest_path.map(Path::to_path_buf),
        dependency_manifest_path: dependency_manifest_path.map(Path::to_path_buf),
        failure_kind: BuildPlanResolveFailureKind::Dependency {
            message: error.to_string(),
        },
    }
}

fn build_plan_cycle_failure(
    visiting: &[String],
    repeated_manifest_path: &str,
) -> BuildPlanResolveError {
    let cycle_start = visiting
        .iter()
        .position(|manifest_path| manifest_path == repeated_manifest_path)
        .unwrap_or(0);
    let mut cycle_manifests = visiting[cycle_start..].to_vec();
    cycle_manifests.push(repeated_manifest_path.to_owned());
    BuildPlanResolveError {
        manifest_path: Some(PathBuf::from(repeated_manifest_path)),
        owner_manifest_path: None,
        dependency_manifest_path: Some(PathBuf::from(repeated_manifest_path)),
        failure_kind: BuildPlanResolveFailureKind::Cycle { cycle_manifests },
    }
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

fn report_project_build_dependency_cycle(
    command_label: &str,
    visiting: &[String],
    repeated_manifest_path: &str,
) {
    let cycle_start = visiting
        .iter()
        .position(|manifest_path| manifest_path == repeated_manifest_path)
        .unwrap_or(0);
    let mut cycle = visiting[cycle_start..].to_vec();
    cycle.push(repeated_manifest_path.to_owned());
    eprintln!("error: {command_label} local package build dependencies contain a cycle");
    eprintln!("note: cycle manifests: {}", cycle.join(" -> "));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_prep_dependency_interface_failure_preserves_dependency_context() {
        let error = target_prep_dependency_interface_failure(
            Path::new("workspace/dep/qlang.toml"),
            "dep",
            Path::new("workspace/dep/dep.qi"),
            "missing interface",
        );

        let PrepareProjectTargetBuildFailureKind::DependencyInterface {
            dependency_manifest_path,
            dependency_package,
            interface_path,
            message,
        } = error.failure_kind
        else {
            panic!("expected dependency interface target-prep failure");
        };

        assert_eq!(
            dependency_manifest_path,
            PathBuf::from("workspace/dep/qlang.toml")
        );
        assert_eq!(dependency_package, "dep");
        assert_eq!(interface_path, PathBuf::from("workspace/dep/dep.qi"));
        assert_eq!(
            message,
            "failed to load referenced package interface `workspace/dep/dep.qi`: missing interface"
        );
    }

    #[test]
    fn target_prep_dependency_source_failures_preserve_message_contracts() {
        let read_error = target_prep_dependency_source_read_failure(
            Path::new("workspace/dep/qlang.toml"),
            "dep",
            Path::new("workspace/dep/src/lib.ql"),
            "access denied",
        );
        let parse_error = target_prep_dependency_source_parse_failure(
            Path::new("workspace/dep/qlang.toml"),
            "dep",
            Path::new("workspace/dep/src/lib.ql"),
            "public value bridges",
        );

        let PrepareProjectTargetBuildFailureKind::DependencySource {
            dependency_manifest_path,
            dependency_package,
            source_path,
            message,
        } = read_error.failure_kind
        else {
            panic!("expected dependency source read target-prep failure");
        };
        assert_eq!(
            dependency_manifest_path,
            PathBuf::from("workspace/dep/qlang.toml")
        );
        assert_eq!(dependency_package, "dep");
        assert_eq!(source_path, PathBuf::from("workspace/dep/src/lib.ql"));
        assert_eq!(
            message,
            "failed to access dependency source `workspace/dep/src/lib.ql`: access denied"
        );

        let PrepareProjectTargetBuildFailureKind::DependencySource { message, .. } =
            parse_error.failure_kind
        else {
            panic!("expected dependency source parse target-prep failure");
        };
        assert_eq!(
            message,
            "failed to parse dependency source `workspace/dep/src/lib.ql` while preparing public value bridges"
        );
    }
}
