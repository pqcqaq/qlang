mod support;

use std::fs;

use ql_driver::{ToolchainOptions, discover_toolchain};
use serde_json::Value as JsonValue;
use support::{
    TempDir, executable_output_path, expect_empty_stderr, expect_empty_stdout, expect_exit_code,
    expect_file_exists, expect_stderr_contains, expect_stdout_contains_all, expect_success,
    ql_command, run_command_capture, workspace_root,
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

#[test]
fn test_package_path_selects_requested_target() {
    if !toolchain_available("`ql test --target` package test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-target");
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
fn test_package_path_filters_discovered_tests() {
    if !toolchain_available("`ql test --filter` package test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-filter");
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
fn test_package_path_reports_missing_filter_matches() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-filter-missing");
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
fn test_package_path_reports_missing_tests() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-missing");
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
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test` package without tests");
    let (stdout, stderr) =
        expect_exit_code("project-test-missing", "missing package tests", &output, 1)
            .expect("package-path `ql test` should fail when no tests are discovered");
    expect_empty_stdout("project-test-missing", "missing package tests", &stdout)
        .expect("missing package tests should not print stdout");
    expect_stderr_contains(
        "project-test-missing",
        "missing package tests",
        &stderr,
        "error: `ql test` found no `.ql` test files under",
    )
    .expect("missing package tests should describe missing tests");
}
