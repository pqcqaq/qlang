use std::env;
use std::path::{Path, PathBuf};

use ql_driver::{BuildEmit, BuildOptions, BuildProfile, default_output_path};
use ql_project::{
    WorkspaceBuildTargets, discover_package_build_targets, discover_workspace_build_targets,
    load_project_manifest, package_name,
};

use crate::build_outputs::apply_manifest_default_profile;
use crate::build_reporting::build_json_project_error;
use crate::cli_scan::collect_ql_files;
use crate::cli_utils::{
    normalize_path, package_check_manifest_path_from_project_error,
    package_missing_name_manifest_path_from_project_error, validate_project_package_name,
};
use crate::project_targets::{
    ProjectCommandScope, display_relative_to_root,
    load_workspace_build_targets_for_command_from_request_root, project_request_root,
    resolve_project_workspace_member_command_request_root,
};
use crate::project_workspace::{
    WorkspaceMemberLookupError, render_workspace_member_lookup_error,
    resolve_selected_workspace_member_manifest, resolve_workspace_member_entry_by_package_name,
};
use crate::test_command::TestCommandOptions;
use crate::test_reporting::{
    TestTarget, TestTargetKind, render_test_json_preflight_failure_report,
    render_test_json_preflight_message_report,
};

pub(crate) fn discover_test_targets(
    path: &Path,
    options: &BuildOptions,
    command_options: &TestCommandOptions,
    command_scope: &ProjectCommandScope,
) -> Result<Vec<TestTarget>, u8> {
    match command_scope {
        ProjectCommandScope::Project => {
            let request_root = resolve_project_workspace_member_command_request_root(path);
            discover_project_test_targets(
                path,
                request_root.as_deref().unwrap_or(path),
                options,
                command_options,
            )
        }
        ProjectCommandScope::ProjectTestFile(request) => {
            let discovered = discover_project_test_targets(
                path,
                &request.request_root_manifest_path,
                options,
                command_options,
            )?;
            Ok(select_test_targets_by_path(
                discovered,
                &request.display_path,
                None,
            ))
        }
        ProjectCommandScope::ProjectBuildTarget(_) | ProjectCommandScope::DirectSource => {
            if let Some(package_name) = command_options.package_name.as_deref() {
                if command_options.json {
                    print!(
                        "{}",
                        render_test_json_preflight_message_report(
                            path,
                            command_options,
                            "selector",
                            "target-selection",
                            "`ql test` package selectors require a package or workspace path"
                                .to_owned(),
                            Some(format!("package `{package_name}`")),
                            None,
                        )
                    );
                } else {
                    report_test_package_selector_requires_project_context(package_name);
                }
                return Err(1);
            }
            if let Some(target_path) = command_options.target_path.as_deref() {
                if command_options.json {
                    print!(
                        "{}",
                        render_test_json_preflight_message_report(
                            path,
                            command_options,
                            "selector",
                            "target-selection",
                            "`ql test` target selectors require a package or workspace path"
                                .to_owned(),
                            Some(format!("target `{target_path}`")),
                            None,
                        )
                    );
                } else {
                    report_test_target_selector_requires_project_context(target_path);
                }
                return Err(1);
            }
            Ok(vec![direct_test_target(path, options)?])
        }
    }
}

fn direct_test_target(path: &Path, options: &BuildOptions) -> Result<TestTarget, u8> {
    let working_directory = env::current_dir().map_err(|error| {
        eprintln!("error: failed to determine the current directory for `ql test`: {error}");
        1
    })?;
    Ok(TestTarget {
        display_path: normalize_path(path),
        kind: TestTargetKind::Smoke {
            source_path: path.to_path_buf(),
            working_directory,
            build_options: options.clone(),
            package_manifest_path: None,
        },
    })
}

