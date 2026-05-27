use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use ql_diagnostics::render_diagnostics;
use ql_driver::acquire_build_output_locks;

use crate::build_single_source_reporting::build_output_lock_error_message;
use crate::cli_analysis::analyze_source;
use crate::cli_utils::normalize_path;
use crate::test_reporting::{TestFailure, TestTarget, TestTargetKind};

pub(crate) fn execute_test_binary(
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

pub(crate) fn execute_ui_test(
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

pub(crate) fn report_test_failure(failure: &TestFailure) {
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

pub(crate) fn test_target_manifest_paths(targets: &[TestTarget]) -> Vec<PathBuf> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_path_collection_deduplicates_project_smoke_targets() {
        let manifest = PathBuf::from("workspace/app/qlang.toml");
        let targets = [
            TestTarget {
                display_path: "tests/one.ql".to_owned(),
                kind: TestTargetKind::Smoke {
                    source_path: PathBuf::from("workspace/app/tests/one.ql"),
                    working_directory: PathBuf::from("workspace/app"),
                    build_options: ql_driver::BuildOptions::default(),
                    package_manifest_path: Some(manifest.clone()),
                },
            },
            TestTarget {
                display_path: "tests/two.ql".to_owned(),
                kind: TestTargetKind::Smoke {
                    source_path: PathBuf::from("workspace/app/tests/two.ql"),
                    working_directory: PathBuf::from("workspace/app"),
                    build_options: ql_driver::BuildOptions::default(),
                    package_manifest_path: Some(manifest.clone()),
                },
            },
        ];

        assert_eq!(test_target_manifest_paths(&targets), vec![manifest]);
    }
}
