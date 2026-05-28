mod support;

use std::fs;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

use ql_driver::{ToolchainOptions, discover_toolchain};
use serde_json::Value as JsonValue;
use support::{
    TempDir, assert_no_build_lock_directories, executable_output_path, expect_empty_stderr,
    expect_empty_stdout, expect_exit_code, expect_file_exists, expect_stderr_contains,
    expect_stdout_contains_all, expect_success, ql_command, run_command_capture,
    sleep_program_source, static_library_output_path, wait_for_path_exists, workspace_root,
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

struct DependencySmokeProject {
    temp: TempDir,
    project_root: PathBuf,
    interface_output: PathBuf,
    dependency_output: PathBuf,
    smoke_output: PathBuf,
}

struct WorkspacePackageUnderTestGenericBridgeProject {
    temp: TempDir,
    project_root: PathBuf,
    app_library_output: PathBuf,
    tool_library_output: PathBuf,
    app_smoke_output: PathBuf,
    tool_smoke_output: PathBuf,
}

fn write_dependency_smoke_project(
    prefix: &str,
    dependency_source: &str,
    smoke_source: &str,
) -> DependencySmokeProject {
    let temp = TempDir::new(prefix);
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    fs::create_dir_all(dep_root.join("src")).expect("create dependency source tree");
    fs::create_dir_all(project_root.join("src")).expect("create package source tree");

    temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write("dep/src/lib.ql", dependency_source);
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("app/tests/smoke.ql", smoke_source);

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let smoke_output = executable_output_path(&project_root.join("target/ql/debug/tests"), "smoke");
    assert!(
        !interface_output.exists(),
        "dependency interface should start missing for {prefix}"
    );

    DependencySmokeProject {
        temp,
        project_root,
        interface_output,
        dependency_output,
        smoke_output,
    }
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

fn expect_dependency_smoke_project_passes(
    case_name: &str,
    action: &str,
    command_description: &str,
    fixture: &DependencySmokeProject,
) {
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["test"]).arg(&fixture.project_root);
    let output = run_command_capture(&mut command, command_description);
    let (stdout, stderr) = expect_success(case_name, action, &output)
        .expect("dependency smoke project should pass `ql test`");
    expect_empty_stderr(case_name, action, &stderr)
        .expect("dependency smoke project should not print stderr");
    expect_stdout_contains_all(
        case_name,
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("dependency smoke project should report one passing smoke test");
    expect_file_exists(
        case_name,
        &fixture.interface_output,
        "synced dependency interface",
        action,
    )
    .expect("dependency smoke project should emit the dependency interface");
    expect_file_exists(
        case_name,
        &fixture.dependency_output,
        "dependency package artifact",
        action,
    )
    .expect("dependency smoke project should build the dependency package artifact");
    expect_file_exists(
        case_name,
        &fixture.smoke_output,
        "smoke test executable",
        action,
    )
    .expect("dependency smoke project should emit the smoke test executable");
}

fn expect_direct_dependency_smoke_file_passes(
    case_name: &str,
    action: &str,
    command_description: &str,
    fixture: &DependencySmokeProject,
) {
    let workspace_root = workspace_root();
    let smoke_path = fixture.project_root.join("tests/smoke.ql");
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["test"]).arg(&smoke_path);
    let output = run_command_capture(&mut command, command_description);
    let (stdout, stderr) = expect_success(case_name, action, &output)
        .expect("direct dependency smoke file should pass `ql test`");
    expect_empty_stderr(case_name, action, &stderr)
        .expect("direct dependency smoke file should not print stderr");
    expect_stdout_contains_all(
        case_name,
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("direct dependency smoke file should report one passing smoke test");
    expect_file_exists(
        case_name,
        &fixture.interface_output,
        "synced dependency interface",
        action,
    )
    .expect("direct dependency smoke file should emit the dependency interface");
    expect_file_exists(
        case_name,
        &fixture.dependency_output,
        "dependency package artifact",
        action,
    )
    .expect("direct dependency smoke file should build the dependency package artifact");
    expect_file_exists(
        case_name,
        &fixture.smoke_output,
        "direct dependency smoke executable",
        action,
    )
    .expect("direct dependency smoke file should emit the smoke executable");
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
fn test_package_path_supports_direct_dependency_public_functions() {
    if !toolchain_available("`ql test` dependency public function test") {
        return;
    }

    let fixture = write_dependency_smoke_project(
        "ql-project-test-dependency-public-function",
        "pub fn add(left: Int, right: Int) -> Int { return left + right }\n",
        "use dep.add as sum\n\nfn main() -> Int { return sum(9, 4) - 13 }\n",
    );
    expect_dependency_smoke_project_passes(
        "project-test-dependency-public-function",
        "package dependency public function test",
        "`ql test` dependency public function",
        &fixture,
    );
}

#[test]
fn test_package_path_supports_direct_dependency_public_values() {
    if !toolchain_available("`ql test` dependency public value test") {
        return;
    }

    let fixture = write_dependency_smoke_project(
        "ql-project-test-dependency-public-value",
        "pub const VALUE: Int = 7\npub static READY: Bool = true\npub static VALUES: [Int; 3] = [1, 3, 5]\n",
        "use dep.VALUE as THRESHOLD\nuse dep.READY as ENABLED\nuse dep.VALUES as ITEMS\n\nfn main() -> Int {\n    if ENABLED {\n        return THRESHOLD + ITEMS[1] - 10\n    }\n    return 1\n}\n",
    );
    expect_dependency_smoke_project_passes(
        "project-test-dependency-public-value",
        "package dependency public value test",
        "`ql test` dependency public value",
        &fixture,
    );
}

#[test]
fn test_package_path_supports_direct_dependency_generic_public_functions() {
    if !toolchain_available("`ql test` dependency generic public function test") {
        return;
    }

    let fixture = write_dependency_smoke_project(
        "ql-project-test-dependency-generic-public-function",
        r#"
pub fn identity[T](value: T) -> T {
    return value
}

pub fn first[T, N](values: [T; N]) -> T {
    return values[0]
}
"#,
        "use dep.identity as identity\nuse dep.first as first\n\nfn main() -> Int {\n    let value: Int = identity(7)\n    let picked: Int = first([5, 8, 13])\n    return value + picked - 12\n}\n",
    );
    expect_dependency_smoke_project_passes(
        "project-test-dependency-generic-public-function",
        "package dependency generic public function test",
        "`ql test` dependency generic public function",
        &fixture,
    );
}

#[test]
fn test_package_path_supports_dependency_generic_wrapper_calling_generic_helper() {
    if !toolchain_available("`ql test` dependency generic wrapper helper test") {
        return;
    }

    let fixture = write_dependency_smoke_project(
        "ql-project-test-dependency-generic-wrapper-helper",
        r#"
pub fn first[T, N](values: [T; N]) -> T {
    return values[0]
}

pub fn first_wrapped[T, N](values: [T; N]) -> T {
    return first(values)
}
"#,
        r#"
use dep.first_wrapped as first_wrapped

fn main() -> Int {
    return first_wrapped([7, 8, 9]) - 7
}
"#,
    );
    expect_dependency_smoke_project_passes(
        "project-test-dependency-generic-wrapper-helper",
        "package dependency generic wrapper helper test",
        "`ql test` dependency generic wrapper helper",
        &fixture,
    );
}

#[test]
fn test_package_path_supports_dependency_generic_wrapper_calling_imported_generic_helper() {
    if !toolchain_available("`ql test` dependency generic wrapper imported helper test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-dependency-generic-imported-helper");
    let helper_root = temp.path().join("helper");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(helper_root.join("src"))
        .expect("create helper source tree for imported generic helper test");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dep source tree for imported generic helper test");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for imported generic helper test");
    std::fs::create_dir_all(project_root.join("tests"))
        .expect("create app test tree for imported generic helper test");

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
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write(
        "app/tests/smoke.ql",
        r#"
use dep.reverse_wrapped as reverse_wrapped

fn main() -> Int {
    let reversed: [Int; 3] = reverse_wrapped([7, 8, 9])
    return reversed[0] + reversed[1] + reversed[2] - 24
}
"#,
    );

    let helper_output = static_library_output_path(&helper_root.join("target/ql/debug"), "lib");
    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let interface_output = dep_root.join("dep.qi");
    let smoke_output = executable_output_path(&project_root.join("target/ql/debug/tests"), "smoke");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql test` dependency generic wrapper imported helper",
    );
    let (stdout, stderr) = expect_success(
        "project-test-dependency-generic-imported-helper",
        "package dependency generic wrapper imported helper test",
        &output,
    )
    .expect(
        "package-path `ql test` should specialize dependency wrappers and imported generic helpers",
    );
    expect_empty_stderr(
        "project-test-dependency-generic-imported-helper",
        "package dependency generic wrapper imported helper test",
        &stderr,
    )
    .expect("imported generic helper package test should not print stderr");
    expect_stdout_contains_all(
        "project-test-dependency-generic-imported-helper",
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("imported generic helper package test should report one passing smoke test");
    expect_file_exists(
        "project-test-dependency-generic-imported-helper",
        &helper_output,
        "transitive helper package artifact",
        "package dependency generic wrapper imported helper test",
    )
    .expect("imported generic helper test should build the transitive helper artifact");
    expect_file_exists(
        "project-test-dependency-generic-imported-helper",
        &dep_output,
        "dependency package artifact",
        "package dependency generic wrapper imported helper test",
    )
    .expect("imported generic helper test should build the dependency artifact");
    expect_file_exists(
        "project-test-dependency-generic-imported-helper",
        &interface_output,
        "synced dependency interface",
        "package dependency generic wrapper imported helper test",
    )
    .expect("imported generic helper test should emit the dependency interface");
    expect_file_exists(
        "project-test-dependency-generic-imported-helper",
        &smoke_output,
        "smoke test executable",
        "package dependency generic wrapper imported helper test",
    )
    .expect("imported generic helper test should emit the smoke executable");
}

#[test]
fn test_package_path_supports_dependency_generic_named_expression_args() {
    if !toolchain_available("`ql test` dependency generic function named/expression arguments") {
        return;
    }

    let fixture = write_dependency_smoke_project(
        "ql-project-test-dependency-generic-function-named-expression-args",
        r#"
pub fn identity[T](value: T) -> T {
    return value
}

pub fn choose[T](fallback: T, value: T) -> T {
    return value
}
"#,
        r#"
use dep.choose as choose
use dep.identity as identity

fn bool_score(value: Bool) -> Int {
    if value {
        return 5
    }
    return 0
}

fn main() -> Int {
    let number: Int = choose(value: 7, fallback: 0)
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
    if matched && ordered && number + projected + selected + bool_score(flag) == 22 {
        return 0
    }
    return 1
}
"#,
    );
    expect_dependency_smoke_project_passes(
        "project-test-dependency-generic-function-named-expression-args",
        "package dependency generic function named/expression argument test",
        "`ql test` dependency generic function named/expression arguments",
        &fixture,
    );
}

#[test]
fn test_package_path_supports_direct_dependency_generic_public_functions_from_generic_carriers() {
    if !toolchain_available("`ql test` dependency generic function carrier values") {
        return;
    }

    let fixture = write_dependency_smoke_project(
        "ql-project-test-dependency-generic-function-carrier",
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
        r#"
use dep.Box as Box
use dep.Option as Option
use dep.identity as identity
use dep.is_some as is_some
use dep.keep_box as keep_box

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
    expect_dependency_smoke_project_passes(
        "project-test-dependency-generic-function-carrier",
        "package dependency generic function carrier value test",
        "`ql test` dependency generic function carrier values",
        &fixture,
    );
}

#[test]
fn test_package_path_supports_direct_dependency_generic_public_functions_from_result_context() {
    if !toolchain_available("`ql test` dependency generic function result context") {
        return;
    }

    let fixture = write_dependency_smoke_project(
        "ql-project-test-dependency-generic-function-result-context",
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
        r#"
use dep.Result as Result
use dep.err as result_err
use dep.ok as result_ok

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
    expect_dependency_smoke_project_passes(
        "project-test-dependency-generic-function-result-context",
        "package dependency generic function result-context test",
        "`ql test` dependency generic function result context",
        &fixture,
    );
}

#[test]
fn test_package_path_supports_dependency_zero_arg_generic_context() {
    if !toolchain_available("`ql test` dependency zero-argument generic function context") {
        return;
    }

    let fixture = write_dependency_smoke_project(
        "ql-project-test-dependency-zero-arg-generic-function-context",
        r#"
pub enum Option[T] {
    Some(T),
    None,
}

pub fn none_option[T]() -> Option[T] {
    return Option.None
}
"#,
        r#"
use dep.Option as Option
use dep.none_option as option_none

fn make_none() -> Option[Int] {
    return option_none()
}

fn none_status(value: Option[Int]) -> Int {
    return match value {
        Option.Some(_) => 1,
        Option.None => 0,
    }
}

fn main() -> Int {
    let value: Option[Int] = option_none()
    if none_status(value) + none_status(make_none()) == 0 {
        return 0
    }
    return 1
}
"#,
    );
    expect_dependency_smoke_project_passes(
        "project-test-dependency-zero-arg-generic-function-context",
        "package dependency zero-argument generic function context test",
        "`ql test` dependency zero-argument generic function context",
        &fixture,
    );
}

#[test]
fn test_package_path_allows_unused_direct_dependency_generic_public_function_imports() {
    if !toolchain_available("`ql test` unused dependency generic public function import test") {
        return;
    }

    let fixture = write_dependency_smoke_project(
        "ql-project-test-unused-dependency-generic-public-function-import",
        r#"
pub fn identity[T](value: T) -> T {
    return value
}
"#,
        "use dep.identity as identity\n\nfn main() -> Int {\n    return 0\n}\n",
    );
    expect_dependency_smoke_project_passes(
        "project-test-unused-dependency-generic-public-function-import",
        "package unused dependency generic public function import test",
        "`ql test` unused dependency generic public function import",
        &fixture,
    );
}

#[test]
fn test_package_path_supports_direct_dependency_generic_public_functions_from_nested_context() {
    if !toolchain_available("`ql test` dependency nested generic public function test") {
        return;
    }

    let fixture = write_dependency_smoke_project(
        "ql-project-test-dependency-nested-generic-public-function",
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
        r#"
use dep.Option as Option
use dep.Result as Result
use dep.to_option as result_to_option

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
    expect_dependency_smoke_project_passes(
        "project-test-dependency-nested-generic-public-function",
        "package dependency nested generic public function test",
        "`ql test` dependency nested generic public function",
        &fixture,
    );
}

#[test]
fn test_package_path_supports_direct_dependency_public_struct_functions() {
    if !toolchain_available("`ql test` dependency public struct function test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-dependency-public-struct-function");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source tree");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source tree");

    let dep_manifest = temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "dep/src/lib.ql",
        "pub struct Box { value: Int }\npub fn make_box() -> Box { return Box { value: 7 } }\n",
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
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write(
        "app/tests/smoke.ql",
        "use dep.make_box as make\n\nfn main() -> Int {\n    let value = make()\n    return value.value - 7\n}\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let smoke_output = executable_output_path(&project_root.join("target/ql/debug/tests"), "smoke");
    assert!(
        !interface_output.exists(),
        "dependency interface should start missing for dependency public struct function test"
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test` dependency public struct function");
    let (stdout, stderr) = expect_success(
        "project-test-dependency-public-struct-function",
        "package dependency public struct function test",
        &output,
    )
    .expect("package-path `ql test` should support direct dependency public struct functions");
    expect_empty_stderr(
        "project-test-dependency-public-struct-function",
        "package dependency public struct function test",
        &stderr,
    )
    .expect("dependency public struct function test should not print stderr");
    expect_stdout_contains_all(
        "project-test-dependency-public-struct-function",
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("dependency public struct function test should report one passing smoke test");
    expect_file_exists(
        "project-test-dependency-public-struct-function",
        &interface_output,
        "synced dependency interface",
        "package dependency public struct function test",
    )
    .expect("dependency public struct function test should emit the dependency interface");
    expect_file_exists(
        "project-test-dependency-public-struct-function",
        &dependency_output,
        "dependency package artifact",
        "package dependency public struct function test",
    )
    .expect(
        "dependency public struct function test should also build the dependency package artifact",
    );
    expect_file_exists(
        "project-test-dependency-public-struct-function",
        &smoke_output,
        "smoke test executable",
        "package dependency public struct function test",
    )
    .expect("dependency public struct function test should emit the smoke test executable");
    assert!(
        dep_manifest.exists(),
        "dependency manifest should remain present after dependency public struct function test"
    );
}

#[test]
fn test_package_path_supports_direct_dependency_public_type_alias_functions() {
    if !toolchain_available("`ql test` dependency public type alias function test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-dependency-public-type-alias-function");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source tree");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source tree");

    let dep_manifest = temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "dep/src/lib.ql",
        "pub type Count = Int\npub type Score = Count\npub fn make_score(value: Count) -> Score { return value + 2 }\npub fn unwrap_score(value: Score) -> Int { return value }\n",
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
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write(
        "app/tests/smoke.ql",
        "use dep.make_score as make_score\nuse dep.unwrap_score as unwrap_score\n\nfn main() -> Int {\n    return unwrap_score(make_score(5)) - 7\n}\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let smoke_output = executable_output_path(&project_root.join("target/ql/debug/tests"), "smoke");
    assert!(
        !interface_output.exists(),
        "dependency interface should start missing for dependency public type alias function test"
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql test` dependency public type alias function",
    );
    let (stdout, stderr) = expect_success(
        "project-test-dependency-public-type-alias-function",
        "package dependency public type alias function test",
        &output,
    )
    .expect("package-path `ql test` should support direct dependency public type alias functions");
    expect_empty_stderr(
        "project-test-dependency-public-type-alias-function",
        "package dependency public type alias function test",
        &stderr,
    )
    .expect("dependency public type alias function test should not print stderr");
    expect_stdout_contains_all(
        "project-test-dependency-public-type-alias-function",
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("dependency public type alias function test should report one passing smoke test");
    expect_file_exists(
        "project-test-dependency-public-type-alias-function",
        &interface_output,
        "synced dependency interface",
        "package dependency public type alias function test",
    )
    .expect("dependency public type alias function test should emit the dependency interface");
    expect_file_exists(
        "project-test-dependency-public-type-alias-function",
        &dependency_output,
        "dependency package artifact",
        "package dependency public type alias function test",
    )
    .expect(
        "dependency public type alias function test should also build the dependency package artifact",
    );
    expect_file_exists(
        "project-test-dependency-public-type-alias-function",
        &smoke_output,
        "smoke test executable",
        "package dependency public type alias function test",
    )
    .expect("dependency public type alias function test should emit the smoke test executable");
    assert!(
        dep_manifest.exists(),
        "dependency manifest should remain present after dependency public type alias function test"
    );
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
fn test_package_path_supports_direct_dependency_public_struct_methods() {
    if !toolchain_available("`ql test` dependency public struct method test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-dependency-public-struct-method");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source tree");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source tree");

    temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "dep/src/lib.ql",
        "pub struct Box { value: Int }\n\nimpl Box {\n    pub fn read(self) -> Int {\n        return self.value\n    }\n}\n\npub fn make_box() -> Box { return Box { value: 7 } }\n",
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
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write(
        "app/tests/smoke.ql",
        "use dep.make_box as make\n\nfn main() -> Int {\n    let value = make()\n    return value.read() - 7\n}\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let smoke_output = executable_output_path(&project_root.join("target/ql/debug/tests"), "smoke");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test` dependency public struct method");
    let (stdout, stderr) = expect_success(
        "project-test-dependency-public-struct-method",
        "package dependency public struct method test",
        &output,
    )
    .expect("package-path `ql test` should support direct dependency public struct methods");
    expect_empty_stderr(
        "project-test-dependency-public-struct-method",
        "package dependency public struct method test",
        &stderr,
    )
    .expect("dependency public struct method test should not print stderr");
    expect_stdout_contains_all(
        "project-test-dependency-public-struct-method",
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("dependency public struct method test should report one passing smoke test");
    expect_file_exists(
        "project-test-dependency-public-struct-method",
        &interface_output,
        "synced dependency interface",
        "package dependency public struct method test",
    )
    .expect("dependency public struct method test should emit the dependency interface");
    expect_file_exists(
        "project-test-dependency-public-struct-method",
        &dependency_output,
        "dependency package artifact",
        "package dependency public struct method test",
    )
    .expect("dependency public struct method test should build the dependency package artifact");
    expect_file_exists(
        "project-test-dependency-public-struct-method",
        &smoke_output,
        "smoke test executable",
        "package dependency public struct method test",
    )
    .expect("dependency public struct method test should emit the smoke test executable");
}

#[test]
fn test_package_path_supports_direct_dependency_public_struct_method_values() {
    if !toolchain_available("`ql test` dependency public struct method value test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-dependency-public-struct-method-value");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source tree");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source tree");

    temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "dep/src/lib.ql",
        "pub struct Box { value: Int }\n\nimpl Box {\n    pub fn add(self, delta: Int) -> Int {\n        return self.value + delta\n    }\n}\n\npub fn make_box() -> Box { return Box { value: 7 } }\n",
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
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write(
        "app/tests/smoke.ql",
        "use dep.make_box as make\n\nfn main() -> Int {\n    let value = make()\n    let add = value.add\n    return add(5) - 12\n}\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let smoke_output = executable_output_path(&project_root.join("target/ql/debug/tests"), "smoke");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql test` dependency public struct method value",
    );
    let (stdout, stderr) = expect_success(
        "project-test-dependency-public-struct-method-value",
        "package dependency public struct method value test",
        &output,
    )
    .expect("package-path `ql test` should support direct dependency public struct method values");
    expect_empty_stderr(
        "project-test-dependency-public-struct-method-value",
        "package dependency public struct method value test",
        &stderr,
    )
    .expect("dependency public struct method value test should not print stderr");
    expect_stdout_contains_all(
        "project-test-dependency-public-struct-method-value",
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("dependency public struct method value test should report one passing smoke test");
    expect_file_exists(
        "project-test-dependency-public-struct-method-value",
        &interface_output,
        "synced dependency interface",
        "package dependency public struct method value test",
    )
    .expect("dependency public struct method value test should emit the dependency interface");
    expect_file_exists(
        "project-test-dependency-public-struct-method-value",
        &dependency_output,
        "dependency package artifact",
        "package dependency public struct method value test",
    )
    .expect("dependency public struct method value test should build the dependency artifact");
    expect_file_exists(
        "project-test-dependency-public-struct-method-value",
        &smoke_output,
        "smoke test executable",
        "package dependency public struct method value test",
    )
    .expect("dependency public struct method value test should emit the smoke test executable");
}

#[test]
fn test_package_path_supports_direct_dependency_public_trait_methods() {
    if !toolchain_available("`ql test` dependency public trait method test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-dependency-public-trait-method");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source tree");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source tree");

    temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "dep/src/lib.ql",
        "pub trait Reader {\n    fn read(self) -> Int\n}\n\npub struct Box { value: Int }\n\nimpl Reader for Box {\n    pub fn read(self) -> Int {\n        return self.value\n    }\n}\n\npub fn make_box() -> Box { return Box { value: 9 } }\n",
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
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write(
        "app/tests/smoke.ql",
        "use dep.make_box as make\n\nfn main() -> Int {\n    let value = make()\n    return value.read() - 9\n}\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let smoke_output = executable_output_path(&project_root.join("target/ql/debug/tests"), "smoke");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test` dependency public trait method");
    let (stdout, stderr) = expect_success(
        "project-test-dependency-public-trait-method",
        "package dependency public trait method test",
        &output,
    )
    .expect("package-path `ql test` should support direct dependency public trait methods");
    expect_empty_stderr(
        "project-test-dependency-public-trait-method",
        "package dependency public trait method test",
        &stderr,
    )
    .expect("dependency public trait method test should not print stderr");
    expect_stdout_contains_all(
        "project-test-dependency-public-trait-method",
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("dependency public trait method test should report one passing smoke test");
    expect_file_exists(
        "project-test-dependency-public-trait-method",
        &interface_output,
        "synced dependency interface",
        "package dependency public trait method test",
    )
    .expect("dependency public trait method test should emit the dependency interface");
    expect_file_exists(
        "project-test-dependency-public-trait-method",
        &dependency_output,
        "dependency package artifact",
        "package dependency public trait method test",
    )
    .expect("dependency public trait method test should build the dependency package artifact");
    expect_file_exists(
        "project-test-dependency-public-trait-method",
        &smoke_output,
        "smoke test executable",
        "package dependency public trait method test",
    )
    .expect("dependency public trait method test should emit the smoke test executable");
}

#[test]
fn test_package_path_selects_requested_target() {
    if !toolchain_available("`ql test --target` package test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-target");
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
    temp.write("app/tests/ignored.ql", "fn main() -> Int { return 1 }\n");

    let selected_output =
        executable_output_path(&project_root.join("target/ql/debug/tests"), "smoke");
    let ignored_output =
        executable_output_path(&project_root.join("target/ql/debug/tests"), "ignored");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test"])
        .arg(&project_root)
        .args(["--target", "tests/smoke.ql"]);
    let output = run_command_capture(&mut command, "`ql test --target` package path");
    let (stdout, stderr) = expect_success("project-test-target", "package target test", &output)
        .expect("package-path `ql test --target` should run the selected test");
    expect_empty_stderr("project-test-target", "package target test", &stderr)
        .expect("package-path `ql test --target` should not print stderr");
    expect_stdout_contains_all(
        "project-test-target",
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("package-path `ql test --target` should report the selected test only");
    assert!(
        !stdout.contains("ignored.ql"),
        "package-path `ql test --target` should not run unselected tests, got:\n{stdout}"
    );
    expect_file_exists(
        "project-test-target",
        &selected_output,
        "selected smoke test executable",
        "package target test",
    )
    .expect("package-path `ql test --target` should emit the selected test artifact");
    assert!(
        !ignored_output.exists(),
        "package-path `ql test --target` should not build unselected test artifacts"
    );
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
fn test_workspace_member_file_accepts_package_selector() {
    if !toolchain_available("`ql test` workspace member file package selector test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-workspace-member-file-package-selector");
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
    let smoke_path = temp.write(
        "workspace/packages/app/tests/app_only.ql",
        "fn main() -> Int { return 0 }\n",
    );
    temp.write(
        "workspace/packages/tool/tests/tool_only.ql",
        "fn main() -> Int { return 0 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test"])
        .arg(&smoke_path)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test` workspace member file package selector",
    );
    let (stdout, stderr) = expect_success(
        "project-test-workspace-member-file-package-selector",
        "workspace member file package selector test",
        &output,
    )
    .expect("workspace-member-file `ql test --package app` should keep project-aware semantics");
    expect_empty_stderr(
        "project-test-workspace-member-file-package-selector",
        "workspace member file package selector test",
        &stderr,
    )
    .expect("workspace member file package selector test should not print stderr");
    expect_stdout_contains_all(
        "project-test-workspace-member-file-package-selector",
        &stdout.replace('\\', "/"),
        &[
            "test packages/app/tests/app_only.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("workspace member file package selector test should run the selected package test");
    assert!(
        !stdout
            .replace('\\', "/")
            .contains("packages/tool/tests/tool_only.ql"),
        "workspace member file package selector test should not run tests from other packages: {stdout}"
    );
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
fn test_package_path_json_reports_missing_manifest_preflight_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-json-missing-manifest");
    let project_root = temp.path().join("app");
    fs::create_dir_all(&project_root).expect("create empty package directory");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test", "--json"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test --json` missing manifest");
    let (stdout, stderr) = expect_exit_code(
        "project-test-json-missing-manifest",
        "missing manifest test json preflight failure",
        &output,
        1,
    )
    .expect("package-path `ql test --json` should exit with code 1 for missing manifest");
    expect_empty_stderr(
        "project-test-json-missing-manifest",
        "missing manifest test json preflight failure",
        &stderr,
    )
    .expect("package-path `ql test --json` missing manifest should stay on stdout");

    let json = parse_json_output("project-test-json-missing-manifest", &stdout);
    assert_eq!(json["schema"], "ql.test.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["package_name"], JsonValue::Null);
    assert_eq!(json["status"], "failed");
    assert_eq!(json["discovered_total"], 0);
    assert_eq!(json["selected_total"], 0);
    assert_eq!(json["targets"], serde_json::json!([]));
    assert_eq!(json["failures"], serde_json::json!([]));
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["error_kind"], "manifest");
    assert_eq!(failure["stage"], "manifest-load");
    assert_eq!(failure["selector"], JsonValue::Null);
    assert_eq!(failure["target_count"], JsonValue::Null);
    assert!(
        failure["message"]
            .as_str()
            .expect("missing manifest test json failure should expose a message")
            .contains("could not find `qlang.toml`"),
        "missing manifest test json failure should describe manifest lookup: {json}"
    );
}

#[test]
fn test_package_path_json_reports_missing_source_root_preflight_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-json-missing-source-root");
    let project_root = temp.path().join("app");
    fs::create_dir_all(&project_root).expect("create package directory");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test", "--json"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test --json` missing source root");
    let (stdout, stderr) = expect_exit_code(
        "project-test-json-missing-source-root",
        "missing source root test json preflight failure",
        &output,
        1,
    )
    .expect("package-path `ql test --json` should exit with code 1 for missing source root");
    expect_empty_stderr(
        "project-test-json-missing-source-root",
        "missing source root test json preflight failure",
        &stderr,
    )
    .expect("package-path `ql test --json` missing source root should stay on stdout");

    let json = parse_json_output("project-test-json-missing-source-root", &stdout);
    assert_eq!(json["schema"], "ql.test.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["package_name"], JsonValue::Null);
    assert_eq!(json["status"], "failed");
    assert_eq!(json["discovered_total"], 0);
    assert_eq!(json["selected_total"], 0);
    assert_eq!(json["targets"], serde_json::json!([]));
    assert_eq!(json["failures"], serde_json::json!([]));
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["error_kind"], "manifest");
    assert_eq!(failure["stage"], "target-discovery");
    assert_eq!(failure["selector"], JsonValue::Null);
    assert_eq!(failure["target_count"], JsonValue::Null);
    assert!(
        failure["message"]
            .as_str()
            .expect("missing source root test json failure should expose a message")
            .contains("package source directory"),
        "missing source root test json failure should describe source root lookup: {json}"
    );
}

#[test]
fn test_package_path_json_reports_no_tests_selection_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-json-no-tests");
    let project_root = temp.path().join("app");
    fs::create_dir_all(project_root.join("src")).expect("create package source root");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test", "--json"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test --json` no tests");
    let (stdout, stderr) = expect_exit_code(
        "project-test-json-no-tests",
        "no tests selection json failure",
        &output,
        1,
    )
    .expect("package-path `ql test --json` should exit with code 1 without tests");
    expect_empty_stderr(
        "project-test-json-no-tests",
        "no tests selection json failure",
        &stderr,
    )
    .expect("package-path `ql test --json` no-tests failure should stay on stdout");

    let json = parse_json_output("project-test-json-no-tests", &stdout);
    assert_eq!(json["schema"], "ql.test.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "no-tests");
    assert_eq!(json["discovered_total"], 0);
    assert_eq!(json["selected_total"], 0);
    assert_eq!(json["targets"], serde_json::json!([]));
    assert_eq!(json["failures"], serde_json::json!([]));
    assert_eq!(json["failure"]["kind"], "selection");
    let failure = &json["failure"]["selection_failure"];
    assert_eq!(failure["stage"], "test-discovery");
    assert_eq!(failure["selector"], JsonValue::Null);
    assert_eq!(failure["target_count"], 0);
    assert!(
        failure["message"]
            .as_str()
            .expect("no-tests json failure should expose a message")
            .contains("found no `.ql` test files"),
        "no-tests json failure should describe missing tests: {json}"
    );
}

#[test]
fn test_package_path_json_reports_missing_target_selection_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-json-missing-target");
    let project_root = temp.path().join("app");
    fs::create_dir_all(project_root.join("src")).expect("create package source root");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("app/tests/smoke.ql", "fn main() -> Int { return 0 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test", "--json"])
        .arg(&project_root)
        .args(["--target", "tests/missing.ql"]);
    let output = run_command_capture(&mut command, "`ql test --json --target` missing target");
    let (stdout, stderr) = expect_exit_code(
        "project-test-json-missing-target",
        "missing target selection json failure",
        &output,
        1,
    )
    .expect("package-path `ql test --json --target tests/missing.ql` should fail");
    expect_empty_stderr(
        "project-test-json-missing-target",
        "missing target selection json failure",
        &stderr,
    )
    .expect("package-path `ql test --json --target` miss should stay on stdout");

    let json = parse_json_output("project-test-json-missing-target", &stdout);
    assert_eq!(json["schema"], "ql.test.v1");
    assert_eq!(json["status"], "no-match");
    assert_eq!(json["discovered_total"], 1);
    assert_eq!(json["selected_total"], 0);
    assert_eq!(json["targets"], serde_json::json!([]));
    assert_eq!(json["failure"]["kind"], "selection");
    let failure = &json["failure"]["selection_failure"];
    assert_eq!(failure["stage"], "target-selection");
    assert_eq!(failure["selector"], "target `tests/missing.ql`");
    assert_eq!(failure["target_count"], 1);
    assert!(
        failure["message"]
            .as_str()
            .expect("missing target json failure should expose a message")
            .contains("found no test target `tests/missing.ql`"),
        "missing target json failure should describe target miss: {json}"
    );
}

#[test]
fn test_package_path_json_reports_missing_filter_selection_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-json-missing-filter");
    let project_root = temp.path().join("app");
    fs::create_dir_all(project_root.join("src")).expect("create package source root");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("app/tests/smoke.ql", "fn main() -> Int { return 0 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test", "--json"])
        .arg(&project_root)
        .args(["--filter", "missing"]);
    let output = run_command_capture(&mut command, "`ql test --json --filter` missing filter");
    let (stdout, stderr) = expect_exit_code(
        "project-test-json-missing-filter",
        "missing filter selection json failure",
        &output,
        1,
    )
    .expect("package-path `ql test --json --filter missing` should fail");
    expect_empty_stderr(
        "project-test-json-missing-filter",
        "missing filter selection json failure",
        &stderr,
    )
    .expect("package-path `ql test --json --filter` miss should stay on stdout");

    let json = parse_json_output("project-test-json-missing-filter", &stdout);
    assert_eq!(json["schema"], "ql.test.v1");
    assert_eq!(json["filter"], "missing");
    assert_eq!(json["status"], "no-match");
    assert_eq!(json["discovered_total"], 1);
    assert_eq!(json["selected_total"], 0);
    assert_eq!(json["targets"], serde_json::json!([]));
    assert_eq!(json["failure"]["kind"], "selection");
    let failure = &json["failure"]["selection_failure"];
    assert_eq!(failure["stage"], "filter-selection");
    assert_eq!(failure["selector"], "filter `missing`");
    assert_eq!(failure["target_count"], 1);
    assert!(
        failure["message"]
            .as_str()
            .expect("missing filter json failure should expose a message")
            .contains("found no test files matching `missing`"),
        "missing filter json failure should describe filter miss: {json}"
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
fn test_workspace_path_json_package_selector_reports_missing_source_root_preflight_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package-selector-missing-source-root-json");
    let project_root = temp.path().join("workspace");
    fs::create_dir_all(project_root.join("packages/tool/src")).expect("create tool source root");
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
        "workspace/packages/tool/src/lib.ql",
        "pub fn helper() -> Int { return 1 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test", "--json"])
        .arg(&project_root)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --package` selected workspace package missing source root",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-package-selector-missing-source-root-json",
        "selected workspace package missing source root json",
        &output,
        1,
    )
    .expect("workspace-path `ql test --json --package app` should fail for missing selected source root");
    expect_empty_stderr(
        "project-test-package-selector-missing-source-root-json",
        "selected workspace package missing source root json",
        &stderr,
    )
    .expect("selected workspace package missing source root json should stay on stdout");

    let json = parse_json_output(
        "project-test-package-selector-missing-source-root-json",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.test.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["package_name"], "app");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["error_kind"], "manifest");
    assert_eq!(failure["stage"], "target-discovery");
    assert_eq!(failure["selector"], JsonValue::Null);
    assert_eq!(failure["target_count"], JsonValue::Null);
    assert!(
        failure["message"]
            .as_str()
            .expect("selected workspace missing source root json failure should expose a message")
            .contains("package source directory"),
        "selected workspace missing source root json failure should describe source root lookup: {json}"
    );
}

#[test]
fn test_package_path_json_package_selector_reports_missing_source_root_preflight_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package-path-selector-missing-source-root-json");
    let project_root = temp.path().join("app");
    fs::create_dir_all(&project_root).expect("create package directory");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test", "--json"])
        .arg(&project_root)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --package` package path missing source root",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-package-path-selector-missing-source-root-json",
        "package path selector missing source root json",
        &output,
        1,
    )
    .expect("package-path `ql test --json --package app` should fail for missing source root");
    expect_empty_stderr(
        "project-test-package-path-selector-missing-source-root-json",
        "package path selector missing source root json",
        &stderr,
    )
    .expect("package path selector missing source root json should stay on stdout");

    let json = parse_json_output(
        "project-test-package-path-selector-missing-source-root-json",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.test.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["package_name"], "app");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["error_kind"], "manifest");
    assert_eq!(failure["stage"], "target-discovery");
    assert_eq!(failure["selector"], JsonValue::Null);
    assert_eq!(failure["target_count"], JsonValue::Null);
    assert!(
        failure["message"]
            .as_str()
            .expect("package path missing source root json failure should expose a message")
            .contains("package source directory"),
        "package path missing source root json failure should describe source root lookup: {json}"
    );
}