fn discover_project_test_targets(
    request_path: &Path,
    project_path: &Path,
    options: &BuildOptions,
    command_options: &TestCommandOptions,
) -> Result<Vec<TestTarget>, u8> {
    let members = load_project_test_members(request_path, project_path, command_options)?;
    let request_root = project_request_root(project_path);
    let mut targets = Vec::new();

    for member in members {
        let package_root = member
            .member_manifest_path
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();
        let tests_root = package_root.join("tests");
        if !tests_root.is_dir() {
            continue;
        }

        let files = collect_ql_files(&tests_root).map_err(|error| {
            eprintln!(
                "error: `ql test` failed to read `{}`: {error}",
                normalize_path(&tests_root)
            );
            1
        })?;

        for file in files {
            targets.push(project_test_target(
                &request_root,
                &member,
                &package_root,
                &file,
                options,
                command_options.profile_overridden,
            ));
        }
    }

    Ok(targets)
}

fn load_project_test_members(
    request_path: &Path,
    project_path: &Path,
    command_options: &TestCommandOptions,
) -> Result<Vec<WorkspaceBuildTargets>, u8> {
    let selected_package_name = command_options.package_name.as_deref();
    let Some(selected_package_name) = selected_package_name else {
        if command_options.json {
            let manifest = match load_project_manifest(project_path) {
                Ok(manifest) => manifest,
                Err(error) => {
                    print!(
                        "{}",
                        render_test_json_preflight_failure_report(
                            request_path,
                            command_options,
                            build_json_project_error(request_path, &error, "manifest-load"),
                        )
                    );
                    return Err(1);
                }
            };
            return match discover_workspace_build_targets(&manifest) {
                Ok(members) => Ok(members),
                Err(error) => {
                    print!(
                        "{}",
                        render_test_json_preflight_failure_report(
                            request_path,
                            command_options,
                            build_json_project_error(request_path, &error, "target-discovery"),
                        )
                    );
                    Err(1)
                }
            };
        }
        return load_workspace_build_targets_for_command_from_request_root(
            request_path,
            project_path,
            "`ql test`",
        );
    };

    let manifest = match load_project_manifest(project_path) {
        Ok(manifest) => manifest,
        Err(error) => {
            if command_options.json {
                print!(
                    "{}",
                    render_test_json_preflight_failure_report(
                        request_path,
                        command_options,
                        build_json_project_error(request_path, &error, "manifest-load"),
                    )
                );
            } else {
                report_ql_test_project_error(&error);
            }
            return Err(1);
        }
    };

    if manifest.workspace.is_some() {
        if let Err(message) = validate_project_package_name(selected_package_name) {
            if command_options.json {
                print!(
                    "{}",
                    render_test_json_preflight_message_report(
                        request_path,
                        command_options,
                        "selector",
                        "package-selection",
                        format!("`ql test` {message}"),
                        Some(format!("package `{selected_package_name}`")),
                        None,
                    )
                );
            } else {
                eprintln!("error: `ql test` {message}");
            }
            return Err(1);
        }

        let member_manifest = if command_options.json {
            let (_, member_manifest_path) = match resolve_workspace_member_entry_by_package_name(
                &manifest,
                selected_package_name,
            ) {
                Ok(member) => member,
                Err(error) => {
                    let (error_kind, target_count) = match &error {
                        WorkspaceMemberLookupError::Missing => ("selector", Some(0)),
                        WorkspaceMemberLookupError::Ambiguous { matches } => {
                            ("selector", Some(matches.len()))
                        }
                        WorkspaceMemberLookupError::InspectionFailure { .. } => ("manifest", None),
                    };
                    print!(
                        "{}",
                        render_test_json_preflight_message_report(
                            request_path,
                            command_options,
                            error_kind,
                            "package-selection",
                            render_workspace_member_lookup_error(
                                &manifest,
                                selected_package_name,
                                &error,
                            ),
                            Some(format!("package `{selected_package_name}`")),
                            target_count,
                        )
                    );
                    return Err(1);
                }
            };
            match load_project_manifest(&member_manifest_path) {
                Ok(member_manifest) => member_manifest,
                Err(error) => {
                    print!(
                        "{}",
                        render_test_json_preflight_failure_report(
                            request_path,
                            command_options,
                            build_json_project_error(request_path, &error, "package-selection"),
                        )
                    );
                    return Err(1);
                }
            }
        } else {
            let (_, member_manifest) = resolve_selected_workspace_member_manifest(
                &manifest,
                request_path,
                selected_package_name,
                "`ql test`",
                "--package",
            )?;
            member_manifest
        };

        return Ok(vec![project_test_build_targets_from_manifest(
            request_path,
            command_options,
            &member_manifest,
            Some(&manifest),
        )?]);
    }

    if let Err(message) = validate_project_package_name(selected_package_name) {
        if command_options.json {
            print!(
                "{}",
                render_test_json_preflight_message_report(
                    request_path,
                    command_options,
                    "selector",
                    "package-selection",
                    format!("`ql test` {message}"),
                    Some(format!("package `{selected_package_name}`")),
                    None,
                )
            );
        } else {
            eprintln!("error: `ql test` {message}");
        }
        return Err(1);
    }
    let current_package_name = match package_name(&manifest) {
        Ok(package_name) => package_name,
        Err(error) => {
            if command_options.json {
                print!(
                    "{}",
                    render_test_json_preflight_failure_report(
                        request_path,
                        command_options,
                        build_json_project_error(request_path, &error, "package-selection"),
                    )
                );
            } else {
                eprintln!("error: `ql test` {error}");
            }
            return Err(1);
        }
    };
    if current_package_name != selected_package_name {
        let normalized_path = normalize_path(request_path);
        if command_options.json {
            print!(
                "{}",
                render_test_json_preflight_message_report(
                    request_path,
                    command_options,
                    "selector",
                    "package-selection",
                    format!(
                        "package selector matched no workspace members under `{normalized_path}`"
                    ),
                    Some(format!("package `{selected_package_name}`")),
                    Some(0),
                )
            );
        } else {
            eprintln!(
                "error: `ql test` package selector matched no workspace members under `{normalized_path}`"
            );
            eprintln!("note: selector: package `{selected_package_name}`");
            eprintln!(
                "hint: rerun `ql test {normalized_path}` to inspect all workspace members, or adjust `--package`"
            );
        }
        return Err(1);
    }
    Ok(vec![project_test_build_targets_from_manifest(
        request_path,
        command_options,
        &manifest,
        None,
    )?])
}

