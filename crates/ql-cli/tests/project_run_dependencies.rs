mod support;

use ql_driver::{ToolchainOptions, discover_toolchain};
use serde_json::Value as JsonValue;
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

struct DependencyRunProject {
    temp: TempDir,
    project_root: std::path::PathBuf,
    interface_output: std::path::PathBuf,
    dependency_output: std::path::PathBuf,
    executable_output: std::path::PathBuf,
}

fn write_dependency_run_project(
    prefix: &str,
    dependency_source: &str,
    app_source: &str,
) -> DependencyRunProject {
    let temp = TempDir::new(prefix);
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
    temp.write("app/src/main.ql", app_source);

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let executable_output = executable_output_path(&project_root.join("target/ql/debug"), "main");
    assert!(
        !interface_output.exists(),
        "dependency interface should start missing for {prefix}"
    );

    DependencyRunProject {
        temp,
        project_root,
        interface_output,
        dependency_output,
        executable_output,
    }
}

fn expect_dependency_run_project_exits(
    case_name: &str,
    action: &str,
    command_description: &str,
    fixture: &DependencyRunProject,
    expected_exit_code: i32,
) {
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["run"]).arg(&fixture.project_root);
    let output = run_command_capture(&mut command, command_description);
    let (stdout, stderr) = expect_exit_code(case_name, action, &output, expected_exit_code)
        .expect("dependency run project should exit with the expected program status");
    expect_silent_output(case_name, action, &stdout, &stderr)
        .expect("dependency run project should leave stdout/stderr to the program");
    expect_file_exists(
        case_name,
        &fixture.interface_output,
        "synced dependency interface",
        action,
    )
    .expect("dependency run project should emit the dependency interface");
    expect_file_exists(
        case_name,
        &fixture.dependency_output,
        "dependency package artifact",
        action,
    )
    .expect("dependency run project should build the dependency package artifact");
    expect_file_exists(
        case_name,
        &fixture.executable_output,
        "package executable",
        action,
    )
    .expect("dependency run project should emit the executable artifact");
}