#[test]
fn test_workspace_path_json_rejects_duplicate_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package-selector-duplicate-json");
    let project_root = temp.path().join("workspace");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/a", "packages/b"]
"#,
    );
    temp.write(
        "workspace/packages/a/qlang.toml",
        r#"
[package]
name = "util"
"#,
    );
    temp.write(
        "workspace/packages/a/src/lib.ql",
        "pub fn a() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/b/qlang.toml",
        r#"
[package]
name = "util"
"#,
    );
    temp.write(
        "workspace/packages/b/src/lib.ql",
        "pub fn b() -> Int { return 2 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test", "--json"])
        .arg(&project_root)
        .args(["--package", "util"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --package` duplicate workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-package-selector-duplicate-json",
        "duplicate workspace package selector json",
        &output,
        1,
    )
    .expect("workspace-path `ql test --json --package util` should reject duplicate package names");
    expect_empty_stderr(
        "project-test-package-selector-duplicate-json",
        "duplicate workspace package selector json",
        &stderr,
    )
    .expect("duplicate workspace package selector json should stay on stdout");

    let json = parse_json_output("project-test-package-selector-duplicate-json", &stdout);
    assert_eq!(json["schema"], "ql.test.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["package_name"], "util");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["error_kind"], "selector");
    assert_eq!(failure["stage"], "package-selection");
    assert_eq!(failure["selector"], "package `util`");
    assert_eq!(failure["target_count"], 2);
    assert!(
        failure["message"]
            .as_str()
            .expect("duplicate package selector json failure should expose a message")
            .contains("contains multiple members for package `util`"),
        "duplicate package selector json failure should describe ambiguous package matches: {json}"
    );
}

#[test]
fn test_package_path_json_rejects_unknown_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package-path-selector-missing-json");
    let project_root = temp.path().join("app");
    fs::create_dir_all(project_root.join("src")).expect("create package source root");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test", "--json"])
        .arg(&project_root)
        .args(["--package", "missing"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --package` unknown package path package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-package-path-selector-missing-json",
        "unknown package path package selector json",
        &output,
        1,
    )
    .expect("package-path `ql test --json --package missing` should reject package mismatch");
    expect_empty_stderr(
        "project-test-package-path-selector-missing-json",
        "unknown package path package selector json",
        &stderr,
    )
    .expect("unknown package path package selector json should stay on stdout");

    let json = parse_json_output("project-test-package-path-selector-missing-json", &stdout);
    assert_eq!(json["schema"], "ql.test.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["package_name"], "missing");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["error_kind"], "selector");
    assert_eq!(failure["stage"], "package-selection");
    assert_eq!(failure["selector"], "package `missing`");
    assert_eq!(failure["target_count"], 0);
    assert!(
        failure["message"]
            .as_str()
            .expect("package path selector json failure should expose a message")
            .contains("matched no workspace members"),
        "package path selector json failure should describe package mismatch: {json}"
    );
}

#[test]
fn test_workspace_path_rejects_unknown_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package-selector-missing");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app package source tree");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]
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
        "workspace/packages/app/src/lib.ql",
        "pub fn helper() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/app/tests/smoke.ql",
        "fn main() -> Int { return 0 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test"])
        .arg(&project_root)
        .args(["--package", "missing"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --package` unknown workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-package-selector-missing",
        "unknown workspace package selector",
        &output,
        1,
    )
    .expect("workspace-path `ql test --package` should reject unknown packages");
    expect_empty_stdout(
        "project-test-package-selector-missing",
        "unknown workspace package selector",
        &stdout,
    )
    .expect("unknown workspace package selector should not print stdout");
    expect_stderr_contains(
        "project-test-package-selector-missing",
        "unknown workspace package selector",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql test` package selector matched no workspace members under `{}`",
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .expect("unknown workspace package selector should report the missing package");
    expect_stderr_contains(
        "project-test-package-selector-missing",
        "unknown workspace package selector",
        &stderr.replace('\\', "/"),
        &format!(
            "hint: rerun `ql test {}` to inspect all workspace members, or adjust `--package`",
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .expect("unknown workspace package selector should suggest inspecting discovered members");
}

#[test]
fn test_workspace_path_rejects_duplicate_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package-selector-duplicate");
    let project_root = temp.path().join("workspace");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/a", "packages/b"]
"#,
    );
    temp.write(
        "workspace/packages/a/qlang.toml",
        r#"
[package]
name = "util"
"#,
    );
    temp.write(
        "workspace/packages/a/src/lib.ql",
        "pub fn a() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/a/tests/smoke.ql",
        "fn main() -> Int { return 0 }\n",
    );
    temp.write(
        "workspace/packages/b/qlang.toml",
        r#"
[package]
name = "util"
"#,
    );
    temp.write(
        "workspace/packages/b/src/lib.ql",
        "pub fn b() -> Int { return 2 }\n",
    );
    temp.write(
        "workspace/packages/b/tests/smoke.ql",
        "fn main() -> Int { return 0 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test"])
        .arg(&project_root)
        .args(["--package", "util"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --package` duplicate workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-package-selector-duplicate",
        "duplicate workspace package selector",
        &output,
        1,
    )
    .expect("workspace-path `ql test --package` should reject duplicate package names");
    expect_empty_stdout(
        "project-test-package-selector-duplicate",
        "duplicate workspace package selector",
        &stdout,
    )
    .expect("duplicate workspace package selector should not print stdout");
    expect_stderr_contains(
        "project-test-package-selector-duplicate",
        "duplicate workspace package selector",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql test` workspace manifest `{}` contains multiple members for package `util`: packages/a ({}/packages/a/qlang.toml), packages/b ({}/packages/b/qlang.toml)",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/"),
            project_root.to_string_lossy().replace('\\', "/"),
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .expect("duplicate workspace package selector should list matching members");
}

#[test]
fn test_workspace_path_rejects_invalid_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package-selector-invalid");
    let project_root = temp.path().join("workspace");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]
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
        "workspace/packages/app/src/lib.ql",
        "pub fn helper() -> Int { return 1 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test"])
        .arg(&project_root)
        .args(["--package", "packages/app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --package` invalid workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-package-selector-invalid",
        "invalid workspace package selector",
        &output,
        1,
    )
    .expect("workspace-path `ql test --package` should reject invalid package names");
    expect_empty_stdout(
        "project-test-package-selector-invalid",
        "invalid workspace package selector",
        &stdout,
    )
    .expect("invalid workspace package selector should not print stdout");
    expect_stderr_contains(
        "project-test-package-selector-invalid",
        "invalid workspace package selector",
        &stderr,
        "error: `ql test` does not accept package name `packages/app` because it contains a path separator",
    )
    .expect("invalid workspace package selector should report package-name validation");
}

