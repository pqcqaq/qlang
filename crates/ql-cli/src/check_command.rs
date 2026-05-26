use std::path::Path;

use ql_diagnostics::Diagnostic;
use serde_json::{Value as JsonValue, json};

use crate::cli_json_diagnostics::diagnostics_json;
use crate::cli_utils::normalize_path;

use super::check_path;

pub(crate) fn check_cli_path(args: impl Iterator<Item = String>) -> Result<(), u8> {
    let options = parse_check_args(args)?;
    check_path(
        Path::new(&options.path),
        options.sync_interfaces,
        options.json,
        options.package_name.as_deref(),
    )
}

struct CheckCliOptions {
    path: String,
    sync_interfaces: bool,
    json: bool,
    package_name: Option<String>,
}

fn parse_check_args(args: impl Iterator<Item = String>) -> Result<CheckCliOptions, u8> {
    let remaining = args.collect::<Vec<_>>();
    let mut path = None;
    let mut sync_interfaces = false;
    let mut json = false;
    let mut package_name = None;
    let mut index = 0;

    while index < remaining.len() {
        match remaining[index].as_str() {
            "--sync-interfaces" => {
                sync_interfaces = true;
            }
            "--json" => {
                json = true;
            }
            "--package" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!("error: `ql check --package` expects a package name");
                    return Err(1);
                };
                if package_name.is_some() {
                    eprintln!("error: `ql check` received multiple `--package` selectors");
                    return Err(1);
                }
                package_name = Some(value.to_owned());
            }
            other if other.starts_with('-') => {
                eprintln!("error: unknown `ql check` option `{other}`");
                return Err(1);
            }
            other => {
                if path.is_some() {
                    eprintln!("error: unknown `ql check` argument `{other}`");
                    return Err(1);
                }
                path = Some(other.to_owned());
            }
        }

        index += 1;
    }

    let Some(path) = path else {
        eprintln!("error: `ql check` expects a file or directory path");
        return Err(1);
    };

    Ok(CheckCliOptions {
        path,
        sync_interfaces,
        json,
        package_name,
    })
}

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
