mod support;

use std::fs;
use std::path::{Path, PathBuf};

use ql_analysis::analyze_source;
use ql_diagnostics::render_diagnostics;
use serde_json::Value as JsonValue;
use support::{
    TempDir, expect_empty_stderr, expect_exit_code, expect_stderr_contains,
    expect_stdout_contains_all, expect_success, ql_command, run_command_capture, workspace_root,
};

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

struct WorkspacePackageUiFixture {
    temp: TempDir,
    project_root: PathBuf,
    app_root: PathBuf,
}

fn write_workspace_package_ui_fixture(
    prefix: &str,
    matching_app_snapshot: bool,
) -> WorkspacePackageUiFixture {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("workspace");
    let app_root = project_root.join("packages").join("app");
    fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create workspace app source tree");
    fs::create_dir_all(project_root.join("packages/tool/src"))
        .expect("create workspace tool source tree");

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

    let source = "fn main() -> Int { return nope }\n";
    let app_ui_path = temp.write("workspace/packages/app/tests/ui/type_error.ql", source);
    let app_snapshot = if matching_app_snapshot {
        ui_snapshot("tests/ui/type_error.ql", source)
    } else {
        "error: wrong app snapshot\n".to_owned()
    };
    fs::write(app_ui_path.with_extension("stderr"), app_snapshot)
        .expect("write selected workspace ui stderr snapshot");

    let tool_ui_path = temp.write("workspace/packages/tool/tests/ui/tool_error.ql", source);
    fs::write(
        tool_ui_path.with_extension("stderr"),
        "error: unselected tool snapshot\n",
    )
    .expect("write unselected workspace ui stderr snapshot");

    WorkspacePackageUiFixture {
        temp,
        project_root,
        app_root,
    }
}

struct DirectProjectUiFixture {
    temp: TempDir,
    project_root: PathBuf,
    ui_path: PathBuf,
}

fn write_direct_project_ui_fixture(
    prefix: &str,
    matching_snapshot: bool,
) -> DirectProjectUiFixture {
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
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    let source = "fn main() -> Int { return nope }\n";
    let ui_path = temp.write("app/tests/ui/type_error.ql", source);
    let snapshot = if matching_snapshot {
        ui_snapshot("tests/ui/type_error.ql", source)
    } else {
        "error: wrong snapshot\n".to_owned()
    };
    fs::write(ui_path.with_extension("stderr"), snapshot)
        .expect("write expected ui stderr snapshot");

    DirectProjectUiFixture {
        temp,
        project_root,
        ui_path,
    }
}

fn project_ui_json_target(path: &str) -> JsonValue {
    serde_json::json!({
        "path": path,
        "kind": "ui",
        "profile": JsonValue::Null,
    })
}

fn expected_project_ui_json_success(
    request_path: &Path,
    package_name: Option<&str>,
    filter: Option<&str>,
) -> JsonValue {
    expected_project_ui_json_success_with_target(
        request_path,
        package_name,
        filter,
        "tests/ui/type_error.ql",
    )
}

fn expected_workspace_package_ui_json_success(
    request_path: &Path,
    filter: Option<&str>,
) -> JsonValue {
    expected_project_ui_json_success_with_target(
        request_path,
        Some("app"),
        filter,
        "packages/app/tests/ui/type_error.ql",
    )
}

fn expected_project_ui_json_success_with_target(
    request_path: &Path,
    package_name: Option<&str>,
    filter: Option<&str>,
    target_path: &str,
) -> JsonValue {
    serde_json::json!({
        "schema": "ql.test.v1",
        "path": request_path.display().to_string().replace('\\', "/"),
        "requested_profile": "debug",
        "profile_overridden": false,
        "package_name": package_name.map(JsonValue::from).unwrap_or(JsonValue::Null),
        "filter": filter.map(JsonValue::from).unwrap_or(JsonValue::Null),
        "list_only": false,
        "status": "ok",
        "discovered_total": 1,
        "selected_total": 1,
        "targets": [
            project_ui_json_target(target_path),
        ],
        "passed": 1,
        "failed": 0,
        "failures": [],
    })
}

