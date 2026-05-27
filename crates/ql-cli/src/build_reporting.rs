use std::path::Path;

use ql_project::{BuildTarget, WorkspaceBuildTargets};
use serde_json::{json, Value as JsonValue};

use crate::build_plan::{
    BuildPlanResolveError, BuildPlanResolveFailureKind, PrepareProjectTargetBuildError,
    PrepareProjectTargetBuildFailureKind,
};
use crate::cli_utils::normalize_path;
use crate::project_interfaces::{
    EmitPackageInterfaceError, ReferenceInterfacePrepError, ReferenceInterfacePrepFailureKind,
};
use crate::project_targets::project_target_display_path;

mod build_results;
mod failure_details;
mod failure_envelope;
mod workspace_targets;
pub(crate) use build_results::{build_emit_cli_value, build_json_failure, build_json_target};
use failure_details::{BuildJsonInterfaceFailureDetails, BuildJsonTargetPrepDetails};
use failure_envelope::BuildJsonFailureEnvelope;
pub(crate) use workspace_targets::{
    load_workspace_build_targets_for_build_json_from_request_root,
    select_workspace_build_targets_for_build_json,
};

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

fn build_json_diagnostic_file(path: &Path, source: &str, diagnostics: &[Diagnostic]) -> JsonValue {
    json!({
        "path": normalize_path(path),
        "diagnostics": diagnostics_json(source, diagnostics),
    })
}

#[cfg(test)]
#[path = "build_reporting_tests.rs"]
mod tests;
