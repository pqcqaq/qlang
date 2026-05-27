use std::env;
use std::path::{Path, PathBuf};

use ql_driver::{BuildEmit, BuildOptions, BuildProfile, default_output_path};
use ql_project::WorkspaceBuildTargets;

use crate::build_outputs::apply_manifest_default_profile;
use crate::cli_scan::collect_ql_files;
use crate::cli_utils::normalize_path;
use crate::project_targets::{
    ProjectCommandScope, display_relative_to_root, project_request_root,
    resolve_project_workspace_member_command_request_root,
};
use crate::test_command::TestCommandOptions;
use crate::test_reporting::{
    TestTarget, TestTargetKind, render_test_json_preflight_message_report,
};

mod selection;

use selection::load_project_test_members;

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
