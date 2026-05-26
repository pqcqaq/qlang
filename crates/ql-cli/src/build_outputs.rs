use std::collections::BTreeSet;
use std::env;
use std::path::{Path, PathBuf};

use ql_driver::{
    BuildEmit, BuildOptions, BuildProfile, CHeaderSurface, ToolchainError, default_output_path,
};
use ql_project::{BuildTarget, BuildTargetKind, ManifestBuildProfile, WorkspaceBuildTargets};

use crate::cli_utils::normalize_path;

pub(crate) fn build_output_path(path: &Path, options: &BuildOptions) -> Option<PathBuf> {
    match &options.output {
        Some(output_path) => Some(output_path.clone()),
        None => env::current_dir().ok().map(|build_root| {
            default_output_path(&build_root, path, options.profile, options.emit)
        }),
    }
}

pub(crate) fn build_header_output_path(path: &Path, options: &BuildOptions) -> Option<PathBuf> {
    let header = options.c_header.as_ref()?;
    match &header.output {
        Some(output_path) => Some(output_path.clone()),
        None => {
            let artifact_path = build_output_path(path, options)?;
            Some(default_build_header_output_path(
                &artifact_path,
                path,
                header.surface,
            ))
        }
    }
}

pub(crate) fn project_target_build_options(
    member: &WorkspaceBuildTargets,
    target: &BuildTarget,
    options: &BuildOptions,
    emit_overridden: bool,
    profile_overridden: bool,
) -> BuildOptions {
    let mut target_options =
        apply_manifest_default_profile(options, member.default_profile, profile_overridden);
    if !emit_overridden && target.kind == BuildTargetKind::Library {
        target_options.emit = BuildEmit::StaticLibrary;
    }
    if target_options.output.is_none() {
        target_options.output = Some(project_target_output_path(
            &member.member_manifest_path,
            target.path.as_path(),
            target_options.profile,
            target_options.emit,
        ));
    }
    target_options
}

pub(crate) fn project_dependency_target_build_options(
    member: &WorkspaceBuildTargets,
    target: &BuildTarget,
    options: &BuildOptions,
    emit_overridden: bool,
    profile_overridden: bool,
) -> BuildOptions {
    let mut target_options =
        apply_manifest_default_profile(options, member.default_profile, profile_overridden);
    if !emit_overridden && target.kind == BuildTargetKind::Library {
        target_options.emit = BuildEmit::StaticLibrary;
    }
    target_options.output = Some(project_target_output_path(
        &member.member_manifest_path,
        target.path.as_path(),
        target_options.profile,
        target_options.emit,
    ));
    target_options.c_header = None;
    target_options
}

pub(crate) fn apply_manifest_default_profile(
    options: &BuildOptions,
    default_profile: Option<ManifestBuildProfile>,
    profile_overridden: bool,
) -> BuildOptions {
    let mut resolved = options.clone();
    if !profile_overridden && let Some(default_profile) = default_profile {
        resolved.profile = project_manifest_build_profile(default_profile);
    }
    resolved
}

fn project_manifest_build_profile(profile: ManifestBuildProfile) -> BuildProfile {
    match profile {
        ManifestBuildProfile::Debug => BuildProfile::Debug,
        ManifestBuildProfile::Release => BuildProfile::Release,
    }
}

pub(crate) fn project_target_output_path(
    manifest_path: &Path,
    target_path: &Path,
    profile: BuildProfile,
    emit: BuildEmit,
) -> PathBuf {
    let package_root = manifest_path.parent().unwrap_or(Path::new("."));
    let source_root = package_root.join("src");
    let relative_target = target_path
        .strip_prefix(&source_root)
        .unwrap_or(target_path);
    let default_output = default_output_path(package_root, target_path, profile, emit);
    let file_name = default_output
        .file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("artifact"));
    let mut output_path = package_root
        .join("target")
        .join("ql")
        .join(profile.dir_name());
    if let Some(parent) = relative_target.parent()
        && !parent.as_os_str().is_empty()
    {
        output_path = output_path.join(parent);
    }
    output_path.join(file_name)
}

fn default_build_header_output_path(
    artifact_path: &Path,
    input_path: &Path,
    surface: CHeaderSurface,
) -> PathBuf {
    let stem = input_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("module");
    let file_name = match surface {
        CHeaderSurface::Exports => format!("{stem}.h"),
        CHeaderSurface::Imports => format!("{stem}.imports.h"),
        CHeaderSurface::Both => format!("{stem}.ffi.h"),
    };
    artifact_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(file_name)
}

pub(crate) fn colliding_build_header_output_path(
    path: &Path,
    options: &BuildOptions,
) -> Option<PathBuf> {
    let output_path = build_output_path(path, options)?;
    let header_output_path = build_header_output_path(path, options)?;
    (header_output_path == output_path).then_some(header_output_path)
}

