use std::path::Path;

use ql_driver::BuildProfile;

use crate::cli_build_profile::{parse_cli_build_profile, set_cli_build_profile};
use crate::cli_utils::normalize_path;
use crate::project_targets::{ProjectCommandScope, resolve_project_command_scope};
use crate::test_pipeline::{
    discover_test_targets, execute_test_targets, filter_test_targets, list_test_targets,
    report_no_matching_test_target, report_no_matching_tests, report_no_tests_discovered,
    select_test_targets_by_path, test_build_options, test_no_matching_filter_message,
    test_no_matching_target_message, test_no_tests_message,
};
use crate::test_reporting::{render_test_json_report, render_test_json_selection_failure_report};

#[derive(Clone, Debug, Default)]
pub(crate) struct TestCommandOptions {
    pub(crate) profile: BuildProfile,
    pub(crate) profile_overridden: bool,
    pub(crate) list_only: bool,
    pub(crate) json: bool,
    pub(crate) filter: Option<String>,
    pub(crate) package_name: Option<String>,
    pub(crate) target_path: Option<String>,
}

pub(crate) fn test_cli_path(args: &mut impl Iterator<Item = String>) -> Result<(), u8> {
    let options = parse_test_args(args)?;
    test_path(Path::new(&options.path), &options.command_options)
}

fn test_path(path: &Path, command_options: &TestCommandOptions) -> Result<(), u8> {
    let build_options = test_build_options(command_options.profile);
    let command_scope = resolve_project_command_scope(path);
    let discovered_targets =
        discover_test_targets(path, &build_options, command_options, &command_scope)?;
    let discovered_total = discovered_targets.len();

    if discovered_targets.is_empty() {
        if command_options.json {
            print!(
                "{}",
                render_test_json_selection_failure_report(
                    path,
                    command_options,
                    "no-tests",
                    discovered_total,
                    "test-discovery",
                    test_no_tests_message(path, command_options.package_name.as_deref()),
                    command_options
                        .package_name
                        .as_deref()
                        .map(|package_name| format!("package `{package_name}`")),
                )
            );
        } else {
            report_no_tests_discovered(path, command_options.package_name.as_deref());
        }
        return Err(1);
    }

    let targets = if let Some(target_path) = command_options.target_path.as_deref() {
        let selected = select_test_targets_by_path(
            discovered_targets,
            target_path,
            command_options.package_name.as_deref(),
        );
        if selected.is_empty() {
            if command_options.json {
                print!(
                    "{}",
                    render_test_json_selection_failure_report(
                        path,
                        command_options,
                        "no-match",
                        discovered_total,
                        "target-selection",
                        test_no_matching_target_message(
                            path,
                            target_path,
                            command_options.package_name.as_deref(),
                        ),
                        Some(format!("target `{target_path}`")),
                    )
                );
            } else {
                report_no_matching_test_target(
                    path,
                    target_path,
                    command_options.package_name.as_deref(),
                );
            }
            return Err(1);
        }
        selected
    } else {
        discovered_targets
    };

    let targets = filter_test_targets(targets, command_options.filter.as_deref());
    if targets.is_empty() {
        if command_options.json {
            print!(
                "{}",
                render_test_json_selection_failure_report(
                    path,
                    command_options,
                    "no-match",
                    discovered_total,
                    "filter-selection",
                    test_no_matching_filter_message(
                        path,
                        command_options.filter.as_deref().unwrap_or_default(),
                        command_options.package_name.as_deref(),
                    ),
                    command_options
                        .filter
                        .as_deref()
                        .map(|filter| format!("filter `{filter}`")),
                )
            );
        } else {
            report_no_matching_tests(
                path,
                command_options.filter.as_deref().unwrap_or_default(),
                command_options.package_name.as_deref(),
            );
        }
        return Err(1);
    }

    if command_options.list_only {
        if command_options.json {
            print!(
                "{}",
                render_test_json_report(
                    path,
                    command_options,
                    "listed",
                    discovered_total,
                    &targets,
                    None,
                )
            );
        } else {
            list_test_targets(&targets);
        }
        return Ok(());
    }

    let execution_report = execute_test_targets(
        path,
        match &command_scope {
            ProjectCommandScope::ProjectTestFile(request) => {
                Some(request.request_root_manifest_path.as_path())
            }
            _ => None,
        },
        &targets,
        command_options.json,
        &build_options,
        command_options.profile_overridden,
    )?;
    if command_options.json {
        print!(
            "{}",
            render_test_json_report(
                path,
                command_options,
                execution_report.status(),
                discovered_total,
                &targets,
                Some(&execution_report),
            )
        );
    }

    if execution_report.is_success() {
        Ok(())
    } else {
        Err(1)
    }
}

