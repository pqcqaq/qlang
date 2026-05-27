use std::path::Path;

use ql_driver::{BuildArtifact, BuildError, BuildOptions};
use ql_project::{BuildTarget, WorkspaceBuildTargets, load_project_manifest};
use serde_json::{Value as JsonValue, json};

use crate::build_reporting::{build_emit_cli_value, build_json_failure, build_json_target};
use crate::cli_utils::normalize_path;
use crate::project_interfaces::EmitPackageInterfaceResult;
use crate::project_targets::{
    ProjectCommandScope, project_target_display_path, resolve_project_command_scope,
};

pub(crate) fn emit_build_json_failure(
    json_report: &mut Option<BuildJsonReport>,
    failure: JsonValue,
) -> Result<(), u8> {
    let mut report = json_report
        .take()
        .expect("json report should exist for `ql build --json` failure reporting");
    report.record_preflight_failure(failure);
    print!("{}", report.into_json());
    Err(1)
}

#[derive(Debug)]
pub(crate) struct BuildJsonReport {
    scope: &'static str,
    path: String,
    project_manifest_path: Option<String>,
    requested_emit: &'static str,
    requested_profile: &'static str,
    profile_overridden: bool,
    emit_interface: bool,
    built_targets: Vec<JsonValue>,
    interfaces: Vec<JsonValue>,
    failure: Option<JsonValue>,
}

impl BuildJsonReport {
    pub(crate) fn new(
        path: &Path,
        project_request_root: Option<&Path>,
        options: &BuildOptions,
        profile_overridden: bool,
        emit_interface: bool,
    ) -> Self {
        let command_scope = resolve_project_command_scope(path);
        let (project_scope, project_manifest_path) =
            if let Some(request_root) = project_request_root {
                (
                    true,
                    load_project_manifest(request_root)
                        .ok()
                        .map(|manifest| normalize_path(&manifest.manifest_path)),
                )
            } else {
                match &command_scope {
                    ProjectCommandScope::Project => (
                        true,
                        load_project_manifest(path)
                            .ok()
                            .map(|manifest| normalize_path(&manifest.manifest_path)),
                    ),
                    ProjectCommandScope::ProjectBuildTarget(request) => (
                        true,
                        Some(normalize_path(&request.request_root_manifest_path)),
                    ),
                    ProjectCommandScope::ProjectTestFile(_) | ProjectCommandScope::DirectSource => {
                        (false, None)
                    }
                }
            };
        Self {
            scope: if project_scope { "project" } else { "file" },
            path: normalize_path(path),
            project_manifest_path,
            requested_emit: build_emit_cli_value(options.emit),
            requested_profile: options.profile.dir_name(),
            profile_overridden,
            emit_interface,
            built_targets: Vec::new(),
            interfaces: Vec::new(),
            failure: None,
        }
    }

    pub(crate) fn scope(&self) -> &'static str {
        self.scope
    }

    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) fn project_manifest_path(&self) -> Option<&str> {
        self.project_manifest_path.as_deref()
    }

    pub(crate) fn requested_profile(&self) -> &'static str {
        self.requested_profile
    }

    pub(crate) fn profile_overridden(&self) -> bool {
        self.profile_overridden
    }

    pub(crate) fn record_source_target(&mut self, path: &Path, artifact: &BuildArtifact) {
        self.built_targets.push(build_json_target(
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
        selected: bool,
    ) {
        self.built_targets.push(build_json_target(
            Some(&member.member_manifest_path),
            Some(member.package_name.as_str()),
            target.kind.as_str(),
            project_target_display_path(&member.member_manifest_path, &target.path),
            artifact,
            selected,
        ));
    }

    pub(crate) fn record_source_failure(&mut self, path: &Path, error: &BuildError) {
        self.failure = Some(build_json_failure(
            None,
            None,
            "source",
            normalize_path(path),
            true,
            error,
        ));
    }

    pub(crate) fn record_preflight_failure(&mut self, failure: JsonValue) {
        self.failure = Some(failure);
    }

    pub(crate) fn record_project_failure(
        &mut self,
        member: &WorkspaceBuildTargets,
        target: &BuildTarget,
        error: &BuildError,
        selected: bool,
    ) {
        self.failure = Some(build_json_failure(
            Some(&member.member_manifest_path),
            Some(member.package_name.as_str()),
            target.kind.as_str(),
            project_target_display_path(&member.member_manifest_path, &target.path),
            selected,
            error,
        ));
    }

    pub(crate) fn record_interface_result(
        &mut self,
        manifest_path: Option<&Path>,
        package_name: Option<&str>,
        selected: bool,
        result: EmitPackageInterfaceResult,
    ) {
        let (status, path) = match result {
            EmitPackageInterfaceResult::Wrote(path) => ("wrote", path),
            EmitPackageInterfaceResult::UpToDate(path) => ("up-to-date", path),
        };
        self.interfaces.push(json!({
            "manifest_path": manifest_path.map(normalize_path),
            "package_name": package_name,
            "selected": selected,
            "status": status,
            "path": normalize_path(&path),
        }));
    }

    pub(crate) fn into_json(self) -> String {
        let rendered = serde_json::to_string_pretty(&json!({
            "schema": "ql.build.v1",
            "path": self.path,
            "scope": self.scope,
            "project_manifest_path": self.project_manifest_path,
            "requested_emit": self.requested_emit,
            "requested_profile": self.requested_profile,
            "profile_overridden": self.profile_overridden,
            "emit_interface": self.emit_interface,
            "status": if self.failure.is_some() { "failed" } else { "ok" },
            "built_targets": self.built_targets,
            "interfaces": self.interfaces,
            "failure": self.failure,
        }))
        .expect("build json report should serialize");
        format!("{rendered}\n")
    }
}

#[cfg(test)]
#[path = "build_json_report_tests.rs"]
mod tests;
