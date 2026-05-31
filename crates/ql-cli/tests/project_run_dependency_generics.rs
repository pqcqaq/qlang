mod support;

use std::process::Stdio;

use ql_driver::{ToolchainOptions, discover_toolchain};
use serde_json::Value as JsonValue;
use support::{
    TempDir, assert_no_build_lock_directories, executable_output_path, expect_empty_stderr,
    expect_exit_code, expect_file_exists, expect_silent_output, ql_command, run_command_capture,
    static_library_output_path, workspace_root,
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
