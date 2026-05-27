use std::path::Path;

use ql_driver::{BuildEmit, BuildOptions, BuildProfile};

use crate::build_reporting::build_json_preflight_failure;
use crate::build_single_source::{
    build_single_source_target_result, build_single_source_target_silent,
};
use crate::project_targets::{
    ProjectCommandPathError, ProjectTargetSelector, ResolvedProjectCommandPath,
    report_project_source_path_rejects_target_selector,
    report_project_target_selector_requires_project_context, resolve_project_command_path,
};
use crate::run_execution::{emit_run_json_execution, run_built_executable};
use crate::run_project::{run_project_path, run_project_path_json};
use crate::run_reporting::RunJsonReport;

pub(crate) fn run_path(
    path: &Path,
    profile: BuildProfile,
    profile_overridden: bool,
    selector: &ProjectTargetSelector,
    program_args: &[String],
    json: bool,
) -> Result<(), u8> {
    let options = run_build_options(profile);
    match resolve_project_command_path(path, selector) {
        Ok(ResolvedProjectCommandPath::Project {
            request_root_manifest_path,
            selector,
        }) => {
            if json {
                return run_project_path_json(
                    path,
                    request_root_manifest_path.as_deref().unwrap_or(path),
                    &options,
                    profile_overridden,
                    &selector,
                    program_args,
                );
            }
            return run_project_path(
                path,
                &options,
                profile_overridden,
                &selector,
                program_args,
                request_root_manifest_path.as_deref(),
            );
        }
        Ok(ResolvedProjectCommandPath::DirectSource) => {}
        Err(ProjectCommandPathError::SourcePathRejectsSelector) => {
            if json {
                let mut report =
                    RunJsonReport::new(path, None, &options, profile_overridden, program_args);
                report.record_preflight_failure(build_json_preflight_failure(
                    path,
                    None,
                    None,
                    None,
                    "selector",
                    "project-context",
                    "direct project source paths do not support target selectors".to_owned(),
                    Some(selector.describe()),
                    None,
                    None,
                ));
                print!("{}", report.into_json());
            } else {
                report_project_source_path_rejects_target_selector("`ql run`", path, selector);
            }
            return Err(1);
        }
        Err(ProjectCommandPathError::SelectorRequiresProjectContext) => {
            if json {
                let mut report =
                    RunJsonReport::new(path, None, &options, profile_overridden, program_args);
                report.record_preflight_failure(build_json_preflight_failure(
                    path,
                    None,
                    None,
                    None,
                    "selector",
                    "project-context",
                    "target selectors require a package or workspace path".to_owned(),
                    Some(selector.describe()),
                    None,
                    None,
                ));
                print!("{}", report.into_json());
            } else {
                report_project_target_selector_requires_project_context("`ql run`", selector);
            }
            return Err(1);
        }
    }

    if json {
        return run_path_json(path, &options, profile_overridden, program_args);
    }

    let artifact = build_single_source_target_silent(path, &options, false)?;
    run_built_executable(&artifact.path, program_args)
}

fn run_path_json(
    path: &Path,
    options: &BuildOptions,
    profile_overridden: bool,
    program_args: &[String],
) -> Result<(), u8> {
    let mut report = RunJsonReport::new(path, None, options, profile_overridden, program_args);
    match build_single_source_target_result(path, options) {
        Ok(artifact) => {
            report.record_source_target(path, &artifact);
            emit_run_json_execution(report, &artifact.path, program_args)
        }
        Err(error) => {
            report.record_source_build_failure(path, &error);
            print!("{}", report.into_json());
            Err(1)
        }
    }
}

fn run_build_options(profile: BuildProfile) -> BuildOptions {
    let mut options = BuildOptions {
        emit: BuildEmit::Executable,
        ..BuildOptions::default()
    };
    options.profile = profile;
    options
}

#[cfg(test)]
#[path = "run_pipeline_tests.rs"]
mod tests;
