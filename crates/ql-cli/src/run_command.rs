use std::path::Path;

use ql_driver::BuildProfile;

use crate::cli_build_profile::{parse_cli_build_profile, set_cli_build_profile};
use crate::project_targets::{
    ProjectTargetSelector, list_runnable_targets_path, parse_project_target_selector_option,
};

use super::run_path;

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
