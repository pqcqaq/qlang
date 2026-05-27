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

#[derive(Clone, Debug)]
struct BuildJsonTargetPrepDetails {
    error_kind: &'static str,
    message: String,
    dependency_manifest_path: JsonValue,
    dependency_package: JsonValue,
    interface_path: JsonValue,
    symbol: JsonValue,
    first_dependency_package: JsonValue,
    first_dependency_manifest_path: JsonValue,
    conflicting_dependency_package: JsonValue,
    conflicting_dependency_manifest_path: JsonValue,
    io_path: JsonValue,
}

impl BuildJsonTargetPrepDetails {
    fn new(error_kind: &'static str, message: String) -> Self {
        Self {
            error_kind,
            message,
            dependency_manifest_path: JsonValue::Null,
            dependency_package: JsonValue::Null,
            interface_path: JsonValue::Null,
            symbol: JsonValue::Null,
            first_dependency_package: JsonValue::Null,
            first_dependency_manifest_path: JsonValue::Null,
            conflicting_dependency_package: JsonValue::Null,
            conflicting_dependency_manifest_path: JsonValue::Null,
            io_path: JsonValue::Null,
        }
    }

    fn dependency_manifest_path(mut self, path: &Path) -> Self {
        self.dependency_manifest_path = json!(normalize_path(path));
        self
    }

    fn dependency_package(mut self, package: &str) -> Self {
        self.dependency_package = json!(package);
        self
    }

    fn interface_path(mut self, path: &Path) -> Self {
        self.interface_path = json!(normalize_path(path));
        self
    }

    fn symbol(mut self, symbol: &str) -> Self {
        self.symbol = json!(symbol);
        self
    }

    fn io_path(mut self, path: &Path) -> Self {
        self.io_path = json!(normalize_path(path));
        self
    }

    fn dependency_conflict(
        mut self,
        symbol: &str,
        first_package: &str,
        first_manifest_path: &Path,
        conflicting_package: &str,
        conflicting_manifest_path: &Path,
    ) -> Self {
        self.symbol = json!(symbol);
        self.first_dependency_package = json!(first_package);
        self.first_dependency_manifest_path = json!(normalize_path(first_manifest_path));
        self.conflicting_dependency_package = json!(conflicting_package);
        self.conflicting_dependency_manifest_path =
            json!(normalize_path(conflicting_manifest_path));
        self
    }

