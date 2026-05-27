use std::io::Write;
use std::path::Path;

use ql_driver::{BuildEmit, BuildOptions, BuildProfile};

use crate::build_plan::{
    prepare_project_dependency_builds, prepare_project_test_package_builds,
    select_project_build_plan_root_members,
};
use crate::build_single_source::{
    build_single_source_target_quiet, build_single_source_target_silent,
};
use crate::project_reference_interfaces::prepare_reference_interfaces_for_manifests;
use crate::project_target_build::{
    build_project_test_source_target_quiet, build_project_test_source_target_silent,
};
use crate::project_targets::load_workspace_build_targets_for_command_from_request_root;
use crate::test_execution::{
    execute_test_binary, execute_ui_test, report_test_failure, test_target_manifest_paths,
};
use crate::test_reporting::{TestExecutionReport, TestFailure, TestTarget, TestTargetKind};

pub(crate) fn test_build_options(profile: BuildProfile) -> BuildOptions {
    let mut options = BuildOptions {
        emit: BuildEmit::Executable,
        ..BuildOptions::default()
    };
    options.profile = profile;
    options
}

pub(crate) fn execute_test_targets(
    path: &Path,
    project_request_root: Option<&Path>,
    targets: &[TestTarget],
    json: bool,
    options: &BuildOptions,
    profile_overridden: bool,
) -> Result<TestExecutionReport, u8> {
    let manifest_paths = test_target_manifest_paths(targets);
    let workspace_members = if !manifest_paths.is_empty() {
        prepare_reference_interfaces_for_manifests(&manifest_paths, "`ql test`", false)?;
        let request_root = project_request_root.unwrap_or(path);
        let workspace_members = load_workspace_build_targets_for_command_from_request_root(
            path,
            request_root,
            "`ql test`",
        )?;
        let selected_members =
            select_project_build_plan_root_members(&workspace_members, &manifest_paths);
        prepare_project_dependency_builds(
            &workspace_members,
            &selected_members,
            "`ql test`",
            options,
            profile_overridden,
        )?;
        prepare_project_test_package_builds(
            &workspace_members,
            &selected_members,
            "`ql test`",
            options,
            profile_overridden,
        )?;
        Some(workspace_members)
    } else {
        None
    };

    let mut report = TestExecutionReport::default();

    for target in targets {
        if !json {
            print!("test {} ... ", target.display_path);
            let _ = std::io::stdout().flush();
        }

        match &target.kind {
            TestTargetKind::Smoke {
                source_path,
                working_directory,
                build_options,
                package_manifest_path,
            } => match if let (Some(workspace_members), Some(package_manifest_path)) =
                (workspace_members.as_ref(), package_manifest_path.as_ref())
            {
                if json {
                    build_project_test_source_target_quiet(
                        workspace_members,
                        "`ql test`",
                        package_manifest_path,
                        source_path,
                        build_options,
                        options,
                        profile_overridden,
                    )
                } else {
                    build_project_test_source_target_silent(
                        workspace_members,
                        "`ql test`",
                        package_manifest_path,
                        source_path,
                        build_options,
                        options,
                        profile_overridden,
                    )
                }
            } else if json {
                build_single_source_target_quiet(source_path, build_options, false)
            } else {
                build_single_source_target_silent(source_path, build_options, false)
            } {
                Ok(artifact) => match execute_test_binary(&artifact.path, working_directory) {
                    Ok((exit_code, _stdout, _stderr)) if exit_code == Some(0) => {
                        if !json {
                            println!("ok");
                        }
                        report.passed += 1;
                    }
                    Ok((exit_code, stdout, stderr)) => {
                        if !json {
                            println!("FAILED");
                        }
                        report.failed += 1;
                        report.failures.push(TestFailure::Run {
                            display_path: target.display_path.clone(),
                            exit_code,
                            stdout,
                            stderr,
                        });
                    }
                    Err(error) => {
                        if !json {
                            println!("FAILED");
                        }
                        report.failed += 1;
                        report.failures.push(TestFailure::Spawn {
                            display_path: target.display_path.clone(),
                            error,
                        });
                    }
                },
                Err(_) => {
                    if !json {
                        println!("FAILED");
                    }
                    report.failed += 1;
                    report.failures.push(TestFailure::Build {
                        display_path: target.display_path.clone(),
                    });
                }
            },
            TestTargetKind::Ui {
                source_path,
                diagnostic_path,
                snapshot_path,
            } => match execute_ui_test(source_path, diagnostic_path, snapshot_path) {
                Ok(()) => {
                    if !json {
                        println!("ok");
                    }
                    report.passed += 1;
                }
                Err(detail) => {
                    if !json {
                        println!("FAILED");
                    }
                    report.failed += 1;
                    report.failures.push(TestFailure::Ui {
                        display_path: target.display_path.clone(),
                        detail,
                    });
                }
            },
        }
    }

    if json {
        return Ok(report);
    }

    if report.failures.is_empty() {
        println!();
        println!("test result: ok. {} passed; 0 failed", report.passed);
        return Ok(report);
    }

    eprintln!();
    eprintln!("failures:");
    for failure in &report.failures {
        report_test_failure(failure);
    }
    eprintln!();
    eprintln!(
        "test result: FAILED. {} passed; {} failed",
        report.passed, report.failed
    );
    Ok(report)
}
