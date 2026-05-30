mod support;

use std::fs;
use std::path::PathBuf;

use ql_driver::{ToolchainOptions, discover_toolchain};
use serde_json::Value as JsonValue;
use support::{
    TempDir, executable_output_path, expect_empty_stderr, expect_file_exists,
    expect_stdout_contains_all, expect_success, ql_command, run_command_capture,
    static_library_output_path, workspace_root,
};

struct CurrentPackageSmokeProject {
    temp: TempDir,
    project_root: PathBuf,
    smoke_path: PathBuf,
    package_output: PathBuf,
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

fn toolchain_available(context: &str) -> bool {
    let Ok(_toolchain) = discover_toolchain(&ToolchainOptions::default()) else {
        eprintln!(
            "skipping {context}: no clang-style compiler found via ql-driver toolchain discovery"
        );
        return false;
    };
    true
}

fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&support::normalize(stdout))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
}

fn write_current_package_smoke_project(
    prefix: &str,
    library_source: &str,
    smoke_source: &str,
) -> CurrentPackageSmokeProject {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("app");
    fs::create_dir_all(project_root.join("src")).expect("create package source root");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", library_source);
    let smoke_path = temp.write("app/tests/smoke.ql", smoke_source);
    let package_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");
    let smoke_output = executable_output_path(&project_root.join("target/ql/debug/tests"), "smoke");

    CurrentPackageSmokeProject {
        temp,
        project_root,
        smoke_path,
        package_output,
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

fn expect_current_package_project_passes(
    case_name: &str,
    action: &str,
    command_description: &str,
    fixture: &CurrentPackageSmokeProject,
) {
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["test"]).arg(&fixture.project_root);
    let output = run_command_capture(&mut command, command_description);
    expect_current_package_output(case_name, action, &output, fixture);
}

fn expect_current_package_direct_file_passes(
    case_name: &str,
    action: &str,
    command_description: &str,
    fixture: &CurrentPackageSmokeProject,
) {
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["test"]).arg(&fixture.smoke_path);
    let output = run_command_capture(&mut command, command_description);
    expect_current_package_output(case_name, action, &output, fixture);
}

