use std::path::Path;

use ql_diagnostics::Diagnostic;
use ql_driver::{BuildArtifact, BuildEmit, BuildError};
use ql_project::{
    BuildTarget, WorkspaceBuildTargets, discover_workspace_build_targets, load_project_manifest,
};
use serde_json::{Value as JsonValue, json};

use crate::build_plan::{
    BuildPlanResolveError, BuildPlanResolveFailureKind, PrepareProjectTargetBuildError,
    PrepareProjectTargetBuildFailureKind,
};
use crate::cli_json_diagnostics::diagnostics_json;
use crate::cli_utils::normalize_path;
use crate::project_interfaces::{
    EmitPackageInterfaceError, ReferenceInterfacePrepError, ReferenceInterfacePrepFailureKind,
};
use crate::project_targets::project_target_display_path;

pub(crate) fn build_emit_cli_value(emit: BuildEmit) -> &'static str {
    match emit {
        BuildEmit::LlvmIr => "llvm-ir",
        BuildEmit::Assembly => "asm",
        BuildEmit::Object => "obj",
        BuildEmit::Executable => "exe",
        BuildEmit::DynamicLibrary => "dylib",
        BuildEmit::StaticLibrary => "staticlib",
    }
}

pub(crate) fn build_json_target(
    manifest_path: Option<&Path>,
    package_name: Option<&str>,
    kind: &str,
    display_path: String,
    artifact: &BuildArtifact,
    selected: bool,
) -> JsonValue {
    json!({
        "manifest_path": manifest_path.map(normalize_path),
        "package_name": package_name,
        "selected": selected,
        "dependency_only": !selected,
        "kind": kind,
        "path": display_path,
        "emit": build_emit_cli_value(artifact.emit),
        "profile": artifact.profile.dir_name(),
        "artifact_path": normalize_path(&artifact.path),
        "c_header_path": artifact.c_header.as_ref().map(|header| normalize_path(&header.path)),
    })
}

pub(crate) fn build_json_failure(
    manifest_path: Option<&Path>,
    package_name: Option<&str>,
    kind: &str,
    display_path: String,
    selected: bool,
    error: &BuildError,
) -> JsonValue {
    let manifest_path = manifest_path.map(normalize_path);
    match error {
        BuildError::InvalidInput(message) => json!({
            "manifest_path": manifest_path,
            "package_name": package_name,
            "selected": selected,
            "dependency_only": !selected,
            "kind": kind,
            "path": display_path,
            "error_kind": "invalid-input",
            "message": message,
        }),
        BuildError::Io { path, error } => json!({
            "manifest_path": manifest_path,
            "package_name": package_name,
            "selected": selected,
            "dependency_only": !selected,
            "kind": kind,
            "path": display_path,
            "error_kind": "io",
            "message": format!("failed to access `{}`: {error}", normalize_path(path)),
            "io_path": normalize_path(path),
        }),
        BuildError::Toolchain {
            error,
            preserved_artifacts,
        } => json!({
            "manifest_path": manifest_path,
            "package_name": package_name,
            "selected": selected,
            "dependency_only": !selected,
            "kind": kind,
            "path": display_path,
            "error_kind": "toolchain",
            "message": error.to_string(),
            "preserved_artifacts": preserved_artifacts
                .iter()
                .map(|path| normalize_path(path))
                .collect::<Vec<_>>(),
            "intermediate_ir": preserved_artifacts
                .iter()
                .find(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.contains(".codegen.ll"))
                })
                .map(|path| normalize_path(path)),
        }),
        BuildError::Diagnostics {
            path,
            source,
            diagnostics,
        } => json!({
            "manifest_path": manifest_path,
            "package_name": package_name,
            "selected": selected,
            "dependency_only": !selected,
            "kind": kind,
            "path": display_path,
            "error_kind": "diagnostics",
            "message": "build produced diagnostics",
            "diagnostic_file": build_json_diagnostic_file(path, source, diagnostics),
        }),
    }
}