fn project_test_build_targets_from_manifest(
    request_path: &Path,
    command_options: &TestCommandOptions,
    manifest: &ql_project::ProjectManifest,
    workspace_manifest: Option<&ql_project::ProjectManifest>,
) -> Result<WorkspaceBuildTargets, u8> {
    let workspace_default_profile = workspace_manifest
        .and_then(|manifest| manifest.profile.as_ref().map(|profile| profile.default));
    Ok(WorkspaceBuildTargets {
        member_manifest_path: manifest.manifest_path.clone(),
        package_name: match package_name(manifest) {
            Ok(package_name) => package_name.to_owned(),
            Err(error) => {
                return Err(report_ql_test_project_preflight_error(
                    request_path,
                    command_options,
                    &error,
                    "target-discovery",
                ));
            }
        },
        default_profile: manifest
            .profile
            .as_ref()
            .map(|profile| profile.default)
            .or(workspace_default_profile),
        targets: match discover_package_build_targets(manifest) {
            Ok(targets) => targets,
            Err(error) => {
                return Err(report_ql_test_project_preflight_error(
                    request_path,
                    command_options,
                    &error,
                    "target-discovery",
                ));
            }
        },
    })
}

fn report_ql_test_project_preflight_error(
    request_path: &Path,
    command_options: &TestCommandOptions,
    error: &ql_project::ProjectError,
    stage: &str,
) -> u8 {
    if command_options.json {
        print!(
            "{}",
            render_test_json_preflight_failure_report(
                request_path,
                command_options,
                build_json_project_error(request_path, error, stage),
            )
        );
        return 1;
    }

    report_ql_test_project_error(error)
}

