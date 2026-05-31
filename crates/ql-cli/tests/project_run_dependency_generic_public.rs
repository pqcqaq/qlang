mod support;

use ql_driver::{ToolchainOptions, discover_toolchain};
use serde_json::Value as JsonValue;
use support::project_run_dependencies::{
    expect_dependency_run_project_exits, write_dependency_run_project,
};
use support::{
    TempDir, executable_output_path, expect_empty_stderr, expect_exit_code, expect_file_exists,
    expect_silent_output, ql_command, run_command_capture, static_library_output_path,
    workspace_root,
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
fn run_package_path_supports_direct_dependency_generic_public_functions() {
    if !toolchain_available("`ql run` dependency generic public function test") {
        return;
    }

    let fixture = write_dependency_run_project(
        "ql-project-run-dependency-generic-public-function",
        r#"
pub fn identity[T](value: T) -> T {
    return value
}

pub fn first[T, N](values: [T; N]) -> T {
    return values[0]
}
"#,
        r#"
use dep.first as first
use dep.identity as identity

fn main() -> Int {
    let value: Int = identity(7)
    let picked: Int = first([5, 8, 13])
    return value + picked
}
"#,
    );
    expect_dependency_run_project_exits(
        "project-run-dependency-generic-public-function",
        "package path run with dependency generic public function",
        "`ql run` dependency generic public function",
        &fixture,
        12,
    );
}

#[test]
fn run_package_path_json_supports_direct_dependency_generic_public_functions() {
    if !toolchain_available("`ql run --json` dependency generic public function test") {
        return;
    }

    let fixture = write_dependency_run_project(
        "ql-project-run-json-dependency-generic-public-function",
        r#"
pub fn identity[T](value: T) -> T {
    return value
}

pub fn first[T, N](values: [T; N]) -> T {
    return values[0]
}
"#,
        r#"
use dep.first as first
use dep.identity as identity

fn main() -> Int {
    let value: Int = identity(7)
    let picked: Int = first([5, 8, 13])
    return value + picked
}
"#,
    );

    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["run"])
        .arg(&fixture.project_root)
        .arg("--json");
    let output = run_command_capture(&mut command, "`ql run --json` dependency generic function");
    let (stdout, stderr) = expect_exit_code(
        "project-run-json-dependency-generic-public-function",
        "package path run json with dependency generic public function",
        &output,
        12,
    )
    .expect("package-path `ql run --json` should preserve dependency generic program status");
    expect_empty_stderr(
        "project-run-json-dependency-generic-public-function",
        "package path run json with dependency generic public function",
        &stderr,
    )
    .expect("dependency generic run json should keep stderr empty");

    let json = parse_json_output(
        "project-run-json-dependency-generic-public-function",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.run.v1");
    assert_eq!(
        json["path"],
        fixture
            .project_root
            .display()
            .to_string()
            .replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        fixture
            .project_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    assert_eq!(json["requested_profile"], "debug");
    assert_eq!(json["profile_overridden"], false);
    assert_eq!(json["program_args"], serde_json::json!([]));
    assert_eq!(json["status"], "completed");
    assert_eq!(json["failure"], JsonValue::Null);
    assert_eq!(
        json["built_target"],
        serde_json::json!({
            "manifest_path": fixture.project_root.join("qlang.toml").display().to_string().replace('\\', "/"),
            "package_name": "app",
            "selected": true,
            "dependency_only": false,
            "kind": "bin",
            "path": "src/main.ql",
            "emit": "exe",
            "profile": "debug",
            "artifact_path": fixture.executable_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        })
    );
    assert_eq!(
        json["execution"],
        serde_json::json!({
            "exit_code": 12,
            "stdout": "",
            "stderr": "",
        })
    );
    expect_file_exists(
        "project-run-json-dependency-generic-public-function",
        &fixture.interface_output,
        "synced dependency interface",
        "package path run json with dependency generic public function",
    )
    .expect("dependency generic run json should emit the dependency interface");
    expect_file_exists(
        "project-run-json-dependency-generic-public-function",
        &fixture.dependency_output,
        "dependency package artifact",
        "package path run json with dependency generic public function",
    )
    .expect("dependency generic run json should build the dependency artifact");
    expect_file_exists(
        "project-run-json-dependency-generic-public-function",
        &fixture.executable_output,
        "package executable",
        "package path run json with dependency generic public function",
    )
    .expect("dependency generic run json should emit the executable artifact");
}

#[test]
fn run_package_path_supports_dependency_generic_wrapper_calling_imported_generic_helper() {
    if !toolchain_available("`ql run` dependency generic wrapper imported helper test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-dependency-generic-imported-helper");
    let helper_root = temp.path().join("helper");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(helper_root.join("src"))
        .expect("create helper source tree for imported generic helper run test");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dep source tree for imported generic helper run test");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for imported generic helper run test");

    temp.write(
        "helper/qlang.toml",
        r#"
[package]
name = "helper"
"#,
    );
    temp.write(
        "helper/src/lib.ql",
        r#"
pub fn reverse_array[T, N](values: [T; N]) -> [T; N] {
    var result = values
    var index = 0
    for value in values {
        result[index] = values[N - index - 1];
        index = index + 1
    }
    return result
}
"#,
    );
    temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"

[dependencies]
helper = "../helper"
"#,
    );
    temp.write(
        "dep/src/lib.ql",
        r#"
use helper.reverse_array as reverse_array

pub fn reverse_wrapped[T, N](values: [T; N]) -> [T; N] {
    return reverse_array(values)
}
"#,
    );
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    temp.write(
        "app/src/main.ql",
        r#"
use dep.reverse_wrapped as reverse_wrapped

fn main() -> Int {
    let reversed: [Int; 3] = reverse_wrapped([7, 8, 9])
    return reversed[0] + reversed[1] + reversed[2]
}
"#,
    );

    let helper_output = static_library_output_path(&helper_root.join("target/ql/debug"), "lib");
    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let interface_output = dep_root.join("dep.qi");
    let executable_output = executable_output_path(&project_root.join("target/ql/debug"), "main");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql run` dependency generic wrapper imported helper",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-run-dependency-generic-imported-helper",
        "package path run with dependency generic wrapper imported helper",
        &output,
        24,
    )
    .expect(
        "package-path `ql run` should specialize dependency wrappers and imported generic helpers",
    );
    expect_silent_output(
        "project-run-dependency-generic-imported-helper",
        "package path run with dependency generic wrapper imported helper",
        &stdout,
        &stderr,
    )
    .expect("imported generic helper run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-dependency-generic-imported-helper",
        &helper_output,
        "transitive helper package artifact",
        "package path run with dependency generic wrapper imported helper",
    )
    .expect("imported generic helper run should build the transitive helper artifact");
    expect_file_exists(
        "project-run-dependency-generic-imported-helper",
        &dep_output,
        "dependency package artifact",
        "package path run with dependency generic wrapper imported helper",
    )
    .expect("imported generic helper run should build the dependency artifact");
    expect_file_exists(
        "project-run-dependency-generic-imported-helper",
        &interface_output,
        "synced dependency interface",
        "package path run with dependency generic wrapper imported helper",
    )
    .expect("imported generic helper run should emit the dependency interface");
    expect_file_exists(
        "project-run-dependency-generic-imported-helper",
        &executable_output,
        "package executable",
        "package path run with dependency generic wrapper imported helper",
    )
    .expect("imported generic helper run should emit the executable artifact");
}
