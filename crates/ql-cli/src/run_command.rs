use std::path::Path;

use ql_driver::{BuildEmit, BuildOptions, BuildProfile};

use crate::cli_build_profile::{parse_cli_build_profile, set_cli_build_profile};
use crate::project_targets::{
    ProjectCommandPathError, ProjectTargetSelector, ResolvedProjectCommandPath,
    list_runnable_targets_path, parse_project_target_selector_option,
    report_project_source_path_rejects_target_selector,
    report_project_target_selector_requires_project_context, resolve_project_command_path,
};
use crate::{
    RunJsonReport, build_json_preflight_failure, build_single_source_target_result,
    build_single_source_target_silent, emit_run_json_execution, run_built_executable,
    run_project_path, run_project_path_json,
};

pub(crate) fn run_cli_path(args: &mut impl Iterator<Item = String>) -> Result<(), u8> {
    let options = parse_run_args(args)?;

    if options.list {
        return list_runnable_targets_path(
            Path::new(&options.path),
            &options.selector,
            options.json,
        );
    }
    run_path(
        Path::new(&options.path),
        options.profile,
        options.profile_overridden,
        &options.selector,
        &options.program_args,
        options.json,
    )
}

fn run_path(
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

struct RunCliOptions {
    path: String,
    profile: BuildProfile,
    profile_overridden: bool,
    selector: ProjectTargetSelector,
    program_args: Vec<String>,
    json: bool,
    list: bool,
}

fn parse_run_args(args: &mut impl Iterator<Item = String>) -> Result<RunCliOptions, u8> {
    let remaining = args.collect::<Vec<_>>();
    let mut path = None;
    let mut profile_override = None;
    let mut selector = ProjectTargetSelector::default();
    let mut program_args = Vec::new();
    let mut json = false;
    let mut list = false;
    let mut passthrough = false;
    let mut index = 0;

    while index < remaining.len() {
        let argument = &remaining[index];
        if passthrough {
            program_args.push(argument.clone());
            index += 1;
            continue;
        }

        if parse_project_target_selector_option("`ql run`", &remaining, &mut index, &mut selector)?
        {
            index += 1;
            continue;
        }

        match argument.as_str() {
            "--" => {
                passthrough = true;
            }
            "--json" => {
                json = true;
            }
            "--list" => {
                list = true;
            }
            "--release" => {
                set_cli_build_profile("`ql run`", &mut profile_override, BuildProfile::Release)?;
            }
            "--profile" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!("error: `ql run --profile` expects `debug` or `release`");
                    return Err(1);
                };
                let parsed = parse_cli_build_profile("`ql run`", value)?;
                set_cli_build_profile("`ql run`", &mut profile_override, parsed)?;
            }
            other if other.starts_with('-') => {
                eprintln!("error: unknown `ql run` option `{other}`");
                return Err(1);
            }
            other => {
                if path.is_some() {
                    eprintln!("error: unknown `ql run` argument `{other}`");
                    eprintln!(
                        "hint: use `ql run <file-or-dir> -- <args...>` to pass arguments to the built executable"
                    );
                    return Err(1);
                }
                path = Some(other.to_owned());
            }
        }

        index += 1;
    }

    let Some(path) = path else {
        eprintln!("error: `ql run` expects a file or directory path");
        return Err(1);
    };

    let profile_overridden = profile_override.is_some();
    Ok(RunCliOptions {
        path,
        profile: profile_override.unwrap_or_default(),
        profile_overridden,
        selector,
        program_args,
        json,
        list,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<RunCliOptions, u8> {
        parse_run_args(&mut args.iter().map(|arg| arg.to_string()))
    }

    #[test]
    fn parse_run_args_accepts_json_list_profile_and_program_args() {
        let options = parse(&[
            "app",
            "--json",
            "--list",
            "--profile",
            "release",
            "--",
            "--user-flag",
        ])
        .expect("run args should parse");

        assert_eq!(options.path, "app");
        assert!(options.json);
        assert!(options.list);
        assert!(options.profile_overridden);
        assert!(matches!(options.profile, BuildProfile::Release));
        assert_eq!(options.program_args, vec!["--user-flag".to_owned()]);
    }

    #[test]
    fn run_build_options_always_requests_executable_output() {
        let options = run_build_options(BuildProfile::Release);

        assert!(matches!(options.emit, BuildEmit::Executable));
        assert!(matches!(options.profile, BuildProfile::Release));
    }
}