#[test]
fn test_workspace_path_package_selector_surfaces_broken_member_metadata() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package-selector-broken-member");
    let project_root = temp.path().join("workspace");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken"]
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
        "workspace/packages/app/src/lib.ql",
        "pub fn helper() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/app/tests/smoke.ql",
        "fn main() -> Int { return 0 }\n",
    );
    temp.write(
        "workspace/packages/broken/qlang.toml",
        r#"
[package]
version = "0.1.0"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test"])
        .arg(&project_root)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --package` broken workspace member metadata",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-package-selector-broken-member",
        "broken workspace member package selector",
        &output,
        1,
    )
    .expect("workspace-path `ql test --package` should reject unresolved member metadata");
    expect_empty_stdout(
        "project-test-package-selector-broken-member",
        "broken workspace member package selector",
        &stdout,
    )
    .expect("broken workspace member package selector should not print stdout");
    expect_stderr_contains(
        "project-test-package-selector-broken-member",
        "broken workspace member package selector",
        &stderr.replace('\\', "/"),
        "error: `ql test` failed to inspect workspace member `packages/broken`: manifest",
    )
    .expect("broken workspace member package selector should surface the broken member");
    expect_stderr_contains(
        "project-test-package-selector-broken-member",
        "broken workspace member package selector",
        &stderr,
        "does not declare `[package].name`",
    )
    .expect("broken workspace member package selector should preserve package-name detail");
}

#[test]
fn test_workspace_path_package_selector_lists_selected_member_with_unbuildable_unselected_member() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package-selector-unselected-source-root");
    let project_root = temp.path().join("workspace");
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
        "workspace/packages/app/src/lib.ql",
        "pub fn helper() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/app/tests/smoke.ql",
        "fn main() -> Int { return 0 }\n",
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test"])
        .arg(&project_root)
        .args(["--package", "app", "--list"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --package --list` skips unselected member target discovery",
    );
    let (stdout, stderr) = expect_success(
        "project-test-package-selector-unselected-source-root",
        "selected workspace package test listing with unbuildable unselected member",
        &output,
    )
    .expect(
        "workspace-path `ql test --package app --list` should not inspect unselected build targets",
    );
    expect_empty_stderr(
        "project-test-package-selector-unselected-source-root",
        "selected workspace package test listing with unbuildable unselected member",
        &stderr,
    )
    .expect("selected workspace package test listing should not print stderr");
    expect_stdout_contains_all(
        "project-test-package-selector-unselected-source-root",
        &stdout.replace('\\', "/"),
        &["packages/app/tests/smoke.ql", "test listing: 1 discovered"],
    )
    .expect("selected workspace package test listing should list only selected package tests");
}

