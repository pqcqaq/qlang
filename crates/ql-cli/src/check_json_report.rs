use std::path::Path;

use ql_diagnostics::Diagnostic;
use serde_json::{Value as JsonValue, json};

use crate::cli_json_diagnostics::diagnostics_json;
use crate::cli_utils::normalize_path;

#[derive(Debug)]
pub(crate) struct CheckJsonReport {
    scope: &'static str,
    sync_interfaces: bool,
    project_manifest_path: Option<String>,
    checked_files: Vec<String>,
    loaded_interfaces: Vec<String>,
    written_interfaces: Vec<String>,
    diagnostic_files: Vec<JsonValue>,
    failing_manifests: Vec<String>,
}

impl CheckJsonReport {
    pub(crate) fn new(
        scope: &'static str,
        sync_interfaces: bool,
        project_manifest_path: Option<&Path>,
    ) -> Self {
        Self {
            scope,
            sync_interfaces,
            project_manifest_path: project_manifest_path.map(normalize_path),
            checked_files: Vec::new(),
            loaded_interfaces: Vec::new(),
            written_interfaces: Vec::new(),
            diagnostic_files: Vec::new(),
            failing_manifests: Vec::new(),
        }
    }

    pub(crate) fn record_checked_file(&mut self, path: &Path) {
        self.checked_files.push(normalize_path(path));
    }

    pub(crate) fn record_loaded_interface(&mut self, path: &Path) {
        self.loaded_interfaces.push(normalize_path(path));
    }

    pub(crate) fn record_written_interface(&mut self, path: &Path) {
        self.written_interfaces.push(normalize_path(path));
    }

    pub(crate) fn record_source_diagnostics(
        &mut self,
        path: &Path,
        source: &str,
        diagnostics: &[Diagnostic],
        owner_manifest_path: Option<&Path>,
    ) {
        if let Some(owner_manifest_path) = owner_manifest_path {
            let manifest_path = normalize_path(owner_manifest_path);
            if !self
                .failing_manifests
                .iter()
                .any(|existing| existing == &manifest_path)
            {
                self.failing_manifests.push(manifest_path);
            }
        }
        self.diagnostic_files.push(check_json_diagnostic_file(
            path,
            source,
            diagnostics,
            owner_manifest_path,
        ));
    }

    pub(crate) fn into_json(self) -> String {
        let status = if self.diagnostic_files.is_empty() {
            "ok"
        } else {
            "diagnostics"
        };
        let rendered = serde_json::to_string_pretty(&json!({
            "schema": "ql.check.v1",
            "scope": self.scope,
            "sync_interfaces": self.sync_interfaces,
            "project_manifest_path": self.project_manifest_path,
            "status": status,
            "checked_files": self.checked_files,
            "loaded_interfaces": self.loaded_interfaces,
            "written_interfaces": self.written_interfaces,
            "diagnostic_files": self.diagnostic_files,
            "failing_manifests": self.failing_manifests,
        }))
        .expect("check json report should serialize");
        format!("{rendered}\n")
    }
}

fn check_json_diagnostic_file(
    path: &Path,
    source: &str,
    diagnostics: &[Diagnostic],
    owner_manifest_path: Option<&Path>,
) -> JsonValue {
    json!({
        "path": normalize_path(path),
        "owner_manifest_path": owner_manifest_path.map(normalize_path),
        "diagnostics": diagnostics_json(source, diagnostics),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_json_report_preserves_success_contract() {
        let mut report = CheckJsonReport::new("files", true, None);
        report.record_checked_file(Path::new("src/main.ql"));
        report.record_loaded_interface(Path::new("deps/math/target/math.qlmi"));
        report.record_written_interface(Path::new("target/app.qlmi"));

        let rendered: JsonValue =
            serde_json::from_str(&report.into_json()).expect("report should parse as json");

        assert_eq!(rendered["schema"], "ql.check.v1");
        assert_eq!(rendered["scope"], "files");
        assert_eq!(rendered["sync_interfaces"], true);
        assert_eq!(rendered["project_manifest_path"], JsonValue::Null);
        assert_eq!(rendered["status"], "ok");
        assert_eq!(rendered["checked_files"][0], "src/main.ql");
        assert_eq!(
            rendered["loaded_interfaces"][0],
            "deps/math/target/math.qlmi"
        );
        assert_eq!(rendered["written_interfaces"][0], "target/app.qlmi");
        assert_eq!(
            rendered["diagnostic_files"]
                .as_array()
                .expect("diagnostics should be an array")
                .len(),
            0
        );
        assert_eq!(
            rendered["failing_manifests"]
                .as_array()
                .expect("failing manifests should be an array")
                .len(),
            0
        );
    }
}