fn expected_workspace_package_ui_json_listing(
    request_path: &Path,
    filter: Option<&str>,
) -> JsonValue {
    expected_project_ui_json_listing_with_target(
        request_path,
        Some("app"),
        filter,
        "packages/app/tests/ui/type_error.ql",
    )
}

fn expected_direct_project_ui_json_listing(
    request_path: &Path,
    package_name: Option<&str>,
    filter: Option<&str>,
) -> JsonValue {
    expected_project_ui_json_listing_with_target(
        request_path,
        package_name,
        filter,
        "tests/ui/type_error.ql",
    )
}

fn expected_project_ui_json_listing_with_target(
    request_path: &Path,
    package_name: Option<&str>,
    filter: Option<&str>,
    target_path: &str,
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
        "discovered_total": 1,
        "selected_total": 1,
        "targets": [
            project_ui_json_target(target_path),
        ],
        "passed": 0,
        "failed": 0,
        "failures": [],
    })
}

fn assert_workspace_package_ui_list_selection_failure_json(
    case_name: &str,
    stdout: &str,
    request_path: &Path,
    filter: Option<&str>,
    stage: &str,
    selector: &str,
    expected_message_fragment: &str,
) {
    let json = parse_json_output(case_name, stdout);
    assert_eq!(json["schema"], "ql.test.v1");
    assert_eq!(
        json["path"],
        request_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["requested_profile"], "debug");
    assert_eq!(json["profile_overridden"], false);
    assert_eq!(json["package_name"], "app");
    assert_eq!(
        json["filter"],
        filter.map(JsonValue::from).unwrap_or(JsonValue::Null)
    );
    assert_eq!(json["list_only"], true);
    assert_eq!(json["status"], "no-match");
    assert_eq!(json["discovered_total"], 1);
    assert_eq!(json["selected_total"], 0);
    assert_eq!(json["targets"], serde_json::json!([]));
    assert_eq!(json["passed"], 0);
    assert_eq!(json["failed"], 0);
    assert_eq!(json["failures"], serde_json::json!([]));
    assert_eq!(json["failure"]["kind"], "selection");
    let failure = &json["failure"]["selection_failure"];
    assert_eq!(failure["stage"], stage);
    assert_eq!(failure["selector"], selector);
    assert_eq!(failure["target_count"], 1);
    assert!(
        failure["message"]
            .as_str()
            .expect("workspace ui list selection failure json should expose a message")
            .contains(expected_message_fragment),
        "workspace ui list selection failure json should describe the selector miss: {json}"
    );
}

fn assert_project_ui_snapshot_mismatch_json(case_name: &str, stdout: &str, request_path: &Path) {
    assert_project_ui_snapshot_mismatch_json_with_target(
        case_name,
        stdout,
        request_path,
        "tests/ui/type_error.ql",
    );
}

fn assert_workspace_package_ui_snapshot_mismatch_json(
    case_name: &str,
    stdout: &str,
    request_path: &Path,
) {
    let json = assert_project_ui_snapshot_mismatch_json_with_target(
        case_name,
        stdout,
        request_path,
        "packages/app/tests/ui/type_error.ql",
    );
    assert_eq!(json["package_name"], "app");
    assert_eq!(json["filter"], JsonValue::Null);
}

