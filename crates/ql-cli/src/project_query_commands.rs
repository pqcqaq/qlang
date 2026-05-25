use std::env;
use std::path::PathBuf;

use crate::project_dependencies::{project_dependencies_path, project_dependents_path};
use crate::project_graph::project_graph_path;
use crate::project_status::project_status_path;
use crate::project_targets::{
    ProjectTargetSelector, parse_project_target_selector_option, project_targets_path,
};

pub(crate) fn project_status_cli_path(args: &mut impl Iterator<Item = String>) -> Result<(), u8> {
    let options = parse_project_package_query_args(args, "`ql project status`")?;
    project_status_path(
        &options.path.unwrap_or_else(default_project_query_path),
        options.package_name.as_deref(),
        options.json,
    )
}

pub(crate) fn project_graph_cli_path(args: &mut impl Iterator<Item = String>) -> Result<(), u8> {
    let options = parse_project_package_query_args(args, "`ql project graph`")?;
    project_graph_path(
        &options.path.unwrap_or_else(default_project_query_path),
        options.package_name.as_deref(),
        options.json,
    )
}

pub(crate) fn project_dependencies_cli_path(
    args: &mut impl Iterator<Item = String>,
) -> Result<(), u8> {
    let options = parse_project_dependency_query_args(args, "`ql project dependencies`")?;
    project_dependencies_path(
        &options.path.unwrap_or_else(default_project_query_path),
        options.package_name.as_deref(),
        options.json,
    )
}

pub(crate) fn project_dependents_cli_path(
    args: &mut impl Iterator<Item = String>,
) -> Result<(), u8> {
    let options = parse_project_dependency_query_args(args, "`ql project dependents`")?;
    project_dependents_path(
        &options.path.unwrap_or_else(default_project_query_path),
        options.package_name.as_deref(),
        options.json,
    )
}

pub(crate) fn project_targets_cli_path(args: &mut impl Iterator<Item = String>) -> Result<(), u8> {
    let remaining = args.collect::<Vec<_>>();
    let mut path = None;
    let mut selector = ProjectTargetSelector::default();
    let mut json = false;
    let mut index = 0;

    while index < remaining.len() {
        if parse_project_target_selector_option(
            "`ql project targets`",
            &remaining,
            &mut index,
            &mut selector,
        )? {
            index += 1;
            continue;
        }

        match remaining[index].as_str() {
            "--json" => {
                json = true;
            }
            other if other.starts_with('-') => {
                eprintln!("error: unknown `ql project targets` option `{other}`");
                return Err(1);
            }
            other => {
                if path.is_some() {
                    eprintln!("error: unknown `ql project targets` argument `{other}`");
                    return Err(1);
                }
                path = Some(PathBuf::from(other));
            }
        }

        index += 1;
    }

    project_targets_path(
        &path.unwrap_or_else(default_project_query_path),
        &selector,
        json,
    )
}

struct ProjectPackageQueryOptions {
    path: Option<PathBuf>,
    package_name: Option<String>,
    json: bool,
}

fn parse_project_dependency_query_args(
    args: &mut impl Iterator<Item = String>,
    command_label: &str,
) -> Result<ProjectPackageQueryOptions, u8> {
    let remaining = args.collect::<Vec<_>>();
    let mut path = None;
    let mut package_name = None;
    let mut json = false;
    let mut index = 0;

    while index < remaining.len() {
        match remaining[index].as_str() {
            "--name" | "--package" => {
                let selector_option = remaining[index].clone();
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!("error: {command_label} {selector_option} expects a package name");
                    return Err(1);
                };
                if package_name.is_some() {
                    eprintln!("error: {command_label} received package selector more than once");
                    return Err(1);
                }
                package_name = Some(value.clone());
            }
            "--json" => {
                json = true;
            }
            other if other.starts_with('-') => {
                eprintln!("error: unknown {command_label} option `{other}`");
                return Err(1);
            }
            other => {
                if path.is_some() {
                    eprintln!("error: unknown {command_label} argument `{other}`");
                    return Err(1);
                }
                path = Some(PathBuf::from(other));
            }
        }

        index += 1;
    }

    Ok(ProjectPackageQueryOptions {
        path,
        package_name,
        json,
    })
}

fn parse_project_package_query_args(
    args: &mut impl Iterator<Item = String>,
    command_label: &str,
) -> Result<ProjectPackageQueryOptions, u8> {
    let remaining = args.collect::<Vec<_>>();
    let mut path = None;
    let mut package_name = None;
    let mut json = false;
    let mut index = 0;

    while index < remaining.len() {
        match remaining[index].as_str() {
            "--package" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!("error: {command_label} --package expects a package name");
                    return Err(1);
                };
                if package_name.is_some() {
                    eprintln!("error: {command_label} received `--package` more than once");
                    return Err(1);
                }
                package_name = Some(value.clone());
            }
            "--json" => {
                json = true;
            }
            other if other.starts_with('-') => {
                eprintln!("error: unknown {command_label} option `{other}`");
                return Err(1);
            }
            other => {
                if path.is_some() {
                    eprintln!("error: unknown {command_label} argument `{other}`");
                    return Err(1);
                }
                path = Some(PathBuf::from(other));
            }
        }

        index += 1;
    }

    Ok(ProjectPackageQueryOptions {
        path,
        package_name,
        json,
    })
}

fn default_project_query_path() -> PathBuf {
    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}