fn report_ql_test_project_error(error: &ql_project::ProjectError) -> u8 {
    if let ql_project::ProjectError::ManifestNotFound { start } = error {
        eprintln!(
            "error: `ql test` requires a package or workspace manifest; could not find `qlang.toml` starting from `{}`",
            normalize_path(start)
        );
    } else if let Some(manifest_path) = package_missing_name_manifest_path_from_project_error(error)
    {
        eprintln!(
            "error: `ql test` manifest `{}` does not declare `[package].name`",
            normalize_path(manifest_path)
        );
    } else if let ql_project::ProjectError::PackageSourceRootNotFound { path } = error {
        eprintln!(
            "error: `ql test` package source directory `{}` does not exist",
            normalize_path(path)
        );
    } else if let Some(manifest_path) = package_check_manifest_path_from_project_error(error) {
        eprintln!("error: `ql test` {error}");
        eprintln!(
            "note: failing package manifest: {}",
            normalize_path(manifest_path)
        );
    } else {
        eprintln!("error: `ql test` {error}");
    }
    1
}

fn project_test_target(
    request_root: &Path,
    member: &WorkspaceBuildTargets,
    package_root: &Path,
    file: &Path,
    options: &BuildOptions,
    profile_overridden: bool,
) -> TestTarget {
    let display_path = display_relative_to_root(request_root, file);
    if is_project_ui_test(package_root, file) {
        return TestTarget {
            display_path,
            kind: TestTargetKind::Ui {
                source_path: file.to_path_buf(),
                diagnostic_path: package_test_command_path(package_root, file),
                snapshot_path: file.with_extension("stderr"),
            },
        };
    }

    let mut build_options =
        apply_manifest_default_profile(options, member.default_profile, profile_overridden);
    build_options.output = Some(project_test_output_path(
        &member.member_manifest_path,
        file,
        build_options.profile,
    ));
    TestTarget {
        display_path,
        kind: TestTargetKind::Smoke {
            source_path: file.to_path_buf(),
            working_directory: package_root.to_path_buf(),
            build_options,
            package_manifest_path: Some(member.member_manifest_path.clone()),
        },
    }
}

fn project_test_output_path(
    manifest_path: &Path,
    test_path: &Path,
    profile: BuildProfile,
) -> PathBuf {
    let package_root = manifest_path.parent().unwrap_or(Path::new("."));
    let tests_root = package_root.join("tests");
    let relative_test = test_path.strip_prefix(&tests_root).unwrap_or(test_path);
    let default_output =
        default_output_path(package_root, test_path, profile, BuildEmit::Executable);
    let file_name = default_output
        .file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("test"));
    let mut output_path = package_root
        .join("target")
        .join("ql")
        .join(profile.dir_name())
        .join("tests");
    if let Some(parent) = relative_test.parent()
        && !parent.as_os_str().is_empty()
    {
        output_path = output_path.join(parent);
    }
    output_path.join(file_name)
}

pub(crate) fn filter_test_targets(
    targets: Vec<TestTarget>,
    filter: Option<&str>,
) -> Vec<TestTarget> {
    let Some(filter) = filter else {
        return targets;
    };
    targets
        .into_iter()
        .filter(|target| target.display_path.contains(filter))
        .collect()
}

pub(crate) fn select_test_targets_by_path(
    targets: Vec<TestTarget>,
    target_path: &str,
    package_name: Option<&str>,
) -> Vec<TestTarget> {
    targets
        .into_iter()
        .filter(|target| test_target_matches_path(target, target_path, package_name))
        .collect()
}

fn test_target_matches_path(
    target: &TestTarget,
    target_path: &str,
    package_name: Option<&str>,
) -> bool {
    if target.display_path == target_path {
        return true;
    }

    if let Some(package_name) = package_name
        && let Some(package_relative_path) = target
            .display_path
            .strip_prefix(&format!("packages/{package_name}/"))
        && package_relative_path == target_path
    {
        return true;
    }

    match &target.kind {
        TestTargetKind::Smoke { source_path, .. } | TestTargetKind::Ui { source_path, .. } => {
            normalize_path(source_path) == target_path
        }
    }
}

fn report_test_package_selector_requires_project_context(package_name: &str) {
    eprintln!("error: `ql test` package selectors require a package or workspace path");
    eprintln!("note: selector: package `{package_name}`");
}

