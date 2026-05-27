use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use ql_driver::{BuildEmit, BuildOptions, BuildProfile, acquire_build_output_locks};
use ql_project::{BuildTarget, ManifestBuildProfile, WorkspaceBuildTargets};
use serde_json::Value as JsonValue;

use crate::build_outputs::{apply_manifest_default_profile, project_target_output_path};
use crate::build_plan::{
    BuildTargetJsonError, prepare_project_dependency_builds, resolve_project_build_plan_members,
    select_project_build_plan_root_members,
};
use crate::build_reporting::{
    build_json_preflight_failure, load_workspace_build_targets_for_build_json_from_request_root,
    select_workspace_build_targets_for_build_json,
};
use crate::build_single_source::{
    build_single_source_target_result, build_single_source_target_silent,
};
use crate::build_single_source_reporting::build_output_lock_error_message;
use crate::cli_utils::normalize_path;
use crate::project_reference_interfaces::prepare_reference_interfaces_for_manifests;
use crate::project_target_build::{
    build_project_source_target_result, build_project_source_target_silent,
};
use crate::project_targets::{
    ProjectCommandPathError, ProjectTargetSelector, ResolvedProjectCommandPath,
    is_runnable_project_target, load_workspace_build_targets_for_command_from_request_root,
    project_target_display_path, report_project_source_path_rejects_target_selector,
    report_project_target_selector_requires_project_context, resolve_project_command_path,
    select_workspace_build_targets,
};
use crate::run_reporting::RunJsonReport;

#[derive(Clone, Debug)]
struct RunnableProjectTarget {
    member_manifest_path: PathBuf,
    package_name: String,
    default_profile: Option<ManifestBuildProfile>,
    target: BuildTarget,
}

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

fn run_project_path_json(
    path: &Path,
    project_request_root: &Path,
    options: &BuildOptions,
    profile_overridden: bool,
    selector: &ProjectTargetSelector,
    program_args: &[String],
) -> Result<(), u8> {
    let mut report = RunJsonReport::new(
        path,
        Some(project_request_root),
        options,
        profile_overridden,
        program_args,
    );
    let all_members = match load_workspace_build_targets_for_build_json_from_request_root(
        path,
        project_request_root,
    ) {
        Ok(members) => members,
        Err(failure) => {
            report.record_preflight_failure(failure);
            print!("{}", report.into_json());
            return Err(1);
        }
    };
    let members = match select_workspace_build_targets_for_build_json(
        path,
        &all_members,
        selector,
        "build targets",
    ) {
        Ok(members) => members,
        Err(failure) => {
            report.record_preflight_failure(failure);
            print!("{}", report.into_json());
            return Err(1);
        }
    };
    let runnable = match select_runnable_project_target_for_run_json(path, &members, selector) {
        Ok(runnable) => runnable,
        Err(failure) => {
            report.record_preflight_failure(failure);
            print!("{}", report.into_json());
            return Err(1);
        }
    };
    prepare_reference_interfaces_for_manifests(
        std::slice::from_ref(&runnable.member_manifest_path),
        "`ql run`",
        false,
    )?;
    let runnable_members = select_project_build_plan_root_members(
        &all_members,
        std::slice::from_ref(&runnable.member_manifest_path),
    );
    prepare_project_dependency_builds(
        &all_members,
        &runnable_members,
        "`ql run`",
        options,
        profile_overridden,
    )?;
    let build_plan =
        resolve_project_build_plan_members(&all_members, &runnable_members, "`ql run`")?;

    let mut target_options =
        apply_manifest_default_profile(options, runnable.default_profile, profile_overridden);
    if target_options.output.is_none() {
        target_options.output = Some(project_target_output_path(
            &runnable.member_manifest_path,
            runnable.target.path.as_path(),
            target_options.profile,
            target_options.emit,
        ));
    }

    let report_member = WorkspaceBuildTargets {
        member_manifest_path: runnable.member_manifest_path.clone(),
        package_name: runnable.package_name.clone(),
        default_profile: runnable.default_profile,
        targets: vec![runnable.target.clone()],
    };

    match build_project_source_target_result(
        &build_plan,
        &runnable.member_manifest_path,
        &runnable.target.path,
        &target_options,
        options,
        profile_overridden,
        false,
    ) {
        Ok(artifact) => {
            report.record_project_target(&report_member, &runnable.target, &artifact);
            emit_run_json_execution(report, &artifact.path, program_args)
        }
        Err(BuildTargetJsonError::Early(error)) => {
            report.record_project_target_prep_failure(&report_member, &runnable.target, &error);
            print!("{}", report.into_json());
            Err(1)
        }
        Err(BuildTargetJsonError::Build(error)) => {
            report.record_project_build_failure(&report_member, &runnable.target, &error);
            print!("{}", report.into_json());
            Err(1)
        }
    }
}

