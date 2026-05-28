mod support;

use std::path::{Path, PathBuf};

use ql_driver::{ToolchainOptions, discover_toolchain};
use serde_json::Value as JsonValue;
use support::{
    TempDir, executable_output_path, expect_empty_stderr, expect_exit_code, expect_file_exists,
    expect_success, ql_command, run_command_capture, workspace_root,
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

fn expected_standalone_test_list_json(
    request_path: &Path,
    package_name: Option<&str>,
    targets: Vec<JsonValue>,
    discovered_total: usize,
) -> JsonValue {
    expected_standalone_test_list_json_with_filter(
        request_path,
        package_name,
        None,
        targets,
        discovered_total,
    )
}

fn expected_standalone_test_list_json_with_filter(
    request_path: &Path,
    package_name: Option<&str>,
    filter: Option<&str>,
    targets: Vec<JsonValue>,
    discovered_total: usize,
) -> JsonValue {
    serde_json::json!({
        "schema": "ql.test.v1",
        "path": request_path.display().to_string().replace('\\', "/"),
        "requested_profile": "debug",
        "profile_overridden": false,
        "package_name": package_name.map(JsonValue::from).unwrap_or(JsonValue::Null),
        "filter": filter.map(JsonValue::from).unwrap_or(JsonValue::Null),
        "list_only": true,
        "status": "listed",
        "discovered_total": discovered_total,
        "selected_total": targets.len(),
        "targets": targets,
        "passed": 0,
        "failed": 0,
        "failures": [],
    })
}

fn smoke_test_list_target() -> JsonValue {
    serde_json::json!({
        "path": "tests/smoke.ql",
        "kind": "smoke",
        "profile": "debug",
    })
}

fn ui_test_list_target() -> JsonValue {
    serde_json::json!({
        "path": "tests/ui/type_error.ql",
        "kind": "ui",
        "profile": JsonValue::Null,
    })
}

struct StandaloneTestListFixture {
    temp: TempDir,
    project_root: PathBuf,
    smoke_path: PathBuf,
    smoke_output: PathBuf,
    ui_path: PathBuf,
}

fn write_standalone_test_list_fixture(prefix: &str) -> StandaloneTestListFixture {
    write_standalone_test_fixture(prefix, "fn main() -> Int { return nope }\n")
}

fn write_standalone_passing_test_fixture(prefix: &str) -> StandaloneTestListFixture {
    write_standalone_test_fixture(prefix, "fn main() -> Int { return 0 }\n")
}

fn write_standalone_test_fixture(prefix: &str, smoke_source: &str) -> StandaloneTestListFixture {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source root for test list fixture");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    let smoke_path = temp.write("app/tests/smoke.ql", smoke_source);
    let ui_path = temp.write(
        "app/tests/ui/type_error.ql",
        "fn main() -> Int { return nope }\n",
    );
    let smoke_output = executable_output_path(&project_root.join("target/ql/debug/tests"), "smoke");

    StandaloneTestListFixture {
        temp,
        project_root,
        smoke_path,
        smoke_output,
        ui_path,
    }
}

struct TestListWorkspaceFixture {
    temp: TempDir,
    project_root: PathBuf,
    app_root: PathBuf,
    app_smoke_path: PathBuf,
    tool_ui_path: PathBuf,
}

fn write_test_list_workspace_fixture(prefix: &str) -> TestListWorkspaceFixture {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("workspace");
    let app_root = project_root.join("packages").join("app");
    let tool_root = project_root.join("packages").join("tool");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app source tree for test list workspace fixture");
    std::fs::create_dir_all(tool_root.join("src"))
        .expect("create tool source tree for test list workspace fixture");
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
    let app_smoke_path = temp.write(
        "workspace/packages/app/tests/smoke.ql",
        "fn main() -> Int { return nope }\n",
    );
    let tool_ui_path = temp.write(
        "workspace/packages/tool/tests/ui/type_error.ql",
        "fn main() -> Int { return nope }\n",
    );

    TestListWorkspaceFixture {
        temp,
        project_root,
        app_root,
        app_smoke_path,
        tool_ui_path,
    }
}

fn expected_workspace_test_list_json(
    request_path: &Path,
    targets: Vec<JsonValue>,
    discovered_total: usize,
) -> JsonValue {
    serde_json::json!({
        "schema": "ql.test.v1",
        "path": request_path.display().to_string().replace('\\', "/"),
        "requested_profile": "debug",
        "profile_overridden": false,
        "package_name": JsonValue::Null,
        "filter": JsonValue::Null,
        "list_only": true,
        "status": "listed",
        "discovered_total": discovered_total,
        "selected_total": targets.len(),
        "targets": targets,
        "passed": 0,
        "failed": 0,
        "failures": [],
    })
}

fn expected_workspace_package_test_list_json(
    request_path: &Path,
    targets: Vec<JsonValue>,
    discovered_total: usize,
) -> JsonValue {
    serde_json::json!({
        "schema": "ql.test.v1",
        "path": request_path.display().to_string().replace('\\', "/"),
        "requested_profile": "debug",
        "profile_overridden": false,
        "package_name": "app",
        "filter": JsonValue::Null,
        "list_only": true,
        "status": "listed",
        "discovered_total": discovered_total,
        "selected_total": targets.len(),
        "targets": targets,
        "passed": 0,
        "failed": 0,
        "failures": [],
    })
}

#[test]
fn test_package_path_lists_discovered_tests_as_json() {
    let workspace_root = workspace_root();
    let fixture = write_standalone_test_list_fixture("ql-project-test-list-json");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--list", "--json"])
        .arg(&fixture.project_root);
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
    let expected = expected_standalone_test_list_json(
        &fixture.project_root,
        None,
        vec![smoke_test_list_target(), ui_test_list_target()],
        2,
    );
    assert_eq!(
        actual, expected,
        "package-path `ql test --list --json` should match the stable listing contract"
    );
    assert!(
        !fixture.project_root.join("target").exists(),
        "`ql test --list --json` package path should not build listed tests"
    );
    assert!(
        !fixture.ui_path.with_extension("stderr").exists(),
        "`ql test --list --json` package path should not evaluate listed ui tests"
    );
}

#[test]
fn test_workspace_member_directory_lists_discovered_tests_as_json() {
    let workspace_root = workspace_root();
    let fixture = write_test_list_workspace_fixture("ql-project-test-list-member-dir-json");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--list", "--json"])
        .arg(&fixture.app_root);
    let output = run_command_capture(
        &mut command,
        "`ql test --list --json` workspace member directory",
    );
    let (stdout, stderr) = expect_success(
        "project-test-list-member-dir-json",
        "workspace member directory test json listing",
        &output,
    )
    .expect("workspace member directory `ql test --list --json` should succeed");
    expect_empty_stderr(
        "project-test-list-member-dir-json",
        "workspace member directory test json listing",
        &stderr,
    )
    .expect("workspace member directory `ql test --list --json` should not print stderr");

    let actual = parse_json_output("project-test-list-member-dir-json", &stdout);
    let expected = expected_workspace_test_list_json(
        &fixture.app_root,
        vec![
            serde_json::json!({
                "path": "packages/app/tests/smoke.ql",
                "kind": "smoke",
                "profile": "debug",
            }),
            serde_json::json!({
                "path": "packages/tool/tests/ui/type_error.ql",
                "kind": "ui",
                "profile": JsonValue::Null,
            }),
        ],
        2,
    );
    assert_eq!(
        actual, expected,
        "workspace member directory `ql test --list --json` should keep the outer workspace context"
    );
    assert!(
        !fixture.project_root.join("packages/app/target").exists(),
        "`ql test --list --json` workspace member directory should not build listed tests"
    );
}

#[test]
fn test_workspace_member_directory_package_selector_lists_discovered_tests_as_json() {
    let workspace_root = workspace_root();
    let fixture = write_test_list_workspace_fixture("ql-project-test-list-member-dir-package-json");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--list", "--json"])
        .arg(&fixture.app_root)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --list --json --package` workspace member directory",
    );
    let (stdout, stderr) = expect_success(
        "project-test-list-member-dir-package-json",
        "workspace member directory package selector test json listing",
        &output,
    )
    .expect("workspace member directory `ql test --list --json --package` should succeed");
    expect_empty_stderr(
        "project-test-list-member-dir-package-json",
        "workspace member directory package selector test json listing",
        &stderr,
    )
    .expect("workspace member directory `ql test --list --json --package` should not print stderr");

    let actual = parse_json_output("project-test-list-member-dir-package-json", &stdout);
    let expected = expected_workspace_package_test_list_json(
        &fixture.app_root,
        vec![serde_json::json!({
            "path": "packages/app/tests/smoke.ql",
            "kind": "smoke",
            "profile": "debug",
        })],
        1,
    );
    assert_eq!(
        actual, expected,
        "workspace member directory `ql test --list --json --package` should list only the selected package tests"
    );
    assert!(
        !fixture.project_root.join("packages/app/target").exists(),
        "`ql test --list --json --package` workspace member directory should not build listed tests"
    );
    assert!(
        !fixture.tool_ui_path.with_extension("stderr").exists(),
        "`ql test --list --json --package` workspace member directory should not evaluate unselected ui tests"
    );
}

#[test]
fn test_workspace_member_file_lists_selected_test_as_json() {
    let workspace_root = workspace_root();
    let fixture = write_test_list_workspace_fixture("ql-project-test-list-member-file-json");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--list", "--json"])
        .arg(&fixture.app_smoke_path);
    let output = run_command_capture(
        &mut command,
        "`ql test --list --json` workspace member test file",
    );
    let (stdout, stderr) = expect_success(
        "project-test-list-member-file-json",
        "workspace member file test json listing",
        &output,
    )
    .expect("workspace member file `ql test --list --json` should succeed");
    expect_empty_stderr(
        "project-test-list-member-file-json",
        "workspace member file test json listing",
        &stderr,
    )
    .expect("workspace member file `ql test --list --json` should not print stderr");

    let actual = parse_json_output("project-test-list-member-file-json", &stdout);
    let expected = expected_workspace_test_list_json(
        &fixture.app_smoke_path,
        vec![serde_json::json!({
            "path": "packages/app/tests/smoke.ql",
            "kind": "smoke",
            "profile": "debug",
        })],
        1,
    );
    assert_eq!(
        actual, expected,
        "workspace member file `ql test --list --json` should list only the selected test while preserving discovered total"
    );
    assert!(
        !fixture.project_root.join("packages/app/target").exists(),
        "`ql test --list --json` workspace member file should not build listed tests"
    );
    assert!(
        !fixture.tool_ui_path.with_extension("stderr").exists(),
        "`ql test --list --json` workspace member file should not evaluate unselected ui tests"
    );
}

#[test]
fn test_workspace_member_file_package_selector_lists_selected_test_as_json() {
    let workspace_root = workspace_root();
    let fixture =
        write_test_list_workspace_fixture("ql-project-test-list-member-file-package-json");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--list", "--json"])
        .arg(&fixture.app_smoke_path)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --list --json --package` workspace member test file",
    );
    let (stdout, stderr) = expect_success(
        "project-test-list-member-file-package-json",
        "workspace member file package selector test json listing",
        &output,
    )
    .expect("workspace member file `ql test --list --json --package` should succeed");
    expect_empty_stderr(
        "project-test-list-member-file-package-json",
        "workspace member file package selector test json listing",
        &stderr,
    )
    .expect("workspace member file `ql test --list --json --package` should not print stderr");

    let actual = parse_json_output("project-test-list-member-file-package-json", &stdout);
    let expected = expected_workspace_package_test_list_json(
        &fixture.app_smoke_path,
        vec![serde_json::json!({
            "path": "packages/app/tests/smoke.ql",
            "kind": "smoke",
            "profile": "debug",
        })],
        1,
    );
    assert_eq!(
        actual, expected,
        "workspace member file `ql test --list --json --package` should list only the selected test"
    );
    assert!(
        !fixture.project_root.join("packages/app/target").exists(),
        "`ql test --list --json --package` workspace member file should not build listed tests"
    );
    assert!(
        !fixture.tool_ui_path.with_extension("stderr").exists(),
        "`ql test --list --json --package` workspace member file should not evaluate unselected ui tests"
    );
}