fn expect_current_package_output(
    case_name: &str,
    action: &str,
    output: &std::process::Output,
    fixture: &CurrentPackageSmokeProject,
) {
    let (stdout, stderr) = expect_success(case_name, action, output)
        .expect("current package bridge smoke project should pass `ql test`");
    expect_empty_stderr(case_name, action, &stderr)
        .expect("current package bridge smoke project should not print stderr");
    expect_stdout_contains_all(
        case_name,
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("current package bridge smoke project should report one passing smoke test");
    expect_file_exists(
        case_name,
        &fixture.package_output,
        "current package library",
        action,
    )
    .expect("current package library should be prebuilt for smoke tests");
    expect_file_exists(
        case_name,
        &fixture.smoke_output,
        "current package smoke executable",
        action,
    )
    .expect("current package bridge smoke project should emit the smoke executable");
}

#[test]
fn test_package_tests_can_import_current_package_public_functions() {
    if !toolchain_available("`ql test` current package public function test") {
        return;
    }

    let fixture = write_current_package_smoke_project(
        "ql-project-test-current-package-function",
        "pub fn pass_status(value: Int) -> Int {\n    if value == 42 {\n        return 0\n    }\n    return 1\n}\n",
        "use app.pass_status as pass\n\nfn main() -> Int {\n    return pass(42)\n}\n",
    );
    expect_current_package_project_passes(
        "project-test-current-package-function",
        "package tests importing current package public function",
        "`ql test` current package public function",
        &fixture,
    );
}

#[test]
fn test_package_tests_support_current_package_generic_public_function_single_instantiation() {
    if !toolchain_available("`ql test` current package generic public function test") {
        return;
    }

    let fixture = write_current_package_smoke_project(
        "ql-project-test-current-package-generic-function",
        r#"
pub fn identity[T](value: T) -> T {
    return value
}
"#,
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
    expect_current_package_project_passes(
        "project-test-current-package-generic-function",
        "package tests importing current package generic public function",
        "`ql test` current package generic public function",
        &fixture,
    );
}

#[test]
fn test_package_tests_support_current_package_generic_public_function_multiple_instantiations() {
    if !toolchain_available(
        "`ql test` current package multi-instantiation generic public function test",
    ) {
        return;
    }

    let fixture = write_current_package_smoke_project(
        "ql-project-test-current-package-multiple-generic-instantiations",
        r#"
pub fn identity[T](value: T) -> T {
    return value
}
"#,
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
    expect_current_package_project_passes(
        "project-test-current-package-multiple-generic-instantiations",
        "package tests importing current package generic function with multiple concrete instantiations",
        "`ql test` current package multiple generic public function instantiations",
        &fixture,
    );
}

#[test]
fn test_package_tests_support_current_package_generic_function_named_arguments() {
    if !toolchain_available("`ql test` current package generic function named arguments") {
        return;
    }

    let fixture = write_current_package_smoke_project(
        "ql-project-test-current-package-generic-function-named-args",
        r#"
pub fn choose[T](fallback: T, value: T) -> T {
    return value
}
"#,
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
    expect_current_package_project_passes(
        "project-test-current-package-generic-function-named-args",
        "package tests importing current package generic function with named arguments",
        "`ql test` current package generic public function named arguments",
        &fixture,
    );
}

#[test]
fn test_package_tests_support_current_package_generic_function_expression_arguments() {
    if !toolchain_available("`ql test` current package generic function expression arguments") {
        return;
    }

    let fixture = write_current_package_smoke_project(
        "ql-project-test-current-package-generic-function-expressions",
        r#"
pub fn identity[T](value: T) -> T {
    return value
}

pub fn choose[T](fallback: T, value: T) -> T {
    return value
}
"#,
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
    expect_current_package_project_passes(
        "project-test-current-package-generic-function-expressions",
        "package tests importing current package generic function with expression arguments",
        "`ql test` current package generic public function expression arguments",
        &fixture,
    );
}

#[test]
fn test_package_tests_support_current_package_generic_function_from_generic_carriers() {
    if !toolchain_available("`ql test` current package generic carrier function test") {
        return;
    }

    let fixture = write_current_package_smoke_project(
        "ql-project-test-current-package-generic-function-carrier",
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
    expect_current_package_project_passes(
        "project-test-current-package-generic-function-carrier",
        "package tests importing current package generic function with generic carrier values",
        "`ql test` current package generic carrier function",
        &fixture,
    );
}

#[test]
fn test_package_tests_support_current_package_generic_function_from_result_context() {
    if !toolchain_available("`ql test` current package generic result-context function test") {
        return;
    }

    let fixture = write_current_package_smoke_project(
        "ql-project-test-current-package-generic-function-result-context",
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
    expect_current_package_project_passes(
        "project-test-current-package-generic-function-result-context",
        "package tests importing current package generic function with result context",
        "`ql test` current package generic result-context function",
        &fixture,
    );
}

#[test]
fn test_package_tests_support_current_package_generic_function_from_nested_context() {
    if !toolchain_available("`ql test` current package nested generic function test") {
        return;
    }

    let fixture = write_current_package_smoke_project(
        "ql-project-test-current-package-nested-generic-function",
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
    expect_current_package_project_passes(
        "project-test-current-package-nested-generic-function",
        "package tests importing current package generic function in nested context",
        "`ql test` current package nested generic function",
        &fixture,
    );
}

#[test]
fn test_package_tests_combine_local_generic_and_current_package_public_bridges() {
    if !toolchain_available("`ql test` local generic plus current package bridge test") {
        return;
    }

    let fixture = write_current_package_smoke_project(
        "ql-project-test-local-generic-current-package-bridge",
        r#"
pub fn score(value: Int) -> Int {
    return value + 3
}
"#,
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
    expect_current_package_project_passes(
        "project-test-local-generic-current-package-bridge",
        "package test combining local generic specialization and current package bridge",
        "`ql test` local generic plus current package bridge",
        &fixture,
    );
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
        "workspace all-member bridge json",
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
fn test_direct_project_smoke_file_combines_local_generic_and_package_under_test_bridge() {
    if !toolchain_available(
        "`ql test` direct project smoke file with local generic plus current package bridge",
    ) {
        return;
    }

    let fixture = write_current_package_smoke_project(
        "ql-project-test-direct-file-local-generic-current-package-bridge",
        r#"
pub fn score(value: Int) -> Int {
    return value + 3
}
"#,
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
    expect_current_package_direct_file_passes(
        "project-test-direct-file-local-generic-current-package-bridge",
        "direct project smoke file combining local generic specialization and current package bridge",
        "`ql test` direct project smoke file with local generic plus current package bridge",
        &fixture,
    );
}
