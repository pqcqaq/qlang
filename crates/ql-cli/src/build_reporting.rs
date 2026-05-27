use std::path::Path;

use ql_diagnostics::Diagnostic;
use ql_driver::{BuildArtifact, BuildEmit, BuildError};
use ql_project::{
    discover_workspace_build_targets, load_project_manifest, BuildTarget, WorkspaceBuildTargets,
};
use serde_json::{json, Value as JsonValue};

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

mod failure_details;
use failure_details::{BuildJsonInterfaceFailureDetails, BuildJsonTargetPrepDetails};

#[derive(Clone, Debug)]
struct BuildJsonFailureEnvelope {
    manifest_path: JsonValue,
    package_name: JsonValue,
    selected: JsonValue,
    dependency_only: JsonValue,
    kind: JsonValue,
    path: JsonValue,
}

impl BuildJsonFailureEnvelope {
    fn target(
        manifest_path: Option<&Path>,
        package_name: Option<&str>,
        kind: &str,
        display_path: String,
        selected: bool,
    ) -> Self {
        Self {
            manifest_path: json!(manifest_path.map(normalize_path)),
            package_name: json!(package_name),
            selected: json!(selected),
            dependency_only: json!(!selected),
            kind: json!(kind),
            path: json!(display_path),
        }
    }

    fn preflight(
        request_path: &Path,
        manifest_path: Option<&Path>,
        package_name: Option<&str>,
        selected: Option<bool>,
    ) -> Self {
        Self {
            manifest_path: json!(manifest_path.map(normalize_path)),
            package_name: json!(package_name),
            selected: json!(selected),
            dependency_only: json!(selected.map(|value| !value)),
            kind: JsonValue::Null,
            path: json!(normalize_path(request_path)),
        }
    }

    fn interface(
        display_path: String,
        manifest_path: Option<&Path>,
        package_name: Option<&str>,
        selected: bool,
    ) -> Self {
        Self {
            manifest_path: json!(manifest_path.map(normalize_path)),
            package_name: json!(package_name),
            selected: json!(selected),
            dependency_only: json!(!selected),
            kind: json!("interface"),
            path: json!(display_path),
        }
    }

    fn build_plan(request_path: &Path, manifest_path: Option<&Path>) -> Self {
        Self {
            manifest_path: json!(manifest_path.map(normalize_path)),
            package_name: JsonValue::Null,
            selected: JsonValue::Null,
            dependency_only: JsonValue::Null,
            kind: JsonValue::Null,
            path: json!(normalize_path(request_path)),
        }
    }

    fn base_json(&self, error_kind: &str, message: String) -> JsonValue {
        json!({
            "manifest_path": self.manifest_path.clone(),
            "package_name": self.package_name.clone(),
            "selected": self.selected.clone(),
            "dependency_only": self.dependency_only.clone(),
            "kind": self.kind.clone(),
            "path": self.path.clone(),
            "error_kind": error_kind,
            "message": message,
        })
    }

    fn staged_json(&self, error_kind: &str, stage: &str, message: String) -> JsonValue {
        json!({
            "manifest_path": self.manifest_path.clone(),
            "package_name": self.package_name.clone(),
            "selected": self.selected.clone(),
            "dependency_only": self.dependency_only.clone(),
            "kind": self.kind.clone(),
            "path": self.path.clone(),
            "error_kind": error_kind,
            "stage": stage,
            "message": message,
        })
    }
}

