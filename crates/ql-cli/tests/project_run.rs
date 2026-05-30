mod support;

use std::process::Stdio;
use std::time::{Duration, Instant};

use ql_driver::{ToolchainOptions, discover_toolchain};
use serde_json::Value as JsonValue;
use support::{
    TempDir, assert_no_build_lock_directories, executable_output_path, expect_empty_stderr,
    expect_empty_stdout, expect_exit_code, expect_file_exists, expect_silent_output,
    expect_stderr_contains, expect_success, ql_command, run_command_capture, sleep_program_source,
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

struct WorkspaceDependencyGenericRunProject {
    temp: TempDir,
    project_root: std::path::PathBuf,
    core_interface_output: std::path::PathBuf,
    core_output: std::path::PathBuf,
    app_output: std::path::PathBuf,
    tool_output: std::path::PathBuf,
}

fn write_workspace_dependency_generic_run_project(
    prefix: &str,
) -> WorkspaceDependencyGenericRunProject {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app package source tree for workspace package run test");
    std::fs::create_dir_all(project_root.join("packages/core/src"))
        .expect("create core package source tree for workspace package run test");
    std::fs::create_dir_all(project_root.join("packages/tool/src"))
        .expect("create tool package source tree for workspace package run test");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/core", "packages/tool"]
"#,
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
        "workspace/packages/core/qlang.toml",
        r#"
[package]
name = "core"
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
        "workspace/packages/core/src/lib.ql",
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
        "workspace/packages/app/src/main.ql",
        r#"
use core.first as first
use core.identity as identity

fn main() -> Int {
    let value: Int = identity(7)
    let picked: Int = first([5, 8, 13])
    return value + picked
}
"#,
    );
    temp.write(
        "workspace/packages/tool/src/main.ql",
        "fn main() -> Int { return 99 }\n",
    );

    let core_interface_output = project_root.join("packages/core/core.qi");
    let core_output =
        static_library_output_path(&project_root.join("packages/core/target/ql/debug"), "lib");
    let app_output =
        executable_output_path(&project_root.join("packages/app/target/ql/debug"), "main");
    let tool_output =
        executable_output_path(&project_root.join("packages/tool/target/ql/debug"), "main");
    assert!(
        !core_interface_output.exists(),
        "workspace package selector run should start without a synced core interface"
    );

    WorkspaceDependencyGenericRunProject {
        temp,
        project_root,
        core_interface_output,
        core_output,
        app_output,
        tool_output,
    }
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
    .expect("single-file `ql run` local receiver method should leave the built executable in the default path");
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
    .expect("single-file `ql run` local method value should leave the built executable in the default path");
}

#[test]
fn run_package_path_executes_the_only_runnable_target_with_program_args() {
    if !toolchain_available("`ql run` package test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-package");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for run test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("app/src/main.ql", "fn main() -> Int { return 9 }\n");
    let output_path = executable_output_path(&project_root.join("target/ql/debug"), "main");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["run"])
        .arg(&project_root)
        .arg("--")
        .args(["alpha", "beta"]);
    let output = run_command_capture(&mut command, "`ql run` package path");
    let (stdout, stderr) = expect_exit_code("project-run-package", "package path run", &output, 9)
        .expect("package-path `ql run` should exit with the runnable target status");
    expect_silent_output("project-run-package", "package path run", &stdout, &stderr)
        .expect("package-path `ql run` should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-package",
        &output_path,
        "package executable",
        "package path run",
    )
    .expect("package-path `ql run` should leave the built executable in the package target dir");
}

