use std::path::Path;

use serde_json::{Value as JsonValue, json};

use crate::cli_utils::{
    normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error,
};

#[derive(Debug)]
pub(super) struct ProjectLockJsonReport {
    path: String,
    project_manifest_path: String,
    lockfile_path: String,
    check_only: bool,
    status: &'static str,
    lockfile: JsonValue,
    failure: Option<JsonValue>,
}

impl ProjectLockJsonReport {
    pub(super) fn new(
        path: &Path,
        manifest: &ql_project::ProjectManifest,
        lockfile_path: &Path,
        check_only: bool,
        rendered_lockfile: &str,
    ) -> Self {
        Self {
            path: normalize_path(path),
            project_manifest_path: normalize_path(&manifest.manifest_path),
            lockfile_path: normalize_path(lockfile_path),
            check_only,
            status: if check_only { "up-to-date" } else { "wrote" },
            lockfile: serde_json::from_str(rendered_lockfile)
                .expect("project lockfile should serialize as valid json"),
            failure: None,
        }
    }

    pub(super) fn record_failure(
        &mut self,
        kind: &'static str,
        message: String,
        rerun_command: Option<String>,
    ) {
        let mut failure = json!({
            "kind": kind,
            "message": message,
        });
        if let Some(command) = rerun_command {
            failure["rerun_command"] = json!(command);
        }
        self.status = "failed";
        self.failure = Some(failure);
    }

    pub(super) fn into_json(self) -> String {
        let rendered = serde_json::to_string_pretty(&json!({
            "schema": "ql.project.lock.result.v1",
            "path": self.path,
            "project_manifest_path": self.project_manifest_path,
            "lockfile_path": self.lockfile_path,
            "check_only": self.check_only,
            "status": self.status,
            "lockfile": self.lockfile,
            "failure": self.failure,
        }))
        .expect("project lock json report should serialize");
        format!("{rendered}\n")
    }
}

pub(super) fn render_project_lock_manifest_failure_json(
    path: &Path,
    check_only: bool,
    error: &ql_project::ProjectError,
) -> String {
    let manifest_path = project_lock_error_manifest_path(error).map(normalize_path);
    render_project_lock_preflight_failure_json(
        path,
        manifest_path.clone(),
        None,
        check_only,
        "manifest-load",
        project_lock_load_error_message(check_only, error),
        manifest_path,
    )
}

pub(super) fn render_project_lock_render_failure_json(
    path: &Path,
    manifest: &ql_project::ProjectManifest,
    lockfile_path: &Path,
    check_only: bool,
    error: &ql_project::ProjectError,
) -> String {
    let failing_manifest_path = project_lock_error_manifest_path(error)
        .map(normalize_path)
        .unwrap_or_else(|| normalize_path(&manifest.manifest_path));
    render_project_lock_preflight_failure_json(
        path,
        Some(normalize_path(&manifest.manifest_path)),
        Some(normalize_path(lockfile_path)),
        check_only,
        "lockfile-render",
        project_lock_render_error_message(check_only, error),
        Some(failing_manifest_path),
    )
}

fn render_project_lock_preflight_failure_json(
    path: &Path,
    project_manifest_path: Option<String>,
    lockfile_path: Option<String>,
    check_only: bool,
    stage: &'static str,
    message: String,
    failure_manifest_path: Option<String>,
) -> String {
    let rendered = serde_json::to_string_pretty(&json!({
        "schema": "ql.project.lock.result.v1",
        "path": normalize_path(path),
        "project_manifest_path": project_manifest_path,
        "lockfile_path": lockfile_path,
        "check_only": check_only,
        "status": "failed",
        "lockfile": JsonValue::Null,
        "failure": {
            "kind": "preflight",
            "preflight_failure": {
                "stage": stage,
                "message": message,
                "manifest_path": failure_manifest_path,
            },
        },
    }))
    .expect("project lock preflight failure json should serialize");
    format!("{rendered}\n")
}

fn project_lock_load_error_message(check_only: bool, error: &ql_project::ProjectError) -> String {
    let command_label = if check_only {
        "`ql project lock --check`"
    } else {
        "`ql project lock`"
    };
    if let ql_project::ProjectError::ManifestNotFound { start } = error {
        return format!(
            "{command_label} requires a package or workspace manifest; could not find `qlang.toml` starting from `{}`",
            normalize_path(start)
        );
    }
    if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error) {
        return format!(
            "{command_label} manifest `{}` does not declare `[package].name`",
            normalize_path(manifest_path)
        );
    }
    format!("{command_label} {error}")
}

fn project_lock_render_error_message(check_only: bool, error: &ql_project::ProjectError) -> String {
    let command_label = if check_only {
        "`ql project lock --check`"
    } else {
        "`ql project lock`"
    };
    if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error) {
        return format!(
            "{command_label} manifest `{}` does not declare `[package].name`",
            normalize_path(manifest_path)
        );
    }
    if let ql_project::ProjectError::PackageSourceRootNotFound { path } = error {
        return format!(
            "{command_label} package source directory `{}` does not exist",
            normalize_path(path)
        );
    }
    format!("{command_label} {error}")
}

fn project_lock_error_manifest_path(error: &ql_project::ProjectError) -> Option<&Path> {
    package_missing_name_manifest_path_from_project_error(error)
        .or_else(|| package_check_manifest_path_from_project_error(error))
}