fn assert_project_ui_snapshot_mismatch_json_with_target(
    case_name: &str,
    stdout: &str,
    request_path: &Path,
    target_path: &str,
) -> JsonValue {
    let json = parse_json_output(case_name, stdout);
    assert_eq!(json["schema"], "ql.test.v1");
    assert_eq!(
        json["path"],
        request_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");
    assert_eq!(json["discovered_total"], 1);
    assert_eq!(json["selected_total"], 1);
    assert_eq!(
        json["targets"],
        serde_json::json!([project_ui_json_target(target_path)])
    );
    assert_eq!(json["passed"], 0);
    assert_eq!(json["failed"], 1);
    assert_eq!(json["failures"][0]["path"], target_path);
    assert_eq!(json["failures"][0]["kind"], "ui");
    assert!(
        json["failures"][0]["detail"]
            .as_str()
            .expect("project ui mismatch json should expose detail")
            .contains("ui stderr snapshot mismatch"),
        "project ui mismatch json should describe snapshot mismatch: {json}"
    );
    json
}

#[test]
fn test_workspace_path_package_selector_reports_ui_snapshot_json_success() {
    let fixture = write_workspace_package_ui_fixture("ql-project-test-workspace-ui-json", true);
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--json"])
        .arg(&fixture.project_root)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --package` workspace ui tests",
    );
    let (stdout, stderr) = expect_success(
        "project-test-workspace-ui-json",
        "workspace package selector ui json success",
        &output,
    )
    .expect("workspace-path `ql test --json --package` should run selected ui tests");
    expect_empty_stderr(
        "project-test-workspace-ui-json",
        "workspace package selector ui json success",
        &stderr,
    )
    .expect("workspace package selector ui json success should not print stderr");

    let actual = parse_json_output("project-test-workspace-ui-json", &stdout);
    let expected = expected_workspace_package_ui_json_success(&fixture.project_root, None);
    assert_eq!(
        actual, expected,
        "workspace-path ui json success should preserve the selected-package contract"
    );
}

#[test]
fn test_workspace_member_directory_package_selector_reports_ui_snapshot_json_success() {
    let fixture = write_workspace_package_ui_fixture("ql-project-test-member-dir-ui-json", true);
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--json"])
        .arg(&fixture.app_root)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --package` workspace member directory ui tests",
    );
    let (stdout, stderr) = expect_success(
        "project-test-member-dir-ui-json",
        "workspace member directory package selector ui json success",
        &output,
    )
    .expect("workspace member directory `ql test --json --package` should run selected ui tests");
    expect_empty_stderr(
        "project-test-member-dir-ui-json",
        "workspace member directory package selector ui json success",
        &stderr,
    )
    .expect("workspace member directory ui json success should not print stderr");

    let actual = parse_json_output("project-test-member-dir-ui-json", &stdout);
    let expected = expected_workspace_package_ui_json_success(&fixture.app_root, None);
    assert_eq!(
        actual, expected,
        "workspace member directory ui json success should keep workspace-relative selected targets"
    );
}

#[test]
fn test_workspace_path_package_selector_ui_json_runs_with_target_and_filter_selectors() {
    let fixture =
        write_workspace_package_ui_fixture("ql-project-test-workspace-ui-selectors-json", true);
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--json"])
        .arg(&fixture.project_root)
        .args([
            "--package",
            "app",
            "--target",
            "tests/ui/type_error.ql",
            "--filter",
            "type_error",
        ]);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --package --target --filter` workspace ui tests",
    );
    let (stdout, stderr) = expect_success(
        "project-test-workspace-ui-selectors-json",
        "workspace package selector ui json selector execution",
        &output,
    )
    .expect("workspace-path ui json execution should accept package-relative target and filter selectors");
    expect_empty_stderr(
        "project-test-workspace-ui-selectors-json",
        "workspace package selector ui json selector execution",
        &stderr,
    )
    .expect("workspace package selector ui json selector execution should not print stderr");

    let actual = parse_json_output("project-test-workspace-ui-selectors-json", &stdout);
    let expected =
        expected_workspace_package_ui_json_success(&fixture.project_root, Some("type_error"));
    assert_eq!(
        actual, expected,
        "workspace-path ui json selector execution should preserve the stable json contract"
    );
}

#[test]
fn test_workspace_path_package_selector_lists_ui_snapshot_as_json() {
    let fixture =
        write_workspace_package_ui_fixture("ql-project-test-workspace-ui-list-json", false);
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--list", "--json"])
        .arg(&fixture.project_root)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --list --json --package` workspace ui tests",
    );
    let (stdout, stderr) = expect_success(
        "project-test-workspace-ui-list-json",
        "workspace package selector ui json listing",
        &output,
    )
    .expect("workspace-path `ql test --list --json --package` should list selected ui tests");
    expect_empty_stderr(
        "project-test-workspace-ui-list-json",
        "workspace package selector ui json listing",
        &stderr,
    )
    .expect("workspace package selector ui json listing should not print stderr");

    let actual = parse_json_output("project-test-workspace-ui-list-json", &stdout);
    let expected = expected_workspace_package_ui_json_listing(&fixture.project_root, None);
    assert_eq!(
        actual, expected,
        "workspace-path ui json listing should preserve the selected-package contract"
    );
}