pub(crate) fn build_json_preflight_failure(
    request_path: &Path,
    manifest_path: Option<&Path>,
    package_name: Option<&str>,
    selected: Option<bool>,
    error_kind: &str,
    stage: &str,
    message: String,
    selector: Option<String>,
    conflict_path: Option<String>,
    target_count: Option<usize>,
) -> JsonValue {
    json!({
        "manifest_path": manifest_path.map(normalize_path),
        "package_name": package_name,
        "selected": selected,
        "dependency_only": selected.map(|value| !value),
        "kind": JsonValue::Null,
        "path": normalize_path(request_path),
        "error_kind": error_kind,
        "stage": stage,
        "message": message,
        "selector": selector,
        "conflict_path": conflict_path,
        "target_count": target_count,
    })
}

pub(crate) fn build_json_project_error(
    request_path: &Path,
    error: &ql_project::ProjectError,
    stage: &str,
) -> JsonValue {
    if let ql_project::ProjectError::ManifestNotFound { start } = error {
        return build_json_preflight_failure(
            request_path,
            None,
            None,
            None,
            "manifest",
            stage,
            format!(
                "could not find `qlang.toml` starting from `{}`",
                normalize_path(start)
            ),
            None,
            None,
            None,
        );
    }

    if let Some(manifest_path) =
        crate::cli_utils::package_missing_name_manifest_path_from_project_error(error)
    {
        return build_json_preflight_failure(
            request_path,
            Some(manifest_path),
            None,
            None,
            "manifest",
            stage,
            format!(
                "manifest `{}` does not declare `[package].name`",
                normalize_path(manifest_path)
            ),
            None,
            None,
            None,
        );
    }

    if let ql_project::ProjectError::PackageSourceRootNotFound { path } = error {
        return build_json_preflight_failure(
            request_path,
            None,
            None,
            None,
            "manifest",
            stage,
            format!(
                "package source directory `{}` does not exist",
                normalize_path(path)
            ),
            None,
            None,
            None,
        );
    }

    build_json_preflight_failure(
        request_path,
        crate::cli_utils::package_check_manifest_path_from_project_error(error),
        None,
        None,
        "manifest",
        stage,
        error.to_string(),
        None,
        None,
        None,
    )
}

fn build_json_interface_failure(
    request_path: &Path,
    manifest_path: Option<&Path>,
    package_name: Option<&str>,
    error_kind: &str,
    message: String,
    output_path: Option<String>,
    source_root: Option<String>,
    failing_source_count: Option<usize>,
    first_failing_source: Option<String>,
) -> JsonValue {
    let display_path = output_path
        .clone()
        .or_else(|| manifest_path.map(normalize_path))
        .unwrap_or_else(|| normalize_path(request_path));
    json!({
        "manifest_path": manifest_path.map(normalize_path),
        "package_name": package_name,
        "selected": true,
        "dependency_only": false,
        "kind": "interface",
        "path": display_path,
        "error_kind": error_kind,
        "stage": "emit-interface",
        "message": message,
        "output_path": output_path,
        "source_root": source_root,
        "failing_source_count": failing_source_count,
        "first_failing_source": first_failing_source,
    })
}