pub(crate) fn first_colliding_project_build_output_path(
    members: &[WorkspaceBuildTargets],
    options: &BuildOptions,
    emit_overridden: bool,
    profile_overridden: bool,
) -> Option<PathBuf> {
    let mut seen = BTreeSet::new();
    for member in members {
        for target in &member.targets {
            let target_options = project_target_build_options(
                member,
                target,
                options,
                emit_overridden,
                profile_overridden,
            );
            let output_path = build_output_path(&target.path, &target_options)?;
            if !seen.insert(output_path.clone()) {
                return Some(output_path);
            }
        }
    }
    None
}

pub(crate) fn first_colliding_project_build_header_output_path(
    members: &[WorkspaceBuildTargets],
    options: &BuildOptions,
    emit_overridden: bool,
    profile_overridden: bool,
) -> Option<PathBuf> {
    if options.c_header.is_none() {
        return None;
    }

    let mut seen = BTreeSet::new();
    for member in members {
        for target in &member.targets {
            let target_options = project_target_build_options(
                member,
                target,
                options,
                emit_overridden,
                profile_overridden,
            );
            let output_path = build_header_output_path(&target.path, &target_options)?;
            if !seen.insert(output_path.clone()) {
                return Some(output_path);
            }
        }
    }
    None
}

pub(crate) fn io_targets_build_output_path(io_path: &Path, output_path: &Path) -> bool {
    io_path == output_path || output_path.starts_with(io_path)
}

pub(crate) fn io_targets_build_header_output_path(io_path: &Path, output_path: &Path) -> bool {
    io_path == output_path || output_path.starts_with(io_path)
}

pub(crate) fn toolchain_targets_build_output_path(
    error: &ToolchainError,
    output_path: &Path,
) -> bool {
    match error {
        ToolchainError::InvocationFailed { stderr, .. } => {
            let output_failure = stderr.to_ascii_lowercase();
            let has_output_open_failure = [
                "unable to open output file",
                "cannot open output file",
                "could not open output file",
                "can't open output file",
                "failed to open output file",
                "unable to open file",
                "cannot open file",
                "could not open file",
                "can't open file",
                "failed to open file",
            ]
            .iter()
            .any(|message| output_failure.contains(message));
            if !has_output_open_failure {
                return false;
            }

            let collapsed_output_failure = output_failure
                .chars()
                .filter(|ch| !ch.is_whitespace())
                .collect::<String>();
            let mentions_output_path = collapsed_output_failure
                .contains(&normalize_path(output_path).to_ascii_lowercase())
                || collapsed_output_failure
                    .contains(&output_path.display().to_string().to_ascii_lowercase());
            mentions_output_path
                || toolchain_mentions_codegen_output_path(&collapsed_output_failure, output_path)
        }
        ToolchainError::NotFound { .. } => false,
    }
}

fn toolchain_mentions_codegen_output_path(stderr: &str, output_path: &Path) -> bool {
    let Some(stem) = output_path.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    let Some(extension) = output_path
        .extension()
        .and_then(|extension| extension.to_str())
    else {
        return false;
    };
    let stem = stem.to_ascii_lowercase();
    let extension = extension.to_ascii_lowercase();
    if !stderr.contains(&format!("{stem}.")) || !stderr.contains(&format!(".codegen.{extension}")) {
        return false;
    }

    let Some(parent) = output_path.parent() else {
        return true;
    };
    let normalized_parent = normalize_path(parent).to_ascii_lowercase();
    let display_parent = parent.display().to_string().to_ascii_lowercase();
    [normalized_parent, display_parent]
        .into_iter()
        .map(|path| {
            path.chars()
                .filter(|ch| !ch.is_whitespace())
                .collect::<String>()
        })
        .any(|path| !path.is_empty() && stderr.contains(&path))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use ql_driver::ToolchainError;

    use super::toolchain_targets_build_output_path;

    #[test]
    fn toolchain_output_path_detection_accepts_codegen_temp_artifact() {
        let output_path = Path::new("C:/tmp/workspace/app/build/app.obj");
        let error = ToolchainError::InvocationFailed {
            program: "clang".to_owned(),
            status: Some(9),
            stderr: "unable to open output file 'C:/tmp/workspace/app/build/app.123.codegen.obj': Permission denied".to_owned(),
        };

        assert!(toolchain_targets_build_output_path(&error, output_path));
    }

    #[test]
    fn toolchain_output_path_detection_ignores_non_open_failures() {
        let output_path = Path::new("C:/tmp/workspace/app/build/app.obj");
        let error = ToolchainError::InvocationFailed {
            program: "clang".to_owned(),
            status: Some(1),
            stderr: "undefined symbol while linking C:/tmp/workspace/app/build/app.123.codegen.obj"
                .to_owned(),
        };

        assert!(!toolchain_targets_build_output_path(&error, output_path));
    }
}
