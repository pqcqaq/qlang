mod support;

use std::fs;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

use ql_driver::{ToolchainOptions, discover_toolchain};
use serde_json::Value as JsonValue;
use support::{
    TempDir, assert_no_build_lock_directories, executable_output_path, expect_empty_stderr,
    expect_exit_code, expect_file_exists, expect_stderr_contains, expect_stdout_contains_all,
    expect_success, ql_command, run_command_capture, sleep_program_source,
    static_library_output_path, wait_for_path_exists, workspace_root,
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

struct WorkspacePackageUnderTestGenericBridgeProject {
    temp: TempDir,
    project_root: PathBuf,
    app_library_output: PathBuf,
    tool_library_output: PathBuf,
    app_smoke_output: PathBuf,
    tool_smoke_output: PathBuf,
}

fn write_workspace_package_under_test_generic_bridge_project(
    prefix: &str,
) -> WorkspacePackageUnderTestGenericBridgeProject {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("workspace");
    fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app package source tree");
    fs::create_dir_all(project_root.join("packages/tool/src"))
        .expect("create tool package source tree");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        r#"
pub fn identity[T](value: T) -> T {
    return value
}

pub fn first[T, N](values: [T; N]) -> T {
    return values[0]
}
"#,
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        r#"
pub fn identity[T](value: T) -> T {
    return value
}
"#,
    );
    temp.write(
        "workspace/packages/app/tests/smoke.ql",
        r#"
use app.identity as identity
use app.first as first

fn echo[T](value: T) -> T {
    return value
}

fn main() -> Int {
    let value: Int = echo(identity(7))
    let picked: Int = first([3, 4, 5])
    return value + picked - 10
}
"#,
    );
    temp.write(
        "workspace/packages/tool/tests/smoke.ql",
        r#"
use tool.identity as identity

fn echo[T](value: T) -> T {
    return value
}

fn main() -> Int {
    let enabled: Bool = echo(identity(true))
    if enabled {
        return 0
    }
    return 1
}
"#,
    );

    let app_library_output =
        static_library_output_path(&project_root.join("packages/app/target/ql/debug"), "lib");
    let tool_library_output =
        static_library_output_path(&project_root.join("packages/tool/target/ql/debug"), "lib");
    let app_smoke_output = executable_output_path(
        &project_root.join("packages/app/target/ql/debug/tests"),
        "smoke",
    );
    let tool_smoke_output = executable_output_path(
        &project_root.join("packages/tool/target/ql/debug/tests"),
        "smoke",
    );

    WorkspacePackageUnderTestGenericBridgeProject {
        temp,
        project_root,
        app_library_output,
        tool_library_output,
        app_smoke_output,
        tool_smoke_output,
    }
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

#[test]
fn test_package_path_runs_discovered_tests() {
    if !toolchain_available("`ql test` package test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source root");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("app/tests/smoke.ql", "fn main() -> Int { return 0 }\n");
    temp.write("app/tests/api/basic.ql", "fn main() -> Int { return 0 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test` package path");
    let (stdout, stderr) = expect_success("project-test-package", "package smoke tests", &output)
        .expect("package-path `ql test` should succeed");
    expect_empty_stderr("project-test-package", "package smoke tests", &stderr)
        .expect("package-path `ql test` should not print stderr");
    expect_stdout_contains_all(
        "project-test-package",
        &stdout.replace('\\', "/"),
        &[
            "test tests/api/basic.ql ... ok",
            "test tests/smoke.ql ... ok",
            "test result: ok. 2 passed; 0 failed",
        ],
    )
    .expect("package-path `ql test` should run all discovered tests");
}

#[test]
fn test_package_tests_can_import_current_package_public_functions() {
    if !toolchain_available("`ql test` current package public function test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-current-package-function");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source root");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "app/src/lib.ql",
        "pub fn pass_status(value: Int) -> Int {\n    if value == 42 {\n        return 0\n    }\n    return 1\n}\n",
    );
    temp.write(
        "app/tests/smoke.ql",
        "use app.pass_status as pass\n\nfn main() -> Int {\n    return pass(42)\n}\n",
    );

    let package_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");
    let smoke_output = executable_output_path(&project_root.join("target/ql/debug/tests"), "smoke");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test` current package public function");
    let (stdout, stderr) = expect_success(
        "project-test-current-package-function",
        "package tests importing current package public function",
        &output,
    )
    .expect(
        "package-path `ql test` should let smoke tests import current package public functions",
    );
    expect_empty_stderr(
        "project-test-current-package-function",
        "package tests importing current package public function",
        &stderr,
    )
    .expect("current package public function tests should not print stderr");
    expect_stdout_contains_all(
        "project-test-current-package-function",
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("package-path `ql test` should report the current-package smoke test");
    expect_file_exists(
        "project-test-current-package-function",
        &package_output,
        "current package library",
        "`ql test` current package public function",
    )
    .expect("current package library should be prebuilt for tests");
    expect_file_exists(
        "project-test-current-package-function",
        &smoke_output,
        "current package test executable",
        "`ql test` current package public function",
    )
    .expect("current package test executable should be emitted");
}

#[test]
fn test_package_tests_support_current_package_generic_public_function_single_instantiation() {
    if !toolchain_available("`ql test` current package generic public function test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-current-package-generic-function");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source root for generic function import test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "app/src/lib.ql",
        r#"
pub fn identity[T](value: T) -> T {
    return value
}
"#,
    );
    temp.write(
        "app/tests/smoke.ql",
        r#"
use app.identity as identity

fn check(value: Int) -> Int {
    return identity(value)
}

fn main() -> Int {
    let value: Int = 42
    if check(value) == 42 {
        return 0
    }
    return 1
}
"#,
    );

    let package_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");
    let smoke_output = executable_output_path(&project_root.join("target/ql/debug/tests"), "smoke");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql test` current package generic public function",
    );
    let (stdout, stderr) = expect_success(
        "project-test-current-package-generic-function",
        "package tests importing current package generic public function",
        &output,
    )
    .expect(
        "package-path `ql test` should support current package generic public function imports with a single concrete instantiation",
    );
    expect_empty_stderr(
        "project-test-current-package-generic-function",
        "package tests importing current package generic public function",
        &stderr,
    )
    .expect("generic function package test should not print stderr");
    expect_stdout_contains_all(
        "project-test-current-package-generic-function",
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("generic function package test should report a passing smoke test");
    expect_file_exists(
        "project-test-current-package-generic-function",
        &package_output,
        "current package library",
        "`ql test` current package generic public function",
    )
    .expect("current package library should build before the test bridge runs");
    expect_file_exists(
        "project-test-current-package-generic-function",
        &smoke_output,
        "current package generic function test executable",
        "`ql test` current package generic public function",
    )
    .expect("generic function package test should emit the smoke test executable");
}