pub(crate) fn build_json_emit_interface_failure(
    request_path: &Path,
    manifest_path: Option<&Path>,
    package_name: Option<&str>,
    error: &EmitPackageInterfaceError,
) -> JsonValue {
    match error {
        EmitPackageInterfaceError::ManifestNotFound { start } => build_json_interface_failure(
            request_path,
            None,
            package_name,
            "project-context",
            format!(
                "build-side interface emission requires a package manifest; could not find `qlang.toml` starting from `{}`",
                normalize_path(start)
            ),
            None,
            None,
            None,
            None,
        ),
        EmitPackageInterfaceError::ManifestFailure {
            manifest_path,
            message,
        } => build_json_interface_failure(
            request_path,
            Some(manifest_path),
            package_name,
            "manifest",
            message.clone(),
            None,
            None,
            None,
            None,
        ),
        EmitPackageInterfaceError::NoSourceFilesFailure {
            manifest_path,
            source_root,
        } => build_json_interface_failure(
            request_path,
            Some(manifest_path),
            package_name,
            "package-sources",
            format!(
                "no `.ql` files found under `{}`",
                normalize_path(source_root)
            ),
            None,
            Some(normalize_path(source_root)),
            None,
            None,
        ),
        EmitPackageInterfaceError::SourceRootFailure {
            manifest_path,
            source_root,
        } => build_json_interface_failure(
            request_path,
            Some(manifest_path),
            package_name,
            "package-source-root",
            format!(
                "package source directory `{}` does not exist",
                normalize_path(source_root)
            ),
            None,
            Some(normalize_path(source_root)),
            None,
            None,
        ),
        EmitPackageInterfaceError::OutputPathFailure {
            manifest_path: output_manifest_path,
            output_path,
            message,
        } => build_json_interface_failure(
            request_path,
            output_manifest_path.as_deref().or(manifest_path),
            package_name,
            "interface-output",
            message.clone(),
            Some(normalize_path(output_path)),
            None,
            None,
            None,
        ),
        EmitPackageInterfaceError::SourceFailure {
            failure_count,
            first_failing_source,
            ..
        } => build_json_interface_failure(
            request_path,
            manifest_path,
            package_name,
            "package-sources",
            format!("package interface emission found {failure_count} failing source file(s)"),
            None,
            None,
            Some(*failure_count),
            first_failing_source
                .as_ref()
                .map(|path| normalize_path(path)),
        ),
        EmitPackageInterfaceError::Code { message, .. } => build_json_interface_failure(
            request_path,
            manifest_path,
            package_name,
            "interface",
            message
                .clone()
                .unwrap_or_else(|| "build-side interface emission failed".to_owned()),
            None,
            None,
            None,
            None,
        ),
    }
}