#[test]
fn test_workspace_path_package_selector_ui_list_json_accepts_target_and_filter_selectors() {
    let fixture = write_workspace_package_ui_fixture(
        "ql-project-test-workspace-ui-list-selectors-json",
        false,
    );
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--list", "--json"])
        .arg(&fixture.project_root)
        .args([
            "--package",
            "app",
            "--target",
            "tests/ui/type_error.ql",
            "--filter",
            "type_error",
        ]);
    let output = run_command_capture(
        &mut command,
        "`ql test --list --json --package --target --filter` workspace ui tests",
    );
    let (stdout, stderr) = expect_success(
        "project-test-workspace-ui-list-selectors-json",
        "workspace package selector ui json selector listing",
        &output,
    )
    .expect(
        "workspace-path ui json listing should accept package-relative target and filter selectors",
    );
    expect_empty_stderr(
        "project-test-workspace-ui-list-selectors-json",
        "workspace package selector ui json selector listing",
        &stderr,
    )
    .expect("workspace package selector ui json selector listing should not print stderr");

    let actual = parse_json_output("project-test-workspace-ui-list-selectors-json", &stdout);
    let expected =
        expected_workspace_package_ui_json_listing(&fixture.project_root, Some("type_error"));
    assert_eq!(
        actual, expected,
        "workspace-path ui json selector listing should preserve the stable json contract"
    );
}

