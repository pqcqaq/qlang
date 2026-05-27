use std::path::Path;

use ql_driver::{BuildArtifact, BuildError, BuildOptions};
use ql_project::{BuildTarget, WorkspaceBuildTargets};
use serde_json::{Value as JsonValue, json};

use crate::build_json_report::BuildJsonReport;
use crate::build_plan::PrepareProjectTargetBuildError;
use crate::build_reporting::{
    build_json_failure, build_json_target, build_json_target_prep_failure,
};
use crate::cli_utils::normalize_path;
use crate::project_targets::project_target_display_path;

#[derive(Debug)]
pub(crate) struct RunJsonReport {
    scope: &'static str,
    path: String,
    project_manifest_path: Option<String>,
    requested_profile: &'static str,
    profile_overridden: bool,
    program_args: Vec<String>,
    built_target: Option<JsonValue>,
    execution: Option<JsonValue>,
    failure: Option<JsonValue>,
}

impl RunJsonReport {
    pub(crate) fn new(
        path: &Path,
        project_request_root: Option<&Path>,
        options: &BuildOptions,
        profile_overridden: bool,
        program_args: &[String],
    ) -> Self {
        let build_report = BuildJsonReport::new(
            path,
            project_request_root,
            options,
            profile_overridden,
            false,
        );
        Self {
            scope: build_report.scope(),
            path: build_report.path().to_owned(),
            project_manifest_path: build_report.project_manifest_path().map(str::to_owned),
            requested_profile: build_report.requested_profile(),
            profile_overridden: build_report.profile_overridden(),
            program_args: program_args.to_vec(),
            built_target: None,
            execution: None,
            failure: None,
        }
    }

    pub(crate) fn record_source_target(&mut self, path: &Path, artifact: &BuildArtifact) {
        self.built_target = Some(build_json_target(
            None,
            None,
            "source",
            normalize_path(path),
            artifact,
            true,
        ));
    }

    pub(crate) fn record_project_target(
        &mut self,
        member: &WorkspaceBuildTargets,
        target: &BuildTarget,
        artifact: &BuildArtifact,
    ) {
        self.built_target = Some(build_json_target(
            Some(&member.member_manifest_path),
            Some(member.package_name.as_str()),
            target.kind.as_str(),
            project_target_display_path(&member.member_manifest_path, &target.path),
            artifact,
            true,
        ));
    }

    pub(crate) fn record_source_build_failure(&mut self, path: &Path, error: &BuildError) {
        self.failure = Some(json!({
            "kind": "build",
            "build_failure": build_json_failure(
                None,
                None,
                "source",
                normalize_path(path),
                true,
                error,
            ),
        }));
    }

    pub(crate) fn record_project_build_failure(
        &mut self,
        member: &WorkspaceBuildTargets,
        target: &BuildTarget,
        error: &BuildError,
    ) {
        self.failure = Some(json!({
            "kind": "build",
            "build_failure": build_json_failure(
                Some(&member.member_manifest_path),
                Some(member.package_name.as_str()),
                target.kind.as_str(),
                project_target_display_path(&member.member_manifest_path, &target.path),
                true,
                error,
            ),
        }));
    }

    pub(crate) fn record_project_target_prep_failure(
        &mut self,
        member: &WorkspaceBuildTargets,
        target: &BuildTarget,
        failure: &PrepareProjectTargetBuildError,
    ) {
        self.failure = Some(json!({
            "kind": "build",
            "build_failure": build_json_target_prep_failure(member, target, true, failure),
        }));
    }

    pub(crate) fn record_preflight_failure(&mut self, failure: JsonValue) {
        self.failure = Some(json!({
            "kind": "preflight",
            "preflight_failure": failure,
        }));
    }

    pub(crate) fn record_spawn_failure(&mut self, executable_path: &Path, message: String) {
        self.failure = Some(json!({
            "kind": "spawn",
            "artifact_path": normalize_path(executable_path),
            "message": message,
        }));
    }

    pub(crate) fn record_run_failure(
        &mut self,
        executable_path: &Path,
        message: &str,
        stdout: &str,
        stderr: &str,
    ) {
        self.failure = Some(json!({
            "kind": "run",
            "artifact_path": normalize_path(executable_path),
            "message": message,
            "stdout": stdout,
            "stderr": stderr,
        }));
    }

    pub(crate) fn record_execution(&mut self, exit_code: i32, stdout: &str, stderr: &str) {
        self.execution = Some(json!({
            "exit_code": exit_code,
            "stdout": stdout,
            "stderr": stderr,
        }));
    }

    pub(crate) fn into_json(self) -> String {
        let rendered = serde_json::to_string_pretty(&json!({
            "schema": "ql.run.v1",
            "path": self.path,
            "scope": self.scope,
            "project_manifest_path": self.project_manifest_path,
            "requested_profile": self.requested_profile,
            "profile_overridden": self.profile_overridden,
            "program_args": self.program_args,
            "status": if self.failure.is_some() { "failed" } else { "completed" },
            "built_target": self.built_target,
            "execution": self.execution,
            "failure": self.failure,
        }))
        .expect("run json report should serialize");
        format!("{rendered}\n")
    }
}

#[cfg(test)]
#[path = "run_reporting_tests.rs"]
mod tests;