#[test]
fn test_direct_project_test_file_lists_selected_test_as_json() {
    let workspace_root = workspace_root();
    let fixture = write_standalone_test_list_fixture("ql-project-test-list-direct-file-json");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--list", "--json"])
        .arg(&fixture.smoke_path);
    let output = run_command_capture(
        &mut command,
        "`ql test --list --json` direct project test file",
    );
    let (stdout, stderr) = expect_success(
        "project-test-list-direct-file-json",
        "direct project test file json listing",
        &output,
    )
    .expect("direct project test file `ql test --list --json` should succeed");
    expect_empty_stderr(
        "project-test-list-direct-file-json",
        "direct project test file json listing",
        &stderr,
    )
    .expect("direct project test file `ql test --list --json` should not print stderr");

    let actual = parse_json_output("project-test-list-direct-file-json", &stdout);
    let expected = expected_standalone_test_list_json(
        &fixture.smoke_path,
        None,
        vec![smoke_test_list_target()],
        1,
    );
    assert_eq!(
        actual, expected,
        "direct project test file `ql test --list --json` should match the stable listing contract"
    );
    assert!(
        !fixture.project_root.join("target").exists(),
        "`ql test --list --json` direct project test file should not build listed tests"
    );
    assert!(
        !fixture.ui_path.with_extension("stderr").exists(),
        "`ql test --list --json` direct project test file should not evaluate unselected ui tests"
    );
}

