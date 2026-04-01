use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use ql_driver::{ToolchainOptions, discover_toolchain};

#[test]
fn executable_examples_build_and_run() {
    let workspace_root = workspace_root();
    let examples_root = workspace_root.join("ramdon_tests/executable_examples");
    assert!(
        examples_root.is_dir(),
        "expected sync executable examples under `{}`",
        examples_root.display()
    );

    if !toolchain_available("sync executable example test") {
        return;
    }

    let cases = [
        ExecutableExampleCase {
            name: "sync_minimal",
            source_relative: "ramdon_tests/executable_examples/01_sync_minimal.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_data_shapes",
            source_relative: "ramdon_tests/executable_examples/02_sync_data_shapes.ql",
            expected_exit: 32,
        },
        ExecutableExampleCase {
            name: "sync_extern_c_export",
            source_relative: "ramdon_tests/executable_examples/03_sync_extern_c_export.ql",
            expected_exit: 42,
        },
    ];

    assert_example_cases_run(
        &workspace_root,
        &cases,
        "sync executable example regressions",
    );
}

#[test]
fn async_program_surface_examples_build_and_run() {
    let workspace_root = workspace_root();
    let examples_root = workspace_root.join("ramdon_tests/async_program_surface_examples");
    assert!(
        examples_root.is_dir(),
        "expected async program examples under `{}`",
        examples_root.display()
    );

    if !toolchain_available("async executable example test") {
        return;
    }

    let cases = [
        ExecutableExampleCase {
            name: "async_main_basics",
            source_relative: "ramdon_tests/async_program_surface_examples/04_async_main_basics.ql",
            expected_exit: 28,
        },
        ExecutableExampleCase {
            name: "async_main_aggregates_and_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/05_async_main_aggregates_and_for_await.ql",
            expected_exit: 71,
        },
        ExecutableExampleCase {
            name: "async_main_task_handle_payloads",
            source_relative: "ramdon_tests/async_program_surface_examples/06_async_main_task_handle_payloads.ql",
            expected_exit: 39,
        },
        ExecutableExampleCase {
            name: "async_main_projection_reinit",
            source_relative: "ramdon_tests/async_program_surface_examples/07_async_main_projection_reinit.ql",
            expected_exit: 45,
        },
        ExecutableExampleCase {
            name: "async_main_dynamic_task_arrays",
            source_relative: "ramdon_tests/async_program_surface_examples/08_async_main_dynamic_task_arrays.ql",
            expected_exit: 70,
        },
        ExecutableExampleCase {
            name: "async_main_zero_sized",
            source_relative: "ramdon_tests/async_program_surface_examples/09_async_main_zero_sized.ql",
            expected_exit: 10,
        },
    ];

    assert_example_cases_run(
        &workspace_root,
        &cases,
        "async executable example regressions",
    );
}

fn toolchain_available(context: &str) -> bool {
    let Ok(_toolchain) = discover_toolchain(&ToolchainOptions::default()) else {
        eprintln!(
            "skipping {context}: no clang-style compiler found via ql-driver toolchain discovery"
        );
        return false;
    };
    true
}

fn assert_example_cases_run(
    workspace_root: &Path,
    cases: &[ExecutableExampleCase],
    failure_header: &str,
) {
    let mut failures = Vec::new();
    for case in cases {
        if let Err(message) = run_executable_example_case(workspace_root, case) {
            failures.push(message);
        }
    }

    assert!(
        failures.is_empty(),
        "{failure_header}:\n\n{}",
        failures.join("\n\n")
    );
}

#[derive(Clone, Copy)]
struct ExecutableExampleCase {
    name: &'static str,
    source_relative: &'static str,
    expected_exit: i32,
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!("{prefix}-{unique}"));
        fs::create_dir_all(&path).expect("create temporary executable example test directory");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn run_executable_example_case(
    workspace_root: &Path,
    case: &ExecutableExampleCase,
) -> Result<(), String> {
    let temp = TempDir::new(&format!("ql-executable-example-{}", case.name));
    let output_path = executable_output_path(temp.path());

    let mut build = Command::new(env!("CARGO_BIN_EXE_ql"));
    build.current_dir(workspace_root).args([
        "build",
        case.source_relative,
        "--emit",
        "exe",
        "--output",
        &output_path.to_string_lossy(),
    ]);
    let build_output = build.output().unwrap_or_else(|_| {
        panic!(
            "run `ql build {} --emit exe --output {}`",
            case.source_relative,
            output_path.display()
        )
    });

    let build_stdout = normalize(&String::from_utf8_lossy(&build_output.stdout));
    let build_stderr = normalize(&String::from_utf8_lossy(&build_output.stderr));

    if build_output.status.code().is_none_or(|code| code != 0) {
        return Err(format!(
            "[{}] expected build exit code 0, got {:?}\nstdout:\n{}\nstderr:\n{}",
            case.name,
            build_output.status.code(),
            build_stdout,
            build_stderr
        ));
    }

    let expected_build_stdout = format!("wrote executable: {}", output_path.display());
    if build_stdout.trim() != expected_build_stdout || !build_stderr.trim().is_empty() {
        return Err(format!(
            "[{}] unexpected successful build output\n--- expected stdout ---\n{}\n--- actual stdout ---\n{}\n--- stderr ---\n{}",
            case.name, expected_build_stdout, build_stdout, build_stderr
        ));
    }

    if !output_path.is_file() {
        return Err(format!(
            "[{}] expected built executable at `{}`",
            case.name,
            output_path.display()
        ));
    }

    let run_output = Command::new(&output_path)
        .current_dir(workspace_root)
        .output()
        .unwrap_or_else(|_| panic!("run built executable `{}`", output_path.display()));
    let run_stdout = normalize(&String::from_utf8_lossy(&run_output.stdout));
    let run_stderr = normalize(&String::from_utf8_lossy(&run_output.stderr));

    if run_output.status.code() != Some(case.expected_exit) {
        return Err(format!(
            "[{}] expected runtime exit code {}, got {:?}\nstdout:\n{}\nstderr:\n{}",
            case.name,
            case.expected_exit,
            run_output.status.code(),
            run_stdout,
            run_stderr
        ));
    }

    if !run_stdout.trim().is_empty() || !run_stderr.trim().is_empty() {
        return Err(format!(
            "[{}] expected executable to be silent\nstdout:\n{}\nstderr:\n{}",
            case.name, run_stdout, run_stderr
        ));
    }

    Ok(())
}

fn workspace_root() -> PathBuf {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let crates_dir = crate_dir
        .parent()
        .expect("ql-cli crate should have a parent directory");
    crates_dir
        .parent()
        .expect("workspace root should exist")
        .to_path_buf()
}

fn executable_output_path(root: &Path) -> PathBuf {
    root.join(if cfg!(windows) {
        "artifact.exe"
    } else {
        "artifact"
    })
}

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
}