fn report_test_target_selector_requires_project_context(target_path: &str) {
    eprintln!("error: `ql test` target selectors require a package or workspace path");
    eprintln!("note: selector: target `{target_path}`");
}

pub(crate) fn test_no_tests_message(path: &Path, package_name: Option<&str>) -> String {
    let normalized_path = normalize_path(path);
    if let Some(package_name) = package_name {
        return format!(
            "`ql test` found no `.ql` test files for package `{package_name}` under `{normalized_path}`"
        );
    }
    format!("`ql test` found no `.ql` test files under `{normalized_path}`")
}

pub(crate) fn test_no_matching_filter_message(
    path: &Path,
    filter: &str,
    package_name: Option<&str>,
) -> String {
    let normalized_path = normalize_path(path);
    if let Some(package_name) = package_name {
        return format!(
            "`ql test` found no test files matching `{filter}` for package `{package_name}` under `{normalized_path}`"
        );
    }
    format!("`ql test` found no test files matching `{filter}` under `{normalized_path}`")
}

pub(crate) fn test_no_matching_target_message(
    path: &Path,
    target_path: &str,
    package_name: Option<&str>,
) -> String {
    let normalized_path = normalize_path(path);
    if let Some(package_name) = package_name {
        return format!(
            "`ql test` found no test target `{target_path}` for package `{package_name}` under `{normalized_path}`"
        );
    }
    format!("`ql test` found no test target `{target_path}` under `{normalized_path}`")
}

pub(crate) fn report_no_tests_discovered(path: &Path, package_name: Option<&str>) {
    let normalized_path = normalize_path(path);
    eprintln!("error: {}", test_no_tests_message(path, package_name));
    if let Some(package_name) = package_name {
        eprintln!(
            "hint: add standalone smoke tests under `tests/**/*.ql`, rerun `ql test {normalized_path} --package {package_name} --list`, or adjust `--package`"
        );
        return;
    }
    eprintln!(
        "hint: add standalone smoke tests under `tests/**/*.ql`, or rerun `ql test <file.ql>` for a single file"
    );
}

pub(crate) fn report_no_matching_tests(path: &Path, filter: &str, package_name: Option<&str>) {
    let normalized_path = normalize_path(path);
    eprintln!(
        "error: {}",
        test_no_matching_filter_message(path, filter, package_name)
    );
    if let Some(package_name) = package_name {
        eprintln!(
            "hint: rerun `ql test {normalized_path} --package {package_name} --list` to inspect the discovered tests, or adjust `--filter` / `--package`"
        );
        return;
    }
    eprintln!(
        "hint: rerun `ql test {normalized_path} --list` to inspect the discovered tests, or adjust `--filter`"
    );
}

pub(crate) fn report_no_matching_test_target(
    path: &Path,
    target_path: &str,
    package_name: Option<&str>,
) {
    let normalized_path = normalize_path(path);
    eprintln!(
        "error: {}",
        test_no_matching_target_message(path, target_path, package_name)
    );
    if let Some(package_name) = package_name {
        eprintln!(
            "hint: rerun `ql test {normalized_path} --package {package_name} --list` to inspect the discovered tests, or adjust `--target` / `--package`"
        );
        return;
    }

    eprintln!(
        "hint: rerun `ql test {normalized_path} --list` to inspect the discovered tests, or adjust `--target`"
    );
}

fn is_project_ui_test(package_root: &Path, source_path: &Path) -> bool {
    let Ok(relative) = source_path.strip_prefix(package_root) else {
        return false;
    };
    let mut components = relative
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(segment) => segment.to_str(),
            _ => None,
        });
    matches!(components.next(), Some("tests")) && matches!(components.next(), Some("ui"))
}

fn package_test_command_path(package_root: &Path, source_path: &Path) -> PathBuf {
    source_path
        .strip_prefix(package_root)
        .unwrap_or(source_path)
        .to_path_buf()
}

pub(crate) fn list_test_targets(targets: &[TestTarget]) {
    for target in targets {
        println!("{}", target.display_path);
    }
    println!();
    println!("test listing: {} discovered", targets.len());
}

#[cfg(test)]
#[path = "test_discovery_tests.rs"]
mod tests;
