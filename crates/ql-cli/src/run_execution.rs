use std::io::Write;
use std::path::Path;
use std::process::Command;

use ql_driver::acquire_build_output_locks;

use crate::build_single_source_reporting::build_output_lock_error_message;
use crate::cli_utils::normalize_path;
use crate::run_reporting::RunJsonReport;

pub(crate) fn run_built_executable(
    executable_path: &Path,
    program_args: &[String],
) -> Result<(), u8> {
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    let execution_lock =
        acquire_build_output_locks(vec![executable_path.to_path_buf()]).map_err(|error| {
            eprintln!(
                "error: failed to lock built executable `{}`: {}",
                normalize_path(executable_path),
                build_output_lock_error_message(error)
            );
            1
        })?;
    let mut command = Command::new(executable_path);
    command.args(program_args);
    let status = command.status().map_err(|error| {
        eprintln!(
            "error: failed to run built executable `{}`: {error}",
            normalize_path(executable_path)
        );
        1
    })?;

    match status.code() {
        Some(0) => Ok(()),
        Some(code) => {
            drop(execution_lock);
            std::process::exit(code);
        }
        None => {
            eprintln!(
                "error: built executable `{}` terminated without an exit code",
                normalize_path(executable_path)
            );
            Err(1)
        }
    }
}

#[derive(Clone, Debug)]
struct CapturedExecutableRun {
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn run_built_executable_capture(
    executable_path: &Path,
    program_args: &[String],
) -> Result<CapturedExecutableRun, String> {
    let _execution_lock = acquire_build_output_locks(vec![executable_path.to_path_buf()])
        .map_err(build_output_lock_error_message)?;
    let mut command = Command::new(executable_path);
    command.args(program_args);
    let output = command.output().map_err(|error| {
        format!(
            "failed to run built executable `{}`: {error}",
            normalize_path(executable_path)
        )
    })?;

    Ok(CapturedExecutableRun {
        exit_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

pub(crate) fn emit_run_json_execution(
    mut report: RunJsonReport,
    executable_path: &Path,
    program_args: &[String],
) -> Result<(), u8> {
    match run_built_executable_capture(executable_path, program_args) {
        Ok(captured) => {
            if let Some(exit_code) = captured.exit_code {
                report.record_execution(exit_code, &captured.stdout, &captured.stderr);
                print!("{}", report.into_json());
                if exit_code == 0 {
                    Ok(())
                } else {
                    std::process::exit(exit_code);
                }
            } else {
                report.record_run_failure(
                    executable_path,
                    &format!(
                        "built executable `{}` terminated without an exit code",
                        normalize_path(executable_path)
                    ),
                    &captured.stdout,
                    &captured.stderr,
                );
                print!("{}", report.into_json());
                Err(1)
            }
        }
        Err(error) => {
            report.record_spawn_failure(executable_path, error);
            print!("{}", report.into_json());
            Err(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executable_capture_reports_spawn_failures_with_normalized_path() {
        let missing = Path::new("target/ql/missing-run-target");
        let error = run_built_executable_capture(missing, &[])
            .expect_err("missing executable should fail before capture");

        assert!(error.contains("failed to run built executable `target/ql/missing-run-target`"));
    }
}