#[test]
fn test_package_path_filters_discovered_tests() {
    if !toolchain_available("`ql test --filter` package test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-filter");
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
    command
        .args(["test", "--filter", "basic"])
        .arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test --filter` package path");
    let (stdout, stderr) = expect_success(
        "project-test-filter",
        "filtered package smoke tests",
        &output,
    )
    .expect("package-path `ql test --filter` should succeed");
    expect_empty_stderr(
        "project-test-filter",
        "filtered package smoke tests",
        &stderr,
    )
    .expect("package-path `ql test --filter` should not print stderr");
    expect_stdout_contains_all(
        "project-test-filter",
        &stdout.replace('\\', "/"),
        &[
            "test tests/api/basic.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("package-path `ql test --filter` should run only matching tests");
}

#[test]
fn test_direct_source_file_rejects_package_selector_without_project_context() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-direct-source-package-selector");
    let source_path = temp.write("sample.ql", "fn main() -> Int { return 0 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test"])
        .arg(&source_path)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test` direct source package selector requires project context",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-direct-source-package-selector",
        "direct source package selector preflight failure",
        &output,
        1,
    )
    .expect("direct source file `ql test --package app` should exit with code 1");
    expect_empty_stdout(
        "project-test-direct-source-package-selector",
        "direct source package selector preflight failure",
        &stdout,
    )
    .expect("direct source package selector preflight failure should not print stdout");
    expect_stderr_contains(
        "project-test-direct-source-package-selector",
        "direct source package selector preflight failure",
        &stderr,
        "error: `ql test` package selectors require a package or workspace path",
    )
    .expect("direct source package selector preflight failure should explain the project-context requirement");
    expect_stderr_contains(
        "project-test-direct-source-package-selector",
        "direct source package selector preflight failure",
        &stderr,
        "note: selector: package `app`",
    )
    .expect("direct source package selector preflight failure should print the selector note");
}