#[test]
fn run_package_path_syncs_dependency_interfaces_without_polluting_program_output() {
    if !toolchain_available("`ql run` dependency sync test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-dependency-sync");
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
    temp.write(
        "app/src/main.ql",
        "use dep.q_add as add\n\nfn main() -> Int { return add(6, 7) }\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let executable_output = executable_output_path(&project_root.join("target/ql/debug"), "main");
    assert!(
        !interface_output.exists(),
        "dependency interface should start missing for sync test"
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` dependency sync");
    let (stdout, stderr) = expect_exit_code(
        "project-run-dependency-sync",
        "package path run with dependency sync",
        &output,
        13,
    )
    .expect("package-path `ql run` should sync dependency interfaces before execution");
    expect_silent_output(
        "project-run-dependency-sync",
        "package path run with dependency sync",
        &stdout,
        &stderr,
    )
    .expect("dependency-sync run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-dependency-sync",
        &interface_output,
        "synced dependency interface",
        "package path run with dependency sync",
    )
    .expect("dependency-sync run should emit the dependency interface");
    expect_file_exists(
        "project-run-dependency-sync",
        &dependency_output,
        "dependency package artifact",
        "package path run with dependency sync",
    )
    .expect("dependency-sync run should also build the dependency package artifact");
    expect_file_exists(
        "project-run-dependency-sync",
        &executable_output,
        "package executable",
        "package path run with dependency sync",
    )
    .expect("dependency-sync run should still emit the executable artifact");
}

#[test]
fn run_package_path_supports_direct_dependency_public_functions() {
    if !toolchain_available("`ql run` dependency public function test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-dependency-public-function");
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
        "pub fn add(left: Int, right: Int) -> Int { return left + right }\n",
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
        "use dep.add as sum\n\nfn main() -> Int { return sum(9, 4) }\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let executable_output = executable_output_path(&project_root.join("target/ql/debug"), "main");
    assert!(
        !interface_output.exists(),
        "dependency interface should start missing for direct dependency public function test"
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` dependency public function");
    let (stdout, stderr) = expect_exit_code(
        "project-run-dependency-public-function",
        "package path run with dependency public function",
        &output,
        13,
    )
    .expect("package-path `ql run` should support direct dependency public functions");
    expect_silent_output(
        "project-run-dependency-public-function",
        "package path run with dependency public function",
        &stdout,
        &stderr,
    )
    .expect("dependency public function run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-dependency-public-function",
        &interface_output,
        "synced dependency interface",
        "package path run with dependency public function",
    )
    .expect("dependency public function run should emit the dependency interface");
    expect_file_exists(
        "project-run-dependency-public-function",
        &dependency_output,
        "dependency package artifact",
        "package path run with dependency public function",
    )
    .expect("dependency public function run should also build the dependency package artifact");
    expect_file_exists(
        "project-run-dependency-public-function",
        &executable_output,
        "package executable",
        "package path run with dependency public function",
    )
    .expect("dependency public function run should still emit the executable artifact");
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

#[test]
fn run_package_path_supports_direct_dependency_public_values() {
    if !toolchain_available("`ql run` dependency public value test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-dependency-public-value");
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
        "pub const VALUE: Int = 7\npub static READY: Bool = true\npub static VALUES: [Int; 3] = [1, 3, 5]\n",
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
        "use dep.VALUE as THRESHOLD\nuse dep.READY as ENABLED\nuse dep.VALUES as ITEMS\n\nfn main() -> Int {\n    if ENABLED {\n        return THRESHOLD + ITEMS[1]\n    }\n    return 0\n}\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let executable_output = executable_output_path(&project_root.join("target/ql/debug"), "main");
    assert!(
        !interface_output.exists(),
        "dependency interface should start missing for direct dependency public value test"
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` dependency public value");
    let (stdout, stderr) = expect_exit_code(
        "project-run-dependency-public-value",
        "package path run with dependency public value",
        &output,
        10,
    )
    .expect("package-path `ql run` should support direct dependency public values");
    expect_silent_output(
        "project-run-dependency-public-value",
        "package path run with dependency public value",
        &stdout,
        &stderr,
    )
    .expect("dependency public value run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-dependency-public-value",
        &interface_output,
        "synced dependency interface",
        "package path run with dependency public value",
    )
    .expect("dependency public value run should emit the dependency interface");
    expect_file_exists(
        "project-run-dependency-public-value",
        &dependency_output,
        "dependency package artifact",
        "package path run with dependency public value",
    )
    .expect("dependency public value run should also build the dependency package artifact");
    expect_file_exists(
        "project-run-dependency-public-value",
        &executable_output,
        "package executable",
        "package path run with dependency public value",
    )
    .expect("dependency public value run should still emit the executable artifact");
}

#[test]
fn run_package_path_supports_dependency_public_values_with_function_initializers() {
    if !toolchain_available("`ql run` dependency public value initializer test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-dependency-public-value-function-initializers");
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
        "pub fn add_one(value: Int) -> Int { return value + 1 }\npub fn make_value() -> Int { return add_one(6) }\npub const VALUE: Int = make_value()\npub const APPLY: (Int) -> Int = add_one\n",
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
        "use dep.VALUE as VALUE_ALIAS\nuse dep.APPLY as RUN\n\nfn main() -> Int { return VALUE_ALIAS + RUN(3) }\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let executable_output = executable_output_path(&project_root.join("target/ql/debug"), "main");
    assert!(
        !interface_output.exists(),
        "dependency interface should start missing for dependency public value initializer test"
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` dependency public value initializer");
    let (stdout, stderr) = expect_exit_code(
        "project-run-dependency-public-value-function-initializers",
        "package path run with dependency public value function initializers",
        &output,
        11,
    )
    .expect(
        "package-path `ql run` should support dependency public values with function initializers",
    );
    expect_silent_output(
        "project-run-dependency-public-value-function-initializers",
        "package path run with dependency public value function initializers",
        &stdout,
        &stderr,
    )
    .expect("dependency public value initializer run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-dependency-public-value-function-initializers",
        &interface_output,
        "synced dependency interface",
        "package path run with dependency public value function initializers",
    )
    .expect("dependency public value initializer run should emit the dependency interface");
    expect_file_exists(
        "project-run-dependency-public-value-function-initializers",
        &dependency_output,
        "dependency package artifact",
        "package path run with dependency public value function initializers",
    )
    .expect(
        "dependency public value initializer run should also build the dependency package artifact",
    );
    expect_file_exists(
        "project-run-dependency-public-value-function-initializers",
        &executable_output,
        "package executable",
        "package path run with dependency public value function initializers",
    )
    .expect("dependency public value initializer run should still emit the executable artifact");
}

#[test]
fn run_package_path_supports_direct_dependency_public_struct_functions() {
    if !toolchain_available("`ql run` dependency public struct function test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-dependency-public-struct-function");
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
    temp.write(
        "app/src/main.ql",
        "use dep.make_box as make\n\nfn main() -> Int {\n    let value = make()\n    return value.value\n}\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let executable_output = executable_output_path(&project_root.join("target/ql/debug"), "main");
    assert!(
        !interface_output.exists(),
        "dependency interface should start missing for direct dependency public struct function test"
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` dependency public struct function");
    let (stdout, stderr) = expect_exit_code(
        "project-run-dependency-public-struct-function",
        "package path run with dependency public struct function",
        &output,
        7,
    )
    .expect("package-path `ql run` should support direct dependency public struct functions");
    expect_silent_output(
        "project-run-dependency-public-struct-function",
        "package path run with dependency public struct function",
        &stdout,
        &stderr,
    )
    .expect("dependency public struct function run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-dependency-public-struct-function",
        &interface_output,
        "synced dependency interface",
        "package path run with dependency public struct function",
    )
    .expect("dependency public struct function run should emit the dependency interface");
    expect_file_exists(
        "project-run-dependency-public-struct-function",
        &dependency_output,
        "dependency package artifact",
        "package path run with dependency public struct function",
    )
    .expect(
        "dependency public struct function run should also build the dependency package artifact",
    );
    expect_file_exists(
        "project-run-dependency-public-struct-function",
        &executable_output,
        "package executable",
        "package path run with dependency public struct function",
    )
    .expect("dependency public struct function run should still emit the executable artifact");
}

#[test]
fn run_package_path_supports_direct_dependency_public_type_alias_functions() {
    if !toolchain_available("`ql run` dependency public type alias function test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-dependency-public-type-alias-function");
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
    temp.write(
        "app/src/main.ql",
        "use dep.make_score as make_score\nuse dep.unwrap_score as unwrap_score\n\nfn main() -> Int {\n    return unwrap_score(make_score(5))\n}\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let executable_output = executable_output_path(&project_root.join("target/ql/debug"), "main");
    assert!(
        !interface_output.exists(),
        "dependency interface should start missing for direct dependency public type alias function test"
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql run` dependency public type alias function",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-run-dependency-public-type-alias-function",
        "package path run with dependency public type alias function",
        &output,
        7,
    )
    .expect("package-path `ql run` should support direct dependency public type alias functions");
    expect_silent_output(
        "project-run-dependency-public-type-alias-function",
        "package path run with dependency public type alias function",
        &stdout,
        &stderr,
    )
    .expect("dependency public type alias function run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-dependency-public-type-alias-function",
        &interface_output,
        "synced dependency interface",
        "package path run with dependency public type alias function",
    )
    .expect("dependency public type alias function run should emit the dependency interface");
    expect_file_exists(
        "project-run-dependency-public-type-alias-function",
        &dependency_output,
        "dependency package artifact",
        "package path run with dependency public type alias function",
    )
    .expect(
        "dependency public type alias function run should also build the dependency package artifact",
    );
    expect_file_exists(
        "project-run-dependency-public-type-alias-function",
        &executable_output,
        "package executable",
        "package path run with dependency public type alias function",
    )
    .expect("dependency public type alias function run should still emit the executable artifact");
}

#[test]
fn run_package_path_supports_direct_dependency_public_struct_methods() {
    if !toolchain_available("`ql run` dependency public struct method test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-dependency-public-struct-method");
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
    temp.write(
        "app/src/main.ql",
        "use dep.make_box as make\n\nfn main() -> Int {\n    let value = make()\n    return value.read()\n}\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let executable_output = executable_output_path(&project_root.join("target/ql/debug"), "main");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` dependency public struct method");
    let (stdout, stderr) = expect_exit_code(
        "project-run-dependency-public-struct-method",
        "package path run with dependency public struct method",
        &output,
        7,
    )
    .expect("package-path `ql run` should support direct dependency public struct methods");
    expect_silent_output(
        "project-run-dependency-public-struct-method",
        "package path run with dependency public struct method",
        &stdout,
        &stderr,
    )
    .expect("dependency public struct method run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-dependency-public-struct-method",
        &interface_output,
        "synced dependency interface",
        "package path run with dependency public struct method",
    )
    .expect("dependency public struct method run should emit the dependency interface");
    expect_file_exists(
        "project-run-dependency-public-struct-method",
        &dependency_output,
        "dependency package artifact",
        "package path run with dependency public struct method",
    )
    .expect(
        "dependency public struct method run should also build the dependency package artifact",
    );
    expect_file_exists(
        "project-run-dependency-public-struct-method",
        &executable_output,
        "package executable",
        "package path run with dependency public struct method",
    )
    .expect("dependency public struct method run should still emit the executable artifact");
}

#[test]
fn run_package_path_supports_direct_dependency_public_struct_method_values() {
    if !toolchain_available("`ql run` dependency public struct method value test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-dependency-public-struct-method-value");
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
    temp.write(
        "app/src/main.ql",
        "use dep.make_box as make\n\nfn main() -> Int {\n    let value = make()\n    let add = value.add\n    return add(5)\n}\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let executable_output = executable_output_path(&project_root.join("target/ql/debug"), "main");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql run` dependency public struct method value",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-run-dependency-public-struct-method-value",
        "package path run with dependency public struct method value",
        &output,
        12,
    )
    .expect("package-path `ql run` should support direct dependency public struct method values");
    expect_silent_output(
        "project-run-dependency-public-struct-method-value",
        "package path run with dependency public struct method value",
        &stdout,
        &stderr,
    )
    .expect("dependency public struct method value run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-dependency-public-struct-method-value",
        &interface_output,
        "synced dependency interface",
        "package path run with dependency public struct method value",
    )
    .expect("dependency public struct method value run should emit the dependency interface");
    expect_file_exists(
        "project-run-dependency-public-struct-method-value",
        &dependency_output,
        "dependency package artifact",
        "package path run with dependency public struct method value",
    )
    .expect(
        "dependency public struct method value run should also build the dependency package artifact",
    );
    expect_file_exists(
        "project-run-dependency-public-struct-method-value",
        &executable_output,
        "package executable",
        "package path run with dependency public struct method value",
    )
    .expect("dependency public struct method value run should still emit the executable artifact");
}

#[test]
fn run_package_path_supports_direct_dependency_public_enums() {
    if !toolchain_available("`ql run` dependency public enum test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-dependency-public-enum");
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
        "pub struct Issue { code: Int }\n\npub enum Status {\n    Ready,\n    Failed {\n        issue: Issue,\n    },\n}\n\npub fn load_status() -> Status {\n    return Status.Failed { issue: Issue { code: 3 } }\n}\n",
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
        "use dep.load_status as load\n\nfn main() -> Int {\n    return match load() {\n        Status.Ready => 1,\n        Status.Failed { issue } => issue.code + 4,\n    }\n}\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let executable_output = executable_output_path(&project_root.join("target/ql/debug"), "main");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` dependency public enum");
    let (stdout, stderr) = expect_exit_code(
        "project-run-dependency-public-enum",
        "package path run with dependency public enum",
        &output,
        7,
    )
    .expect("package-path `ql run` should support direct dependency public enums");
    expect_silent_output(
        "project-run-dependency-public-enum",
        "package path run with dependency public enum",
        &stdout,
        &stderr,
    )
    .expect("dependency public enum run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-dependency-public-enum",
        &interface_output,
        "synced dependency interface",
        "package path run with dependency public enum",
    )
    .expect("dependency public enum run should emit the dependency interface");
    expect_file_exists(
        "project-run-dependency-public-enum",
        &dependency_output,
        "dependency package artifact",
        "package path run with dependency public enum",
    )
    .expect("dependency public enum run should also build the dependency package artifact");
    expect_file_exists(
        "project-run-dependency-public-enum",
        &executable_output,
        "package executable",
        "package path run with dependency public enum",
    )
    .expect("dependency public enum run should still emit the executable artifact");
}

#[test]
fn run_package_path_supports_direct_dependency_public_tuple_enums() {
    if !toolchain_available("`ql run` dependency public tuple enum test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-dependency-public-tuple-enum");
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
        "pub struct Issue { code: Int }\n\npub enum Status {\n    Ready,\n    Failed(Issue),\n}\n\npub fn load_status() -> Status {\n    return Status.Failed(Issue { code: 3 })\n}\n",
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
        "use dep.load_status as load\n\nfn main() -> Int {\n    return match load() {\n        Status.Ready => 1,\n        Status.Failed(issue) => issue.code + 4,\n    }\n}\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let executable_output = executable_output_path(&project_root.join("target/ql/debug"), "main");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` dependency public tuple enum");
    let (stdout, stderr) = expect_exit_code(
        "project-run-dependency-public-tuple-enum",
        "package path run with dependency public tuple enum",
        &output,
        7,
    )
    .expect("package-path `ql run` should support direct dependency public tuple enums");
    expect_silent_output(
        "project-run-dependency-public-tuple-enum",
        "package path run with dependency public tuple enum",
        &stdout,
        &stderr,
    )
    .expect("dependency public tuple enum run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-dependency-public-tuple-enum",
        &interface_output,
        "synced dependency interface",
        "package path run with dependency public tuple enum",
    )
    .expect("dependency public tuple enum run should emit the dependency interface");
    expect_file_exists(
        "project-run-dependency-public-tuple-enum",
        &dependency_output,
        "dependency package artifact",
        "package path run with dependency public tuple enum",
    )
    .expect("dependency public tuple enum run should also build the dependency package artifact");
    expect_file_exists(
        "project-run-dependency-public-tuple-enum",
        &executable_output,
        "package executable",
        "package path run with dependency public tuple enum",
    )
    .expect("dependency public tuple enum run should still emit the executable artifact");
}

#[test]
fn run_package_path_supports_direct_dependency_public_trait_methods() {
    if !toolchain_available("`ql run` dependency public trait method test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-dependency-public-trait-method");
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
        "pub trait Reader {\n    fn read(self) -> Int\n}\n\npub struct Box { value: Int }\n\nimpl Reader for Box {\n    pub fn read(self) -> Int {\n        return self.value\n    }\n}\n\npub fn make_box() -> Box { return Box { value: 7 } }\n",
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
        "use dep.make_box as make\n\nfn main() -> Int {\n    let value = make()\n    return value.read()\n}\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let executable_output = executable_output_path(&project_root.join("target/ql/debug"), "main");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` dependency public trait method");
    let (stdout, stderr) = expect_exit_code(
        "project-run-dependency-public-trait-method",
        "package path run with dependency public trait method",
        &output,
        7,
    )
    .expect("package-path `ql run` should support direct dependency public trait methods");
    expect_silent_output(
        "project-run-dependency-public-trait-method",
        "package path run with dependency public trait method",
        &stdout,
        &stderr,
    )
    .expect("dependency public trait method run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-dependency-public-trait-method",
        &interface_output,
        "synced dependency interface",
        "package path run with dependency public trait method",
    )
    .expect("dependency public trait method run should emit the dependency interface");
    expect_file_exists(
        "project-run-dependency-public-trait-method",
        &dependency_output,
        "dependency package artifact",
        "package path run with dependency public trait method",
    )
    .expect("dependency public trait method run should also build the dependency package artifact");
    expect_file_exists(
        "project-run-dependency-public-trait-method",
        &executable_output,
        "package executable",
        "package path run with dependency public trait method",
    )
    .expect("dependency public trait method run should still emit the executable artifact");
}
