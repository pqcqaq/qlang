mod support;

use std::fs;
use std::path::Path;

use ql_analysis::analyze_source;
use ql_diagnostics::render_diagnostics;
use ql_driver::{ToolchainOptions, discover_toolchain};
use serde_json::Value as JsonValue;
use support::{
    TempDir, executable_output_path, expect_empty_stderr, expect_empty_stdout, expect_exit_code,
    expect_file_exists, expect_stderr_contains, expect_stdout_contains_all, expect_success,
    ql_command, run_command_capture, static_library_output_path, workspace_root,
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

fn ui_snapshot(diagnostic_path: &str, source: &str) -> String {
    let diagnostics = match analyze_source(source) {
        Ok(analysis) if analysis.has_errors() => analysis.diagnostics().to_vec(),
        Ok(_) => panic!("expected `{diagnostic_path}` ui fixture to produce diagnostics"),
        Err(diagnostics) => diagnostics,
    };
    normalize_output_text(&render_diagnostics(
        Path::new(diagnostic_path),
        source,
        &diagnostics,
    ))
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
fn test_package_path_uses_manifest_default_release_profile() {
    if !toolchain_available("`ql test` manifest profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-manifest-profile");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source root");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[profile]
default = "release"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("app/tests/smoke.ql", "fn main() -> Int { return 0 }\n");

    let smoke_output =
        executable_output_path(&project_root.join("target/ql/release/tests"), "smoke");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test` manifest default profile");
    let (stdout, stderr) = expect_success(
        "project-test-manifest-profile",
        "manifest default profile test",
        &output,
    )
    .expect("package-path `ql test` should honor the manifest default profile");
    expect_empty_stderr(
        "project-test-manifest-profile",
        "manifest default profile test",
        &stderr,
    )
    .expect("manifest default profile test should not print stderr");
    expect_stdout_contains_all(
        "project-test-manifest-profile",
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("manifest default profile test should still run discovered smoke tests");
    expect_file_exists(
        "project-test-manifest-profile",
        &smoke_output,
        "manifest default profile smoke executable",
        "manifest default profile test",
    )
    .expect("manifest default profile test should emit smoke test artifacts under release");
}

#[test]
fn test_workspace_path_uses_workspace_default_profile() {
    if !toolchain_available("`ql test` workspace profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-workspace-profile");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create workspace package source tree for workspace profile test");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]

[profile]
default = "release"
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

    let smoke_output = executable_output_path(
        &project_root.join("packages/app/target/ql/release/tests"),
        "smoke",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test` workspace default profile");
    let (stdout, stderr) = expect_success(
        "project-test-workspace-profile",
        "workspace default profile test",
        &output,
    )
    .expect("workspace-path `ql test` should honor the workspace default profile");
    expect_empty_stderr(
        "project-test-workspace-profile",
        "workspace default profile test",
        &stderr,
    )
    .expect("workspace default profile test should not print stderr");
    expect_stdout_contains_all(
        "project-test-workspace-profile",
        &stdout.replace('\\', "/"),
        &[
            "test packages/app/tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("workspace default profile test should still run discovered smoke tests");
    expect_file_exists(
        "project-test-workspace-profile",
        &smoke_output,
        "workspace default profile smoke executable",
        "workspace default profile test",
    )
    .expect("workspace default profile test should emit smoke test artifacts under release");
}

#[test]
fn test_workspace_member_file_uses_workspace_default_profile() {
    if !toolchain_available("`ql test` workspace member file profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-workspace-member-file-profile");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create workspace package source tree for workspace member file profile test");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]

[profile]
default = "release"
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
    let smoke_path = temp.write(
        "workspace/packages/app/tests/smoke.ql",
        "fn main() -> Int { return 0 }\n",
    );
    temp.write(
        "workspace/packages/app/tests/other.ql",
        "fn main() -> Int { return 0 }\n",
    );

    let smoke_output = executable_output_path(
        &project_root.join("packages/app/target/ql/release/tests"),
        "smoke",
    );
    let other_output = executable_output_path(
        &project_root.join("packages/app/target/ql/release/tests"),
        "other",
    );
    let debug_smoke_output = executable_output_path(
        &project_root.join("packages/app/target/ql/debug/tests"),
        "smoke",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&smoke_path);
    let output = run_command_capture(&mut command, "`ql test` workspace member file profile");
    let (stdout, stderr) = expect_success(
        "project-test-workspace-member-file-profile",
        "workspace member file profile test",
        &output,
    )
    .expect("workspace-member-file `ql test` should honor the outer workspace profile");
    expect_empty_stderr(
        "project-test-workspace-member-file-profile",
        "workspace member file profile test",
        &stderr,
    )
    .expect("workspace member file profile test should not print stderr");
    expect_stdout_contains_all(
        "project-test-workspace-member-file-profile",
        &stdout.replace('\\', "/"),
        &[
            "test packages/app/tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("workspace member file profile test should run only the selected workspace test");
    assert!(
        !stdout
            .replace('\\', "/")
            .contains("packages/app/tests/other.ql"),
        "workspace member file profile test should not run unselected tests: {stdout}"
    );
    expect_file_exists(
        "project-test-workspace-member-file-profile",
        &smoke_output,
        "workspace member file smoke executable",
        "workspace member file profile test",
    )
    .expect(
        "workspace member file profile test should emit the selected smoke artifact under release",
    );
    assert!(
        !other_output.exists(),
        "workspace member file profile test should not emit unselected test artifacts"
    );
    assert!(
        !debug_smoke_output.exists(),
        "workspace member file profile test should not silently fall back to the debug profile"
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
fn test_package_path_lists_discovered_tests_as_json() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-list-json");
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
    temp.write(
        "app/tests/ui/type_error.ql",
        "fn main() -> Int { return nope }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test", "--list", "--json"])
        .arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test --list --json` package path");
    let (stdout, stderr) = expect_success(
        "project-test-list-json",
        "package test json listing",
        &output,
    )
    .expect("package-path `ql test --list --json` should succeed");
    expect_empty_stderr(
        "project-test-list-json",
        "package test json listing",
        &stderr,
    )
    .expect("package-path `ql test --list --json` should not print stderr");

    let actual = parse_json_output("project-test-list-json", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.test.v1",
        "path": project_root.display().to_string().replace('\\', "/"),
        "requested_profile": "debug",
        "profile_overridden": false,
        "package_name": JsonValue::Null,
        "filter": JsonValue::Null,
        "list_only": true,
        "status": "listed",
        "discovered_total": 2,
        "selected_total": 2,
        "targets": [
            {
                "path": "tests/smoke.ql",
                "kind": "smoke",
                "profile": "debug",
            },
            {
                "path": "tests/ui/type_error.ql",
                "kind": "ui",
                "profile": JsonValue::Null,
            }
        ],
        "passed": 0,
        "failed": 0,
        "failures": [],
    });
    assert_eq!(
        actual, expected,
        "package-path `ql test --list --json` should match the stable listing contract"
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
fn test_workspace_path_selects_requested_package_tests() {
    if !toolchain_available("`ql test --package` workspace test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package-selector");
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
        .arg(&project_root)
        .args(["--package", "app"]);
    let output = run_command_capture(&mut command, "`ql test --package` workspace path");
    let (stdout, stderr) = expect_success(
        "project-test-package-selector",
        "workspace package selector tests",
        &output,
    )
    .expect("workspace-path `ql test --package` should succeed");
    expect_empty_stderr(
        "project-test-package-selector",
        "workspace package selector tests",
        &stderr,
    )
    .expect("workspace-path `ql test --package` should not print stderr");
    expect_stdout_contains_all(
        "project-test-package-selector",
        &stdout.replace('\\', "/"),
        &[
            "test packages/app/tests/app_only.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("workspace-path `ql test --package` should run only the selected package tests");
    assert!(
        !stdout.contains("packages/tool/tests/tool_only.ql"),
        "workspace-path `ql test --package` should not run tests from unselected packages: {stdout}"
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
        &stderr,
        "error: `ql test` package selector matched no packages",
    )
    .expect("unknown workspace package selector should report the missing package");
    expect_stderr_contains(
        "project-test-package-selector-missing",
        "unknown workspace package selector",
        &stderr,
        "`ql project graph",
    )
    .expect("unknown workspace package selector should suggest inspecting discovered members");
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
fn test_package_path_runs_ui_snapshot_tests() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-ui");
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
    let source = "fn main() -> Int { return nope }\n";
    let fixture_path = temp.write("app/tests/ui/type_error.ql", source);
    fs::write(
        fixture_path.with_extension("stderr"),
        ui_snapshot("tests/ui/type_error.ql", source),
    )
    .expect("write expected ui stderr snapshot");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test` package ui tests");
    let (stdout, stderr) = expect_success("project-test-ui", "package ui tests", &output)
        .expect("package-path `ql test` should run ui snapshot tests");
    expect_empty_stderr("project-test-ui", "package ui tests", &stderr)
        .expect("package ui tests should not print stderr");
    expect_stdout_contains_all(
        "project-test-ui",
        &stdout.replace('\\', "/"),
        &[
            "test tests/ui/type_error.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("package-path `ql test` should pass matching ui snapshot tests");
}

#[test]
fn test_direct_project_ui_file_uses_ui_snapshot_semantics() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-ui-direct-file");
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
    let source = "fn main() -> Int { return nope }\n";
    let fixture_path = temp.write("app/tests/ui/type_error.ql", source);
    fs::write(
        fixture_path.with_extension("stderr"),
        ui_snapshot("tests/ui/type_error.ql", source),
    )
    .expect("write expected ui stderr snapshot");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&fixture_path);
    let output = run_command_capture(&mut command, "`ql test` direct project ui file");
    let (stdout, stderr) = expect_success(
        "project-test-ui-direct-file",
        "direct project ui file test",
        &output,
    )
    .expect("direct project ui files should keep package-aware ui test semantics");
    expect_empty_stderr(
        "project-test-ui-direct-file",
        "direct project ui file test",
        &stderr,
    )
    .expect("direct project ui file test should not print stderr");
    expect_stdout_contains_all(
        "project-test-ui-direct-file",
        &stdout.replace('\\', "/"),
        &[
            "test tests/ui/type_error.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("direct project ui file test should execute the ui snapshot workflow");
}

#[test]
fn test_package_path_reports_ui_snapshot_mismatch() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-ui-mismatch");
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
    temp.write(
        "app/tests/ui/type_error.ql",
        "fn main() -> Int { return nope }\n",
    );
    temp.write("app/tests/ui/type_error.stderr", "error: wrong snapshot\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test` ui snapshot mismatch");
    let (stdout, stderr) = expect_exit_code(
        "project-test-ui-mismatch",
        "ui snapshot mismatch tests",
        &output,
        1,
    )
    .expect("package-path `ql test` should fail on ui snapshot mismatches");
    expect_stdout_contains_all(
        "project-test-ui-mismatch",
        &stdout.replace('\\', "/"),
        &["test tests/ui/type_error.ql ... FAILED"],
    )
    .expect("ui snapshot mismatch should mark the test as failed on stdout");
    expect_stderr_contains(
        "project-test-ui-mismatch",
        "ui snapshot mismatch tests",
        &stderr,
        "reason: ui stderr snapshot mismatch",
    )
    .expect("ui snapshot mismatch should explain the snapshot failure");
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
