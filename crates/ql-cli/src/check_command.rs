use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use ql_analysis::{PackageAnalysisError, analyze_package};
use ql_project::{load_project_manifest, package_name, package_source_root};

use crate::check_json_report::CheckJsonReport;
use crate::check_reporting::{
    format_check_command_label, format_workspace_member_check_rerun_command,
    report_check_package_selector_requires_workspace_context,
    report_package_check_manifest_failure, report_package_check_no_sources_failure,
    report_package_check_reference_failure, report_package_check_source_diagnostics_failure,
    report_package_check_source_root_failure,
    report_workspace_member_package_check_manifest_failure,
    report_workspace_member_package_check_no_sources_failure,
    report_workspace_member_package_check_reference_failure,
    report_workspace_member_package_check_source_diagnostics_failure,
    report_workspace_member_package_check_source_root_failure,
};
use crate::cli_analysis::{analyze_source, print_package_analysis_error};
use crate::cli_diagnostics::print_diagnostics;
use crate::cli_scan::collect_ql_files;
use crate::cli_utils::{
    normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error,
};
use crate::project_manifest_paths::{
    record_reference_failure_manifest, workspace_member_manifest_path,
};
use crate::project_reference_interfaces::{
    ensure_reference_interfaces_current, sync_reference_interfaces,
};
use crate::project_reporting::report_workspace_member_failure;
use crate::project_targets::{ProjectCheckCommandScope, resolve_project_check_command_scope};
use crate::project_workspace::select_workspace_members;

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
    let manifest_request_path = match &command_scope {
        ProjectCheckCommandScope::Project {
            request_root_manifest_path,
        } => request_root_manifest_path.as_deref().unwrap_or(path),
        ProjectCheckCommandScope::DirectSource => path,
    };
    if let Some(package_name) = package_name {
        let Ok(manifest) = load_project_manifest(manifest_request_path) else {
            report_check_package_selector_requires_workspace_context(package_name);
            return Err(1);
        };
        if manifest.workspace.is_none() {
            report_check_package_selector_requires_workspace_context(package_name);
            return Err(1);
        }
    }
    if let ProjectCheckCommandScope::Project { .. } = command_scope {
        let check_command_label = format_check_command_label(sync_interfaces);
        let mut package_manifest_path = None;
        let mut json_report = None;
        if let Ok(manifest) = load_project_manifest(manifest_request_path) {
            if manifest.package.is_none() && manifest.workspace.is_some() {
                return check_workspace_manifest(
                    &manifest,
                    manifest_request_path,
                    sync_interfaces,
                    json,
                    package_name,
                );
            }
            package_manifest_path = Some(manifest.manifest_path.clone());
            if json {
                json_report = Some(CheckJsonReport::new(
                    "package",
                    sync_interfaces,
                    Some(&manifest.manifest_path),
                ));
            }
            if !sync_interfaces && ensure_reference_interfaces_current(&manifest).is_err() {
                report_package_check_reference_failure(&manifest.manifest_path, sync_interfaces);
                return Err(1);
            }
        }
        if sync_interfaces {
            let synced_paths = match sync_reference_interfaces(path, &mut BTreeSet::new()) {
                Ok(paths) => paths,
                Err(_) => {
                    if let Some(manifest_path) = package_manifest_path.as_deref() {
                        report_package_check_reference_failure(manifest_path, sync_interfaces);
                    }
                    return Err(1);
                }
            };
            for interface_path in synced_paths {
                if let Some(report) = json_report.as_mut() {
                    report.record_written_interface(&interface_path);
                } else {
                    println!("wrote interface: {}", interface_path.display());
                }
            }
        }
        match analyze_package(path) {
            Ok(package) => {
                if package.modules().is_empty() {
                    let source_root = package_source_root(package.manifest()).expect(
                        "package-aware `ql check` should only succeed for package manifests",
                    );
                    eprintln!(
                        "error: {check_command_label} no `.ql` files found under `{}`",
                        source_root.display()
                    );
                    report_package_check_no_sources_failure(
                        &package.manifest().manifest_path,
                        &source_root,
                        sync_interfaces,
                    );
                    return Err(1);
                }
                for module in package.modules() {
                    if let Some(report) = json_report.as_mut() {
                        report.record_checked_file(module.path());
                    } else {
                        println!("ok: {}", module.path().display());
                    }
                }
                for dependency in package.dependencies() {
                    if let Some(report) = json_report.as_mut() {
                        report.record_loaded_interface(dependency.interface_path());
                    } else {
                        println!(
                            "loaded interface: {}",
                            dependency.interface_path().display()
                        );
                    }
                }
                if let Some(report) = json_report {
                    print!("{}", report.into_json());
                }
                return Ok(());
            }
            Err(PackageAnalysisError::Project(ql_project::ProjectError::ManifestNotFound {
                ..
            })) => {
                if path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.eq_ignore_ascii_case("qlang.toml"))
                {
                    eprintln!(
                        "error: could not find `qlang.toml` starting from `{}`",
                        path.display()
                    );
                    return Err(1);
                }
            }
            Err(PackageAnalysisError::Project(error)) => {
                if let Some(manifest_path) =
                    package_missing_name_manifest_path_from_project_error(&error)
                {
                    eprintln!(
                        "error: {} manifest `{}` does not declare `[package].name`",
                        check_command_label,
                        normalize_path(manifest_path)
                    );
                    report_package_check_manifest_failure(manifest_path, sync_interfaces);
                } else if let Some(manifest_path) =
                    package_check_manifest_path_from_project_error(&error)
                {
                    eprintln!("error: {check_command_label} {error}");
                    report_package_check_manifest_failure(manifest_path, sync_interfaces);
                } else if let ql_project::ProjectError::PackageSourceRootNotFound { path } = &error
                {
                    eprintln!("error: {check_command_label} {error}");
                    report_package_check_source_root_failure(
                        package_manifest_path
                            .as_deref()
                            .expect("package source root failures require a loaded manifest"),
                        path,
                        sync_interfaces,
                    );
                } else {
                    print_package_analysis_error(&PackageAnalysisError::Project(error));
                }
                return Err(1);
            }
            Err(error) => {
                if let PackageAnalysisError::SourceDiagnostics {
                    path,
                    source,
                    diagnostics,
                } = &error
                {
                    if let Some(mut report) = json_report {
                        report.record_source_diagnostics(
                            path,
                            source,
                            diagnostics,
                            package_manifest_path.as_deref(),
                        );
                        print!("{}", report.into_json());
                        return Err(1);
                    }
                }
                print_package_analysis_error(&error);
                if matches!(&error, PackageAnalysisError::SourceDiagnostics { .. }) {
                    report_package_check_source_diagnostics_failure(
                        package_manifest_path
                            .as_deref()
                            .expect("package source diagnostics require a loaded manifest"),
                        sync_interfaces,
                    );
                }
                return Err(1);
            }
        }
    }

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