#[test]
fn test_direct_project_test_file_package_selector_lists_selected_test_as_json() {
    let workspace_root = workspace_root();
    let fixture =
        write_standalone_test_list_fixture("ql-project-test-list-direct-file-package-json");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--list", "--json"])
        .arg(&fixture.smoke_path)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --list --json --package` direct project test file",
    );
    let (stdout, stderr) = expect_success(
        "project-test-list-direct-file-package-json",
        "direct project test file package selector json listing",
        &output,
    )
    .expect("direct project test file `ql test --list --json --package app` should succeed");
    expect_empty_stderr(
        "project-test-list-direct-file-package-json",
        "direct project test file package selector json listing",
        &stderr,
    )
    .expect(
        "direct project test file `ql test --list --json --package app` should not print stderr",
    );

    let actual = parse_json_output("project-test-list-direct-file-package-json", &stdout);
    let expected = expected_standalone_test_list_json(
        &fixture.smoke_path,
        Some("app"),
        vec![smoke_test_list_target()],
        1,
    );
    assert_eq!(
        actual, expected,
        "direct project test file `ql test --list --json --package app` should keep project-aware package context"
    );
    assert!(
        !fixture.project_root.join("target").exists(),
        "`ql test --list --json --package app` direct project test file should not build listed tests"
    );
    assert!(
        !fixture.ui_path.with_extension("stderr").exists(),
        "`ql test --list --json --package app` direct project test file should not evaluate unselected ui tests"
    );
}

#[test]
fn test_direct_project_test_file_target_selector_lists_selected_test_as_json() {
    let workspace_root = workspace_root();
    let fixture =
        write_standalone_test_list_fixture("ql-project-test-list-direct-file-target-json");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--list", "--json"])
        .arg(&fixture.smoke_path)
        .args(["--target", "tests/smoke.ql"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --list --json --target` direct project test file",
    );
    let (stdout, stderr) = expect_success(
        "project-test-list-direct-file-target-json",
        "direct project test file target selector json listing",
        &output,
    )
    .expect(
        "direct project test file `ql test --list --json --target tests/smoke.ql` should succeed",
    );
    expect_empty_stderr(
        "project-test-list-direct-file-target-json",
        "direct project test file target selector json listing",
        &stderr,
    )
    .expect(
        "direct project test file `ql test --list --json --target tests/smoke.ql` should not print stderr",
    );

    let actual = parse_json_output("project-test-list-direct-file-target-json", &stdout);
    let expected = expected_standalone_test_list_json(
        &fixture.smoke_path,
        None,
        vec![smoke_test_list_target()],
        1,
    );
    assert_eq!(
        actual, expected,
        "direct project test file `ql test --list --json --target tests/smoke.ql` should keep the selected file contract"
    );
    assert!(
        !fixture.project_root.join("target").exists(),
        "`ql test --list --json --target tests/smoke.ql` direct project test file should not build listed tests"
    );
    assert!(
        !fixture.ui_path.with_extension("stderr").exists(),
        "`ql test --list --json --target tests/smoke.ql` direct project test file should not evaluate unselected ui tests"
    );
}

