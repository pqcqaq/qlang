mod support;

use ql_driver::{ToolchainOptions, discover_toolchain};
use serde_json::Value as JsonValue;
use support::{
    TempDir, executable_output_path, expect_empty_stderr, expect_exit_code, expect_file_exists,
    expect_silent_output, ql_command, run_command_capture, workspace_root,
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
fn run_single_file_builds_and_executes_program() {
    if !toolchain_available("`ql run` single-file test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-file");
    let source_path = temp.write("demo.ql", "fn main() -> Int { return 7 }\n");
    let output_path = executable_output_path(&temp.path().join("target/ql/debug"), "demo");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&source_path);
    let output = run_command_capture(&mut command, "`ql run` single file");
    let (stdout, stderr) = expect_exit_code("project-run-file", "single-file run", &output, 7)
        .expect("single-file `ql run` should exit with the program status");
    expect_silent_output("project-run-file", "single-file run", &stdout, &stderr)
        .expect("single-file `ql run` should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-file",
        &output_path,
        "single-file executable",
        "single-file run",
    )
    .expect("single-file `ql run` should leave the built executable in the default path");
}

#[test]
fn run_single_file_supports_json_output() {
    if !toolchain_available("`ql run --json` single-file test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-file-json");
    let source_path = temp.write("demo.ql", "fn main() -> Int { return 7 }\n");
    let output_path = executable_output_path(&temp.path().join("target/ql/debug"), "demo");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&source_path).arg("--json");
    let output = run_command_capture(&mut command, "`ql run --json` single file");
    let (stdout, stderr) =
        expect_exit_code("project-run-file-json", "single-file run json", &output, 7)
            .expect("single-file `ql run --json` should preserve the program exit status");
    expect_empty_stderr("project-run-file-json", "single-file run json", &stderr)
        .expect("single-file `ql run --json` should keep stderr empty");

    let json = parse_json_output("project-run-file-json", &stdout);
    assert_eq!(json["schema"], "ql.run.v1");
    assert_eq!(
        json["path"],
        source_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "file");
    assert_eq!(json["project_manifest_path"], JsonValue::Null);
    assert_eq!(json["requested_profile"], "debug");
    assert_eq!(json["profile_overridden"], false);
    assert_eq!(json["program_args"], serde_json::json!([]));
    assert_eq!(json["status"], "completed");
    assert_eq!(json["failure"], JsonValue::Null);
    assert_eq!(
        json["built_target"],
        serde_json::json!({
            "manifest_path": JsonValue::Null,
            "package_name": JsonValue::Null,
            "selected": true,
            "dependency_only": false,
            "kind": "source",
            "path": source_path.display().to_string().replace('\\', "/"),
            "emit": "exe",
            "profile": "debug",
            "artifact_path": output_path.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        })
    );
    assert_eq!(
        json["execution"],
        serde_json::json!({
            "exit_code": 7,
            "stdout": "",
            "stderr": "",
        })
    );
    expect_file_exists(
        "project-run-file-json",
        &output_path,
        "single-file run json executable",
        "single-file run json",
    )
    .expect(
        "single-file `ql run --json` should still leave the built executable in the default path",
    );
}

#[test]
fn run_single_file_executes_local_generic_function_instantiation() {
    if !toolchain_available("`ql run` single-file local generic function test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-file-local-generic");
    let source_path = temp.write(
        "demo.ql",
        r#"
fn first[T, N](values: [T; N]) -> T {
    return values[0]
}

fn len[T, N](values: [T; N]) -> Int {
    return N
}

fn main() -> Int {
    return first([10, 20, 30]) + len([1, 2, 3, 4])
}
"#,
    );
    let output_path = executable_output_path(&temp.path().join("target/ql/debug"), "demo");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&source_path);
    let output = run_command_capture(&mut command, "`ql run` single-file local generic function");
    let (stdout, stderr) = expect_exit_code(
        "project-run-file-local-generic",
        "single-file local generic function run",
        &output,
        14,
    )
    .expect("single-file `ql run` should execute local generic function instantiations");
    expect_silent_output(
        "project-run-file-local-generic",
        "single-file local generic function run",
        &stdout,
        &stderr,
    )
    .expect("single-file local generic function run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-file-local-generic",
        &output_path,
        "single-file local generic function executable",
        "single-file local generic function run",
    )
    .expect("single-file local generic function run should leave the built executable");
}

#[test]
fn run_single_file_supports_local_receiver_methods() {
    if !toolchain_available("`ql run` local receiver method test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-file-local-receiver-method");
    let source_path = temp.write(
        "demo.ql",
        "struct Box { value: Int }\n\nimpl Box {\n    fn read(self) -> Int {\n        return self.value\n    }\n}\n\nfn main() -> Int {\n    let value = Box { value: 7 }\n    return value.read()\n}\n",
    );
    let output_path = executable_output_path(&temp.path().join("target/ql/debug"), "demo");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&source_path);
    let output = run_command_capture(&mut command, "`ql run` local receiver method");
    let (stdout, stderr) = expect_exit_code(
        "project-run-file-local-receiver-method",
        "single-file local receiver method run",
        &output,
        7,
    )
    .expect("single-file `ql run` should execute local receiver methods");
    expect_silent_output(
        "project-run-file-local-receiver-method",
        "single-file local receiver method run",
        &stdout,
        &stderr,
    )
    .expect("single-file `ql run` local receiver method should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-file-local-receiver-method",
        &output_path,
        "single-file local receiver method executable",
        "single-file local receiver method run",
    )
    .expect(
        "single-file `ql run` local receiver method should leave the built executable in the default path",
    );
}

#[test]
fn run_single_file_supports_local_method_value_calls() {
    if !toolchain_available("`ql run` local method value test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-file-local-method-value");
    let source_path = temp.write(
        "demo.ql",
        "struct Box { value: Int }\n\nimpl Box {\n    fn add(self, delta: Int) -> Int {\n        return self.value + delta\n    }\n}\n\nfn main() -> Int {\n    let value = Box { value: 7 }\n    let add = value.add\n    return add(5)\n}\n",
    );
    let output_path = executable_output_path(&temp.path().join("target/ql/debug"), "demo");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&source_path);
    let output = run_command_capture(&mut command, "`ql run` local method value");
    let (stdout, stderr) = expect_exit_code(
        "project-run-file-local-method-value",
        "single-file local method value run",
        &output,
        12,
    )
    .expect("single-file `ql run` should execute local method values");
    expect_silent_output(
        "project-run-file-local-method-value",
        "single-file local method value run",
        &stdout,
        &stderr,
    )
    .expect("single-file `ql run` local method value should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-file-local-method-value",
        &output_path,
        "single-file local method value executable",
        "single-file local method value run",
    )
    .expect(
        "single-file `ql run` local method value should leave the built executable in the default path",
    );
}