#[test]
fn test_workspace_path_package_selector_ui_list_json_reports_missing_target() {
    let fixture = write_workspace_package_ui_fixture(
        "ql-project-test-workspace-ui-list-missing-target-json",
        false,
    );
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--list", "--json"])
        .arg(&fixture.project_root)
        .args(["--package", "app", "--target", "tests/ui/missing.ql"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --list --json --package --target` workspace ui missing target",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-workspace-ui-list-missing-target-json",
        "workspace package selector ui list missing target json",
        &output,
        1,
    )
    .expect("workspace-path ui list json should fail when target selection misses");
    expect_empty_stderr(
        "project-test-workspace-ui-list-missing-target-json",
        "workspace package selector ui list missing target json",
        &stderr,
    )
    .expect("workspace package selector ui list missing target json should stay on stdout");

    assert_workspace_package_ui_list_selection_failure_json(
        "project-test-workspace-ui-list-missing-target-json",
        &stdout,
        &fixture.project_root,
        None,
        "target-selection",
        "target `tests/ui/missing.ql`",
        "found no test target `tests/ui/missing.ql`",
    );
}

#[test]
fn test_workspace_path_package_selector_ui_list_json_reports_missing_filter() {
    let fixture = write_workspace_package_ui_fixture(
        "ql-project-test-workspace-ui-list-missing-filter-json",
        false,
    );
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--list", "--json"])
        .arg(&fixture.project_root)
        .args(["--package", "app", "--filter", "missing"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --list --json --package --filter` workspace ui missing filter",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-workspace-ui-list-missing-filter-json",
        "workspace package selector ui list missing filter json",
        &output,
        1,
    )
    .expect("workspace-path ui list json should fail when filter selection misses");
    expect_empty_stderr(
        "project-test-workspace-ui-list-missing-filter-json",
        "workspace package selector ui list missing filter json",
        &stderr,
    )
    .expect("workspace package selector ui list missing filter json should stay on stdout");

    assert_workspace_package_ui_list_selection_failure_json(
        "project-test-workspace-ui-list-missing-filter-json",
        &stdout,
        &fixture.project_root,
        Some("missing"),
        "filter-selection",
        "filter `missing`",
        "found no test files matching `missing`",
    );
}

#[test]
fn test_workspace_path_package_selector_reports_ui_snapshot_json_mismatch() {
    let fixture =
        write_workspace_package_ui_fixture("ql-project-test-workspace-ui-mismatch-json", false);
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--json"])
        .arg(&fixture.project_root)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --package` workspace ui snapshot mismatch",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-workspace-ui-mismatch-json",
        "workspace package selector ui mismatch json",
        &output,
        1,
    )
    .expect("workspace-path ui json should fail when the selected package snapshot mismatches");
    expect_empty_stderr(
        "project-test-workspace-ui-mismatch-json",
        "workspace package selector ui mismatch json",
        &stderr,
    )
    .expect("workspace package selector ui mismatch json should stay on stdout");

    assert_workspace_package_ui_snapshot_mismatch_json(
        "project-test-workspace-ui-mismatch-json",
        &stdout,
        &fixture.project_root,
    );
}

#[test]
fn test_package_path_runs_ui_snapshot_tests() {
    let workspace_root = workspace_root();
    let fixture = write_direct_project_ui_fixture("ql-project-test-ui", true);

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["test"]).arg(&fixture.project_root);
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
fn test_package_path_reports_ui_snapshot_json_success() {
    let workspace_root = workspace_root();
    let fixture = write_direct_project_ui_fixture("ql-project-test-ui-json", true);

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["test", "--json"]).arg(&fixture.project_root);
    let output = run_command_capture(&mut command, "`ql test --json` package ui tests");
    let (stdout, stderr) =
        expect_success("project-test-ui-json", "package ui json success", &output)
            .expect("package-path `ql test --json` should run ui snapshot tests");
    expect_empty_stderr("project-test-ui-json", "package ui json success", &stderr)
        .expect("package ui json success should not print stderr");

    let actual = parse_json_output("project-test-ui-json", &stdout);
    let expected = expected_project_ui_json_success(&fixture.project_root, None, None);
    assert_eq!(
        actual, expected,
        "package-path `ql test --json` should report ui snapshot success via the stable json contract"
    );
}

#[test]
fn test_direct_project_ui_file_uses_ui_snapshot_semantics() {
    let workspace_root = workspace_root();
    let fixture = write_direct_project_ui_fixture("ql-project-test-ui-direct-file", true);

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["test"]).arg(&fixture.ui_path);
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
fn test_direct_project_ui_file_reports_json_success() {
    let workspace_root = workspace_root();
    let fixture = write_direct_project_ui_fixture("ql-project-test-ui-direct-file-json", true);

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["test", "--json"]).arg(&fixture.ui_path);
    let output = run_command_capture(&mut command, "`ql test --json` direct project ui file");
    let (stdout, stderr) = expect_success(
        "project-test-ui-direct-file-json",
        "direct project ui file json success",
        &output,
    )
    .expect("direct project ui file `ql test --json` should pass");
    expect_empty_stderr(
        "project-test-ui-direct-file-json",
        "direct project ui file json success",
        &stderr,
    )
    .expect("direct project ui file json success should not print stderr");

    let actual = parse_json_output("project-test-ui-direct-file-json", &stdout);
    let expected = expected_project_ui_json_success(&fixture.ui_path, None, None);
    assert_eq!(
        actual, expected,
        "direct project ui file json success should preserve the stable json contract"
    );
}

#[test]
fn test_direct_project_ui_file_json_runs_with_package_target_and_filter_selectors() {
    let workspace_root = workspace_root();
    let fixture =
        write_direct_project_ui_fixture("ql-project-test-ui-direct-file-all-selectors-json", true);

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--json"])
        .arg(&fixture.ui_path)
        .args([
            "--package",
            "app",
            "--target",
            "tests/ui/type_error.ql",
            "--filter",
            "type_error",
        ]);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --package --target --filter` direct project ui file",
    );
    let (stdout, stderr) = expect_success(
        "project-test-ui-direct-file-all-selectors-json",
        "direct project ui file all selector json execution",
        &output,
    )
    .expect("direct project ui file `ql test --json --package app --target tests/ui/type_error.ql --filter type_error` should pass");
    expect_empty_stderr(
        "project-test-ui-direct-file-all-selectors-json",
        "direct project ui file all selector json execution",
        &stderr,
    )
    .expect("direct project ui file selector json execution should not print stderr");

    let actual = parse_json_output("project-test-ui-direct-file-all-selectors-json", &stdout);
    let expected =
        expected_project_ui_json_success(&fixture.ui_path, Some("app"), Some("type_error"));
    assert_eq!(
        actual, expected,
        "direct project ui file selector execution should preserve the stable json contract"
    );
}