fn run_project_path(
    path: &Path,
    options: &BuildOptions,
    profile_overridden: bool,
    selector: &ProjectTargetSelector,
    program_args: &[String],
    project_request_root: Option<&Path>,
) -> Result<(), u8> {
    let request_root = project_request_root.unwrap_or(path);
    let all_members =
        load_workspace_build_targets_for_command_from_request_root(path, request_root, "`ql run`")?;
    let members =
        select_workspace_build_targets(path, &all_members, selector, "`ql run`", "build targets")?;
    let runnable = select_runnable_project_target(path, &members, selector)?;
    prepare_reference_interfaces_for_manifests(
        std::slice::from_ref(&runnable.member_manifest_path),
        "`ql run`",
        false,
    )?;
    let runnable_members = select_project_build_plan_root_members(
        &all_members,
        std::slice::from_ref(&runnable.member_manifest_path),
    );
    prepare_project_dependency_builds(
        &all_members,
        &runnable_members,
        "`ql run`",
        options,
        profile_overridden,
    )?;
    let mut target_options =
        apply_manifest_default_profile(options, runnable.default_profile, profile_overridden);
    if target_options.output.is_none() {
        target_options.output = Some(project_target_output_path(
            &runnable.member_manifest_path,
            runnable.target.path.as_path(),
            target_options.profile,
            target_options.emit,
        ));
    }
    let artifact = build_project_source_target_silent(
        &all_members,
        "`ql run`",
        &runnable.member_manifest_path,
        &runnable.target.path,
        &target_options,
        options,
        profile_overridden,
        false,
        false,
    )?;
    run_built_executable(&artifact.path, program_args)
}

fn collect_runnable_project_targets(
    members: &[WorkspaceBuildTargets],
) -> Vec<RunnableProjectTarget> {
    let mut runnable_targets = Vec::new();
    for member in members {
        for target in &member.targets {
            if is_runnable_project_target(target.kind) {
                runnable_targets.push(RunnableProjectTarget {
                    member_manifest_path: member.member_manifest_path.clone(),
                    package_name: member.package_name.clone(),
                    default_profile: member.default_profile,
                    target: target.clone(),
                });
            }
        }
    }
    runnable_targets
}

fn select_runnable_project_target(
    path: &Path,
    members: &[WorkspaceBuildTargets],
    selector: &ProjectTargetSelector,
) -> Result<RunnableProjectTarget, u8> {
    let mut runnable_targets = collect_runnable_project_targets(members);
    match runnable_targets.len() {
        0 => {
            let normalized_path = normalize_path(path);
            if selector.is_active() {
                eprintln!(
                    "error: `ql run` target selector matched no runnable build targets under `{normalized_path}`"
                );
                eprintln!("note: selector: {}", selector.describe());
                eprintln!(
                    "hint: rerun `ql project targets {normalized_path}` to inspect the discovered build targets"
                );
                return Err(1);
            }
            eprintln!("error: `ql run` found no runnable build targets under `{normalized_path}`");
            eprintln!(
                "hint: add `src/main.ql`, `src/bin/*.ql`, or declare `[[bin]].path`, or rerun `ql project targets {normalized_path}` to inspect the discovered build targets"
            );
            Err(1)
        }
        1 => Ok(runnable_targets
            .pop()
            .expect("runnable target count checked above")),
        count => {
            let normalized_path = normalize_path(path);
            if selector.is_active() {
                eprintln!(
                    "error: `ql run` target selector matched multiple runnable build targets under `{normalized_path}`"
                );
                eprintln!("note: selector: {}", selector.describe());
                eprintln!("note: `{normalized_path}` resolved to {count} runnable build targets");
                for runnable in &runnable_targets {
                    eprintln!(
                        "note: candidate target `{}` from package `{}`",
                        project_target_display_path(
                            &runnable.member_manifest_path,
                            runnable.target.path.as_path()
                        ),
                        runnable.package_name
                    );
                }
                eprintln!(
                    "hint: refine the selector with `--package`, `--bin`, or `--target`, or rerun `ql project targets {normalized_path}` to inspect the discovered build targets"
                );
                return Err(1);
            }
            eprintln!(
                "error: `ql run` found multiple runnable build targets under `{normalized_path}`"
            );
            eprintln!("note: `{normalized_path}` resolved to {count} runnable build targets");
            for runnable in &runnable_targets {
                eprintln!(
                    "note: candidate target `{}` from package `{}`",
                    project_target_display_path(
                        &runnable.member_manifest_path,
                        runnable.target.path.as_path()
                    ),
                    runnable.package_name
                );
            }
            eprintln!(
                "hint: rerun `ql run <source-file>` for a specific target, or `ql project targets {normalized_path}` to inspect the discovered build targets"
            );
            Err(1)
        }
    }
}

