use std::path::Path;

use serde_json::{Value as JsonValue, json};

use crate::cli_utils::normalize_path;

#[derive(Clone, Debug)]
pub(super) struct BuildJsonInterfaceFailureDetails {
    pub(super) error_kind: &'static str,
    pub(super) message: String,
    output_path: Option<String>,
    source_root: Option<String>,
    failing_source_count: Option<usize>,
    first_failing_source: Option<String>,
}

impl BuildJsonInterfaceFailureDetails {
    pub(super) fn new(error_kind: &'static str, message: String) -> Self {
        Self {
            error_kind,
            message,
            output_path: None,
            source_root: None,
            failing_source_count: None,
            first_failing_source: None,
        }
    }

    pub(super) fn output_path(mut self, path: &Path) -> Self {
        self.output_path = Some(normalize_path(path));
        self
    }

    pub(super) fn source_root(mut self, path: &Path) -> Self {
        self.source_root = Some(normalize_path(path));
        self
    }

    pub(super) fn failing_sources(
        mut self,
        count: usize,
        first_failing_source: Option<&Path>,
    ) -> Self {
        self.failing_source_count = Some(count);
        self.first_failing_source = first_failing_source.map(normalize_path);
        self
    }

    pub(super) fn display_path(&self, request_path: &Path, manifest_path: Option<&Path>) -> String {
        self.output_path
            .clone()
            .or_else(|| manifest_path.map(normalize_path))
            .unwrap_or_else(|| normalize_path(request_path))
    }

    pub(super) fn dependency_display_path(
        &self,
        manifest_path: Option<&Path>,
        reference_manifest_path: &str,
    ) -> String {
        self.output_path
            .clone()
            .or_else(|| self.source_root.clone())
            .or_else(|| manifest_path.map(normalize_path))
            .unwrap_or_else(|| reference_manifest_path.to_owned())
    }

    pub(super) fn apply_to(self, json_failure: &mut JsonValue) {
        json_failure["output_path"] = json!(self.output_path);
        json_failure["source_root"] = json!(self.source_root);
        json_failure["failing_source_count"] = json!(self.failing_source_count);
        json_failure["first_failing_source"] = json!(self.first_failing_source);
    }
}

#[derive(Clone, Debug)]
pub(super) struct BuildJsonTargetPrepDetails {
    pub(super) error_kind: &'static str,
    pub(super) message: String,
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
    pub(super) fn new(error_kind: &'static str, message: String) -> Self {
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

    pub(super) fn dependency_manifest_path(mut self, path: &Path) -> Self {
        self.dependency_manifest_path = json!(normalize_path(path));
        self
    }

    pub(super) fn dependency_package(mut self, package: &str) -> Self {
        self.dependency_package = json!(package);
        self
    }

    pub(super) fn interface_path(mut self, path: &Path) -> Self {
        self.interface_path = json!(normalize_path(path));
        self
    }

    pub(super) fn symbol(mut self, symbol: &str) -> Self {
        self.symbol = json!(symbol);
        self
    }

    pub(super) fn io_path(mut self, path: &Path) -> Self {
        self.io_path = json!(normalize_path(path));
        self
    }

    pub(super) fn dependency_conflict(
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

    pub(super) fn apply_to(self, json_failure: &mut JsonValue) {
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