#[test]
fn test_package_tests_support_current_package_generic_public_function_multiple_instantiations() {
    if !toolchain_available(
        "`ql test` current package multi-instantiation generic public function test",
    ) {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-current-package-multiple-generic-instantiations");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source root for multiple generic function instantiations");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "app/src/lib.ql",
        r#"
pub fn identity[T](value: T) -> T {
    return value
}
"#,
    );
    temp.write(
        "app/tests/smoke.ql",
        r#"
use app.identity as identity

fn bool_score(value: Bool) -> Int {
    if value {
        return 5
    }
    return 0
}

fn main() -> Int {
    let number: Int = identity(7)
    let flag: Bool = identity(true)
    if number + bool_score(flag) == 12 {
        return 0
    }
    return 1
}
"#,
    );

    let package_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");
    let smoke_output = executable_output_path(&project_root.join("target/ql/debug/tests"), "smoke");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql test` current package multiple generic public function instantiations",
    );
    let (stdout, stderr) = expect_success(
        "project-test-current-package-multiple-generic-instantiations",
        "package tests importing current package generic function with multiple concrete instantiations",
        &output,
    )
    .expect("package-path `ql test` should support multiple current package generic function instantiations");
    expect_empty_stderr(
        "project-test-current-package-multiple-generic-instantiations",
        "package tests importing current package generic function with multiple concrete instantiations",
        &stderr,
    )
    .expect("multiple generic function package test should not print stderr");
    expect_stdout_contains_all(
        "project-test-current-package-multiple-generic-instantiations",
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("multiple generic function package test should report a passing smoke test");
    expect_file_exists(
        "project-test-current-package-multiple-generic-instantiations",
        &package_output,
        "current package library",
        "`ql test` current package multiple generic public function instantiations",
    )
    .expect("current package library should build before the multiple generic test bridge runs");
    expect_file_exists(
        "project-test-current-package-multiple-generic-instantiations",
        &smoke_output,
        "current package multiple generic function test executable",
        "`ql test` current package multiple generic public function instantiations",
    )
    .expect("multiple generic function package test should emit the smoke test executable");
}

#[test]
fn test_package_tests_support_current_package_generic_function_named_arguments() {
    if !toolchain_available("`ql test` current package generic function named arguments") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-current-package-generic-function-named-args");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source root for generic function named-argument test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "app/src/lib.ql",
        r#"
pub fn choose[T](fallback: T, value: T) -> T {
    return value
}
"#,
    );
    temp.write(
        "app/tests/smoke.ql",
        r#"
use app.choose as choose

fn bool_score(value: Bool) -> Int {
    if value {
        return 5
    }
    return 0
}

fn main() -> Int {
    let number: Int = choose(value: 7, fallback: 0)
    let flag: Bool = choose(value: true, fallback: false)
    if number + bool_score(flag) == 12 {
        return 0
    }
    return 1
}
"#,
    );

    let package_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");
    let smoke_output = executable_output_path(&project_root.join("target/ql/debug/tests"), "smoke");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql test` current package generic public function named arguments",
    );
    let (stdout, stderr) = expect_success(
        "project-test-current-package-generic-function-named-args",
        "package tests importing current package generic function with named arguments",
        &output,
    )
    .expect("package-path `ql test` should infer current package generic public functions from named arguments");
    expect_empty_stderr(
        "project-test-current-package-generic-function-named-args",
        "package tests importing current package generic function with named arguments",
        &stderr,
    )
    .expect("named-argument generic function package test should not print stderr");
    expect_stdout_contains_all(
        "project-test-current-package-generic-function-named-args",
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("named-argument generic function package test should report a passing smoke test");
    expect_file_exists(
        "project-test-current-package-generic-function-named-args",
        &package_output,
        "current package library",
        "`ql test` current package generic public function named arguments",
    )
    .expect("current package library should build before the named-argument test bridge runs");
    expect_file_exists(
        "project-test-current-package-generic-function-named-args",
        &smoke_output,
        "current package generic function named-argument test executable",
        "`ql test` current package generic public function named arguments",
    )
    .expect("named-argument generic function package test should emit the smoke test executable");
}