#[test]
fn run_workspace_path_executes_the_only_runnable_target() {
    if !toolchain_available("`ql run` workspace test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-workspace");
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
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int { return 11 }\n",
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        "pub fn helper() -> Int { return 2 }\n",
    );
    let output_path =
        executable_output_path(&project_root.join("packages/app/target/ql/debug"), "main");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` workspace path");
    let (stdout, stderr) =
        expect_exit_code("project-run-workspace", "workspace path run", &output, 11)
            .expect("workspace-path `ql run` should exit with the runnable member status");
    expect_silent_output(
        "project-run-workspace",
        "workspace path run",
        &stdout,
        &stderr,
    )
    .expect("workspace-path `ql run` should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-workspace",
        &output_path,
        "workspace executable",
        "workspace path run",
    )
    .expect("workspace-path `ql run` should leave the built executable in the member target dir");
}

#[test]
fn run_workspace_package_selector_executes_dependency_generic_member() {
    if !toolchain_available("`ql run --package` workspace dependency generic test") {
        return;
    }

    let fixture = write_workspace_dependency_generic_run_project(
        "ql-project-run-workspace-package-dependency-generic",
    );
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["run"])
        .arg(&fixture.project_root)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql run --package` workspace dependency generic member",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-run-workspace-package-dependency-generic",
        "workspace package selector dependency generic run",
        &output,
        12,
    )
    .expect("workspace-path `ql run --package` should run the selected dependency generic member");
    expect_silent_output(
        "project-run-workspace-package-dependency-generic",
        "workspace package selector dependency generic run",
        &stdout,
        &stderr,
    )
    .expect("workspace package selector dependency generic run should leave output to the program");
    expect_file_exists(
        "project-run-workspace-package-dependency-generic",
        &fixture.core_interface_output,
        "synced core interface",
        "workspace package selector dependency generic run",
    )
    .expect("workspace package selector run should sync the selected member dependency interface");
    expect_file_exists(
        "project-run-workspace-package-dependency-generic",
        &fixture.core_output,
        "core dependency artifact",
        "workspace package selector dependency generic run",
    )
    .expect("workspace package selector run should build selected member dependencies");
    expect_file_exists(
        "project-run-workspace-package-dependency-generic",
        &fixture.app_output,
        "selected app executable",
        "workspace package selector dependency generic run",
    )
    .expect("workspace package selector run should emit the selected app executable");
    assert!(
        !fixture.tool_output.exists(),
        "workspace package selector run should not build unselected runnable workspace members"
    );
    assert_no_build_lock_directories(
        "project-run-workspace-package-dependency-generic",
        &fixture.project_root,
    );
}

struct WorkspacePackageRelativeTargetRunFixture {
    temp: TempDir,
    project_root: std::path::PathBuf,
    workspace_manifest: std::path::PathBuf,
    app_manifest: std::path::PathBuf,
    app_admin_output: std::path::PathBuf,
    app_main_output: std::path::PathBuf,
    tool_output: std::path::PathBuf,
}

fn workspace_package_relative_target_run_fixture(
    prefix: &str,
) -> WorkspacePackageRelativeTargetRunFixture {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src/bin"))
        .expect("create app package source tree");
    std::fs::create_dir_all(project_root.join("packages/tool/src"))
        .expect("create tool package source tree");

    let workspace_manifest = temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    let app_manifest = temp.write(
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
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int { return 11 }\n",
    );
    temp.write(
        "workspace/packages/app/src/bin/admin.ql",
        "fn main() -> Int { return 7 }\n",
    );
    temp.write(
        "workspace/packages/tool/src/main.ql",
        "fn main() -> Int { return 99 }\n",
    );

    let app_admin_output = executable_output_path(
        &project_root.join("packages/app/target/ql/debug/bin"),
        "admin",
    );
    let app_main_output =
        executable_output_path(&project_root.join("packages/app/target/ql/debug"), "main");
    let tool_output =
        executable_output_path(&project_root.join("packages/tool/target/ql/debug"), "main");

    WorkspacePackageRelativeTargetRunFixture {
        temp,
        project_root,
        workspace_manifest,
        app_manifest,
        app_admin_output,
        app_main_output,
        tool_output,
    }
}

