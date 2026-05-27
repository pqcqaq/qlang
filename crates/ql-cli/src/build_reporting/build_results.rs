use std::path::Path;

use ql_diagnostics::Diagnostic;
use ql_driver::{BuildArtifact, BuildEmit, BuildError};
use serde_json::{Value as JsonValue, json};

use crate::cli_json_diagnostics::diagnostics_json;
use crate::cli_utils::normalize_path;

use super::failure_envelope::BuildJsonFailureEnvelope;

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
            failure["preserved_artifacts"] = json!(
                preserved_artifacts
                    .iter()
                    .map(|path| normalize_path(path))
                    .collect::<Vec<_>>()
            );
            failure["intermediate_ir"] = json!(
                preserved_artifacts
                    .iter()
                    .find(|path| {
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .is_some_and(|name| name.contains(".codegen.ll"))
                    })
                    .map(|path| normalize_path(path))
            );
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

fn build_json_diagnostic_file(path: &Path, source: &str, diagnostics: &[Diagnostic]) -> JsonValue {
    json!({
        "path": normalize_path(path),
        "diagnostics": diagnostics_json(source, diagnostics),
    })
}