#[test]
fn test_package_tests_support_current_package_generic_function_expression_arguments() {
    if !toolchain_available("`ql test` current package generic function expression arguments") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-current-package-generic-function-expressions");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source root for generic function expression test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "app/src/lib.ql",
        r#"
pub fn identity[T](value: T) -> T {
    return value
}

pub fn choose[T](fallback: T, value: T) -> T {
    return value
}
"#,
    );
    temp.write(
        "app/tests/smoke.ql",
        r#"
use app.choose as choose
use app.identity as identity

fn main() -> Int {
    let number: Int = identity(1 + 2)
    let flag: Bool = choose(value: !(false || false), fallback: false)
    let ordered: Bool = identity(1 < 2)
    let pair: (Int, Bool) = identity((number, flag))
    let values: [Int; 3] = identity([number, 2 + 3, 4])
    let projected: Int = identity(values[1])
    let tuple_flag: Bool = identity(pair[1])
    let selected: Int = identity(if tuple_flag { projected } else { 0 })
    let matched: Bool = identity(match selected {
        0 => false,
        _ => tuple_flag,
    })
    if matched && ordered && values[0] + values[1] + values[2] + selected == 17 {
        return 0
    }
    return 1
}
"#,
    );

    let package_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");
    let smoke_output = executable_output_path(&project_root.join("target/ql/debug/tests"), "smoke");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql test` current package generic public function expression arguments",
    );
    let (stdout, stderr) = expect_success(
        "project-test-current-package-generic-function-expressions",
        "package tests importing current package generic function with expression arguments",
        &output,
    )
    .expect("package-path `ql test` should infer current package generic public functions from expression arguments");
    expect_empty_stderr(
        "project-test-current-package-generic-function-expressions",
        "package tests importing current package generic function with expression arguments",
        &stderr,
    )
    .expect("expression-argument generic function package test should not print stderr");
    expect_stdout_contains_all(
        "project-test-current-package-generic-function-expressions",
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("expression-argument generic function package test should report a passing smoke test");
    expect_file_exists(
        "project-test-current-package-generic-function-expressions",
        &package_output,
        "current package library",
        "`ql test` current package generic public function expression arguments",
    )
    .expect("current package library should build before the expression-argument test bridge runs");
    expect_file_exists(
        "project-test-current-package-generic-function-expressions",
        &smoke_output,
        "current package generic function expression-argument test executable",
        "`ql test` current package generic public function expression arguments",
    )
    .expect(
        "expression-argument generic function package test should emit the smoke test executable",
    );
}

#[test]
fn test_package_tests_support_current_package_generic_function_from_generic_carriers() {
    if !toolchain_available("`ql test` current package generic carrier function test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-current-package-generic-function-carrier");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source root for generic carrier function import test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "app/src/lib.ql",
        r#"
pub struct Box[T] {
    value: T,
}

pub enum Option[T] {
    Some(T),
    None,
}

pub fn identity[T](value: T) -> T {
    return value
}

pub fn keep_box[T](value: Box[T]) -> Box[T] {
    return value
}

pub fn is_some[T](value: Option[T]) -> Bool {
    return match value {
        Option.Some(_) => true,
        Option.None => false,
    }
}
"#,
    );
    temp.write(
        "app/tests/smoke.ql",
        r#"
use app.Box as Box
use app.Option as Option
use app.identity as identity
use app.is_some as is_some
use app.keep_box as keep_box

fn check(value: Box[Int]) -> Int {
    let kept: Box[Int] = identity(value)
    let nested: Box[Int] = keep_box(kept)
    return nested.value
}

fn main() -> Int {
    let value: Box[Int] = Box { value: 42 }
    if check(value) == 42 && is_some(Option.Some(42)) {
        return 0
    }
    return 1
}
"#,
    );

    let package_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");
    let smoke_output = executable_output_path(&project_root.join("target/ql/debug/tests"), "smoke");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql test` current package generic carrier function",
    );
    let (stdout, stderr) = expect_success(
        "project-test-current-package-generic-function-carrier",
        "package tests importing current package generic function with generic carrier values",
        &output,
    )
    .expect(
        "package-path `ql test` should infer current package generic public functions from generic carrier values",
    );
    expect_empty_stderr(
        "project-test-current-package-generic-function-carrier",
        "package tests importing current package generic function with generic carrier values",
        &stderr,
    )
    .expect("generic carrier function package test should not print stderr");
    expect_stdout_contains_all(
        "project-test-current-package-generic-function-carrier",
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("generic carrier function package test should report a passing smoke test");
    expect_file_exists(
        "project-test-current-package-generic-function-carrier",
        &package_output,
        "current package library",
        "`ql test` current package generic carrier function",
    )
    .expect("current package library should build before the generic carrier test bridge runs");
    expect_file_exists(
        "project-test-current-package-generic-function-carrier",
        &smoke_output,
        "current package generic carrier function test executable",
        "`ql test` current package generic carrier function",
    )
    .expect("generic carrier function package test should emit the smoke test executable");
}

