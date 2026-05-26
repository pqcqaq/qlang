use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use ql_diagnostics::render_diagnostics;
use ql_driver::{BuildOptions, acquire_build_output_locks};

use crate::cli_analysis::analyze_source;
use crate::cli_utils::normalize_path;
use crate::project_reference_interfaces::prepare_reference_interfaces_for_manifests;
use crate::project_targets::load_workspace_build_targets_for_command_from_request_root;
use crate::{
    TestExecutionReport, TestFailure, TestTarget, TestTargetKind, build_output_lock_error_message,
    build_project_test_source_target_quiet, build_project_test_source_target_silent,
    build_single_source_target_quiet, build_single_source_target_silent,
    prepare_project_dependency_builds, prepare_project_test_package_builds,
    select_project_build_plan_root_members,
};

pub(crate) fn list_test_targets(targets: &[TestTarget]) {
    for target in targets {
        println!("{}", target.display_path);
    }
    println!();
    println!("test listing: {} discovered", targets.len());
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

fn execute_test_binary(
    executable_path: &Path,
    working_directory: &Path,
) -> Result<(Option<i32>, String, String), String> {
    let _execution_lock = acquire_build_output_locks(vec![executable_path.to_path_buf()])
        .map_err(build_output_lock_error_message)?;
    let output = Command::new(executable_path)
        .current_dir(working_directory)
        .output()
        .map_err(|error| {
            format!(
                "failed to run `{}`: {error}",
                normalize_path(executable_path)
            )
        })?;
    Ok((
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    ))
}

fn execute_ui_test(
    source_path: &Path,
    diagnostic_path: &Path,
    snapshot_path: &Path,
) -> Result<(), String> {
    let expected = fs::read_to_string(snapshot_path).map_err(|error| {
        format!(
            "reason: failed to read expected stderr snapshot `{}`: {error}",
            normalize_path(snapshot_path)
        )
    })?;
    let source = fs::read_to_string(source_path).map_err(|error| {
        format!(
            "reason: failed to read ui test source `{}`: {error}",
            normalize_path(source_path)
        )
    })?;
    let expected = normalize_output_text(&expected);
    let actual = match analyze_source(&source) {
        Ok(()) => {
            return Err(
                "reason: ui test expected diagnostics, but the source analyzed successfully"
                    .to_owned(),
            );
        }
        Err(diagnostics) => normalize_output_text(&render_diagnostics(
            Path::new(&normalize_path(diagnostic_path)),
            &source,
            &diagnostics,
        )),
    };

    if actual != expected {
        return Err(format!(
            "reason: ui stderr snapshot mismatch\n--- expected ---\n{expected}\n--- actual ---\n{actual}"
        ));
    }

    Ok(())
}

fn report_test_failure(failure: &TestFailure) {
    match failure {
        TestFailure::Build { display_path } => {
            eprintln!("  {display_path}");
            eprintln!("    reason: test failed to build");
        }
        TestFailure::Run {
            display_path,
            exit_code,
            stdout,
            stderr,
        } => {
            eprintln!("  {display_path}");
            match exit_code {
                Some(code) => eprintln!("    reason: test process exited with code {code}"),
                None => eprintln!("    reason: test process terminated without an exit code"),
            }
            if !stdout.trim().is_empty() {
                eprintln!("    stdout:");
                for line in stdout.lines() {
                    eprintln!("      {line}");
                }
            }
            if !stderr.trim().is_empty() {
                eprintln!("    stderr:");
                for line in stderr.lines() {
                    eprintln!("      {line}");
                }
            }
        }
        TestFailure::Spawn {
            display_path,
            error,
        } => {
            eprintln!("  {display_path}");
            eprintln!("    reason: {error}");
        }
        TestFailure::Ui {
            display_path,
            detail,
        } => {
            eprintln!("  {display_path}");
            for line in detail.lines() {
                eprintln!("    {line}");
            }
        }
    }
}

fn test_target_manifest_paths(targets: &[TestTarget]) -> Vec<PathBuf> {
    let mut manifest_paths = Vec::new();
    for target in targets {
        let TestTargetKind::Smoke {
            package_manifest_path: Some(manifest_path),
            ..
        } = &target.kind
        else {
            continue;
        };
        if !manifest_paths.contains(manifest_path) {
            manifest_paths.push(manifest_path.clone());
        }
    }
    manifest_paths
}

fn normalize_output_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}