#[test]
fn run_workspace_package_selector_accepts_package_relative_target_path() {
    if !toolchain_available("`ql run --package --target` package-relative target test") {
        return;
    }

    let fixture =
        workspace_package_relative_target_run_fixture("ql-project-run-package-relative-target");
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["run"]).arg(&fixture.project_root).args([
        "--package",
        "app",
        "--target",
        "src/bin/admin.ql",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql run --package --target` package-relative target",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-run-package-relative-target",
        "workspace package-relative target selector run",
        &output,
        7,
    )
    .expect(
        "workspace-path `ql run --package --target src/bin/admin.ql` should run selected target",
    );
    expect_silent_output(
        "project-run-package-relative-target",
        "workspace package-relative target selector run",
        &stdout,
        &stderr,
    )
    .expect("workspace package-relative target selector run should leave output to the program");
    expect_file_exists(
        "project-run-package-relative-target",
        &fixture.app_admin_output,
        "selected package-relative executable",
        "workspace package-relative target selector run",
    )
    .expect("workspace package-relative target selector run should build selected executable");
    assert!(
        !fixture.app_main_output.exists(),
        "workspace package-relative target selector run should not build unselected app main"
    );
    assert!(
        !fixture.tool_output.exists(),
        "workspace package-relative target selector run should not build unselected package"
    );
}

#[test]
fn run_workspace_package_selector_json_reports_package_relative_target_path() {
    if !toolchain_available("`ql run --package --target --json` package-relative target test") {
        return;
    }

    let fixture = workspace_package_relative_target_run_fixture(
        "ql-project-run-json-package-relative-target",
    );
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["run"]).arg(&fixture.project_root).args([
        "--package",
        "app",
        "--target",
        "src/bin/admin.ql",
        "--json",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql run --package --target --json` package-relative target",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-run-json-package-relative-target",
        "workspace package-relative target selector run json",
        &output,
        7,
    )
    .expect(
        "workspace-path `ql run --package --target src/bin/admin.ql --json` should preserve selected program status",
    );
    expect_empty_stderr(
        "project-run-json-package-relative-target",
        "workspace package-relative target selector run json",
        &stderr,
    )
    .expect("workspace package-relative target selector run json should keep stderr empty");

    let json = parse_json_output("project-run-json-package-relative-target", &stdout);
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
            .workspace_manifest
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
            "manifest_path": fixture.app_manifest.display().to_string().replace('\\', "/"),
            "package_name": "app",
            "selected": true,
            "dependency_only": false,
            "kind": "bin",
            "path": "src/bin/admin.ql",
            "emit": "exe",
            "profile": "debug",
            "artifact_path": fixture.app_admin_output.display().to_string().replace('\\', "/"),
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
        "project-run-json-package-relative-target",
        &fixture.app_admin_output,
        "selected package-relative executable",
        "workspace package-relative target selector run json",
    )
    .expect("workspace package-relative target selector run json should build selected executable");
    assert!(
        !fixture.app_main_output.exists(),
        "workspace package-relative target selector run json should not build unselected app main"
    );
    assert!(
        !fixture.tool_output.exists(),
        "workspace package-relative target selector run json should not build unselected package"
    );
}

#[test]
fn run_workspace_package_selector_list_json_reports_package_relative_target_path() {
    let fixture = workspace_package_relative_target_run_fixture(
        "ql-project-run-list-json-package-relative-target",
    );
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["run"]).arg(&fixture.project_root).args([
        "--list",
        "--json",
        "--package",
        "app",
        "--target",
        "src/bin/admin.ql",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql run --list --json --package --target` package-relative target",
    );
    let (stdout, stderr) = expect_success(
        "project-run-list-json-package-relative-target",
        "workspace package-relative target selector run list json",
        &output,
    )
    .expect(
        "workspace-path `ql run --list --json --package app --target src/bin/admin.ql` should succeed",
    );
    expect_empty_stderr(
        "project-run-list-json-package-relative-target",
        "workspace package-relative target selector run list json",
        &stderr,
    )
    .expect("workspace package-relative target selector run list json should not print stderr");

    let json = parse_json_output("project-run-list-json-package-relative-target", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.project.targets.v1",
        "members": [
            {
                "manifest_path": fixture.app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "targets": [
                    {
                        "kind": "bin",
                        "path": "src/bin/admin.ql",
                    }
                ],
            }
        ],
    });
    assert_eq!(
        json, expected,
        "workspace-path `ql run --list --json --package --target` should report the selected runnable package-relative target"
    );
    assert!(
        !fixture.app_admin_output.exists(),
        "workspace package-relative target selector run list json should not build artifacts"
    );
}

#[test]
fn run_workspace_package_selector_rejects_missing_package_relative_target_path() {
    let fixture = workspace_package_relative_target_run_fixture(
        "ql-project-run-missing-package-relative-target",
    );
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["run"]).arg(&fixture.project_root).args([
        "--package",
        "app",
        "--target",
        "src/bin/missing.ql",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql run --package --target` missing package-relative target",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-run-missing-package-relative-target",
        "workspace missing package-relative target selector run",
        &output,
        1,
    )
    .expect("workspace-path `ql run --package --target src/bin/missing.ql` should fail");
    expect_empty_stdout(
        "project-run-missing-package-relative-target",
        "workspace missing package-relative target selector run",
        &stdout,
    )
    .expect("workspace missing package-relative target selector run should not print stdout");
    expect_stderr_contains(
        "project-run-missing-package-relative-target",
        "workspace missing package-relative target selector run",
        &stderr,
        "error: `ql run` target selector matched no build targets",
    )
    .expect("missing package-relative target selector run should describe selector miss");
    expect_stderr_contains(
        "project-run-missing-package-relative-target",
        "workspace missing package-relative target selector run",
        &stderr,
        "note: selector: package `app`, target `src/bin/missing.ql`",
    )
    .expect("missing package-relative target selector run should print selector note");
    assert!(
        !fixture.app_admin_output.exists(),
        "workspace missing package-relative target selector run should not build artifacts"
    );
}