#[test]
fn test_package_tests_support_current_package_generic_function_from_result_context() {
    if !toolchain_available("`ql test` current package generic result-context function test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-current-package-generic-function-result-context");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source root for generic result-context function import test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "app/src/lib.ql",
        r#"
pub enum Result[T, E] {
    Ok(T),
    Err(E),
}

pub fn ok[T, E](value: T) -> Result[T, E] {
    return Result.Ok(value)
}

pub fn err[T, E](error: E) -> Result[T, E] {
    return Result.Err(error)
}
"#,
    );
    temp.write(
        "app/tests/smoke.ql",
        r#"
use app.Result as Result
use app.ok as result_ok
use app.err as result_err

fn make_ok() -> Result[Int, Int] {
    return result_ok(7)
}

fn ok_status(value: Result[Int, Int]) -> Int {
    return match value {
        Result.Ok(inner) => inner,
        Result.Err(_) => 0,
    }
}

fn err_status(value: Result[Int, Int]) -> Int {
    return match value {
        Result.Ok(_) => 0,
        Result.Err(error) => error,
    }
}

fn main() -> Int {
    let failed: Result[Int, Int] = result_err(3)
    if ok_status(make_ok()) + err_status(failed) == 10 {
        return 0
    }
    return 1
}
"#,
    );

    let package_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");
    let smoke_output = executable_output_path(&project_root.join("target/ql/debug/tests"), "smoke");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql test` current package generic result-context function",
    );
    let (stdout, stderr) = expect_success(
        "project-test-current-package-generic-function-result-context",
        "package tests importing current package generic function with result context",
        &output,
    )
    .expect("package-path `ql test` should infer current package generic public functions from explicit result context");
    expect_empty_stderr(
        "project-test-current-package-generic-function-result-context",
        "package tests importing current package generic function with result context",
        &stderr,
    )
    .expect("generic result-context function package test should not print stderr");
    expect_stdout_contains_all(
        "project-test-current-package-generic-function-result-context",
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("generic result-context function package test should report a passing smoke test");
    expect_file_exists(
        "project-test-current-package-generic-function-result-context",
        &package_output,
        "current package library",
        "`ql test` current package generic result-context function",
    )
    .expect(
        "current package library should build before the generic result-context test bridge runs",
    );
    expect_file_exists(
        "project-test-current-package-generic-function-result-context",
        &smoke_output,
        "current package generic result-context function test executable",
        "`ql test` current package generic result-context function",
    )
    .expect("generic result-context function package test should emit the smoke test executable");
}

#[test]
fn test_package_tests_support_current_package_generic_function_from_nested_context() {
    if !toolchain_available("`ql test` current package nested generic function test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-current-package-nested-generic-function");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source root for nested generic function import test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "app/src/lib.ql",
        r#"
pub enum Option[T] {
    Some(T),
    None,
}

pub enum Result[T, E] {
    Ok(T),
    Err(E),
}

pub fn to_option[T, E](value: Result[T, E]) -> Option[T] {
    return match value {
        Result.Ok(inner) => Option.Some(inner),
        Result.Err(_) => Option.None,
    }
}
"#,
    );
    temp.write(
        "app/tests/smoke.ql",
        r#"
use app.Option as Option
use app.Result as Result
use app.to_option as result_to_option

fn value_or(value: Option[Int], fallback: Int) -> Int {
    return match value {
        Option.Some(inner) => inner,
        Option.None => fallback,
    }
}

fn main() -> Int {
    let ok: Result[Int, Int] = Result.Ok(13)
    let err: Result[Int, Int] = Result.Err(5)
    let status = value_or(result_to_option(ok), 0) + value_or(result_to_option(err), 8)
    return status - 21
}
"#,
    );

    let package_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");
    let smoke_output = executable_output_path(&project_root.join("target/ql/debug/tests"), "smoke");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql test` current package nested generic function",
    );
    let (stdout, stderr) = expect_success(
        "project-test-current-package-nested-generic-function",
        "package tests importing current package generic function in nested context",
        &output,
    )
    .expect(
        "package-path `ql test` should infer current package generic public functions from nested call contexts",
    );
    expect_empty_stderr(
        "project-test-current-package-nested-generic-function",
        "package tests importing current package generic function in nested context",
        &stderr,
    )
    .expect("nested generic function package test should not print stderr");
    expect_stdout_contains_all(
        "project-test-current-package-nested-generic-function",
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("nested generic function package test should report a passing smoke test");
    expect_file_exists(
        "project-test-current-package-nested-generic-function",
        &package_output,
        "current package library",
        "`ql test` current package nested generic function",
    )
    .expect("current package library should build before the nested generic bridge runs");
    expect_file_exists(
        "project-test-current-package-nested-generic-function",
        &smoke_output,
        "current package nested generic function test executable",
        "`ql test` current package nested generic function",
    )
    .expect("nested generic function package test should emit the smoke test executable");
}