#[test]
fn test_direct_source_file_json_rejects_package_selector_without_project_context() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-direct-source-package-selector-json");
    let source_path = temp.write("sample.ql", "fn main() -> Int { return 0 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test", "--json"])
        .arg(&source_path)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --json` direct source package selector requires project context",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-direct-source-package-selector-json",
        "direct source package selector json preflight failure",
        &output,
        1,
    )
    .expect("direct source file `ql test --json --package app` should exit with code 1");
    expect_empty_stderr(
        "project-test-direct-source-package-selector-json",
        "direct source package selector json preflight failure",
        &stderr,
    )
    .expect("direct source package selector json preflight failure should stay on stdout");

    let json = parse_json_output("project-test-direct-source-package-selector-json", &stdout);
    assert_eq!(json["schema"], "ql.test.v1");
    assert_eq!(
        json["path"],
        source_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["package_name"], "app");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["discovered_total"], 0);
    assert_eq!(json["selected_total"], 0);
    assert_eq!(json["targets"], serde_json::json!([]));
    assert_eq!(json["failures"], serde_json::json!([]));
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["error_kind"], "selector");
    assert_eq!(failure["stage"], "target-selection");
    assert_eq!(failure["selector"], "package `app`");
    assert_eq!(failure["target_count"], JsonValue::Null);
    assert!(
        failure["message"]
            .as_str()
            .expect("direct source package selector json failure should expose a message")
            .contains("package selectors require a package or workspace path"),
        "direct source package selector json failure should explain project context: {json}"
    );
}

#[test]
fn test_direct_source_file_json_rejects_target_selector_without_project_context() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-direct-source-target-selector-json");
    let source_path = temp.write("sample.ql", "fn main() -> Int { return 0 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test", "--json"])
        .arg(&source_path)
        .args(["--target", "tests/smoke.ql"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --json` direct source target selector requires project context",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-direct-source-target-selector-json",
        "direct source target selector json preflight failure",
        &output,
        1,
    )
    .expect("direct source file `ql test --json --target tests/smoke.ql` should exit with code 1");
    expect_empty_stderr(
        "project-test-direct-source-target-selector-json",
        "direct source target selector json preflight failure",
        &stderr,
    )
    .expect("direct source target selector json preflight failure should stay on stdout");

    let json = parse_json_output("project-test-direct-source-target-selector-json", &stdout);
    assert_eq!(json["schema"], "ql.test.v1");
    assert_eq!(
        json["path"],
        source_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["package_name"], JsonValue::Null);
    assert_eq!(json["status"], "failed");
    assert_eq!(json["discovered_total"], 0);
    assert_eq!(json["selected_total"], 0);
    assert_eq!(json["targets"], serde_json::json!([]));
    assert_eq!(json["failures"], serde_json::json!([]));
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["error_kind"], "selector");
    assert_eq!(failure["stage"], "target-selection");
    assert_eq!(failure["selector"], "target `tests/smoke.ql`");
    assert_eq!(failure["target_count"], JsonValue::Null);
    assert!(
        failure["message"]
            .as_str()
            .expect("direct source target selector json failure should expose a message")
            .contains("target selectors require a package or workspace path"),
        "direct source target selector json failure should explain project context: {json}"
    );
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
fn test_package_path_reports_missing_filter_matches() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-filter-missing");
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

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test", "--filter", "missing"])
        .arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test --filter` missing matches");
    let (stdout, stderr) = expect_exit_code(
        "project-test-filter-missing",
        "missing filtered package tests",
        &output,
        1,
    )
    .expect("`ql test --filter` should reject filters without matches");
    expect_empty_stdout(
        "project-test-filter-missing",
        "missing filtered package tests",
        &stdout,
    )
    .expect("missing filtered package tests should not print stdout");
    expect_stderr_contains(
        "project-test-filter-missing",
        "missing filtered package tests",
        &stderr,
        "error: `ql test` found no test files matching `missing`",
    )
    .expect("missing filtered package tests should report the unmatched filter");
    expect_stderr_contains(
        "project-test-filter-missing",
        "missing filtered package tests",
        &stderr,
        "--list",
    )
    .expect("missing filtered package tests should suggest listing discovered tests");
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
fn test_direct_project_smoke_file_combines_local_generic_and_direct_dependency_bridge() {
    if !toolchain_available(
        "`ql test` direct project smoke file with local generic plus direct dependency bridge",
    ) {
        return;
    }

    let fixture = write_dependency_smoke_project(
        "ql-project-test-direct-file-local-generic-dependency-bridge",
        "pub fn add(left: Int, right: Int) -> Int { return left + right }\n",
        r#"
use dep.add as sum

fn identity[T](value: T) -> T {
    return value
}

fn main() -> Int {
    let value: Int = identity(sum(9, 4))
    return value - 13
}
"#,
    );
    expect_direct_dependency_smoke_file_passes(
        "project-test-direct-file-local-generic-dependency-bridge",
        "direct project smoke file combining local generic specialization and direct dependency bridge",
        "`ql test` direct project smoke file with local generic plus direct dependency bridge",
        &fixture,
    );
}

#[test]
fn test_direct_project_smoke_file_supports_dependency_generic_public_function_bridge() {
    if !toolchain_available("`ql test` direct project smoke file dependency generic bridge") {
        return;
    }

    let fixture = write_dependency_smoke_project(
        "ql-project-test-direct-file-dependency-generic-bridge",
        r#"
pub fn identity[T](value: T) -> T {
    return value
}

pub fn first[T, N](values: [T; N]) -> T {
    return values[0]
}
"#,
        r#"
use dep.identity as identity
use dep.first as first

fn main() -> Int {
    let value: Int = identity(7)
    let picked: Int = first([5, 8, 13])
    return value + picked - 12
}
"#,
    );
    expect_direct_dependency_smoke_file_passes(
        "project-test-direct-file-dependency-generic-bridge",
        "direct project smoke file dependency generic bridge",
        "`ql test` direct project smoke file dependency generic bridge",
        &fixture,
    );
}

#[test]
fn test_direct_project_smoke_file_supports_dependency_generic_wrapper_helper_bridge() {
    if !toolchain_available(
        "`ql test` direct project smoke file dependency generic wrapper helper bridge",
    ) {
        return;
    }

    let fixture = write_dependency_smoke_project(
        "ql-project-test-direct-file-dependency-generic-wrapper-helper",
        r#"
pub fn first[T, N](values: [T; N]) -> T {
    return values[0]
}

pub fn first_wrapped[T, N](values: [T; N]) -> T {
    return first(values)
}
"#,
        r#"
use dep.first_wrapped as first_wrapped

fn main() -> Int {
    return first_wrapped([7, 8, 9]) - 7
}
"#,
    );
    expect_direct_dependency_smoke_file_passes(
        "project-test-direct-file-dependency-generic-wrapper-helper",
        "direct project smoke file dependency generic wrapper helper bridge",
        "`ql test` direct project smoke file dependency generic wrapper helper bridge",
        &fixture,
    );
}

#[test]
fn test_direct_project_smoke_file_dependency_generic_bridge_reports_json_success() {
    if !toolchain_available("`ql test --json` direct project smoke file dependency generic bridge")
    {
        return;
    }

    let fixture = write_dependency_smoke_project(
        "ql-project-test-direct-file-dependency-generic-json",
        r#"
pub fn identity[T](value: T) -> T {
    return value
}

pub fn first[T, N](values: [T; N]) -> T {
    return values[0]
}
"#,
        r#"
use dep.identity as identity
use dep.first as first

fn main() -> Int {
    let value: Int = identity(7)
    let picked: Int = first([5, 8, 13])
    return value + picked - 12
}
"#,
    );
    let workspace_root = workspace_root();
    let smoke_path = fixture.project_root.join("tests/smoke.ql");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["test", "--json"]).arg(&smoke_path);
    let output = run_command_capture(
        &mut command,
        "`ql test --json` direct project smoke file dependency generic bridge",
    );
    let (stdout, stderr) = expect_success(
        "project-test-direct-file-dependency-generic-json",
        "direct project smoke file dependency generic json bridge",
        &output,
    )
    .expect("direct project smoke files should report dependency generic bridge success as json");
    expect_empty_stderr(
        "project-test-direct-file-dependency-generic-json",
        "direct project smoke file dependency generic json bridge",
        &stderr,
    )
    .expect("direct project dependency generic json bridge should not print stderr");

    let actual = parse_json_output("project-test-direct-file-dependency-generic-json", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.test.v1",
        "path": smoke_path.display().to_string().replace('\\', "/"),
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
                "path": "tests/smoke.ql",
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
        "direct project smoke file `ql test --json` should preserve the project test-file contract"
    );
    expect_file_exists(
        "project-test-direct-file-dependency-generic-json",
        &fixture.interface_output,
        "synced dependency interface",
        "`ql test --json` direct project smoke file dependency generic bridge",
    )
    .expect("direct project dependency generic json bridge should emit the dependency interface");
    expect_file_exists(
        "project-test-direct-file-dependency-generic-json",
        &fixture.dependency_output,
        "dependency package artifact",
        "`ql test --json` direct project smoke file dependency generic bridge",
    )
    .expect("direct project dependency generic json bridge should build the dependency artifact");
    expect_file_exists(
        "project-test-direct-file-dependency-generic-json",
        &fixture.smoke_output,
        "direct project dependency generic json smoke executable",
        "`ql test --json` direct project smoke file dependency generic bridge",
    )
    .expect("direct project dependency generic json bridge should emit the smoke executable");
}

#[test]
fn test_project_path_json_allows_concurrent_dependency_generic_smoke_tests() {
    if !toolchain_available("concurrent `ql test --json` dependency generic smoke test") {
        return;
    }

    let fixture = write_dependency_smoke_project(
        "ql-project-test-json-concurrent-dependency-generic",
        r#"
pub fn identity[T](value: T) -> T {
    return value
}

pub fn first[T, N](values: [T; N]) -> T {
    return values[0]
}
"#,
        r#"
use dep.identity as identity
use dep.first as first

fn main() -> Int {
    let value: Int = identity(7)
    let picked: Int = first([5, 8, 13])
    return value + picked - 12
}
"#,
    );
    let workspace_root = workspace_root();
    let mut children = Vec::new();
    for index in 0..3 {
        let mut command = ql_command(&workspace_root);
        command.current_dir(fixture.temp.path());
        command.args(["test", "--json"]).arg(&fixture.project_root);
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let child = command
            .spawn()
            .unwrap_or_else(|error| panic!("spawn concurrent ql test #{index}: {error}"));
        children.push((index, child));
    }

    for (index, child) in children {
        let output = child
            .wait_with_output()
            .unwrap_or_else(|error| panic!("wait for concurrent ql test #{index}: {error}"));
        let (stdout, stderr) = expect_success(
            "project-test-json-concurrent-dependency-generic",
            &format!("concurrent dependency generic smoke test #{index}"),
            &output,
        )
        .unwrap_or_else(|message| panic!("{message}"));
        expect_empty_stderr(
            "project-test-json-concurrent-dependency-generic",
            &format!("concurrent dependency generic smoke test #{index}"),
            &stderr,
        )
        .unwrap_or_else(|message| panic!("{message}"));

        let json = parse_json_output("project-test-json-concurrent-dependency-generic", &stdout);
        assert_eq!(json["schema"], "ql.test.v1");
        assert_eq!(
            json["status"], "ok",
            "concurrent test #{index} failed: {json}"
        );
        assert_eq!(json["passed"], 1);
        assert_eq!(json["failed"], 0);
        assert_eq!(json["failures"], serde_json::json!([]));
    }

    expect_file_exists(
        "project-test-json-concurrent-dependency-generic",
        &fixture.interface_output,
        "synced dependency interface",
        "concurrent dependency generic smoke test",
    )
    .expect("concurrent dependency generic smoke tests should emit the dependency interface");
    expect_file_exists(
        "project-test-json-concurrent-dependency-generic",
        &fixture.dependency_output,
        "dependency package artifact",
        "concurrent dependency generic smoke test",
    )
    .expect("concurrent dependency generic smoke tests should build the dependency artifact");
    expect_file_exists(
        "project-test-json-concurrent-dependency-generic",
        &fixture.smoke_output,
        "dependency generic smoke executable",
        "concurrent dependency generic smoke test",
    )
    .expect("concurrent dependency generic smoke tests should emit the smoke executable");
    assert_no_build_lock_directories(
        "project-test-json-concurrent-dependency-generic",
        &fixture.project_root,
    );
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

#[test]
fn test_package_path_reports_missing_tests() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-missing");
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

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test` missing tests");
    let (stdout, stderr) =
        expect_exit_code("project-test-missing", "missing package tests", &output, 1)
            .expect("`ql test` should reject package paths without discovered tests");
    expect_empty_stdout("project-test-missing", "missing package tests", &stdout)
        .expect("missing package tests should not print stdout");
    expect_stderr_contains(
        "project-test-missing",
        "missing package tests",
        &stderr,
        "error: `ql test` found no `.ql` test files",
    )
    .expect("missing package tests should report the missing smoke tests");
}