#[test]
fn run_workspace_package_selector_json_reports_missing_package_relative_target_path() {
    let fixture = workspace_package_relative_target_run_fixture(
        "ql-project-run-json-missing-package-relative-target",
    );
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["run"]).arg(&fixture.project_root).args([
        "--package",
        "app",
        "--target",
        "src/bin/missing.ql",
        "--json",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql run --package --target --json` missing package-relative target",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-run-json-missing-package-relative-target",
        "workspace missing package-relative target selector run json",
        &output,
        1,
    )
    .expect("workspace-path `ql run --package --target src/bin/missing.ql --json` should fail");
    expect_empty_stderr(
        "project-run-json-missing-package-relative-target",
        "workspace missing package-relative target selector run json",
        &stderr,
    )
    .expect("workspace missing package-relative target selector run json should not print stderr");

    let json = parse_json_output("project-run-json-missing-package-relative-target", &stdout);
    assert_eq!(json["schema"], "ql.run.v1");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["built_target"], JsonValue::Null);
    assert_eq!(json["execution"], JsonValue::Null);
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["error_kind"], "selector");
    assert_eq!(failure["stage"], "target-selection");
    assert_eq!(
        failure["selector"],
        "package `app`, target `src/bin/missing.ql`"
    );
    assert_eq!(failure["target_count"], 0);
    assert!(
        failure["message"]
            .as_str()
            .expect("missing target run json failure should expose message")
            .contains("target selector matched no build targets"),
        "missing package-relative target selector run json should describe selector miss: {json}"
    );
    assert!(
        !fixture.app_admin_output.exists(),
        "workspace missing package-relative target selector run json should not build artifacts"
    );
}

#[test]
fn run_workspace_package_selector_json_reports_dependency_generic_member() {
    if !toolchain_available("`ql run --json --package` workspace dependency generic test") {
        return;
    }

    let fixture = write_workspace_dependency_generic_run_project(
        "ql-project-run-json-workspace-package-dependency-generic",
    );
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["run"])
        .arg(&fixture.project_root)
        .args(["--package", "app", "--json"]);
    let output = run_command_capture(
        &mut command,
        "`ql run --json --package` workspace dependency generic member",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-run-json-workspace-package-dependency-generic",
        "workspace package selector dependency generic run json",
        &output,
        12,
    )
    .expect("workspace-path `ql run --json --package` should preserve selected program status");
    expect_empty_stderr(
        "project-run-json-workspace-package-dependency-generic",
        "workspace package selector dependency generic run json",
        &stderr,
    )
    .expect("workspace package selector dependency generic run json should keep stderr empty");

    let json = parse_json_output(
        "project-run-json-workspace-package-dependency-generic",
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
            "manifest_path": fixture.project_root.join("packages/app/qlang.toml").display().to_string().replace('\\', "/"),
            "package_name": "app",
            "selected": true,
            "dependency_only": false,
            "kind": "bin",
            "path": "src/main.ql",
            "emit": "exe",
            "profile": "debug",
            "artifact_path": fixture.app_output.display().to_string().replace('\\', "/"),
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
        "project-run-json-workspace-package-dependency-generic",
        &fixture.core_interface_output,
        "synced core interface",
        "workspace package selector dependency generic run json",
    )
    .expect("workspace package selector run json should sync the selected dependency interface");
    expect_file_exists(
        "project-run-json-workspace-package-dependency-generic",
        &fixture.core_output,
        "core dependency artifact",
        "workspace package selector dependency generic run json",
    )
    .expect("workspace package selector run json should build selected member dependencies");
    expect_file_exists(
        "project-run-json-workspace-package-dependency-generic",
        &fixture.app_output,
        "selected app executable",
        "workspace package selector dependency generic run json",
    )
    .expect("workspace package selector run json should emit the selected app executable");
    assert!(
        !fixture.tool_output.exists(),
        "workspace package selector run json should not build unselected runnable workspace members"
    );
}

#[test]
fn run_workspace_package_selector_json_allows_concurrent_dependency_generic_runs() {
    if !toolchain_available("concurrent `ql run --json --package` workspace dependency test") {
        return;
    }

    let fixture = write_workspace_dependency_generic_run_project(
        "ql-project-run-json-workspace-concurrent-dependency-generic",
    );
    let workspace_root = workspace_root();
    let mut children = Vec::new();
    for index in 0..3 {
        let mut command = ql_command(&workspace_root);
        command.current_dir(fixture.temp.path());
        command
            .args(["run"])
            .arg(&fixture.project_root)
            .args(["--package", "app", "--json"]);
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let child = command
            .spawn()
            .unwrap_or_else(|error| panic!("spawn concurrent ql run #{index}: {error}"));
        children.push((index, child));
    }

    for (index, child) in children {
        let output = child
            .wait_with_output()
            .unwrap_or_else(|error| panic!("wait for concurrent ql run #{index}: {error}"));
        let (stdout, stderr) = expect_exit_code(
            "project-run-json-workspace-concurrent-dependency-generic",
            &format!("concurrent workspace dependency generic run #{index}"),
            &output,
            12,
        )
        .unwrap_or_else(|message| panic!("{message}"));
        expect_empty_stderr(
            "project-run-json-workspace-concurrent-dependency-generic",
            &format!("concurrent workspace dependency generic run #{index}"),
            &stderr,
        )
        .unwrap_or_else(|message| panic!("{message}"));

        let json = parse_json_output(
            "project-run-json-workspace-concurrent-dependency-generic",
            &stdout,
        );
        assert_eq!(json["schema"], "ql.run.v1");
        assert_eq!(
            json["status"], "completed",
            "concurrent run #{index} failed: {json}"
        );
        assert_eq!(json["failure"], JsonValue::Null);
        assert_eq!(json["execution"]["exit_code"], 12);
        assert_eq!(json["built_target"]["package_name"], "app");
    }

    expect_file_exists(
        "project-run-json-workspace-concurrent-dependency-generic",
        &fixture.core_interface_output,
        "synced core interface",
        "concurrent workspace dependency generic run",
    )
    .expect("concurrent run should sync the selected dependency interface");
    expect_file_exists(
        "project-run-json-workspace-concurrent-dependency-generic",
        &fixture.core_output,
        "core dependency artifact",
        "concurrent workspace dependency generic run",
    )
    .expect("concurrent run should build selected member dependencies");
    expect_file_exists(
        "project-run-json-workspace-concurrent-dependency-generic",
        &fixture.app_output,
        "selected app executable",
        "concurrent workspace dependency generic run",
    )
    .expect("concurrent run should emit the selected app executable");
    assert!(
        !fixture.tool_output.exists(),
        "concurrent run should not build unselected runnable workspace members"
    );
    assert_no_build_lock_directories(
        "project-run-json-workspace-concurrent-dependency-generic",
        &fixture.project_root,
    );
}

#[test]
fn run_project_json_holds_executable_lock_while_program_runs() {
    if !toolchain_available("`ql run --json` executable lock during execution test") {
        return;
    }

    let temp = TempDir::new("ql-project-run-json-executable-lock-during-execution");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for executable lock run test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/main.ql", &sleep_program_source(900));
    let executable_output = executable_output_path(&project_root.join("target/ql/debug"), "main");
    let workspace_root = workspace_root();

    let mut first = ql_command(&workspace_root);
    first.current_dir(temp.path());
    first.args(["run", "--json"]).arg(&project_root);
    first.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut first = first
        .spawn()
        .expect("spawn first long-running `ql run --json`");
    wait_for_path_exists(
        "project-run-json-executable-lock-during-execution",
        "first long-running run executable",
        &executable_output,
        Duration::from_secs(20),
    )
    .expect("first long-running run should create the executable before execution");
    std::thread::sleep(Duration::from_millis(50));
    assert!(
        first
            .try_wait()
            .expect("poll first long-running `ql run --json`")
            .is_none(),
        "first long-running run should still hold the executable while the second run starts"
    );

    let mut second = ql_command(&workspace_root);
    second.current_dir(temp.path());
    second.args(["run", "--json"]).arg(&project_root);
    second.stdout(Stdio::piped()).stderr(Stdio::piped());
    let second_started = Instant::now();
    let second = second
        .spawn()
        .expect("spawn second `ql run --json` against same executable");

    let second_output = second
        .wait_with_output()
        .expect("wait for second executable-lock `ql run --json`");
    let second_elapsed = second_started.elapsed();
    let first_output = first
        .wait_with_output()
        .expect("wait for first executable-lock `ql run --json`");

    for (label, output) in [("first", first_output), ("second", second_output)] {
        let (stdout, stderr) = expect_success(
            "project-run-json-executable-lock-during-execution",
            &format!("{label} long-running executable-lock run"),
            &output,
        )
        .unwrap_or_else(|message| panic!("{message}"));
        expect_empty_stderr(
            "project-run-json-executable-lock-during-execution",
            &format!("{label} long-running executable-lock run"),
            &stderr,
        )
        .unwrap_or_else(|message| panic!("{message}"));
        let json = parse_json_output("project-run-json-executable-lock-during-execution", &stdout);
        assert_eq!(json["schema"], "ql.run.v1");
        assert_eq!(json["status"], "completed");
        assert_eq!(json["execution"]["exit_code"], 0);
    }

    assert!(
        second_elapsed >= Duration::from_millis(1000),
        "second run should wait for the first execution lock before rebuilding the same executable; elapsed {second_elapsed:?}"
    );
    assert_no_build_lock_directories(
        "project-run-json-executable-lock-during-execution",
        &project_root,
    );
}

#[test]
fn run_project_source_file_list_uses_workspace_context_and_only_reports_runnable_targets() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-list-workspace-source");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app source tree for run list test");
    std::fs::create_dir_all(project_root.join("packages/tool/src"))
        .expect("create tool source tree for run list test");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    let app_manifest = temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let app_main = temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn helper() -> Int { return 2 }\n",
    );
    let tool_manifest = temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/tool/src/main.ql",
        "fn main() -> Int { return 3 }\n",
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        "pub fn helper() -> Int { return 4 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["run"])
        .arg(&app_main)
        .args(["--list", "--json"]);
    let output = run_command_capture(
        &mut command,
        "`ql run --list --json` workspace member source path",
    );
    let (stdout, stderr) = expect_success(
        "project-run-list-workspace-source",
        "workspace member source runnable target listing",
        &output,
    )
    .expect("workspace member source `ql run --list --json` should succeed");
    expect_empty_stderr(
        "project-run-list-workspace-source",
        "workspace member source runnable target listing",
        &stderr,
    )
    .expect("workspace member source `ql run --list --json` should not print stderr");

    let json = parse_json_output("project-run-list-workspace-source", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.project.targets.v1",
        "members": [
            {
                "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "targets": [
                    {
                        "kind": "bin",
                        "path": "src/main.ql",
                    }
                ],
            },
            {
                "manifest_path": tool_manifest.display().to_string().replace('\\', "/"),
                "package_name": "tool",
                "targets": [
                    {
                        "kind": "bin",
                        "path": "src/main.ql",
                    }
                ],
            }
        ],
    });
    assert_eq!(
        json, expected,
        "workspace member source `ql run --list --json` should resolve the outer workspace and only report runnable targets"
    );
    assert!(
        !project_root.join("packages/app/target").exists(),
        "`ql run --list --json` should not build the selected source"
    );
}

#[test]
fn run_project_member_directory_list_uses_workspace_context_and_only_reports_runnable_targets() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-list-workspace-member-dir");
    let project_root = temp.path().join("workspace");
    let app_root = project_root.join("packages").join("app");
    let tool_root = project_root.join("packages").join("tool");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app source tree for run list workspace member directory test");
    std::fs::create_dir_all(tool_root.join("src"))
        .expect("create tool source tree for run list workspace member directory test");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    let app_manifest = temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn helper() -> Int { return 2 }\n",
    );
    let tool_manifest = temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/tool/src/main.ql",
        "fn main() -> Int { return 3 }\n",
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        "pub fn helper() -> Int { return 4 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["run"])
        .arg(&app_root)
        .args(["--list", "--json"]);
    let output = run_command_capture(
        &mut command,
        "`ql run --list --json` workspace member directory",
    );
    let (stdout, stderr) = expect_success(
        "project-run-list-workspace-member-dir",
        "workspace member directory runnable target listing",
        &output,
    )
    .expect("workspace member directory `ql run --list --json` should succeed");
    expect_empty_stderr(
        "project-run-list-workspace-member-dir",
        "workspace member directory runnable target listing",
        &stderr,
    )
    .expect("workspace member directory `ql run --list --json` should not print stderr");

    let json = parse_json_output("project-run-list-workspace-member-dir", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.project.targets.v1",
        "members": [
            {
                "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "targets": [
                    {
                        "kind": "bin",
                        "path": "src/main.ql",
                    }
                ],
            },
            {
                "manifest_path": tool_manifest.display().to_string().replace('\\', "/"),
                "package_name": "tool",
                "targets": [
                    {
                        "kind": "bin",
                        "path": "src/main.ql",
                    }
                ],
            }
        ],
    });
    assert_eq!(
        json, expected,
        "workspace member directory `ql run --list --json` should resolve the outer workspace and only report runnable targets"
    );
    assert!(
        !project_root.join("packages/app/target").exists(),
        "`ql run --list --json` workspace member directory should not build the selected directory"
    );
}

#[test]
fn run_preserves_large_exit_code() {
    if !toolchain_available("`ql run` large-exit-code test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-large-exit");
    let source_path = temp.write("large_exit.ql", "fn main() -> Int { return 690 }\n");
    let output_path = executable_output_path(&temp.path().join("target/ql/debug"), "large_exit");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&source_path);
    let output = run_command_capture(&mut command, "`ql run` large exit code");
    let (stdout, stderr) = expect_exit_code(
        "project-run-large-exit",
        "large-exit-code run",
        &output,
        690,
    )
    .expect("`ql run` should preserve the child exit code");
    expect_silent_output(
        "project-run-large-exit",
        "large-exit-code run",
        &stdout,
        &stderr,
    )
    .expect("large-exit-code `ql run` should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-large-exit",
        &output_path,
        "large-exit executable",
        "large-exit-code run",
    )
    .expect("large-exit-code `ql run` should still leave the built executable in place");
    assert_no_build_lock_directories("project-run-large-exit", temp.path());
}

#[test]
fn run_project_path_selects_requested_binary_target() {
    if !toolchain_available("`ql run --bin` package test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-select-bin");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src/bin"))
        .expect("create package source tree for target selector run test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/main.ql", "fn main() -> Int { return 1 }\n");
    temp.write("app/src/bin/admin.ql", "fn main() -> Int { return 2 }\n");
    let output_path = executable_output_path(&project_root.join("target/ql/debug/bin"), "admin");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["run"])
        .arg(&project_root)
        .args(["--bin", "admin"]);
    let output = run_command_capture(&mut command, "`ql run --bin` package path");
    let (stdout, stderr) = expect_exit_code(
        "project-run-select-bin",
        "selected binary target run",
        &output,
        2,
    )
    .expect("package-path `ql run --bin` should exit with the selected binary status");
    expect_silent_output(
        "project-run-select-bin",
        "selected binary target run",
        &stdout,
        &stderr,
    )
    .expect("package-path `ql run --bin` should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-select-bin",
        &output_path,
        "selected binary executable",
        "selected binary target run",
    )
    .expect(
        "package-path `ql run --bin` should build the selected executable in the bin target dir",
    );
}