fn check_workspace_manifest(
    manifest: &ql_project::ProjectManifest,
    request_path: &Path,
    sync_interfaces: bool,
    json: bool,
    selected_package_name: Option<&str>,
) -> Result<(), u8> {
    let Some(_) = &manifest.workspace else {
        return Ok(());
    };

    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let check_command_label = format_check_command_label(sync_interfaces);
    let selected_members = select_workspace_members(
        manifest,
        request_path,
        selected_package_name,
        &check_command_label,
        "--package",
    )?;
    let mut sync_visited = BTreeSet::new();
    let mut synced_interfaces = BTreeSet::new();
    let mut failing_members = 0usize;
    let mut first_failing_member_manifest = None;
    let mut json_report = json
        .then(|| CheckJsonReport::new("workspace", sync_interfaces, Some(&manifest.manifest_path)));
    let mut json_supported_failure_only = true;

    for member in &selected_members {
        let member_path = manifest_dir.join(member);
        let member_manifest = match load_project_manifest(&member_path) {
            Ok(manifest) => manifest,
            Err(error) => {
                json_supported_failure_only = false;
                let member_manifest_path = workspace_member_manifest_path(&member_path);
                if let Some(manifest_path) =
                    package_missing_name_manifest_path_from_project_error(&error)
                {
                    eprintln!(
                        "error: {} manifest `{}` does not declare `[package].name`",
                        check_command_label,
                        normalize_path(manifest_path)
                    );
                    report_workspace_member_package_check_manifest_failure(
                        manifest_path,
                        sync_interfaces,
                    );
                } else if let Some(manifest_path) =
                    package_check_manifest_path_from_project_error(&error)
                {
                    eprintln!("error: {check_command_label} {error}");
                    report_workspace_member_package_check_manifest_failure(
                        manifest_path,
                        sync_interfaces,
                    );
                } else {
                    eprintln!("error: {check_command_label} {error}");
                    let rerun_command = format_workspace_member_check_rerun_command(
                        &normalize_path(&member_manifest_path),
                        sync_interfaces,
                    );
                    let rerun_hint = format!(
                        "hint: rerun `{rerun_command}` after fixing the workspace member manifest"
                    );
                    report_workspace_member_failure(
                        &member_manifest_path,
                        Some(rerun_hint.as_str()),
                    );
                }
                failing_members += 1;
                record_reference_failure_manifest(
                    &mut first_failing_member_manifest,
                    member_manifest_path,
                );
                continue;
            }
        };

        if let Err(error) = package_name(&member_manifest) {
            json_supported_failure_only = false;
            eprintln!("error: {check_command_label} {error}");
            report_workspace_member_package_check_manifest_failure(
                &member_manifest.manifest_path,
                sync_interfaces,
            );
            failing_members += 1;
            record_reference_failure_manifest(
                &mut first_failing_member_manifest,
                member_manifest.manifest_path.clone(),
            );
            continue;
        }

        if !sync_interfaces && ensure_reference_interfaces_current(&member_manifest).is_err() {
            json_supported_failure_only = false;
            report_workspace_member_package_check_reference_failure(
                &member_manifest.manifest_path,
                sync_interfaces,
            );
            failing_members += 1;
            record_reference_failure_manifest(
                &mut first_failing_member_manifest,
                member_manifest.manifest_path.clone(),
            );
            continue;
        }

        if sync_interfaces {
            let synced_paths = match sync_reference_interfaces(&member_path, &mut sync_visited) {
                Ok(paths) => paths,
                Err(_) => {
                    json_supported_failure_only = false;
                    report_workspace_member_package_check_reference_failure(
                        &member_manifest.manifest_path,
                        sync_interfaces,
                    );
                    failing_members += 1;
                    record_reference_failure_manifest(
                        &mut first_failing_member_manifest,
                        member_manifest.manifest_path.clone(),
                    );
                    continue;
                }
            };
            for interface_path in synced_paths {
                let display_path =
                    fs::canonicalize(&interface_path).unwrap_or_else(|_| interface_path.clone());
                if synced_interfaces.insert(display_path.clone()) {
                    if let Some(report) = json_report.as_mut() {
                        report.record_written_interface(&display_path);
                    } else {
                        println!("wrote interface: {}", display_path.display());
                    }
                }
            }
        }

        match analyze_package(&member_path) {
            Ok(package) => {
                if package.modules().is_empty() {
                    json_supported_failure_only = false;
                    let source_root = package_source_root(package.manifest()).expect(
                        "package-aware `ql check` should only succeed for package manifests",
                    );
                    eprintln!(
                        "error: {check_command_label} no `.ql` files found under `{}`",
                        source_root.display()
                    );
                    report_workspace_member_package_check_no_sources_failure(
                        &member_manifest.manifest_path,
                        &source_root,
                        sync_interfaces,
                    );
                    failing_members += 1;
                    record_reference_failure_manifest(
                        &mut first_failing_member_manifest,
                        member_manifest.manifest_path.clone(),
                    );
                    continue;
                }
                for module in package.modules() {
                    if let Some(report) = json_report.as_mut() {
                        report.record_checked_file(module.path());
                    } else {
                        println!("ok: {}", module.path().display());
                    }
                }
                for dependency in package.dependencies() {
                    if let Some(report) = json_report.as_mut() {
                        report.record_loaded_interface(dependency.interface_path());
                    } else {
                        println!(
                            "loaded interface: {}",
                            dependency.interface_path().display()
                        );
                    }
                }
            }
            Err(PackageAnalysisError::Project(
                ql_project::ProjectError::PackageSourceRootNotFound { path },
            )) => {
                json_supported_failure_only = false;
                eprintln!(
                    "error: {check_command_label} package source directory `{}` does not exist",
                    normalize_path(&path)
                );
                report_workspace_member_package_check_source_root_failure(
                    &member_manifest.manifest_path,
                    &path,
                    sync_interfaces,
                );
                failing_members += 1;
                record_reference_failure_manifest(
                    &mut first_failing_member_manifest,
                    member_manifest.manifest_path.clone(),
                );
            }
            Err(error) => {
                if let PackageAnalysisError::SourceDiagnostics {
                    path,
                    source,
                    diagnostics,
                } = &error
                {
                    if let Some(report) = json_report.as_mut() {
                        report.record_source_diagnostics(
                            path,
                            source,
                            diagnostics,
                            Some(&member_manifest.manifest_path),
                        );
                        failing_members += 1;
                        record_reference_failure_manifest(
                            &mut first_failing_member_manifest,
                            member_manifest.manifest_path.clone(),
                        );
                        continue;
                    }
                }
                json_supported_failure_only = false;
                print_package_analysis_error(&error);
                if matches!(&error, PackageAnalysisError::SourceDiagnostics { .. }) {
                    report_workspace_member_package_check_source_diagnostics_failure(
                        &member_manifest.manifest_path,
                        sync_interfaces,
                    );
                } else {
                    report_workspace_member_failure(&member_manifest.manifest_path, None);
                }
                failing_members += 1;
                record_reference_failure_manifest(
                    &mut first_failing_member_manifest,
                    member_manifest.manifest_path.clone(),
                );
            }
        }
    }

    if json_supported_failure_only {
        if let Some(report) = json_report {
            print!("{}", report.into_json());
        }
        if failing_members > 0 {
            return Err(1);
        }
        return Ok(());
    }

    if failing_members > 0 {
        eprintln!("error: {check_command_label} found {failing_members} failing member(s)");
        if failing_members > 1 {
            if let Some(path) = &first_failing_member_manifest {
                eprintln!(
                    "note: first failing member manifest: {}",
                    normalize_path(path)
                );
            }
        }
        return Err(1);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<CheckCliOptions, u8> {
        parse_check_args(args.iter().map(|arg| arg.to_string()))
    }

    #[test]
    fn parse_check_args_accepts_path_package_json_and_sync_interfaces() {
        let options = parse(&["pkg", "--package", "app", "--json", "--sync-interfaces"])
            .expect("check args should parse");

        assert_eq!(options.path, "pkg");
        assert_eq!(options.package_name.as_deref(), Some("app"));
        assert!(options.json);
        assert!(options.sync_interfaces);
    }

    #[test]
    fn parse_check_args_rejects_missing_path() {
        assert!(parse(&["--json"]).is_err());
    }

    #[test]
    fn parse_check_args_rejects_duplicate_package_selector() {
        assert!(parse(&["pkg", "--package", "app", "--package", "lib"]).is_err());
    }
}