#[test]
fn test_direct_project_ui_file_lists_with_package_target_and_filter_selectors_as_json() {
    let workspace_root = workspace_root();
    let fixture = write_direct_project_ui_fixture(
        "ql-project-test-ui-direct-file-list-selectors-json",
        false,
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--list", "--json"])
        .arg(&fixture.ui_path)
        .args([
            "--package",
            "app",
            "--target",
            "tests/ui/type_error.ql",
            "--filter",
            "type_error",
        ]);
    let output = run_command_capture(
        &mut command,
        "`ql test --list --json --package --target --filter` direct project ui file",
    );
    let (stdout, stderr) = expect_success(
        "project-test-ui-direct-file-list-selectors-json",
        "direct project ui file all selector json listing",
        &output,
    )
    .expect("direct project ui file `ql test --list --json --package app --target tests/ui/type_error.ql --filter type_error` should list without executing snapshots");
    expect_empty_stderr(
        "project-test-ui-direct-file-list-selectors-json",
        "direct project ui file all selector json listing",
        &stderr,
    )
    .expect("direct project ui file selector json listing should not print stderr");

    let actual = parse_json_output("project-test-ui-direct-file-list-selectors-json", &stdout);
    let expected =
        expected_direct_project_ui_json_listing(&fixture.ui_path, Some("app"), Some("type_error"));
    assert_eq!(
        actual, expected,
        "direct project ui file selector listing should preserve the stable json contract"
    );
}

#[test]
fn test_direct_project_ui_file_json_reports_snapshot_mismatch() {
    let workspace_root = workspace_root();
    let fixture =
        write_direct_project_ui_fixture("ql-project-test-ui-direct-file-mismatch-json", false);

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["test", "--json"]).arg(&fixture.ui_path);
    let output = run_command_capture(
        &mut command,
        "`ql test --json` direct project ui file mismatch",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-ui-direct-file-mismatch-json",
        "direct project ui file mismatch json",
        &output,
        1,
    )
    .expect("direct project ui file `ql test --json` should fail on snapshot mismatch");
    expect_empty_stderr(
        "project-test-ui-direct-file-mismatch-json",
        "direct project ui file mismatch json",
        &stderr,
    )
    .expect("direct project ui file mismatch json should stay on stdout");

    assert_project_ui_snapshot_mismatch_json(
        "project-test-ui-direct-file-mismatch-json",
        &stdout,
        &fixture.ui_path,
    );
}

#[test]
fn test_package_path_reports_ui_snapshot_json_mismatch() {
    let workspace_root = workspace_root();
    let fixture = write_direct_project_ui_fixture("ql-project-test-ui-mismatch-json", false);

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["test", "--json"]).arg(&fixture.project_root);
    let output = run_command_capture(&mut command, "`ql test --json` ui snapshot mismatch");
    let (stdout, stderr) = expect_exit_code(
        "project-test-ui-mismatch-json",
        "ui snapshot mismatch json tests",
        &output,
        1,
    )
    .expect("package-path `ql test --json` should fail on ui snapshot mismatches");
    expect_empty_stderr(
        "project-test-ui-mismatch-json",
        "ui snapshot mismatch json tests",
        &stderr,
    )
    .expect("package-path ui snapshot mismatch json should stay on stdout");

    assert_project_ui_snapshot_mismatch_json(
        "project-test-ui-mismatch-json",
        &stdout,
        &fixture.project_root,
    );
}

#[test]
fn test_package_path_reports_ui_snapshot_mismatch() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-ui-mismatch");
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
