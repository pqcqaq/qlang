use std::path::{Path, PathBuf};

use ql_driver::BuildOptions;
use serde_json::{Value as JsonValue, json};

use crate::build_json_preflight_failure;
use crate::cli_utils::normalize_path;
use crate::test_command::TestCommandOptions;

#[derive(Clone, Debug)]
pub(crate) struct TestTarget {
    pub(crate) display_path: String,
    pub(crate) kind: TestTargetKind,
}

#[derive(Clone, Debug)]
pub(crate) enum TestTargetKind {
    Smoke {
        source_path: PathBuf,
        working_directory: PathBuf,
        build_options: BuildOptions,
        package_manifest_path: Option<PathBuf>,
    },
    Ui {
        source_path: PathBuf,
        diagnostic_path: PathBuf,
        snapshot_path: PathBuf,
    },
}

#[derive(Clone, Debug)]
pub(crate) enum TestFailure {
    Build {
        display_path: String,
    },
    Run {
        display_path: String,
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
    },
    Spawn {
        display_path: String,
        error: String,
    },
    Ui {
        display_path: String,
        detail: String,
    },
}

#[derive(Clone, Debug, Default)]
pub(crate) struct TestExecutionReport {
    pub(crate) passed: usize,
    pub(crate) failed: usize,
    pub(crate) failures: Vec<TestFailure>,
}

impl TestExecutionReport {
    pub(crate) fn status(&self) -> &'static str {
        if self.failures.is_empty() {
            "ok"
        } else {
            "failed"
        }
    }

    pub(crate) fn is_success(&self) -> bool {
        self.failures.is_empty()
    }
}

pub(crate) fn render_test_json_report(
    path: &Path,
    command_options: &TestCommandOptions,
    status: &'static str,
    discovered_total: usize,
    targets: &[TestTarget],
    execution_report: Option<&TestExecutionReport>,
) -> String {
    let rendered = serde_json::to_string_pretty(&json!({
        "schema": "ql.test.v1",
        "path": normalize_path(path),
        "requested_profile": command_options.profile.dir_name(),
        "profile_overridden": command_options.profile_overridden,
        "package_name": command_options.package_name.as_deref(),
        "filter": command_options.filter.as_deref(),
        "list_only": command_options.list_only,
        "status": status,
        "discovered_total": discovered_total,
        "selected_total": targets.len(),
        "targets": targets.iter().map(test_json_target).collect::<Vec<_>>(),
        "passed": execution_report.map_or(0, |report| report.passed),
        "failed": execution_report.map_or(0, |report| report.failed),
        "failures": execution_report
            .map(|report| report.failures.iter().map(test_json_failure).collect::<Vec<_>>())
            .unwrap_or_default(),
    }))
    .expect("test json report should serialize");
    format!("{rendered}\n")
}

pub(crate) fn render_test_json_preflight_failure_report(
    path: &Path,
    command_options: &TestCommandOptions,
    failure: JsonValue,
) -> String {
    let rendered = serde_json::to_string_pretty(&json!({
        "schema": "ql.test.v1",
        "path": normalize_path(path),
        "requested_profile": command_options.profile.dir_name(),
        "profile_overridden": command_options.profile_overridden,
        "package_name": command_options.package_name.as_deref(),
        "filter": command_options.filter.as_deref(),
        "list_only": command_options.list_only,
        "status": "failed",
        "discovered_total": 0,
        "selected_total": 0,
        "targets": [],
        "passed": 0,
        "failed": 0,
        "failures": [],
        "failure": {
            "kind": "preflight",
            "preflight_failure": failure,
        },
    }))
    .expect("test preflight json report should serialize");
    format!("{rendered}\n")
}

pub(crate) fn render_test_json_preflight_message_report(
    path: &Path,
    command_options: &TestCommandOptions,
    error_kind: &str,
    stage: &str,
    message: String,
    selector: Option<String>,
    target_count: Option<usize>,
) -> String {
    render_test_json_preflight_failure_report(
        path,
        command_options,
        build_json_preflight_failure(
            path,
            None,
            None,
            None,
            error_kind,
            stage,
            message,
            selector,
            None,
            target_count,
        ),
    )
}

pub(crate) fn render_test_json_selection_failure_report(
    path: &Path,
    command_options: &TestCommandOptions,
    status: &'static str,
    discovered_total: usize,
    stage: &'static str,
    message: String,
    selector: Option<String>,
) -> String {
    let rendered = serde_json::to_string_pretty(&json!({
        "schema": "ql.test.v1",
        "path": normalize_path(path),
        "requested_profile": command_options.profile.dir_name(),
        "profile_overridden": command_options.profile_overridden,
        "package_name": command_options.package_name.as_deref(),
        "filter": command_options.filter.as_deref(),
        "list_only": command_options.list_only,
        "status": status,
        "discovered_total": discovered_total,
        "selected_total": 0,
        "targets": [],
        "passed": 0,
        "failed": 0,
        "failures": [],
        "failure": {
            "kind": "selection",
            "selection_failure": {
                "stage": stage,
                "message": message,
                "selector": selector,
                "target_count": discovered_total,
            },
        },
    }))
    .expect("test selection failure json report should serialize");
    format!("{rendered}\n")
}

fn test_json_target(target: &TestTarget) -> JsonValue {
    match &target.kind {
        TestTargetKind::Smoke { build_options, .. } => json!({
            "path": target.display_path,
            "kind": "smoke",
            "profile": build_options.profile.dir_name(),
        }),
        TestTargetKind::Ui { .. } => json!({
            "path": target.display_path,
            "kind": "ui",
            "profile": JsonValue::Null,
        }),
    }
}

fn test_json_failure(failure: &TestFailure) -> JsonValue {
    match failure {
        TestFailure::Build { display_path } => json!({
            "path": display_path,
            "kind": "build",
        }),
        TestFailure::Run {
            display_path,
            exit_code,
            stdout,
            stderr,
        } => json!({
            "path": display_path,
            "kind": "run",
            "exit_code": exit_code,
            "stdout": stdout,
            "stderr": stderr,
        }),
        TestFailure::Spawn {
            display_path,
            error,
        } => json!({
            "path": display_path,
            "kind": "spawn",
            "error": error,
        }),
        TestFailure::Ui {
            display_path,
            detail,
        } => json!({
            "path": display_path,
            "kind": "ui",
            "detail": detail,
        }),
    }
}
