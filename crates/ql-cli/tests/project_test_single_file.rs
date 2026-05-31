mod support;

use ql_driver::{ToolchainOptions, discover_toolchain};
use serde_json::Value as JsonValue;
use support::{
    TempDir, executable_output_path, expect_empty_stderr, expect_file_exists,
    expect_stdout_contains_all, expect_success, ql_command, run_command_capture, workspace_root,
};

fn toolchain_available(context: &str) -> bool {
    let Ok(_toolchain) = discover_toolchain(&ToolchainOptions::default()) else {
        eprintln!(
            "skipping {context}: no clang-style compiler found via ql-driver toolchain discovery"
        );
        return false;
    };
    true
}

fn normalize_output_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&normalize_output_text(stdout))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
}

#[test]
fn test_single_file_runs_as_smoke_test() {
    if !toolchain_available("`ql test` single-file test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-file");
    let source_path = temp.write("smoke.ql", "fn main() -> Int { return 0 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&source_path);
    let output = run_command_capture(&mut command, "`ql test` single file");
    let (stdout, stderr) = expect_success("project-test-file", "single-file smoke test", &output)
        .expect("single-file `ql test` should succeed");
    expect_empty_stderr("project-test-file", "single-file smoke test", &stderr)
        .expect("single-file `ql test` should not print stderr");
    expect_stdout_contains_all(
        "project-test-file",
        &stdout.replace('\\', "/"),
        &[
            &format!("test {} ... ok", source_path.display()).replace('\\', "/"),
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("single-file `ql test` should report one passing smoke test");
}

#[test]
fn test_single_file_runs_local_generic_function_instantiation() {
    if !toolchain_available("`ql test` single-file local generic function test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-file-local-generic");
    let source_path = temp.write(
        "smoke.ql",
        r#"
fn first[T, N](values: [T; N]) -> T {
    return values[0]
}

fn len[T, N](values: [T; N]) -> Int {
    return N
}

fn main() -> Int {
    return first([10, 20, 30]) + len([1, 2, 3, 4]) - 14
}
"#,
    );
    let output_path = executable_output_path(&temp.path().join("target/ql/debug"), "smoke");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&source_path);
    let output = run_command_capture(&mut command, "`ql test` single-file local generic function");
    let (stdout, stderr) = expect_success(
        "project-test-file-local-generic",
        "single-file local generic function smoke test",
        &output,
    )
    .expect("single-file `ql test` should execute local generic function instantiations");
    expect_empty_stderr(
        "project-test-file-local-generic",
        "single-file local generic function smoke test",
        &stderr,
    )
    .expect("single-file local generic smoke test should not print stderr");
    expect_stdout_contains_all(
        "project-test-file-local-generic",
        &stdout.replace('\\', "/"),
        &[
            &format!("test {} ... ok", source_path.display()).replace('\\', "/"),
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("single-file local generic smoke test should report one passing test");
    expect_file_exists(
        "project-test-file-local-generic",
        &output_path,
        "single-file local generic smoke executable",
        "single-file local generic smoke test",
    )
    .expect("single-file local generic smoke test should leave the executable");
}

#[test]
fn test_single_file_json_reports_local_generic_function_instantiation_success() {
    if !toolchain_available("`ql test --json` single-file local generic function test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-file-local-generic-json");
    let source_path = temp.write(
        "smoke.ql",
        r#"
fn first[T, N](values: [T; N]) -> T {
    return values[0]
}

fn len[T, N](values: [T; N]) -> Int {
    return N
}

fn main() -> Int {
    return first([10, 20, 30]) + len([1, 2, 3, 4]) - 14
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test", "--json"]).arg(&source_path);
    let output = run_command_capture(
        &mut command,
        "`ql test --json` single-file local generic function",
    );
    let (stdout, stderr) = expect_success(
        "project-test-file-local-generic-json",
        "single-file local generic function json test",
        &output,
    )
    .expect("single-file `ql test --json` should execute local generic function instantiations");
    expect_empty_stderr(
        "project-test-file-local-generic-json",
        "single-file local generic function json test",
        &stderr,
    )
    .expect("single-file local generic json test should not print stderr");

    let actual = parse_json_output("project-test-file-local-generic-json", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.test.v1",
        "path": source_path.display().to_string().replace('\\', "/"),
        "requested_profile": "debug",
        "profile_overridden": false,
        "package_name": JsonValue::Null,
        "filter": JsonValue::Null,
        "list_only": false,
        "status": "ok",
        "discovered_total": 1,
        "selected_total": 1,
        "targets": [
            {
                "path": source_path.display().to_string().replace('\\', "/"),
                "kind": "smoke",
                "profile": "debug",
            }
        ],
        "passed": 1,
        "failed": 0,
        "failures": [],
    });
    assert_eq!(
        actual, expected,
        "single-file `ql test --json` should report the stable local generic success contract"
    );
}