    fn apply_to(self, json_failure: &mut JsonValue) {
        json_failure["dependency_manifest_path"] = self.dependency_manifest_path;
        json_failure["dependency_package"] = self.dependency_package;
        json_failure["interface_path"] = self.interface_path;
        json_failure["symbol"] = self.symbol;
        json_failure["first_dependency_package"] = self.first_dependency_package;
        json_failure["first_dependency_manifest_path"] = self.first_dependency_manifest_path;
        json_failure["conflicting_dependency_package"] = self.conflicting_dependency_package;
        json_failure["conflicting_dependency_manifest_path"] =
            self.conflicting_dependency_manifest_path;
        json_failure["io_path"] = self.io_path;
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
    let envelope =
        BuildJsonFailureEnvelope::interface(display_path, manifest_path, package_name, true);
    let mut failure = envelope.staged_json(error_kind, "emit-interface", message);
    failure["output_path"] = json!(output_path);
    failure["source_root"] = json!(source_root);
    failure["failing_source_count"] = json!(failing_source_count);
    failure["first_failing_source"] = json!(first_failing_source);
    failure
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
    let envelope = BuildJsonFailureEnvelope::interface(display_path, manifest_path, None, false);
    let mut json_failure = envelope.staged_json(error_kind, "dependency-interface-prep", message);
    json_failure["output_path"] = json!(output_path);
    json_failure["source_root"] = json!(source_root);
    json_failure["failing_source_count"] = json!(failing_source_count);
    json_failure["first_failing_source"] = json!(first_failing_source);
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
mod tests {
    use std::path::PathBuf;

    use ql_driver::ToolchainError;
    use ql_project::{BuildTargetKind, ManifestBuildProfile};

    use crate::project_interfaces::ReferenceInterfacePrepFailure;

    use super::*;

    #[test]
    fn build_json_failure_preserves_toolchain_target_context_and_artifacts() {
        let failure = build_json_failure(
            Some(Path::new("workspace/app/qlang.toml")),
            Some("app"),
            "bin",
            "src/main.ql".to_owned(),
            false,
            &BuildError::Toolchain {
                error: ToolchainError::InvocationFailed {
                    program: "clang".to_owned(),
                    status: Some(7),
                    stderr: "mock link failure".to_owned(),
                },
                preserved_artifacts: vec![
                    PathBuf::from("target/ql/debug/main.123.codegen.ll"),
                    PathBuf::from(if cfg!(windows) {
                        "target/ql/debug/main.123.codegen.obj"
                    } else {
                        "target/ql/debug/main.123.codegen.o"
                    }),
                ],
            },
        );

        assert_eq!(failure["manifest_path"], "workspace/app/qlang.toml");
        assert_eq!(failure["package_name"], "app");
        assert_eq!(failure["selected"], false);
        assert_eq!(failure["dependency_only"], true);
        assert_eq!(failure["kind"], "bin");
        assert_eq!(failure["path"], "src/main.ql");
        assert_eq!(failure["error_kind"], "toolchain");
        assert_eq!(
            failure["message"],
            "toolchain `clang` failed with exit code 7: mock link failure"
        );
        assert_eq!(
            failure["intermediate_ir"],
            "target/ql/debug/main.123.codegen.ll"
        );
        assert_eq!(
            failure["preserved_artifacts"]
                .as_array()
                .expect("preserved artifacts should be an array")
                .len(),
            2
        );
    }

    #[test]
    fn build_json_preflight_failure_preserves_nullable_target_context() {
        let failure = build_json_preflight_failure(
            Path::new("workspace"),
            Some(Path::new("workspace/qlang.toml")),
            Some("app"),
            Some(true),
            "selector",
            "target-selection",
            "target selector matched no build targets".to_owned(),
            Some("target `missing.ql`".to_owned()),
            Some("workspace/src/missing.ql".to_owned()),
            Some(0),
        );

        assert_eq!(failure["manifest_path"], "workspace/qlang.toml");
        assert_eq!(failure["package_name"], "app");
        assert_eq!(failure["selected"], true);
        assert_eq!(failure["dependency_only"], false);
        assert!(failure["kind"].is_null());
        assert_eq!(failure["path"], "workspace");
        assert_eq!(failure["error_kind"], "selector");
        assert_eq!(failure["stage"], "target-selection");
        assert_eq!(failure["selector"], "target `missing.ql`");
        assert_eq!(failure["conflict_path"], "workspace/src/missing.ql");
        assert_eq!(failure["target_count"], 0);
    }

    #[test]
    fn build_json_emit_interface_failure_preserves_interface_target_context() {
        let failure = build_json_emit_interface_failure(
            Path::new("workspace/app"),
            Some(Path::new("workspace/app/qlang.toml")),
            Some("app"),
            &EmitPackageInterfaceError::OutputPathFailure {
                manifest_path: Some(PathBuf::from("workspace/app/qlang.toml")),
                output_path: PathBuf::from("workspace/app/app.qi"),
                message: "failed to write interface".to_owned(),
            },
        );

        assert_eq!(failure["manifest_path"], "workspace/app/qlang.toml");
        assert_eq!(failure["package_name"], "app");
        assert_eq!(failure["selected"], true);
        assert_eq!(failure["dependency_only"], false);
        assert_eq!(failure["kind"], "interface");
        assert_eq!(failure["path"], "workspace/app/app.qi");
        assert_eq!(failure["error_kind"], "interface-output");
        assert_eq!(failure["stage"], "emit-interface");
        assert_eq!(failure["message"], "failed to write interface");
        assert_eq!(failure["output_path"], "workspace/app/app.qi");
        assert!(failure["source_root"].is_null());
        assert!(failure["failing_source_count"].is_null());
        assert!(failure["first_failing_source"].is_null());
    }

    #[test]
    fn build_json_dependency_interface_prep_failure_preserves_dependency_context() {
        let failure = build_json_dependency_interface_prep_failure(
            Path::new("workspace/app"),
            &ReferenceInterfacePrepError {
                failure_count: 2,
                first_failure_manifest: Some(PathBuf::from("workspace/lib/qlang.toml")),
                first_failure: ReferenceInterfacePrepFailure {
                    owner_manifest_path: Some(PathBuf::from("workspace/app/qlang.toml")),
                    reference: Some("lib".to_owned()),
                    reference_manifest_path: PathBuf::from("workspace/lib/qlang.toml"),
                    manifest_path: Some(PathBuf::from("workspace/lib/qlang.toml")),
                    failure_kind: ReferenceInterfacePrepFailureKind::InterfaceEmit(
                        EmitPackageInterfaceError::NoSourceFilesFailure {
                            manifest_path: PathBuf::from("workspace/lib/qlang.toml"),
                            source_root: PathBuf::from("workspace/lib/src"),
                        },
                    ),
                },
            },
        );

        assert_eq!(failure["manifest_path"], "workspace/lib/qlang.toml");
        assert!(failure["package_name"].is_null());
        assert_eq!(failure["selected"], false);
        assert_eq!(failure["dependency_only"], true);
        assert_eq!(failure["kind"], "interface");
        assert_eq!(failure["path"], "workspace/lib/src");
        assert_eq!(failure["error_kind"], "package-sources");
        assert_eq!(failure["stage"], "dependency-interface-prep");
        assert_eq!(failure["source_root"], "workspace/lib/src");
        assert!(failure["output_path"].is_null());
        assert!(failure["failing_source_count"].is_null());
        assert_eq!(failure["owner_manifest_path"], "workspace/app/qlang.toml");
        assert_eq!(
            failure["reference_manifest_path"],
            "workspace/lib/qlang.toml"
        );
        assert_eq!(failure["reference"], "lib");
        assert_eq!(failure["failing_dependency_count"], 2);
        assert_eq!(
            failure["first_failing_dependency_manifest"],
            "workspace/lib/qlang.toml"
        );
    }

    #[test]
    fn build_json_build_plan_failure_preserves_cycle_context() {
        let failure = build_json_build_plan_failure(
            Path::new("workspace/app"),
            &BuildPlanResolveError {
                manifest_path: Some(PathBuf::from("workspace/app/qlang.toml")),
                owner_manifest_path: None,
                dependency_manifest_path: Some(PathBuf::from("workspace/app/qlang.toml")),
                failure_kind: BuildPlanResolveFailureKind::Cycle {
                    cycle_manifests: vec![
                        "workspace/app/qlang.toml".to_owned(),
                        "workspace/lib/qlang.toml".to_owned(),
                        "workspace/app/qlang.toml".to_owned(),
                    ],
                },
            },
        );

        assert_eq!(failure["manifest_path"], "workspace/app/qlang.toml");
        assert!(failure["package_name"].is_null());
        assert!(failure["selected"].is_null());
        assert!(failure["dependency_only"].is_null());
        assert!(failure["kind"].is_null());
        assert_eq!(failure["path"], "workspace/app");
        assert_eq!(failure["error_kind"], "cycle");
        assert_eq!(failure["stage"], "build-plan");
        assert!(failure["owner_manifest_path"].is_null());
        assert_eq!(
            failure["dependency_manifest_path"],
            "workspace/app/qlang.toml"
        );
        assert_eq!(
            failure["cycle_manifests"],
            json!([
                "workspace/app/qlang.toml",
                "workspace/lib/qlang.toml",
                "workspace/app/qlang.toml"
            ])
        );
    }

    #[test]
    fn build_json_target_prep_failure_preserves_target_context_and_detail_fields() {
        let member = WorkspaceBuildTargets {
            member_manifest_path: PathBuf::from("workspace/app/qlang.toml"),
            package_name: "app".to_owned(),
            default_profile: Some(ManifestBuildProfile::Debug),
            targets: Vec::new(),
        };
        let target = BuildTarget {
            kind: BuildTargetKind::Binary,
            path: PathBuf::from("workspace/app/src/main.ql"),
        };
        let failure = build_json_target_prep_failure(
            &member,
            &target,
            false,
            &PrepareProjectTargetBuildError {
                failure_kind: PrepareProjectTargetBuildFailureKind::DependencyInterface {
                    dependency_manifest_path: PathBuf::from("workspace/lib/qlang.toml"),
                    dependency_package: "lib".to_owned(),
                    interface_path: PathBuf::from("workspace/lib/lib.qi"),
                    message: "missing dependency interface".to_owned(),
                },
            },
        );

        assert_eq!(failure["manifest_path"], "workspace/app/qlang.toml");
        assert_eq!(failure["package_name"], "app");
        assert_eq!(failure["selected"], false);
        assert_eq!(failure["dependency_only"], true);
        assert_eq!(failure["kind"], "bin");
        assert_eq!(failure["path"], "src/main.ql");
        assert_eq!(failure["error_kind"], "dependency-interface");
        assert_eq!(failure["stage"], "target-prep");
        assert_eq!(failure["message"], "missing dependency interface");
        assert_eq!(
            failure["dependency_manifest_path"],
            "workspace/lib/qlang.toml"
        );
        assert_eq!(failure["dependency_package"], "lib");
        assert_eq!(failure["interface_path"], "workspace/lib/lib.qi");
        assert!(failure["symbol"].is_null());
        assert!(failure["io_path"].is_null());
    }

    #[test]
    fn build_json_target_prep_failure_preserves_conflict_detail_fields() {
        let member = WorkspaceBuildTargets {
            member_manifest_path: PathBuf::from("workspace/app/qlang.toml"),
            package_name: "app".to_owned(),
            default_profile: Some(ManifestBuildProfile::Debug),
            targets: Vec::new(),
        };
        let target = BuildTarget {
            kind: BuildTargetKind::Library,
            path: PathBuf::from("workspace/app/src/lib.ql"),
        };
        let failure = build_json_target_prep_failure(
            &member,
            &target,
            true,
            &PrepareProjectTargetBuildError {
                failure_kind: PrepareProjectTargetBuildFailureKind::DependencyTypeConflict {
                    symbol: "SharedType".to_owned(),
                    first_package: "dep_a".to_owned(),
                    first_manifest_path: PathBuf::from("workspace/dep_a/qlang.toml"),
                    conflicting_package: "dep_b".to_owned(),
                    conflicting_manifest_path: PathBuf::from("workspace/dep_b/qlang.toml"),
                },
            },
        );

        assert_eq!(failure["selected"], true);
        assert_eq!(failure["dependency_only"], false);
        assert_eq!(failure["kind"], "lib");
        assert_eq!(failure["path"], "src/lib.ql");
        assert_eq!(failure["error_kind"], "dependency-type-conflict");
        assert_eq!(failure["symbol"], "SharedType");
        assert_eq!(failure["first_dependency_package"], "dep_a");
        assert_eq!(
            failure["first_dependency_manifest_path"],
            "workspace/dep_a/qlang.toml"
        );
        assert_eq!(failure["conflicting_dependency_package"], "dep_b");
        assert_eq!(
            failure["conflicting_dependency_manifest_path"],
            "workspace/dep_b/qlang.toml"
        );
        assert!(failure["dependency_manifest_path"].is_null());
        assert!(failure["io_path"].is_null());
    }

    #[test]
    fn build_json_target_prep_failure_preserves_source_read_io_path() {
        let member = WorkspaceBuildTargets {
            member_manifest_path: PathBuf::from("workspace/app/qlang.toml"),
            package_name: "app".to_owned(),
            default_profile: Some(ManifestBuildProfile::Debug),
            targets: Vec::new(),
        };
        let target = BuildTarget {
            kind: BuildTargetKind::Source,
            path: PathBuf::from("workspace/app/src/main.ql"),
        };
        let failure = build_json_target_prep_failure(
            &member,
            &target,
            true,
            &PrepareProjectTargetBuildError {
                failure_kind: PrepareProjectTargetBuildFailureKind::SourceRead {
                    path: PathBuf::from("workspace/app/src/main.ql"),
                    message: "failed to read source".to_owned(),
                },
            },
        );

        assert_eq!(failure["kind"], "source");
        assert_eq!(failure["error_kind"], "io");
        assert_eq!(failure["message"], "failed to read source");
        assert_eq!(failure["io_path"], "workspace/app/src/main.ql");
        assert!(failure["dependency_manifest_path"].is_null());
        assert!(failure["dependency_package"].is_null());
        assert!(failure["symbol"].is_null());
    }
}
