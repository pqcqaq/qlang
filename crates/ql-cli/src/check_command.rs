use std::fs;
use std::path::Path;

use crate::check_json_report::CheckJsonReport;
use crate::check_reporting::report_check_package_selector_requires_workspace_context;
use crate::cli_analysis::analyze_source;
use crate::cli_diagnostics::print_diagnostics;
use crate::cli_scan::collect_ql_files;
use crate::project_targets::{ProjectCheckCommandScope, resolve_project_check_command_scope};

mod project;

use project::{CheckProjectPathOutcome, check_project_path};

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

fn check_path(
    path: &Path,
    sync_interfaces: bool,
    json: bool,
    package_name: Option<&str>,
) -> Result<(), u8> {
    let command_scope = resolve_project_check_command_scope(path);
    match command_scope {
        ProjectCheckCommandScope::Project {
            request_root_manifest_path,
        } => {
            let manifest_request_path = request_root_manifest_path.as_deref().unwrap_or(path);
            match check_project_path(
                path,
                manifest_request_path,
                sync_interfaces,
                json,
                package_name,
            )? {
                CheckProjectPathOutcome::Checked => Ok(()),
                CheckProjectPathOutcome::FallbackToFiles => {
                    check_files_path(path, sync_interfaces, json)
                }
            }
        }
        ProjectCheckCommandScope::DirectSource => {
            if let Some(package_name) = package_name {
                report_check_package_selector_requires_workspace_context(package_name);
                return Err(1);
            }
            check_files_path(path, sync_interfaces, json)
        }
    }
}

fn check_files_path(path: &Path, sync_interfaces: bool, json: bool) -> Result<(), u8> {
    let files = collect_ql_files(path).map_err(|error| {
        eprintln!("error: {error}");
        1
    })?;

    if files.is_empty() {
        eprintln!("error: no `.ql` files found under `{}`", path.display());
        return Err(1);
    }

    let mut has_errors = false;
    let mut json_report = json.then(|| CheckJsonReport::new("files", sync_interfaces, None));

    for file in files {
        let source = fs::read_to_string(&file).map_err(|error| {
            eprintln!("error: failed to read `{}`: {error}", file.display());
            1
        })?;

        match analyze_source(&source) {
            Ok(()) => {
                if let Some(report) = json_report.as_mut() {
                    report.record_checked_file(&file);
                } else {
                    println!("ok: {}", file.display());
                }
            }
            Err(diagnostics) => {
                has_errors = true;
                if let Some(report) = json_report.as_mut() {
                    report.record_source_diagnostics(&file, &source, &diagnostics, None);
                } else {
                    print_diagnostics(&file, &source, &diagnostics);
                }
            }
        }
    }

    if let Some(report) = json_report {
        print!("{}", report.into_json());
    }

    if has_errors { Err(1) } else { Ok(()) }
}

#[cfg(test)]
#[path = "check_command_tests.rs"]
mod tests;