#[test]
fn test_package_tests_combine_local_generic_and_current_package_public_bridges() {
    if !toolchain_available("`ql test` local generic plus current package bridge test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-local-generic-current-package-bridge");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source root for combined bridge test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "app/src/lib.ql",
        r#"
pub fn score(value: Int) -> Int {
    return value + 3
}
"#,
    );
    temp.write(
        "app/tests/smoke.ql",
        r#"
use app.score as score

fn identity[T](value: T) -> T {
    return value
}

fn main() -> Int {
    let value: Int = identity(score(4))
    if value == 7 {
        return 0
    }
    return 1
}
"#,
    );

    let package_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");
    let smoke_output = executable_output_path(&project_root.join("target/ql/debug/tests"), "smoke");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql test` local generic plus current package bridge",
    );
    let (stdout, stderr) = expect_success(
        "project-test-local-generic-current-package-bridge",
        "package test combining local generic specialization and current package bridge",
        &output,
    )
    .expect("package-path `ql test` should compose local generic and package-under-test bridges");
    expect_empty_stderr(
        "project-test-local-generic-current-package-bridge",
        "package test combining local generic specialization and current package bridge",
        &stderr,
    )
    .expect("combined bridge package test should not print stderr");
    expect_stdout_contains_all(
        "project-test-local-generic-current-package-bridge",
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("combined bridge package test should report one passing smoke test");
    expect_file_exists(
        "project-test-local-generic-current-package-bridge",
        &package_output,
        "current package library",
        "`ql test` local generic plus current package bridge",
    )
    .expect("combined bridge package test should build the current package library");
    expect_file_exists(
        "project-test-local-generic-current-package-bridge",
        &smoke_output,
        "combined bridge smoke executable",
        "`ql test` local generic plus current package bridge",
    )
    .expect("combined bridge package test should emit the smoke executable");
}

#[test]
fn test_workspace_path_prebuilds_selected_members_that_are_also_dependencies() {
    if !toolchain_available("`ql test` selected dependency member test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-selected-dependency-member");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/core/src"))
        .expect("create core package source tree");
    std::fs::create_dir_all(project_root.join("packages/core/tests"))
        .expect("create core package tests");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app package source tree");
    std::fs::create_dir_all(project_root.join("packages/app/tests"))
        .expect("create app package tests");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/core", "packages/app"]
"#,
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );
    temp.write(
        "workspace/packages/core/src/lib.ql",
        "pub fn answer() -> Int { return 42 }\n",
    );
    temp.write(
        "workspace/packages/core/tests/smoke.ql",
        "fn main() -> Int { return 0 }\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
core = "../core"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn helper() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/app/tests/smoke.ql",
        "use core.answer as answer\n\nfn main() -> Int {\n    return answer() - 42\n}\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql test` selected dependency member workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-test-selected-dependency-member",
        "workspace selected dependency member test",
        &output,
    )
    .expect("workspace `ql test` should prebuild selected members that are also dependencies");
    expect_empty_stderr(
        "project-test-selected-dependency-member",
        "workspace selected dependency member test",
        &stderr,
    )
    .expect("selected dependency member test should not print stderr");
    expect_stdout_contains_all(
        "project-test-selected-dependency-member",
        &stdout.replace('\\', "/"),
        &[
            "test packages/core/tests/smoke.ql ... ok",
            "test packages/app/tests/smoke.ql ... ok",
            "test result: ok. 2 passed; 0 failed",
        ],
    )
    .expect("workspace selected dependency member test should report two passing tests");
}

#[test]
fn test_package_path_lists_discovered_tests_without_running_them() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-list");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source root");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("app/tests/smoke.ql", "fn main() -> Int { return nope }\n");
    temp.write("app/tests/api/basic.ql", "this is not valid qlang\n");
    temp.write(
        "app/tests/ui/type_error.ql",
        "fn main() -> Int { return nope }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root).arg("--list");
    let output = run_command_capture(&mut command, "`ql test --list` package path");
    let (stdout, stderr) = expect_success("project-test-list", "package test listing", &output)
        .expect("package-path `ql test --list` should succeed");
    expect_empty_stderr("project-test-list", "package test listing", &stderr)
        .expect("package-path `ql test --list` should not print stderr");
    expect_stdout_contains_all(
        "project-test-list",
        &stdout.replace('\\', "/"),
        &[
            "tests/api/basic.ql",
            "tests/smoke.ql",
            "tests/ui/type_error.ql",
            "test listing: 3 discovered",
        ],
    )
    .expect("package-path `ql test --list` should print discovered tests without building them");
}

