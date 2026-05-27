use std::path::Path;

use serde_json::{Value as JsonValue, json};

use crate::cli_utils::normalize_path;

#[derive(Clone, Debug)]
pub(super) struct BuildJsonFailureEnvelope {
    manifest_path: JsonValue,
    package_name: JsonValue,
    selected: JsonValue,
    dependency_only: JsonValue,
    kind: JsonValue,
    path: JsonValue,
}

impl BuildJsonFailureEnvelope {
    pub(super) fn target(
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

    pub(super) fn preflight(
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

    pub(super) fn interface(
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

    pub(super) fn build_plan(request_path: &Path, manifest_path: Option<&Path>) -> Self {
        Self {
            manifest_path: json!(manifest_path.map(normalize_path)),
            package_name: JsonValue::Null,
            selected: JsonValue::Null,
            dependency_only: JsonValue::Null,
            kind: JsonValue::Null,
            path: json!(normalize_path(request_path)),
        }
    }

    pub(super) fn base_json(&self, error_kind: &str, message: String) -> JsonValue {
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

    pub(super) fn staged_json(&self, error_kind: &str, stage: &str, message: String) -> JsonValue {
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