#[test]
fn test_package_path_syncs_dependency_interfaces_without_polluting_test_output() {
    if !toolchain_available("`ql test` dependency sync test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-dependency-sync");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source tree");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source root");
    temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "dep/src/lib.ql",
        "extern \"c\" pub fn q_add(left: Int, right: Int) -> Int { return left + right }\n",
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
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write(
        "app/tests/smoke.ql",
        "use dep.q_add as add\n\nfn main() -> Int {\n    if add(5, 8) == 13 {\n        return 0\n    }\n    return 1\n}\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    assert!(
        !interface_output.exists(),
        "dependency interface should start missing for sync test"
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test` dependency sync");
    let (stdout, stderr) = expect_success(
        "project-test-dependency-sync",
        "package tests with dependency sync",
        &output,
    )
    .expect("package-path `ql test` should sync dependency interfaces before smoke tests");
    expect_empty_stderr(
        "project-test-dependency-sync",
        "package tests with dependency sync",
        &stderr,
    )
    .expect("dependency-sync tests should not print stderr");
    expect_stdout_contains_all(
        "project-test-dependency-sync",
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("dependency-sync tests should still render the normal smoke test output");
    assert!(
        !stdout.contains("wrote interface:"),
        "dependency-sync tests should not print interface sync messages on stdout: {stdout}"
    );
    assert!(
        !stdout.contains("wrote staticlib:"),
        "dependency-sync tests should not print dependency build messages on stdout: {stdout}"
    );
    expect_file_exists(
        "project-test-dependency-sync",
        &interface_output,
        "synced dependency interface",
        "package tests with dependency sync",
    )
    .expect("dependency-sync tests should emit the dependency interface");
    expect_file_exists(
        "project-test-dependency-sync",
        &dependency_output,
        "dependency package artifact",
        "package tests with dependency sync",
    )
    .expect("dependency-sync tests should also build the dependency package artifact");
}