#[test]
fn test_package_path_reports_json_success() {
    if !toolchain_available("`ql test --json` package test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-json-success");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source root");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("app/tests/smoke.ql", "fn main() -> Int { return 0 }\n");
    temp.write("app/tests/api/basic.ql", "fn main() -> Int { return 0 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test", "--json"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test --json` package path");
    let (stdout, stderr) = expect_success(
        "project-test-json-success",
        "package test json success",
        &output,
    )
    .expect("package-path `ql test --json` should succeed");
    expect_empty_stderr(
        "project-test-json-success",
        "package test json success",
        &stderr,
    )
    .expect("package-path `ql test --json` should not print stderr");

    let actual = parse_json_output("project-test-json-success", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.test.v1",
        "path": project_root.display().to_string().replace('\\', "/"),
        "requested_profile": "debug",
        "profile_overridden": false,
        "package_name": JsonValue::Null,
        "filter": JsonValue::Null,
        "list_only": false,
        "status": "ok",
        "discovered_total": 2,
        "selected_total": 2,
        "targets": [
            {
                "path": "tests/api/basic.ql",
                "kind": "smoke",
                "profile": "debug",
            },
            {
                "path": "tests/smoke.ql",
                "kind": "smoke",
                "profile": "debug",
            }
        ],
        "passed": 2,
        "failed": 0,
        "failures": [],
    });
    assert_eq!(
        actual, expected,
        "package-path `ql test --json` should match the stable success contract"
    );
}

#[test]
fn test_workspace_path_runs_member_tests_and_skips_members_without_tests() {
    if !toolchain_available("`ql test` workspace test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-workspace");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app package source tree");
    std::fs::create_dir_all(project_root.join("packages/tool/src"))
        .expect("create tool package source tree");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn helper() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        "pub fn helper() -> Int { return 2 }\n",
    );
    temp.write(
        "workspace/packages/app/tests/smoke.ql",
        "fn main() -> Int { return 0 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test` workspace path");
    let (stdout, stderr) =
        expect_success("project-test-workspace", "workspace smoke tests", &output)
            .expect("workspace-path `ql test` should succeed");
    expect_empty_stderr("project-test-workspace", "workspace smoke tests", &stderr)
        .expect("workspace-path `ql test` should not print stderr");
    expect_stdout_contains_all(
        "project-test-workspace",
        &stdout.replace('\\', "/"),
        &[
            "test packages/app/tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("workspace-path `ql test` should run member tests and skip members without tests");
}

#[test]
fn test_workspace_path_runs_package_under_test_generic_bridges_for_all_members() {
    if !toolchain_available("`ql test` workspace all-member package-under-test generic bridge") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_workspace_package_under_test_generic_bridge_project(
        "ql-project-test-workspace-all-member-current-package-generic-bridge",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["test"]).arg(&fixture.project_root);
    let output = run_command_capture(
        &mut command,
        "`ql test` workspace all-member package-under-test generic bridge",
    );
    let (stdout, stderr) = expect_success(
        "project-test-workspace-all-member-current-package-generic-bridge",
        "workspace all-member package-under-test generic bridge",
        &output,
    )
    .expect("workspace-path `ql test` should bridge every selected package under test");
    expect_empty_stderr(
        "project-test-workspace-all-member-current-package-generic-bridge",
        "workspace all-member package-under-test generic bridge",
        &stderr,
    )
    .expect("workspace all-member bridge test should not print stderr");
    expect_stdout_contains_all(
        "project-test-workspace-all-member-current-package-generic-bridge",
        &stdout.replace('\\', "/"),
        &[
            "test packages/app/tests/smoke.ql ... ok",
            "test packages/tool/tests/smoke.ql ... ok",
            "test result: ok. 2 passed; 0 failed",
        ],
    )
    .expect("workspace all-member bridge test should report both passing smoke tests");
    expect_file_exists(
        "project-test-workspace-all-member-current-package-generic-bridge",
        &fixture.app_library_output,
        "app package-under-test library",
        "`ql test` workspace all-member package-under-test generic bridge",
    )
    .expect("workspace all-member bridge test should build the app library");
    expect_file_exists(
        "project-test-workspace-all-member-current-package-generic-bridge",
        &fixture.tool_library_output,
        "tool package-under-test library",
        "`ql test` workspace all-member package-under-test generic bridge",
    )
    .expect("workspace all-member bridge test should build the tool library");
    expect_file_exists(
        "project-test-workspace-all-member-current-package-generic-bridge",
        &fixture.app_smoke_output,
        "app smoke executable",
        "`ql test` workspace all-member package-under-test generic bridge",
    )
    .expect("workspace all-member bridge test should emit the app smoke executable");
    expect_file_exists(
        "project-test-workspace-all-member-current-package-generic-bridge",
        &fixture.tool_smoke_output,
        "tool smoke executable",
        "`ql test` workspace all-member package-under-test generic bridge",
    )
    .expect("workspace all-member bridge test should emit the tool smoke executable");
}

#[test]
fn test_workspace_path_package_under_test_generic_bridges_report_json_success() {
    if !toolchain_available(
        "`ql test --json` workspace all-member package-under-test generic bridge",
    ) {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_workspace_package_under_test_generic_bridge_project(
        "ql-project-test-workspace-all-member-current-package-generic-bridge-json",
    );
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["test", "--json"]).arg(&fixture.project_root);
    let output = run_command_capture(
        &mut command,
        "`ql test --json` workspace all-member package-under-test generic bridge",
    );
    let (stdout, stderr) = expect_success(
        "project-test-workspace-all-member-current-package-generic-bridge-json",
        "workspace all-member package-under-test generic bridge json",
        &output,
    )
    .expect("workspace-path `ql test --json` should bridge every selected package under test");
    expect_empty_stderr(
        "project-test-workspace-all-member-current-package-generic-bridge-json",
        "workspace all-member package-under-test generic bridge json",
        &stderr,
    )
    .expect("workspace all-member bridge json test should not print stderr");

    let actual = parse_json_output(
        "project-test-workspace-all-member-current-package-generic-bridge-json",
        &stdout,
    );
    let expected = serde_json::json!({
        "schema": "ql.test.v1",
        "path": fixture.project_root.display().to_string().replace('\\', "/"),
        "requested_profile": "debug",
        "profile_overridden": false,
        "package_name": JsonValue::Null,
        "filter": JsonValue::Null,
        "list_only": false,
        "status": "ok",
        "discovered_total": 2,
        "selected_total": 2,
        "targets": [
            {
                "path": "packages/app/tests/smoke.ql",
                "kind": "smoke",
                "profile": "debug",
            },
            {
                "path": "packages/tool/tests/smoke.ql",
                "kind": "smoke",
                "profile": "debug",
            },
        ],
        "passed": 2,
        "failed": 0,
        "failures": [],
    });
    assert_eq!(
        actual, expected,
        "workspace full-member bridge json test should preserve the stable ql.test.v1 contract",
    );
    expect_file_exists(
        "project-test-workspace-all-member-current-package-generic-bridge-json",
        &fixture.app_smoke_output,
        "app smoke executable",
        "`ql test --json` workspace all-member package-under-test generic bridge",
    )
    .expect("workspace all-member bridge json test should emit the app smoke executable");
    expect_file_exists(
        "project-test-workspace-all-member-current-package-generic-bridge-json",
        &fixture.tool_smoke_output,
        "tool smoke executable",
        "`ql test --json` workspace all-member package-under-test generic bridge",
    )
    .expect("workspace all-member bridge json test should emit the tool smoke executable");
}

#[test]
fn test_package_path_reports_failing_test_process() {
    if !toolchain_available("`ql test` failing-test case") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-failure");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source root");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("app/tests/fail.ql", "fn main() -> Int { return 7 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test` failing package");
    let (stdout, stderr) = expect_exit_code(
        "project-test-failure",
        "failing package smoke tests",
        &output,
        1,
    )
    .expect("`ql test` should report failing test processes");
    expect_stdout_contains_all(
        "project-test-failure",
        &stdout.replace('\\', "/"),
        &["test tests/fail.ql ... FAILED"],
    )
    .expect("failing package smoke tests should mark the test as failed on stdout");
    expect_stderr_contains(
        "project-test-failure",
        "failing package smoke tests",
        &stderr,
        "reason: test process exited with code 7",
    )
    .expect("failing package smoke tests should report the child exit code");
    expect_stderr_contains(
        "project-test-failure",
        "failing package smoke tests",
        &stderr,
        "test result: FAILED. 0 passed; 1 failed",
    )
    .expect("failing package smoke tests should print the failed summary");
}

#[test]
fn test_package_path_reports_json_build_failure_without_stderr_noise() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-json-build-failure");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source root");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("app/tests/broken.ql", "fn main() -> Int { return nope }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test", "--json"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test --json` build failure");
    let (stdout, stderr) = expect_exit_code(
        "project-test-json-build-failure",
        "package test json build failure",
        &output,
        1,
    )
    .expect("package-path `ql test --json` should surface build failures in json");
    expect_empty_stderr(
        "project-test-json-build-failure",
        "package test json build failure",
        &stderr,
    )
    .expect("package-path `ql test --json` build failures should not print stderr noise");

    let actual = parse_json_output("project-test-json-build-failure", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.test.v1",
        "path": project_root.display().to_string().replace('\\', "/"),
        "requested_profile": "debug",
        "profile_overridden": false,
        "package_name": JsonValue::Null,
        "filter": JsonValue::Null,
        "list_only": false,
        "status": "failed",
        "discovered_total": 1,
        "selected_total": 1,
        "targets": [
            {
                "path": "tests/broken.ql",
                "kind": "smoke",
                "profile": "debug",
            }
        ],
        "passed": 0,
        "failed": 1,
        "failures": [
            {
                "path": "tests/broken.ql",
                "kind": "build",
            }
        ],
    });
    assert_eq!(
        actual, expected,
        "package-path `ql test --json` should report build failures via the stable json contract"
    );
}

#[test]
fn test_direct_project_smoke_file_combines_local_generic_and_package_under_test_bridge() {
    if !toolchain_available(
        "`ql test` direct project smoke file with local generic plus current package bridge",
    ) {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-direct-file-local-generic-current-package-bridge");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source root for direct project smoke file bridge test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "app/src/lib.ql",
        r#"
pub fn score(value: Int) -> Int {
    return value + 3
}
"#,
    );
    let smoke_path = temp.write(
        "app/tests/smoke.ql",
        r#"
use app.score as score

fn identity[T](value: T) -> T {
    return value
}

fn main() -> Int {
    let value: Int = identity(score(4))
    if value == 7 {
        return 0
    }
    return 1
}
"#,
    );

    let package_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");
    let smoke_output = executable_output_path(&project_root.join("target/ql/debug/tests"), "smoke");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&smoke_path);
    let output = run_command_capture(
        &mut command,
        "`ql test` direct project smoke file with local generic plus current package bridge",
    );
    let (stdout, stderr) = expect_success(
        "project-test-direct-file-local-generic-current-package-bridge",
        "direct project smoke file combining local generic specialization and current package bridge",
        &output,
    )
    .expect(
        "direct project smoke files should compose local generic and package-under-test bridges",
    );
    expect_empty_stderr(
        "project-test-direct-file-local-generic-current-package-bridge",
        "direct project smoke file combining local generic specialization and current package bridge",
        &stderr,
    )
    .expect("direct project bridge smoke file should not print stderr");
    expect_stdout_contains_all(
        "project-test-direct-file-local-generic-current-package-bridge",
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("direct project bridge smoke file should report one passing smoke test");
    expect_file_exists(
        "project-test-direct-file-local-generic-current-package-bridge",
        &package_output,
        "current package library",
        "`ql test` direct project smoke file with local generic plus current package bridge",
    )
    .expect("direct project bridge smoke file should build the current package library");
    expect_file_exists(
        "project-test-direct-file-local-generic-current-package-bridge",
        &smoke_output,
        "direct project bridge smoke executable",
        "`ql test` direct project smoke file with local generic plus current package bridge",
    )
    .expect("direct project bridge smoke file should emit the smoke executable");
}

#[test]
fn test_project_path_json_holds_executable_lock_while_smoke_test_runs() {
    if !toolchain_available("`ql test --json` executable lock during smoke execution test") {
        return;
    }

    let temp = TempDir::new("ql-project-test-json-executable-lock-during-execution");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for executable lock test");
    std::fs::create_dir_all(project_root.join("tests"))
        .expect("create package test tree for executable lock test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("app/tests/smoke.ql", &sleep_program_source(900));
    let smoke_output = executable_output_path(&project_root.join("target/ql/debug/tests"), "smoke");
    let workspace_root = workspace_root();

    let mut first = ql_command(&workspace_root);
    first.current_dir(temp.path());
    first.args(["test", "--json"]).arg(&project_root);
    first.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut first = first
        .spawn()
        .expect("spawn first long-running `ql test --json`");
    wait_for_path_exists(
        "project-test-json-executable-lock-during-execution",
        "first long-running smoke test executable",
        &smoke_output,
        Duration::from_secs(20),
    )
    .expect("first long-running smoke test should create the executable before execution");
    std::thread::sleep(Duration::from_millis(50));
    assert!(
        first
            .try_wait()
            .expect("poll first long-running `ql test --json`")
            .is_none(),
        "first long-running smoke test should still hold the executable while the second test starts"
    );

    let mut second = ql_command(&workspace_root);
    second.current_dir(temp.path());
    second.args(["test", "--json"]).arg(&project_root);
    second.stdout(Stdio::piped()).stderr(Stdio::piped());
    let second_started = Instant::now();
    let second = second
        .spawn()
        .expect("spawn second `ql test --json` against same smoke executable");

    let second_output = second
        .wait_with_output()
        .expect("wait for second executable-lock `ql test --json`");
    let second_elapsed = second_started.elapsed();
    let first_output = first
        .wait_with_output()
        .expect("wait for first executable-lock `ql test --json`");

    for (label, output) in [("first", first_output), ("second", second_output)] {
        let (stdout, stderr) = expect_success(
            "project-test-json-executable-lock-during-execution",
            &format!("{label} long-running executable-lock test"),
            &output,
        )
        .unwrap_or_else(|message| panic!("{message}"));
        expect_empty_stderr(
            "project-test-json-executable-lock-during-execution",
            &format!("{label} long-running executable-lock test"),
            &stderr,
        )
        .unwrap_or_else(|message| panic!("{message}"));
        let json = parse_json_output(
            "project-test-json-executable-lock-during-execution",
            &stdout,
        );
        assert_eq!(json["schema"], "ql.test.v1");
        assert_eq!(json["status"], "ok");
        assert_eq!(json["passed"], 1);
        assert_eq!(json["failed"], 0);
    }

    assert!(
        second_elapsed >= Duration::from_millis(1000),
        "second test should wait for the first execution lock before rebuilding the same executable; elapsed {second_elapsed:?}"
    );
    assert_no_build_lock_directories(
        "project-test-json-executable-lock-during-execution",
        &project_root,
    );
}