pub(crate) fn build_json_failure(
    manifest_path: Option<&Path>,
    package_name: Option<&str>,
    kind: &str,
    display_path: String,
    selected: bool,
    error: &BuildError,
) -> JsonValue {
    let target =
        BuildJsonFailureEnvelope::target(manifest_path, package_name, kind, display_path, selected);
    match error {
        BuildError::InvalidInput(message) => target.base_json("invalid-input", message.clone()),
        BuildError::Io { path, error } => {
            let mut failure = target.base_json(
                "io",
                format!("failed to access `{}`: {error}", normalize_path(path)),
            );
            failure["io_path"] = json!(normalize_path(path));
            failure
        }
        BuildError::Toolchain {
            error,
            preserved_artifacts,
        } => {
            let mut failure = target.base_json("toolchain", error.to_string());
            failure["preserved_artifacts"] = json!(preserved_artifacts
                .iter()
                .map(|path| normalize_path(path))
                .collect::<Vec<_>>());
            failure["intermediate_ir"] = json!(preserved_artifacts
                .iter()
                .find(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.contains(".codegen.ll"))
                })
                .map(|path| normalize_path(path)));
            failure
        }
        BuildError::Diagnostics {
            path,
            source,
            diagnostics,
        } => {
            let mut failure =
                target.base_json("diagnostics", "build produced diagnostics".to_owned());
            failure["diagnostic_file"] = build_json_diagnostic_file(path, source, diagnostics);
            failure
        }
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
    let envelope =
        BuildJsonFailureEnvelope::preflight(request_path, manifest_path, package_name, selected);
    let mut failure = envelope.staged_json(error_kind, stage, message);
    failure["selector"] = json!(selector);
    failure["conflict_path"] = json!(conflict_path);
    failure["target_count"] = json!(target_count);
    failure
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
    details: BuildJsonInterfaceFailureDetails,
) -> JsonValue {
    let display_path = details.display_path(request_path, manifest_path);
    let envelope =
        BuildJsonFailureEnvelope::interface(display_path, manifest_path, package_name, true);
    let mut failure = envelope.staged_json(
        details.error_kind,
        "emit-interface",
        details.message.clone(),
    );
    details.apply_to(&mut failure);
    failure
}

pub(crate) fn build_json_emit_interface_failure(
    request_path: &Path,
    manifest_path: Option<&Path>,
    package_name: Option<&str>,
    error: &EmitPackageInterfaceError,
) -> JsonValue {
    let failure_manifest_path = match error {
        EmitPackageInterfaceError::ManifestNotFound { .. } => None,
        EmitPackageInterfaceError::ManifestFailure { manifest_path, .. }
        | EmitPackageInterfaceError::NoSourceFilesFailure { manifest_path, .. }
        | EmitPackageInterfaceError::SourceRootFailure { manifest_path, .. } => {
            Some(manifest_path.as_path())
        }
        EmitPackageInterfaceError::OutputPathFailure {
            manifest_path: output_manifest_path,
            ..
        } => output_manifest_path.as_deref().or(manifest_path),
        EmitPackageInterfaceError::SourceFailure { .. }
        | EmitPackageInterfaceError::Code { .. } => manifest_path,
    };
    build_json_interface_failure(
        request_path,
        failure_manifest_path,
        package_name,
        build_json_interface_failure_details(
            error,
            "build-side interface emission requires a package manifest; ",
            "build-side interface emission failed",
        ),
    )
}

