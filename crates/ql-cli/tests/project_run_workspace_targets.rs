mod support;

use ql_driver::{ToolchainOptions, discover_toolchain};
use serde_json::Value as JsonValue;
use support::{
    TempDir, executable_output_path, expect_empty_stderr, expect_empty_stdout, expect_exit_code,
    expect_file_exists, expect_silent_output, expect_stderr_contains, expect_success, ql_command,
    run_command_capture, workspace_root,
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