pub(crate) fn build_json_dependency_interface_prep_failure(
    _request_path: &Path,
    failure: &ReferenceInterfacePrepError,
) -> JsonValue {
    let manifest_path = failure.first_failure.manifest_path.as_deref().or(Some(
        failure.first_failure.reference_manifest_path.as_path(),
    ));
    let reference_manifest_path = normalize_path(&failure.first_failure.reference_manifest_path);
    let owner_manifest_path = failure
        .first_failure
        .owner_manifest_path
        .as_ref()
        .map(|path| normalize_path(path));
    let first_failing_dependency_manifest = failure
        .first_failure_manifest
        .as_ref()
        .map(|path| normalize_path(path));
    let (error_kind, message, output_path, source_root, failing_source_count, first_failing_source) =
        match &failure.first_failure.failure_kind {
            ReferenceInterfacePrepFailureKind::Project {
                error_kind,
                message,
                source_root,
            } => (
                *error_kind,
                message.clone(),
                None,
                source_root.as_ref().map(|path| normalize_path(path)),
                None,
                None,
            ),
            ReferenceInterfacePrepFailureKind::InterfaceEmit(error) => match error {
                EmitPackageInterfaceError::ManifestNotFound { start } => (
                    "project-context",
                    format!(
                        "could not find `qlang.toml` starting from `{}`",
                        normalize_path(start)
                    ),
                    None,
                    None,
                    None,
                    None,
                ),
                EmitPackageInterfaceError::ManifestFailure { message, .. } => {
                    ("manifest", message.clone(), None, None, None, None)
                }
                EmitPackageInterfaceError::NoSourceFilesFailure { source_root, .. } => (
                    "package-sources",
                    format!(
                        "no `.ql` files found under `{}`",
                        normalize_path(source_root)
                    ),
                    None,
                    Some(normalize_path(source_root)),
                    None,
                    None,
                ),
                EmitPackageInterfaceError::SourceRootFailure { source_root, .. } => (
                    "package-source-root",
                    format!(
                        "package source directory `{}` does not exist",
                        normalize_path(source_root)
                    ),
                    None,
                    Some(normalize_path(source_root)),
                    None,
                    None,
                ),
                EmitPackageInterfaceError::OutputPathFailure {
                    output_path,
                    message,
                    ..
                } => (
                    "interface-output",
                    message.clone(),
                    Some(normalize_path(output_path)),
                    None,
                    None,
                    None,
                ),
                EmitPackageInterfaceError::SourceFailure {
                    failure_count,
                    first_failing_source,
                    ..
                } => (
                    "package-sources",
                    format!(
                        "package interface emission found {failure_count} failing source file(s)"
                    ),
                    None,
                    None,
                    Some(*failure_count),
                    first_failing_source
                        .as_ref()
                        .map(|path| normalize_path(path)),
                ),
                EmitPackageInterfaceError::Code { message, .. } => (
                    "interface",
                    message
                        .clone()
                        .unwrap_or_else(|| "dependency interface preparation failed".to_owned()),
                    None,
                    None,
                    None,
                    None,
                ),
            },
        };
    let display_path = output_path
        .clone()
        .or_else(|| source_root.clone())
        .or_else(|| manifest_path.map(normalize_path))
        .unwrap_or_else(|| reference_manifest_path.clone());
    json!({
        "manifest_path": manifest_path.map(normalize_path),
        "package_name": JsonValue::Null,
        "selected": false,
        "dependency_only": true,
        "kind": "interface",
        "path": display_path,
        "error_kind": error_kind,
        "stage": "dependency-interface-prep",
        "message": message,
        "output_path": output_path,
        "source_root": source_root,
        "failing_source_count": failing_source_count,
        "first_failing_source": first_failing_source,
        "owner_manifest_path": owner_manifest_path,
        "reference_manifest_path": reference_manifest_path,
        "reference": failure.first_failure.reference.clone(),
        "failing_dependency_count": failure.failure_count,
        "first_failing_dependency_manifest": first_failing_dependency_manifest,
    })
}

pub(crate) fn build_json_build_plan_failure(
    request_path: &Path,
    failure: &BuildPlanResolveError,
) -> JsonValue {
    match &failure.failure_kind {
        BuildPlanResolveFailureKind::Dependency { message } => json!({
            "manifest_path": failure.manifest_path.as_ref().map(|path| normalize_path(path)),
            "package_name": JsonValue::Null,
            "selected": JsonValue::Null,
            "dependency_only": JsonValue::Null,
            "kind": JsonValue::Null,
            "path": normalize_path(request_path),
            "error_kind": "dependency",
            "stage": "build-plan",
            "message": message,
            "owner_manifest_path": failure.owner_manifest_path.as_ref().map(|path| normalize_path(path)),
            "dependency_manifest_path": failure.dependency_manifest_path.as_ref().map(|path| normalize_path(path)),
            "cycle_manifests": JsonValue::Null,
        }),
        BuildPlanResolveFailureKind::Cycle { cycle_manifests } => json!({
            "manifest_path": failure.manifest_path.as_ref().map(|path| normalize_path(path)),
            "package_name": JsonValue::Null,
            "selected": JsonValue::Null,
            "dependency_only": JsonValue::Null,
            "kind": JsonValue::Null,
            "path": normalize_path(request_path),
            "error_kind": "cycle",
            "stage": "build-plan",
            "message": "local package build dependencies contain a cycle",
            "owner_manifest_path": JsonValue::Null,
            "dependency_manifest_path": failure.dependency_manifest_path.as_ref().map(|path| normalize_path(path)),
            "cycle_manifests": cycle_manifests,
        }),
    }
}