fn select_runnable_project_target_for_run_json(
    path: &Path,
    members: &[WorkspaceBuildTargets],
    selector: &ProjectTargetSelector,
) -> Result<RunnableProjectTarget, JsonValue> {
    let mut runnable_targets = collect_runnable_project_targets(members);
    match runnable_targets.len() {
        0 => {
            let normalized_path = normalize_path(path);
            let (error_kind, message, selector) = if selector.is_active() {
                (
                    "selector",
                    format!(
                        "target selector matched no runnable build targets under `{normalized_path}`"
                    ),
                    Some(selector.describe()),
                )
            } else {
                (
                    "project",
                    format!("found no runnable build targets under `{normalized_path}`"),
                    None,
                )
            };
            Err(build_json_preflight_failure(
                path,
                None,
                None,
                None,
                error_kind,
                "target-selection",
                message,
                selector,
                None,
                Some(0),
            ))
        }
        1 => Ok(runnable_targets
            .pop()
            .expect("runnable target count checked above")),
        count => {
            let normalized_path = normalize_path(path);
            let (error_kind, message, selector) = if selector.is_active() {
                (
                    "selector",
                    format!(
                        "target selector matched multiple runnable build targets under `{normalized_path}`"
                    ),
                    Some(selector.describe()),
                )
            } else {
                (
                    "project",
                    format!("found multiple runnable build targets under `{normalized_path}`"),
                    None,
                )
            };
            Err(build_json_preflight_failure(
                path,
                None,
                None,
                None,
                error_kind,
                "target-selection",
                message,
                selector,
                None,
                Some(count),
            ))
        }
    }
}

fn run_built_executable(executable_path: &Path, program_args: &[String]) -> Result<(), u8> {
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    let execution_lock =
        acquire_build_output_locks(vec![executable_path.to_path_buf()]).map_err(|error| {
            eprintln!(
                "error: failed to lock built executable `{}`: {}",
                normalize_path(executable_path),
                build_output_lock_error_message(error)
            );
            1
        })?;
    let mut command = Command::new(executable_path);
    command.args(program_args);
    let status = command.status().map_err(|error| {
        eprintln!(
            "error: failed to run built executable `{}`: {error}",
            normalize_path(executable_path)
        );
        1
    })?;

    match status.code() {
        Some(0) => Ok(()),
        Some(code) => {
            drop(execution_lock);
            std::process::exit(code);
        }
        None => {
            eprintln!(
                "error: built executable `{}` terminated without an exit code",
                normalize_path(executable_path)
            );
            Err(1)
        }
    }
}

#[derive(Clone, Debug)]
struct CapturedExecutableRun {
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn run_built_executable_capture(
    executable_path: &Path,
    program_args: &[String],
) -> Result<CapturedExecutableRun, String> {
    let _execution_lock = acquire_build_output_locks(vec![executable_path.to_path_buf()])
        .map_err(build_output_lock_error_message)?;
    let mut command = Command::new(executable_path);
    command.args(program_args);
    let output = command.output().map_err(|error| {
        format!(
            "failed to run built executable `{}`: {error}",
            normalize_path(executable_path)
        )
    })?;

    Ok(CapturedExecutableRun {
        exit_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn emit_run_json_execution(
    mut report: RunJsonReport,
    executable_path: &Path,
    program_args: &[String],
) -> Result<(), u8> {
    match run_built_executable_capture(executable_path, program_args) {
        Ok(captured) => {
            if let Some(exit_code) = captured.exit_code {
                report.record_execution(exit_code, &captured.stdout, &captured.stderr);
                print!("{}", report.into_json());
                if exit_code == 0 {
                    Ok(())
                } else {
                    std::process::exit(exit_code);
                }
            } else {
                report.record_run_failure(
                    executable_path,
                    &format!(
                        "built executable `{}` terminated without an exit code",
                        normalize_path(executable_path)
                    ),
                    &captured.stdout,
                    &captured.stderr,
                );
                print!("{}", report.into_json());
                Err(1)
            }
        }
        Err(error) => {
            report.record_spawn_failure(executable_path, error);
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
mod tests {
    use super::*;

    #[test]
    fn run_build_options_always_requests_executable_output() {
        let options = run_build_options(BuildProfile::Release);

        assert!(matches!(options.emit, BuildEmit::Executable));
        assert!(matches!(options.profile, BuildProfile::Release));
    }
}