#[test]
fn test_direct_project_test_file_filter_lists_selected_test_as_json() {
    let workspace_root = workspace_root();
    let fixture =
        write_standalone_test_list_fixture("ql-project-test-list-direct-file-filter-json");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--list", "--json"])
        .arg(&fixture.smoke_path)
        .args(["--filter", "smoke"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --list --json --filter` direct project test file",
    );
    let (stdout, stderr) = expect_success(
        "project-test-list-direct-file-filter-json",
        "direct project test file filter json listing",
        &output,
    )
    .expect("direct project test file `ql test --list --json --filter smoke` should succeed");
    expect_empty_stderr(
        "project-test-list-direct-file-filter-json",
        "direct project test file filter json listing",
        &stderr,
    )
    .expect(
        "direct project test file `ql test --list --json --filter smoke` should not print stderr",
    );

    let actual = parse_json_output("project-test-list-direct-file-filter-json", &stdout);
    let expected = expected_standalone_test_list_json_with_filter(
        &fixture.smoke_path,
        None,
        Some("smoke"),
        vec![smoke_test_list_target()],
        1,
    );
    assert_eq!(
        actual, expected,
        "direct project test file `ql test --list --json --filter smoke` should keep the selected file contract"
    );
    assert!(
        !fixture.project_root.join("target").exists(),
        "`ql test --list --json --filter smoke` direct project test file should not build listed tests"
    );
    assert!(
        !fixture.ui_path.with_extension("stderr").exists(),
        "`ql test --list --json --filter smoke` direct project test file should not evaluate unselected ui tests"
    );
}

#[test]
fn test_direct_project_test_file_json_rejects_mismatched_package_selector() {
    let workspace_root = workspace_root();
    let fixture =
        write_standalone_test_list_fixture("ql-project-test-direct-file-package-mismatch-json");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--json"])
        .arg(&fixture.smoke_path)
        .args(["--package", "missing"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --package` direct project test file package mismatch",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-direct-file-package-mismatch-json",
        "direct project test file package selector mismatch json",
        &output,
        1,
    )
    .expect("direct project test file `ql test --json --package missing` should reject package mismatch");
    expect_empty_stderr(
        "project-test-direct-file-package-mismatch-json",
        "direct project test file package selector mismatch json",
        &stderr,
    )
    .expect("direct project test file package mismatch json should stay on stdout");

    let json = parse_json_output("project-test-direct-file-package-mismatch-json", &stdout);
    assert_eq!(json["schema"], "ql.test.v1");
    assert_eq!(
        json["path"],
        fixture.smoke_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["package_name"], "missing");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["discovered_total"], 0);
    assert_eq!(json["selected_total"], 0);
    assert_eq!(json["targets"], serde_json::json!([]));
    assert_eq!(json["failures"], serde_json::json!([]));
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["error_kind"], "selector");
    assert_eq!(failure["stage"], "package-selection");
    assert_eq!(failure["selector"], "package `missing`");
    assert_eq!(failure["target_count"], 0);
    assert!(
        failure["message"]
            .as_str()
            .expect("direct project test file package mismatch json should expose a message")
            .contains("matched no workspace members"),
        "direct project test file package mismatch json should describe package mismatch: {json}"
    );
}