struct TestCliOptions {
    path: String,
    command_options: TestCommandOptions,
}

fn parse_test_args(args: &mut impl Iterator<Item = String>) -> Result<TestCliOptions, u8> {
    let remaining = args.collect::<Vec<_>>();
    let mut path = None;
    let mut command_options = TestCommandOptions::default();
    let mut profile_override = None;
    let mut index = 0;

    while index < remaining.len() {
        match remaining[index].as_str() {
            "--release" => {
                set_cli_build_profile("`ql test`", &mut profile_override, BuildProfile::Release)?;
            }
            "--profile" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!("error: `ql test --profile` expects `debug` or `release`");
                    return Err(1);
                };
                let parsed = parse_cli_build_profile("`ql test`", value)?;
                set_cli_build_profile("`ql test`", &mut profile_override, parsed)?;
            }
            "--package" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!("error: `ql test --package` expects a package name");
                    return Err(1);
                };
                if command_options.package_name.is_some() {
                    eprintln!("error: `ql test` received multiple `--package` selectors");
                    return Err(1);
                }
                command_options.package_name = Some(value.to_owned());
            }
            "--list" => {
                command_options.list_only = true;
            }
            "--json" => {
                command_options.json = true;
            }
            "--filter" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!("error: `ql test --filter` expects a substring");
                    return Err(1);
                };
                command_options.filter = Some(value.to_owned());
            }
            "--target" => {
                index += 1;
                let Some(value) = remaining.get(index) else {
                    eprintln!("error: `ql test --target` expects a test path");
                    return Err(1);
                };
                if command_options.target_path.is_some() {
                    eprintln!("error: `ql test` received multiple `--target` selectors");
                    return Err(1);
                }
                command_options.target_path = Some(normalize_path(Path::new(value)));
            }
            other if other.starts_with('-') => {
                eprintln!("error: unknown `ql test` option `{other}`");
                return Err(1);
            }
            other => {
                if path.is_some() {
                    eprintln!("error: unknown `ql test` argument `{other}`");
                    return Err(1);
                }
                path = Some(other.to_owned());
            }
        }

        index += 1;
    }

    let Some(path) = path else {
        eprintln!("error: `ql test` expects a file or directory path");
        return Err(1);
    };

    command_options.profile = profile_override.unwrap_or_default();
    command_options.profile_overridden = profile_override.is_some();

    Ok(TestCliOptions {
        path,
        command_options,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_test_args_records_profile_selectors_and_filters() {
        let mut args = vec![
            "pkg".to_owned(),
            "--profile".to_owned(),
            "release".to_owned(),
            "--package".to_owned(),
            "app".to_owned(),
            "--target".to_owned(),
            "tests/./api/../smoke.ql".to_owned(),
            "--filter".to_owned(),
            "smoke".to_owned(),
            "--list".to_owned(),
            "--json".to_owned(),
        ]
        .into_iter();

        let parsed = parse_test_args(&mut args).expect("valid test args");

        assert_eq!(parsed.path, "pkg");
        assert_eq!(parsed.command_options.profile, BuildProfile::Release);
        assert!(parsed.command_options.profile_overridden);
        assert!(parsed.command_options.list_only);
        assert!(parsed.command_options.json);
        assert_eq!(parsed.command_options.package_name.as_deref(), Some("app"));
        assert_eq!(parsed.command_options.filter.as_deref(), Some("smoke"));
        assert_eq!(
            parsed.command_options.target_path.as_deref(),
            Some("tests/smoke.ql")
        );
    }

    #[test]
    fn parse_test_args_rejects_duplicate_target_selectors() {
        let mut args = vec![
            "pkg".to_owned(),
            "--target".to_owned(),
            "tests/a.ql".to_owned(),
            "--target".to_owned(),
            "tests/b.ql".to_owned(),
        ]
        .into_iter();

        assert_eq!(parse_test_args(&mut args).map(|_| ()), Err(1));
    }
}
