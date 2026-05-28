mod support;

use std::fs;
use std::process::Stdio;

use ql_driver::{ToolchainOptions, discover_toolchain};
use serde_json::Value as JsonValue;
use support::{
    DependencySmokeProject, TempDir, assert_no_build_lock_directories, executable_output_path,
    expect_empty_stderr, expect_file_exists, expect_stdout_contains_all, expect_success,
    ql_command, run_command_capture, static_library_output_path, workspace_root,
    write_dependency_smoke_project,
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

fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&support::normalize(stdout))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
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
    fs::create_dir_all(helper_root.join("src"))
        .expect("create helper source tree for imported generic helper test");
    fs::create_dir_all(dep_root.join("src"))
        .expect("create dep source tree for imported generic helper test");
    fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for imported generic helper test");
    fs::create_dir_all(project_root.join("tests"))
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
fn test_package_path_supports_direct_dependency_public_struct_functions() {
    if !toolchain_available("`ql test` dependency public struct function test") {
        return;
    }

    let fixture = write_dependency_smoke_project(
        "ql-project-test-dependency-public-struct-function",
        "pub struct Box { value: Int }\npub fn make_box() -> Box { return Box { value: 7 } }\n",
        "use dep.make_box as make\n\nfn main() -> Int {\n    let value = make()\n    return value.value - 7\n}\n",
    );
    expect_dependency_smoke_project_passes(
        "project-test-dependency-public-struct-function",
        "package dependency public struct function test",
        "`ql test` dependency public struct function",
        &fixture,
    );
    assert!(
        fixture.dependency_manifest.exists(),
        "dependency manifest should remain present after dependency public struct function test"
    );
}

#[test]
fn test_package_path_supports_direct_dependency_public_type_alias_functions() {
    if !toolchain_available("`ql test` dependency public type alias function test") {
        return;
    }

    let fixture = write_dependency_smoke_project(
        "ql-project-test-dependency-public-type-alias-function",
        "pub type Count = Int\npub type Score = Count\npub fn make_score(value: Count) -> Score { return value + 2 }\npub fn unwrap_score(value: Score) -> Int { return value }\n",
        "use dep.make_score as make_score\nuse dep.unwrap_score as unwrap_score\n\nfn main() -> Int {\n    return unwrap_score(make_score(5)) - 7\n}\n",
    );
    expect_dependency_smoke_project_passes(
        "project-test-dependency-public-type-alias-function",
        "package dependency public type alias function test",
        "`ql test` dependency public type alias function",
        &fixture,
    );
    assert!(
        fixture.dependency_manifest.exists(),
        "dependency manifest should remain present after dependency public type alias function test"
    );
}

#[test]
fn test_package_path_supports_direct_dependency_public_struct_methods() {
    if !toolchain_available("`ql test` dependency public struct method test") {
        return;
    }

    let fixture = write_dependency_smoke_project(
        "ql-project-test-dependency-public-struct-method",
        "pub struct Box { value: Int }\n\nimpl Box {\n    pub fn read(self) -> Int {\n        return self.value\n    }\n}\n\npub fn make_box() -> Box { return Box { value: 7 } }\n",
        "use dep.make_box as make\n\nfn main() -> Int {\n    let value = make()\n    return value.read() - 7\n}\n",
    );
    expect_dependency_smoke_project_passes(
        "project-test-dependency-public-struct-method",
        "package dependency public struct method test",
        "`ql test` dependency public struct method",
        &fixture,
    );
}

#[test]
fn test_package_path_supports_direct_dependency_public_struct_method_values() {
    if !toolchain_available("`ql test` dependency public struct method value test") {
        return;
    }

    let fixture = write_dependency_smoke_project(
        "ql-project-test-dependency-public-struct-method-value",
        "pub struct Box { value: Int }\n\nimpl Box {\n    pub fn add(self, delta: Int) -> Int {\n        return self.value + delta\n    }\n}\n\npub fn make_box() -> Box { return Box { value: 7 } }\n",
        "use dep.make_box as make\n\nfn main() -> Int {\n    let value = make()\n    let add = value.add\n    return add(5) - 12\n}\n",
    );
    expect_dependency_smoke_project_passes(
        "project-test-dependency-public-struct-method-value",
        "package dependency public struct method value test",
        "`ql test` dependency public struct method value",
        &fixture,
    );
}

#[test]
fn test_package_path_supports_direct_dependency_public_trait_methods() {
    if !toolchain_available("`ql test` dependency public trait method test") {
        return;
    }

    let fixture = write_dependency_smoke_project(
        "ql-project-test-dependency-public-trait-method",
        "pub trait Reader {\n    fn read(self) -> Int\n}\n\npub struct Box { value: Int }\n\nimpl Reader for Box {\n    pub fn read(self) -> Int {\n        return self.value\n    }\n}\n\npub fn make_box() -> Box { return Box { value: 9 } }\n",
        "use dep.make_box as make\n\nfn main() -> Int {\n    let value = make()\n    return value.read() - 9\n}\n",
    );
    expect_dependency_smoke_project_passes(
        "project-test-dependency-public-trait-method",
        "package dependency public trait method test",
        "`ql test` dependency public trait method",
        &fixture,
    );
}

#[test]
fn test_package_path_syncs_dependency_interfaces_without_polluting_test_output() {
    if !toolchain_available("`ql test` dependency sync test") {
        return;
    }

    let fixture = write_dependency_smoke_project(
        "ql-project-test-dependency-sync",
        "extern \"c\" pub fn q_add(left: Int, right: Int) -> Int { return left + right }\n",
        "use dep.q_add as add\n\nfn main() -> Int {\n    if add(5, 8) == 13 {\n        return 0\n    }\n    return 1\n}\n",
    );

    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["test"]).arg(&fixture.project_root);
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
        &fixture.interface_output,
        "synced dependency interface",
        "package tests with dependency sync",
    )
    .expect("dependency-sync tests should emit the dependency interface");
    expect_file_exists(
        "project-test-dependency-sync",
        &fixture.dependency_output,
        "dependency package artifact",
        "package tests with dependency sync",
    )
    .expect("dependency-sync tests should also build the dependency package artifact");
}