#[test]
fn test_direct_project_test_file_json_reports_missing_target_selection_failure() {
    let workspace_root = workspace_root();
    let fixture =
        write_standalone_test_list_fixture("ql-project-test-direct-file-missing-target-json");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--json"])
        .arg(&fixture.smoke_path)
        .args(["--target", "tests/missing.ql"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --target` direct project test file missing target",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-direct-file-missing-target-json",
        "direct project test file missing target json",
        &output,
        1,
    )
    .expect("direct project test file `ql test --json --target tests/missing.ql` should fail");
    expect_empty_stderr(
        "project-test-direct-file-missing-target-json",
        "direct project test file missing target json",
        &stderr,
    )
    .expect("direct project test file missing target json should stay on stdout");

    let json = parse_json_output("project-test-direct-file-missing-target-json", &stdout);
    assert_eq!(json["schema"], "ql.test.v1");
    assert_eq!(
        json["path"],
        fixture.smoke_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["package_name"], JsonValue::Null);
    assert_eq!(json["status"], "no-match");
    assert_eq!(json["discovered_total"], 1);
    assert_eq!(json["selected_total"], 0);
    assert_eq!(json["targets"], serde_json::json!([]));
    assert_eq!(json["failures"], serde_json::json!([]));
    assert_eq!(json["failure"]["kind"], "selection");
    let failure = &json["failure"]["selection_failure"];
    assert_eq!(failure["stage"], "target-selection");
    assert_eq!(failure["selector"], "target `tests/missing.ql`");
    assert_eq!(failure["target_count"], 1);
    assert!(
        failure["message"]
            .as_str()
            .expect("direct project test file missing target json should expose a message")
            .contains("found no test target `tests/missing.ql`"),
        "direct project test file missing target json should describe target miss: {json}"
    );
}

#[test]
fn test_direct_project_test_file_json_reports_missing_filter_selection_failure() {
    let workspace_root = workspace_root();
    let fixture =
        write_standalone_test_list_fixture("ql-project-test-direct-file-missing-filter-json");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--json"])
        .arg(&fixture.smoke_path)
        .args(["--filter", "missing"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --filter` direct project test file missing filter",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-direct-file-missing-filter-json",
        "direct project test file missing filter json",
        &output,
        1,
    )
    .expect("direct project test file `ql test --json --filter missing` should fail");
    expect_empty_stderr(
        "project-test-direct-file-missing-filter-json",
        "direct project test file missing filter json",
        &stderr,
    )
    .expect("direct project test file missing filter json should stay on stdout");

    let json = parse_json_output("project-test-direct-file-missing-filter-json", &stdout);
    assert_eq!(json["schema"], "ql.test.v1");
    assert_eq!(
        json["path"],
        fixture.smoke_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["package_name"], JsonValue::Null);
    assert_eq!(json["filter"], "missing");
    assert_eq!(json["status"], "no-match");
    assert_eq!(json["discovered_total"], 1);
    assert_eq!(json["selected_total"], 0);
    assert_eq!(json["targets"], serde_json::json!([]));
    assert_eq!(json["failures"], serde_json::json!([]));
    assert_eq!(json["failure"]["kind"], "selection");
    let failure = &json["failure"]["selection_failure"];
    assert_eq!(failure["stage"], "filter-selection");
    assert_eq!(failure["selector"], "filter `missing`");
    assert_eq!(failure["target_count"], 1);
    assert!(
        failure["message"]
            .as_str()
            .expect("direct project test file missing filter json should expose a message")
            .contains("found no test files matching `missing`"),
        "direct project test file missing filter json should describe filter miss: {json}"
    );
}

#[test]
fn test_direct_project_test_file_json_runs_with_package_target_and_filter_selectors() {
    if !toolchain_available(
        "`ql test --json --package --target --filter` direct project smoke file",
    ) {
        return;
    }

    let workspace_root = workspace_root();
    let fixture =
        write_standalone_passing_test_fixture("ql-project-test-direct-file-all-selectors-json");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--json"])
        .arg(&fixture.smoke_path)
        .args([
            "--package",
            "app",
            "--target",
            "tests/smoke.ql",
            "--filter",
            "smoke",
        ]);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --package --target --filter` direct project test file",
    );
    let (stdout, stderr) = expect_success(
        "project-test-direct-file-all-selectors-json",
        "direct project test file all selector json execution",
        &output,
    )
    .expect("direct project test file `ql test --json --package app --target tests/smoke.ql --filter smoke` should pass");
    expect_empty_stderr(
        "project-test-direct-file-all-selectors-json",
        "direct project test file all selector json execution",
        &stderr,
    )
    .expect("direct project test file selector json execution should not print stderr");

    let actual = parse_json_output("project-test-direct-file-all-selectors-json", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.test.v1",
        "path": fixture.smoke_path.display().to_string().replace('\\', "/"),
        "requested_profile": "debug",
        "profile_overridden": false,
        "package_name": "app",
        "filter": "smoke",
        "list_only": false,
        "status": "ok",
        "discovered_total": 1,
        "selected_total": 1,
        "targets": [
            smoke_test_list_target(),
        ],
        "passed": 1,
        "failed": 0,
        "failures": [],
    });
    assert_eq!(
        actual, expected,
        "direct project test file selector execution should preserve the stable json contract"
    );
    expect_file_exists(
        "project-test-direct-file-all-selectors-json",
        &fixture.smoke_output,
        "selected smoke test executable",
        "direct project test file all selector json execution",
    )
    .expect("direct project test file selector execution should build the selected smoke test");
    assert!(
        !fixture.ui_path.with_extension("stderr").exists(),
        "direct project test file selector execution should not evaluate unselected ui tests"
    );
}