fn build_json_interface_failure_details(
    error: &EmitPackageInterfaceError,
    manifest_not_found_prefix: &str,
    code_default_message: &str,
) -> BuildJsonInterfaceFailureDetails {
    match error {
        EmitPackageInterfaceError::ManifestNotFound { start } => {
            BuildJsonInterfaceFailureDetails::new(
                "project-context",
                format!(
                    "{manifest_not_found_prefix}could not find `qlang.toml` starting from `{}`",
                    normalize_path(start)
                ),
            )
        }
        EmitPackageInterfaceError::ManifestFailure { message, .. } => {
            BuildJsonInterfaceFailureDetails::new("manifest", message.clone())
        }
        EmitPackageInterfaceError::NoSourceFilesFailure { source_root, .. } => {
            BuildJsonInterfaceFailureDetails::new(
                "package-sources",
                format!(
                    "no `.ql` files found under `{}`",
                    normalize_path(source_root)
                ),
            )
            .source_root(source_root)
        }
        EmitPackageInterfaceError::SourceRootFailure { source_root, .. } => {
            BuildJsonInterfaceFailureDetails::new(
                "package-source-root",
                format!(
                    "package source directory `{}` does not exist",
                    normalize_path(source_root)
                ),
            )
            .source_root(source_root)
        }
        EmitPackageInterfaceError::OutputPathFailure {
            output_path,
            message,
            ..
        } => BuildJsonInterfaceFailureDetails::new("interface-output", message.clone())
            .output_path(output_path),
        EmitPackageInterfaceError::SourceFailure {
            failure_count,
            first_failing_source,
            ..
        } => BuildJsonInterfaceFailureDetails::new(
            "package-sources",
            format!("package interface emission found {failure_count} failing source file(s)"),
        )
        .failing_sources(*failure_count, first_failing_source.as_deref()),
        EmitPackageInterfaceError::Code { message, .. } => BuildJsonInterfaceFailureDetails::new(
            "interface",
            message
                .clone()
                .unwrap_or_else(|| code_default_message.to_owned()),
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
    let details = match &failure.first_failure.failure_kind {
        ReferenceInterfacePrepFailureKind::Project {
            error_kind,
            message,
            source_root,
        } => {
            let details = BuildJsonInterfaceFailureDetails::new(*error_kind, message.clone());
            if let Some(source_root) = source_root {
                details.source_root(source_root)
            } else {
                details
            }
        }
        ReferenceInterfacePrepFailureKind::InterfaceEmit(error) => {
            build_json_interface_failure_details(
                error,
                "",
                "dependency interface preparation failed",
            )
        }
    };
    let display_path = details.dependency_display_path(manifest_path, &reference_manifest_path);
    let envelope = BuildJsonFailureEnvelope::interface(display_path, manifest_path, None, false);
    let mut json_failure = envelope.staged_json(
        details.error_kind,
        "dependency-interface-prep",
        details.message.clone(),
    );
    details.apply_to(&mut json_failure);
    json_failure["owner_manifest_path"] = json!(owner_manifest_path);
    json_failure["reference_manifest_path"] = json!(reference_manifest_path);
    json_failure["reference"] = json!(failure.first_failure.reference.clone());
    json_failure["failing_dependency_count"] = json!(failure.failure_count);
    json_failure["first_failing_dependency_manifest"] = json!(first_failing_dependency_manifest);
    json_failure
}

pub(crate) fn build_json_build_plan_failure(
    request_path: &Path,
    failure: &BuildPlanResolveError,
) -> JsonValue {
    match &failure.failure_kind {
        BuildPlanResolveFailureKind::Dependency { message } => {
            let envelope = BuildJsonFailureEnvelope::build_plan(
                request_path,
                failure.manifest_path.as_deref(),
            );
            let mut json_failure =
                envelope.staged_json("dependency", "build-plan", message.clone());
            json_failure["owner_manifest_path"] = json!(failure
                .owner_manifest_path
                .as_ref()
                .map(|path| normalize_path(path)));
            json_failure["dependency_manifest_path"] = json!(failure
                .dependency_manifest_path
                .as_ref()
                .map(|path| normalize_path(path)));
            json_failure["cycle_manifests"] = JsonValue::Null;
            json_failure
        }
        BuildPlanResolveFailureKind::Cycle { cycle_manifests } => {
            let envelope = BuildJsonFailureEnvelope::build_plan(
                request_path,
                failure.manifest_path.as_deref(),
            );
            let mut json_failure = envelope.staged_json(
                "cycle",
                "build-plan",
                "local package build dependencies contain a cycle".to_owned(),
            );
            json_failure["owner_manifest_path"] = JsonValue::Null;
            json_failure["dependency_manifest_path"] = json!(failure
                .dependency_manifest_path
                .as_ref()
                .map(|path| normalize_path(path)));
            json_failure["cycle_manifests"] = json!(cycle_manifests);
            json_failure
        }
    }
}

pub(crate) fn build_json_target_prep_failure(
    member: &WorkspaceBuildTargets,
    target: &BuildTarget,
    selected: bool,
    failure: &PrepareProjectTargetBuildError,
) -> JsonValue {
    let details = match &failure.failure_kind {
        PrepareProjectTargetBuildFailureKind::DependencyManifest {
            dependency_manifest_path,
            error_kind,
            message,
        } => {
            let details = BuildJsonTargetPrepDetails::new(*error_kind, message.clone());
            if let Some(path) = dependency_manifest_path {
                details.dependency_manifest_path(path)
            } else {
                details
            }
        }
        PrepareProjectTargetBuildFailureKind::DependencyInterface {
            dependency_manifest_path,
            dependency_package,
            interface_path,
            message,
        } => BuildJsonTargetPrepDetails::new("dependency-interface", message.clone())
            .dependency_manifest_path(dependency_manifest_path)
            .dependency_package(dependency_package)
            .interface_path(interface_path),
        PrepareProjectTargetBuildFailureKind::DependencyExternConflict {
            symbol,
            first_package,
            first_manifest_path,
            conflicting_package,
            conflicting_manifest_path,
        } => BuildJsonTargetPrepDetails::new(
            "dependency-extern-conflict",
            format!("found conflicting direct dependency extern imports for `{symbol}`"),
        )
        .dependency_conflict(
            symbol,
            first_package,
            first_manifest_path,
            conflicting_package,
            conflicting_manifest_path,
        ),
        PrepareProjectTargetBuildFailureKind::DependencyFunctionConflict {
            symbol,
            first_package,
            first_manifest_path,
            conflicting_package,
            conflicting_manifest_path,
        } => BuildJsonTargetPrepDetails::new(
            "dependency-function-conflict",
            format!("found conflicting direct dependency public function imports for `{symbol}`"),
        )
        .dependency_conflict(
            symbol,
            first_package,
            first_manifest_path,
            conflicting_package,
            conflicting_manifest_path,
        ),
        PrepareProjectTargetBuildFailureKind::DependencySource {
            dependency_manifest_path,
            dependency_package,
            source_path,
            message,
        } => BuildJsonTargetPrepDetails::new("dependency-source", message.clone())
            .dependency_manifest_path(dependency_manifest_path)
            .dependency_package(dependency_package)
            .io_path(source_path),
        PrepareProjectTargetBuildFailureKind::DependencyFunctionLocalConflict {
            symbol,
            dependency_package,
            dependency_manifest_path,
        } => BuildJsonTargetPrepDetails::new(
            "dependency-function-local-conflict",
            format!(
                "cannot synthesize direct dependency public function bridge for `{symbol}` because the root source already defines the same top-level name"
            ),
        )
        .dependency_manifest_path(dependency_manifest_path)
        .dependency_package(dependency_package)
        .symbol(symbol),
        PrepareProjectTargetBuildFailureKind::DependencyFunctionUnsupportedGeneric {
            symbol,
            dependency_package,
            dependency_manifest_path,
        } => BuildJsonTargetPrepDetails::new(
            "dependency-function-unsupported-generic",
            format!(
                "cannot synthesize direct dependency public function bridge for generic function `{symbol}` yet"
            ),
        )
        .dependency_manifest_path(dependency_manifest_path)
        .dependency_package(dependency_package)
        .symbol(symbol),
        PrepareProjectTargetBuildFailureKind::DependencyTypeConflict {
            symbol,
            first_package,
            first_manifest_path,
            conflicting_package,
            conflicting_manifest_path,
        } => BuildJsonTargetPrepDetails::new(
            "dependency-type-conflict",
            format!("found conflicting direct dependency public type imports for `{symbol}`"),
        )
        .dependency_conflict(
            symbol,
            first_package,
            first_manifest_path,
            conflicting_package,
            conflicting_manifest_path,
        ),
        PrepareProjectTargetBuildFailureKind::DependencyTypeLocalConflict {
            symbol,
            dependency_package,
            dependency_manifest_path,
        } => BuildJsonTargetPrepDetails::new(
            "dependency-type-local-conflict",
            format!(
                "cannot synthesize direct dependency public type bridge for `{symbol}` because the root source already defines the same top-level name"
            ),
        )
        .dependency_manifest_path(dependency_manifest_path)
        .dependency_package(dependency_package)
        .symbol(symbol),
        PrepareProjectTargetBuildFailureKind::DependencyValueConflict {
            symbol,
            first_package,
            first_manifest_path,
            conflicting_package,
            conflicting_manifest_path,
        } => BuildJsonTargetPrepDetails::new(
            "dependency-value-conflict",
            format!("found conflicting direct dependency public value imports for `{symbol}`"),
        )
        .dependency_conflict(
            symbol,
            first_package,
            first_manifest_path,
            conflicting_package,
            conflicting_manifest_path,
        ),
        PrepareProjectTargetBuildFailureKind::DependencyValueLocalConflict {
            symbol,
            dependency_package,
            dependency_manifest_path,
        } => BuildJsonTargetPrepDetails::new(
            "dependency-value-local-conflict",
            format!(
                "cannot synthesize direct dependency public value bridge for `{symbol}` because the root source already defines the same top-level name"
            ),
        )
        .dependency_manifest_path(dependency_manifest_path)
        .dependency_package(dependency_package)
        .symbol(symbol),
        PrepareProjectTargetBuildFailureKind::SourceRead { path, message } => {
            BuildJsonTargetPrepDetails::new("io", message.clone()).io_path(path)
        }
    };

    let envelope = BuildJsonFailureEnvelope::target(
        Some(&member.member_manifest_path),
        Some(member.package_name.as_str()),
        target.kind.as_str(),
        project_target_display_path(&member.member_manifest_path, &target.path),
        selected,
    );
    let mut json_failure =
        envelope.staged_json(details.error_kind, "target-prep", details.message.clone());
    details.apply_to(&mut json_failure);
    json_failure
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

#[cfg(test)]
#[path = "build_reporting_tests.rs"]
mod tests;