pub(crate) fn build_json_target_prep_failure(
    member: &WorkspaceBuildTargets,
    target: &BuildTarget,
    selected: bool,
    failure: &PrepareProjectTargetBuildError,
) -> JsonValue {
    let (
        error_kind,
        message,
        dependency_manifest_path,
        dependency_package,
        interface_path,
        symbol,
        first_dependency_package,
        first_dependency_manifest_path,
        conflicting_dependency_package,
        conflicting_dependency_manifest_path,
        io_path,
    ) = match &failure.failure_kind {
        PrepareProjectTargetBuildFailureKind::DependencyManifest {
            dependency_manifest_path,
            error_kind,
            message,
        } => (
            *error_kind,
            message.clone(),
            dependency_manifest_path
                .as_ref()
                .map(|path| json!(normalize_path(path)))
                .unwrap_or(JsonValue::Null),
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
        ),
        PrepareProjectTargetBuildFailureKind::DependencyInterface {
            dependency_manifest_path,
            dependency_package,
            interface_path,
            message,
        } => (
            "dependency-interface",
            message.clone(),
            json!(normalize_path(dependency_manifest_path)),
            json!(dependency_package),
            json!(normalize_path(interface_path)),
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
        ),
        PrepareProjectTargetBuildFailureKind::DependencyExternConflict {
            symbol,
            first_package,
            first_manifest_path,
            conflicting_package,
            conflicting_manifest_path,
        } => (
            "dependency-extern-conflict",
            format!("found conflicting direct dependency extern imports for `{symbol}`"),
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            json!(symbol),
            json!(first_package),
            json!(normalize_path(first_manifest_path)),
            json!(conflicting_package),
            json!(normalize_path(conflicting_manifest_path)),
            JsonValue::Null,
        ),
        PrepareProjectTargetBuildFailureKind::DependencyFunctionConflict {
            symbol,
            first_package,
            first_manifest_path,
            conflicting_package,
            conflicting_manifest_path,
        } => (
            "dependency-function-conflict",
            format!("found conflicting direct dependency public function imports for `{symbol}`"),
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            json!(symbol),
            json!(first_package),
            json!(normalize_path(first_manifest_path)),
            json!(conflicting_package),
            json!(normalize_path(conflicting_manifest_path)),
            JsonValue::Null,
        ),
        PrepareProjectTargetBuildFailureKind::DependencySource {
            dependency_manifest_path,
            dependency_package,
            source_path,
            message,
        } => (
            "dependency-source",
            message.clone(),
            json!(normalize_path(dependency_manifest_path)),
            json!(dependency_package),
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            json!(normalize_path(source_path)),
        ),
        PrepareProjectTargetBuildFailureKind::DependencyFunctionLocalConflict {
            symbol,
            dependency_package,
            dependency_manifest_path,
        } => (
            "dependency-function-local-conflict",
            format!(
                "cannot synthesize direct dependency public function bridge for `{symbol}` because the root source already defines the same top-level name"
            ),
            json!(normalize_path(dependency_manifest_path)),
            json!(dependency_package),
            JsonValue::Null,
            json!(symbol),
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
        ),
        PrepareProjectTargetBuildFailureKind::DependencyFunctionUnsupportedGeneric {
            symbol,
            dependency_package,
            dependency_manifest_path,
        } => (
            "dependency-function-unsupported-generic",
            format!(
                "cannot synthesize direct dependency public function bridge for generic function `{symbol}` yet"
            ),
            json!(normalize_path(dependency_manifest_path)),
            json!(dependency_package),
            JsonValue::Null,
            json!(symbol),
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
        ),
        PrepareProjectTargetBuildFailureKind::DependencyTypeConflict {
            symbol,
            first_package,
            first_manifest_path,
            conflicting_package,
            conflicting_manifest_path,
        } => (
            "dependency-type-conflict",
            format!("found conflicting direct dependency public type imports for `{symbol}`"),
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            json!(symbol),
            json!(first_package),
            json!(normalize_path(first_manifest_path)),
            json!(conflicting_package),
            json!(normalize_path(conflicting_manifest_path)),
            JsonValue::Null,
        ),
        PrepareProjectTargetBuildFailureKind::DependencyTypeLocalConflict {
            symbol,
            dependency_package,
            dependency_manifest_path,
        } => (
            "dependency-type-local-conflict",
            format!(
                "cannot synthesize direct dependency public type bridge for `{symbol}` because the root source already defines the same top-level name"
            ),
            json!(normalize_path(dependency_manifest_path)),
            json!(dependency_package),
            JsonValue::Null,
            json!(symbol),
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
        ),
        PrepareProjectTargetBuildFailureKind::DependencyValueConflict {
            symbol,
            first_package,
            first_manifest_path,
            conflicting_package,
            conflicting_manifest_path,
        } => (
            "dependency-value-conflict",
            format!("found conflicting direct dependency public value imports for `{symbol}`"),
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            json!(symbol),
            json!(first_package),
            json!(normalize_path(first_manifest_path)),
            json!(conflicting_package),
            json!(normalize_path(conflicting_manifest_path)),
            JsonValue::Null,
        ),
        PrepareProjectTargetBuildFailureKind::DependencyValueLocalConflict {
            symbol,
            dependency_package,
            dependency_manifest_path,
        } => (
            "dependency-value-local-conflict",
            format!(
                "cannot synthesize direct dependency public value bridge for `{symbol}` because the root source already defines the same top-level name"
            ),
            json!(normalize_path(dependency_manifest_path)),
            json!(dependency_package),
            JsonValue::Null,
            json!(symbol),
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
        ),
        PrepareProjectTargetBuildFailureKind::SourceRead { path, message } => (
            "io",
            message.clone(),
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            JsonValue::Null,
            json!(normalize_path(path)),
        ),
    };

    json!({
        "manifest_path": normalize_path(&member.member_manifest_path),
        "package_name": member.package_name,
        "selected": selected,
        "dependency_only": !selected,
        "kind": target.kind.as_str(),
        "path": project_target_display_path(&member.member_manifest_path, &target.path),
        "error_kind": error_kind,
        "stage": "target-prep",
        "message": message,
        "dependency_manifest_path": dependency_manifest_path,
        "dependency_package": dependency_package,
        "interface_path": interface_path,
        "symbol": symbol,
        "first_dependency_package": first_dependency_package,
        "first_dependency_manifest_path": first_dependency_manifest_path,
        "conflicting_dependency_package": conflicting_dependency_package,
        "conflicting_dependency_manifest_path": conflicting_dependency_manifest_path,
        "io_path": io_path,
    })
}

pub(crate) fn load_workspace_build_targets_for_build_json_from_request_root(
    request_path: &Path,
    request_root: &Path,
) -> Result<Vec<WorkspaceBuildTargets>, JsonValue> {
    let manifest = load_project_manifest(request_root)
        .map_err(|error| build_json_project_error(request_path, &error, "manifest-load"))?;
    discover_workspace_build_targets(&manifest)
        .map_err(|error| build_json_project_error(request_path, &error, "target-discovery"))
}

pub(crate) fn select_workspace_build_targets_for_build_json(
    path: &Path,
    members: &[WorkspaceBuildTargets],
    selector: &crate::project_targets::ProjectTargetSelector,
    target_label: &str,
) -> Result<Vec<WorkspaceBuildTargets>, JsonValue> {
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
        let normalized_path = normalize_path(path);
        return Err(build_json_preflight_failure(
            path,
            None,
            None,
            None,
            "selector",
            "target-selection",
            format!("target selector matched no {target_label} under `{normalized_path}`"),
            Some(selector.describe()),
            None,
            Some(0),
        ));
    }

    Ok(selected)
}

fn build_json_diagnostic_file(path: &Path, source: &str, diagnostics: &[Diagnostic]) -> JsonValue {
    json!({
        "path": normalize_path(path),
        "diagnostics": diagnostics_json(source, diagnostics),
    })
}
