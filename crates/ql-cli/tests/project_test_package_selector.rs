mod support;

use std::fs;
use std::path::PathBuf;

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

struct WorkspaceTestPackageSelectorProject {
    temp: TempDir,
    project_root: PathBuf,
    app_root: PathBuf,
    selected_smoke_output: PathBuf,
    selected_extra_output: PathBuf,
    unselected_smoke_output: PathBuf,
}

fn write_workspace_test_package_selector_project(
    prefix: &str,
) -> WorkspaceTestPackageSelectorProject {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("workspace");
    let app_root = project_root.join("packages").join("app");
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
        "workspace/packages/app/tests/api/extra.ql",
        "fn main() -> Int { return 0 }\n",
    );
    temp.write(
        "workspace/packages/tool/tests/tool_only.ql",
        "fn main() -> Int { return 0 }\n",
    );

    let selected_extra_output = executable_output_path(
        &project_root.join("packages/app/target/ql/debug/tests/api"),
        "extra",
    );
    let selected_smoke_output = executable_output_path(
        &project_root.join("packages/app/target/ql/debug/tests"),
        "app_only",
    );
    let unselected_smoke_output = executable_output_path(
        &project_root.join("packages/tool/target/ql/debug/tests"),
        "tool_only",
    );

    WorkspaceTestPackageSelectorProject {
        temp,
        project_root,
        app_root,
        selected_smoke_output,
        selected_extra_output,
        unselected_smoke_output,
    }
}

#[test]
fn test_workspace_path_selects_requested_package_tests() {
    if !toolchain_available("`ql test --package` workspace test") {
        return;
    }

    let fixture = write_workspace_test_package_selector_project("ql-project-test-package-selector");
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test"])
        .arg(&fixture.project_root)
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
            "test packages/app/tests/api/extra.ql ... ok",
            "test packages/app/tests/app_only.ql ... ok",
            "test result: ok. 2 passed; 0 failed",
        ],
    )
    .expect("workspace-path `ql test --package` should run only the selected package tests");
    assert!(
        !stdout.contains("packages/tool/tests/tool_only.ql"),
        "workspace-path `ql test --package` should not run tests from unselected packages: {stdout}"
    );
    expect_file_exists(
        "project-test-package-selector",
        &fixture.selected_smoke_output,
        "selected package smoke executable",
        "workspace package selector tests",
    )
    .expect("workspace-path `ql test --package` should emit selected package test artifacts");
    expect_file_exists(
        "project-test-package-selector",
        &fixture.selected_extra_output,
        "selected package nested smoke executable",
        "workspace package selector tests",
    )
    .expect(
        "workspace-path `ql test --package` should emit selected nested package test artifacts",
    );
    assert!(
        !fixture.unselected_smoke_output.exists(),
        "workspace-path `ql test --package` should not build tests from unselected packages"
    );
}

#[test]
fn test_workspace_member_directory_selects_requested_package_tests() {
    if !toolchain_available("`ql test --package` workspace member directory test") {
        return;
    }

    let fixture = write_workspace_test_package_selector_project(
        "ql-project-test-member-dir-package-selector",
    );
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test"])
        .arg(&fixture.app_root)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --package` workspace member directory",
    );
    let (stdout, stderr) = expect_success(
        "project-test-member-dir-package-selector",
        "workspace member directory package selector tests",
        &output,
    )
    .expect("workspace member directory `ql test --package` should succeed");
    expect_empty_stderr(
        "project-test-member-dir-package-selector",
        "workspace member directory package selector tests",
        &stderr,
    )
    .expect("workspace member directory `ql test --package` should not print stderr");
    expect_stdout_contains_all(
        "project-test-member-dir-package-selector",
        &stdout.replace('\\', "/"),
        &[
            "test packages/app/tests/api/extra.ql ... ok",
            "test packages/app/tests/app_only.ql ... ok",
            "test result: ok. 2 passed; 0 failed",
        ],
    )
    .expect(
        "workspace member directory `ql test --package` should run only the selected package tests",
    );
    assert!(
        !stdout.contains("packages/tool/tests/tool_only.ql"),
        "workspace member directory `ql test --package` should not run tests from unselected packages: {stdout}"
    );
    expect_file_exists(
        "project-test-member-dir-package-selector",
        &fixture.selected_smoke_output,
        "selected package smoke executable",
        "workspace member directory package selector tests",
    )
    .expect("workspace member directory `ql test --package` should emit selected package test artifacts");
    expect_file_exists(
        "project-test-member-dir-package-selector",
        &fixture.selected_extra_output,
        "selected package nested smoke executable",
        "workspace member directory package selector tests",
    )
    .expect(
        "workspace member directory `ql test --package` should emit selected nested package test artifacts",
    );
    assert!(
        !fixture.unselected_smoke_output.exists(),
        "workspace member directory `ql test --package` should not build tests from unselected packages"
    );
}

#[test]
fn test_workspace_member_directory_package_selector_filters_selected_package_tests() {
    if !toolchain_available("`ql test --filter --package` workspace member directory test") {
        return;
    }

    let fixture =
        write_workspace_test_package_selector_project("ql-project-test-member-dir-package-filter");
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--filter", "api/extra"])
        .arg(&fixture.app_root)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --filter --package` workspace member directory",
    );
    let (stdout, stderr) = expect_success(
        "project-test-member-dir-package-filter",
        "workspace member directory package selector filtered tests",
        &output,
    )
    .expect("workspace member directory `ql test --filter --package` should succeed");
    expect_empty_stderr(
        "project-test-member-dir-package-filter",
        "workspace member directory package selector filtered tests",
        &stderr,
    )
    .expect("workspace member directory `ql test --filter --package` should not print stderr");
    expect_stdout_contains_all(
        "project-test-member-dir-package-filter",
        &stdout.replace('\\', "/"),
        &[
            "test packages/app/tests/api/extra.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect(
        "workspace member directory `ql test --filter --package` should run only matching selected-package tests",
    );
    assert!(
        !stdout.contains("packages/app/tests/app_only.ql")
            && !stdout.contains("packages/tool/tests/tool_only.ql"),
        "workspace member directory `ql test --filter --package` should not run unselected tests: {stdout}"
    );
    expect_file_exists(
        "project-test-member-dir-package-filter",
        &fixture.selected_extra_output,
        "filtered package smoke executable",
        "workspace member directory package selector filtered tests",
    )
    .expect("workspace member directory `ql test --filter --package` should emit the filtered test artifact");
    assert!(
        !fixture.selected_smoke_output.exists() && !fixture.unselected_smoke_output.exists(),
        "workspace member directory `ql test --filter --package` should not build unselected package tests"
    );
}

#[test]
fn test_workspace_path_package_selector_accepts_package_relative_target() {
    if !toolchain_available("`ql test --target --package` workspace package-relative target test") {
        return;
    }

    let fixture =
        write_workspace_test_package_selector_project("ql-project-test-package-relative-target");
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["test"]).arg(&fixture.project_root).args([
        "--package",
        "app",
        "--target",
        "tests/api/extra.ql",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql test --target --package` workspace package-relative target",
    );
    let (stdout, stderr) = expect_success(
        "project-test-package-relative-target",
        "workspace package selector package-relative target tests",
        &output,
    )
    .expect("workspace `ql test --target --package` should accept package-relative targets");
    expect_empty_stderr(
        "project-test-package-relative-target",
        "workspace package selector package-relative target tests",
        &stderr,
    )
    .expect("workspace `ql test --target --package` should not print stderr");
    expect_stdout_contains_all(
        "project-test-package-relative-target",
        &stdout.replace('\\', "/"),
        &[
            "test packages/app/tests/api/extra.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("workspace `ql test --target --package` should run only the package-relative target");
    assert!(
        !stdout.contains("packages/app/tests/app_only.ql")
            && !stdout.contains("packages/tool/tests/tool_only.ql"),
        "workspace `ql test --target --package` should not run unselected tests: {stdout}"
    );
    expect_file_exists(
        "project-test-package-relative-target",
        &fixture.selected_extra_output,
        "package-relative target smoke executable",
        "workspace package selector package-relative target tests",
    )
    .expect("workspace `ql test --target --package` should emit the selected target artifact");
    assert!(
        !fixture.selected_smoke_output.exists() && !fixture.unselected_smoke_output.exists(),
        "workspace `ql test --target --package` should not build unselected package tests"
    );
}

#[test]
fn test_workspace_member_directory_package_selector_accepts_package_relative_target() {
    if !toolchain_available("`ql test --target --package` workspace member directory test") {
        return;
    }

    let fixture =
        write_workspace_test_package_selector_project("ql-project-test-member-dir-package-target");
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["test"]).arg(&fixture.app_root).args([
        "--package",
        "app",
        "--target",
        "tests/api/extra.ql",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql test --target --package` workspace member directory",
    );
    let (stdout, stderr) = expect_success(
        "project-test-member-dir-package-target",
        "workspace member directory package selector target tests",
        &output,
    )
    .expect(
        "workspace member directory `ql test --target --package` should accept package-relative targets",
    );
    expect_empty_stderr(
        "project-test-member-dir-package-target",
        "workspace member directory package selector target tests",
        &stderr,
    )
    .expect("workspace member directory `ql test --target --package` should not print stderr");
    expect_stdout_contains_all(
        "project-test-member-dir-package-target",
        &stdout.replace('\\', "/"),
        &[
            "test packages/app/tests/api/extra.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect(
        "workspace member directory `ql test --target --package` should run only the selected target",
    );
    assert!(
        !stdout.contains("packages/app/tests/app_only.ql")
            && !stdout.contains("packages/tool/tests/tool_only.ql"),
        "workspace member directory `ql test --target --package` should not run unselected tests: {stdout}"
    );
    expect_file_exists(
        "project-test-member-dir-package-target",
        &fixture.selected_extra_output,
        "target-selected package smoke executable",
        "workspace member directory package selector target tests",
    )
    .expect("workspace member directory `ql test --target --package` should emit the selected target artifact");
    assert!(
        !fixture.selected_smoke_output.exists() && !fixture.unselected_smoke_output.exists(),
        "workspace member directory `ql test --target --package` should not build unselected package tests"
    );
}

#[test]
fn test_workspace_path_package_selector_reports_json_success() {
    if !toolchain_available("`ql test --json --package` workspace test") {
        return;
    }

    let fixture =
        write_workspace_test_package_selector_project("ql-project-test-package-selector-json");
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--json"])
        .arg(&fixture.project_root)
        .args(["--package", "app"]);
    let output = run_command_capture(&mut command, "`ql test --json --package` workspace path");
    let (stdout, stderr) = expect_success(
        "project-test-package-selector-json",
        "workspace package selector json tests",
        &output,
    )
    .expect("workspace-path `ql test --json --package` should succeed");
    expect_empty_stderr(
        "project-test-package-selector-json",
        "workspace package selector json tests",
        &stderr,
    )
    .expect("workspace-path `ql test --json --package` should not print stderr");

    let actual = parse_json_output("project-test-package-selector-json", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.test.v1",
        "path": fixture.project_root.display().to_string().replace('\\', "/"),
        "requested_profile": "debug",
        "profile_overridden": false,
        "package_name": "app",
        "filter": JsonValue::Null,
        "list_only": false,
        "status": "ok",
        "discovered_total": 2,
        "selected_total": 2,
        "targets": [
            {
                "path": "packages/app/tests/api/extra.ql",
                "kind": "smoke",
                "profile": "debug",
            },
            {
                "path": "packages/app/tests/app_only.ql",
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
        "workspace-path `ql test --json --package` should match the stable selected-package contract"
    );
    expect_file_exists(
        "project-test-package-selector-json",
        &fixture.selected_smoke_output,
        "selected package smoke executable",
        "workspace package selector json tests",
    )
    .expect("workspace-path `ql test --json --package` should emit selected package artifacts");
    expect_file_exists(
        "project-test-package-selector-json",
        &fixture.selected_extra_output,
        "selected package nested smoke executable",
        "workspace package selector json tests",
    )
    .expect(
        "workspace-path `ql test --json --package` should emit selected nested package artifacts",
    );
    assert!(
        !fixture.unselected_smoke_output.exists(),
        "workspace-path `ql test --json --package` should not build unselected package tests"
    );
}

#[test]
fn test_workspace_member_directory_package_selector_reports_json_success() {
    if !toolchain_available("`ql test --json --package` workspace member directory test") {
        return;
    }

    let fixture = write_workspace_test_package_selector_project(
        "ql-project-test-member-dir-package-selector-json",
    );
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--json"])
        .arg(&fixture.app_root)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --package` workspace member directory",
    );
    let (stdout, stderr) = expect_success(
        "project-test-member-dir-package-selector-json",
        "workspace member directory package selector json tests",
        &output,
    )
    .expect("workspace member directory `ql test --json --package` should succeed");
    expect_empty_stderr(
        "project-test-member-dir-package-selector-json",
        "workspace member directory package selector json tests",
        &stderr,
    )
    .expect("workspace member directory `ql test --json --package` should not print stderr");

    let actual = parse_json_output("project-test-member-dir-package-selector-json", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.test.v1",
        "path": fixture.app_root.display().to_string().replace('\\', "/"),
        "requested_profile": "debug",
        "profile_overridden": false,
        "package_name": "app",
        "filter": JsonValue::Null,
        "list_only": false,
        "status": "ok",
        "discovered_total": 2,
        "selected_total": 2,
        "targets": [
            {
                "path": "packages/app/tests/api/extra.ql",
                "kind": "smoke",
                "profile": "debug",
            },
            {
                "path": "packages/app/tests/app_only.ql",
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
        "workspace member directory `ql test --json --package` should match the stable selected-package contract"
    );
    expect_file_exists(
        "project-test-member-dir-package-selector-json",
        &fixture.selected_smoke_output,
        "selected package smoke executable",
        "workspace member directory package selector json tests",
    )
    .expect(
        "workspace member directory `ql test --json --package` should emit selected package artifacts",
    );
    expect_file_exists(
        "project-test-member-dir-package-selector-json",
        &fixture.selected_extra_output,
        "selected package nested smoke executable",
        "workspace member directory package selector json tests",
    )
    .expect(
        "workspace member directory `ql test --json --package` should emit selected nested package artifacts",
    );
    assert!(
        !fixture.unselected_smoke_output.exists(),
        "workspace member directory `ql test --json --package` should not build unselected package tests"
    );
}

#[test]
fn test_workspace_path_json_rejects_unknown_package_selector() {
    let fixture = write_workspace_test_package_selector_project(
        "ql-project-test-package-selector-missing-json",
    );
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--json"])
        .arg(&fixture.project_root)
        .args(["--package", "missing"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --package` unknown workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-package-selector-missing-json",
        "unknown workspace package selector json",
        &output,
        1,
    )
    .expect("workspace-path `ql test --json --package missing` should reject unknown packages");
    expect_empty_stderr(
        "project-test-package-selector-missing-json",
        "unknown workspace package selector json",
        &stderr,
    )
    .expect("unknown workspace package selector json should stay on stdout");

    let json = parse_json_output("project-test-package-selector-missing-json", &stdout);
    assert_eq!(json["schema"], "ql.test.v1");
    assert_eq!(
        json["path"],
        fixture
            .project_root
            .display()
            .to_string()
            .replace('\\', "/")
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
            .expect("unknown package selector json failure should expose a message")
            .contains("does not contain package `missing`"),
        "unknown package selector json failure should describe the missing package: {json}"
    );
}

#[test]
fn test_workspace_path_json_package_selector_reports_missing_source_root_preflight_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package-selector-missing-source-root-json");
    let project_root = temp.path().join("workspace");
    fs::create_dir_all(project_root.join("packages/tool/src")).expect("create tool source root");
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
        "workspace/packages/tool/src/lib.ql",
        "pub fn helper() -> Int { return 1 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test", "--json"])
        .arg(&project_root)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --package` selected workspace package missing source root",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-package-selector-missing-source-root-json",
        "selected workspace package missing source root json",
        &output,
        1,
    )
    .expect("workspace-path `ql test --json --package app` should fail for missing selected source root");
    expect_empty_stderr(
        "project-test-package-selector-missing-source-root-json",
        "selected workspace package missing source root json",
        &stderr,
    )
    .expect("selected workspace package missing source root json should stay on stdout");

    let json = parse_json_output(
        "project-test-package-selector-missing-source-root-json",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.test.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["package_name"], "app");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["error_kind"], "manifest");
    assert_eq!(failure["stage"], "target-discovery");
    assert_eq!(failure["selector"], JsonValue::Null);
    assert_eq!(failure["target_count"], JsonValue::Null);
    assert!(
        failure["message"]
            .as_str()
            .expect("selected workspace missing source root json failure should expose a message")
            .contains("package source directory"),
        "selected workspace missing source root json failure should describe source root lookup: {json}"
    );
}

#[test]
fn test_package_path_json_package_selector_reports_missing_source_root_preflight_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package-path-selector-missing-source-root-json");
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
    command
        .args(["test", "--json"])
        .arg(&project_root)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --package` package path missing source root",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-package-path-selector-missing-source-root-json",
        "package path selector missing source root json",
        &output,
        1,
    )
    .expect("package-path `ql test --json --package app` should fail for missing source root");
    expect_empty_stderr(
        "project-test-package-path-selector-missing-source-root-json",
        "package path selector missing source root json",
        &stderr,
    )
    .expect("package path selector missing source root json should stay on stdout");

    let json = parse_json_output(
        "project-test-package-path-selector-missing-source-root-json",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.test.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["package_name"], "app");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["error_kind"], "manifest");
    assert_eq!(failure["stage"], "target-discovery");
    assert_eq!(failure["selector"], JsonValue::Null);
    assert_eq!(failure["target_count"], JsonValue::Null);
    assert!(
        failure["message"]
            .as_str()
            .expect("package path missing source root json failure should expose a message")
            .contains("package source directory"),
        "package path missing source root json failure should describe source root lookup: {json}"
    );
}

#[test]
fn test_workspace_path_json_rejects_duplicate_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package-selector-duplicate-json");
    let project_root = temp.path().join("workspace");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/a", "packages/b"]
"#,
    );
    temp.write(
        "workspace/packages/a/qlang.toml",
        r#"
[package]
name = "util"
"#,
    );
    temp.write(
        "workspace/packages/a/src/lib.ql",
        "pub fn a() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/b/qlang.toml",
        r#"
[package]
name = "util"
"#,
    );
    temp.write(
        "workspace/packages/b/src/lib.ql",
        "pub fn b() -> Int { return 2 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test", "--json"])
        .arg(&project_root)
        .args(["--package", "util"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --package` duplicate workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-package-selector-duplicate-json",
        "duplicate workspace package selector json",
        &output,
        1,
    )
    .expect("workspace-path `ql test --json --package util` should reject duplicate package names");
    expect_empty_stderr(
        "project-test-package-selector-duplicate-json",
        "duplicate workspace package selector json",
        &stderr,
    )
    .expect("duplicate workspace package selector json should stay on stdout");

    let json = parse_json_output("project-test-package-selector-duplicate-json", &stdout);
    assert_eq!(json["schema"], "ql.test.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["package_name"], "util");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["error_kind"], "selector");
    assert_eq!(failure["stage"], "package-selection");
    assert_eq!(failure["selector"], "package `util`");
    assert_eq!(failure["target_count"], 2);
    assert!(
        failure["message"]
            .as_str()
            .expect("duplicate package selector json failure should expose a message")
            .contains("contains multiple members for package `util`"),
        "duplicate package selector json failure should describe ambiguous package matches: {json}"
    );
}

#[test]
fn test_workspace_path_json_rejects_invalid_package_selector() {
    let fixture = write_workspace_test_package_selector_project(
        "ql-project-test-package-selector-invalid-json",
    );
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--json"])
        .arg(&fixture.project_root)
        .args(["--package", "packages/app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --package` invalid workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-package-selector-invalid-json",
        "invalid workspace package selector json",
        &output,
        1,
    )
    .expect("workspace-path `ql test --json --package packages/app` should reject invalid package names");
    expect_empty_stderr(
        "project-test-package-selector-invalid-json",
        "invalid workspace package selector json",
        &stderr,
    )
    .expect("invalid workspace package selector json should stay on stdout");

    let json = parse_json_output("project-test-package-selector-invalid-json", &stdout);
    assert_eq!(json["schema"], "ql.test.v1");
    assert_eq!(json["package_name"], "packages/app");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["error_kind"], "selector");
    assert_eq!(failure["stage"], "package-selection");
    assert_eq!(failure["selector"], "package `packages/app`");
    assert_eq!(failure["target_count"], JsonValue::Null);
    assert!(
        failure["message"]
            .as_str()
            .expect("invalid package selector json failure should expose a message")
            .contains("contains a path separator"),
        "invalid package selector json failure should describe package-name validation: {json}"
    );
}

#[test]
fn test_package_path_json_rejects_unknown_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package-path-selector-missing-json");
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
    command
        .args(["test", "--json"])
        .arg(&project_root)
        .args(["--package", "missing"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --package` unknown package path package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-package-path-selector-missing-json",
        "unknown package path package selector json",
        &output,
        1,
    )
    .expect("package-path `ql test --json --package missing` should reject package mismatch");
    expect_empty_stderr(
        "project-test-package-path-selector-missing-json",
        "unknown package path package selector json",
        &stderr,
    )
    .expect("unknown package path package selector json should stay on stdout");

    let json = parse_json_output("project-test-package-path-selector-missing-json", &stdout);
    assert_eq!(json["schema"], "ql.test.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["package_name"], "missing");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["error_kind"], "selector");
    assert_eq!(failure["stage"], "package-selection");
    assert_eq!(failure["selector"], "package `missing`");
    assert_eq!(failure["target_count"], 0);
    assert!(
        failure["message"]
            .as_str()
            .expect("package path selector json failure should expose a message")
            .contains("matched no workspace members"),
        "package path selector json failure should describe package mismatch: {json}"
    );
}

#[test]
fn test_workspace_path_rejects_unknown_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package-selector-missing");
    let project_root = temp.path().join("workspace");
    fs::create_dir_all(project_root.join("packages/app/src"))
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
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql test` package selector matched no workspace members under `{}`",
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .expect("unknown workspace package selector should report the missing package");
    expect_stderr_contains(
        "project-test-package-selector-missing",
        "unknown workspace package selector",
        &stderr.replace('\\', "/"),
        &format!(
            "hint: rerun `ql test {}` to inspect all workspace members, or adjust `--package`",
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .expect("unknown workspace package selector should suggest inspecting discovered members");
}

#[test]
fn test_workspace_path_rejects_duplicate_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package-selector-duplicate");
    let project_root = temp.path().join("workspace");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/a", "packages/b"]
"#,
    );
    temp.write(
        "workspace/packages/a/qlang.toml",
        r#"
[package]
name = "util"
"#,
    );
    temp.write(
        "workspace/packages/a/src/lib.ql",
        "pub fn a() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/a/tests/smoke.ql",
        "fn main() -> Int { return 0 }\n",
    );
    temp.write(
        "workspace/packages/b/qlang.toml",
        r#"
[package]
name = "util"
"#,
    );
    temp.write(
        "workspace/packages/b/src/lib.ql",
        "pub fn b() -> Int { return 2 }\n",
    );
    temp.write(
        "workspace/packages/b/tests/smoke.ql",
        "fn main() -> Int { return 0 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test"])
        .arg(&project_root)
        .args(["--package", "util"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --package` duplicate workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-package-selector-duplicate",
        "duplicate workspace package selector",
        &output,
        1,
    )
    .expect("workspace-path `ql test --package` should reject duplicate package names");
    expect_empty_stdout(
        "project-test-package-selector-duplicate",
        "duplicate workspace package selector",
        &stdout,
    )
    .expect("duplicate workspace package selector should not print stdout");
    expect_stderr_contains(
        "project-test-package-selector-duplicate",
        "duplicate workspace package selector",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql test` workspace manifest `{}` contains multiple members for package `util`: packages/a ({}/packages/a/qlang.toml), packages/b ({}/packages/b/qlang.toml)",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/"),
            project_root.to_string_lossy().replace('\\', "/"),
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .expect("duplicate workspace package selector should list matching members");
}

#[test]
fn test_workspace_path_rejects_invalid_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package-selector-invalid");
    let project_root = temp.path().join("workspace");
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

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test"])
        .arg(&project_root)
        .args(["--package", "packages/app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --package` invalid workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-package-selector-invalid",
        "invalid workspace package selector",
        &output,
        1,
    )
    .expect("workspace-path `ql test --package` should reject invalid package names");
    expect_empty_stdout(
        "project-test-package-selector-invalid",
        "invalid workspace package selector",
        &stdout,
    )
    .expect("invalid workspace package selector should not print stdout");
    expect_stderr_contains(
        "project-test-package-selector-invalid",
        "invalid workspace package selector",
        &stderr,
        "error: `ql test` does not accept package name `packages/app` because it contains a path separator",
    )
    .expect("invalid workspace package selector should report package-name validation");
}

#[test]
fn test_workspace_path_package_selector_surfaces_broken_member_metadata() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package-selector-broken-member");
    let project_root = temp.path().join("workspace");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken"]
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
    temp.write(
        "workspace/packages/broken/qlang.toml",
        r#"
[package]
version = "0.1.0"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test"])
        .arg(&project_root)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --package` broken workspace member metadata",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-package-selector-broken-member",
        "broken workspace member package selector",
        &output,
        1,
    )
    .expect("workspace-path `ql test --package` should reject unresolved member metadata");
    expect_empty_stdout(
        "project-test-package-selector-broken-member",
        "broken workspace member package selector",
        &stdout,
    )
    .expect("broken workspace member package selector should not print stdout");
    expect_stderr_contains(
        "project-test-package-selector-broken-member",
        "broken workspace member package selector",
        &stderr.replace('\\', "/"),
        "error: `ql test` failed to inspect workspace member `packages/broken`: manifest",
    )
    .expect("broken workspace member package selector should surface the broken member");
    expect_stderr_contains(
        "project-test-package-selector-broken-member",
        "broken workspace member package selector",
        &stderr,
        "does not declare `[package].name`",
    )
    .expect("broken workspace member package selector should preserve package-name detail");
}

#[test]
fn test_workspace_path_package_selector_lists_selected_member_with_unbuildable_unselected_member() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package-selector-unselected-source-root");
    let project_root = temp.path().join("workspace");
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
        "workspace/packages/app/src/lib.ql",
        "pub fn helper() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/app/tests/smoke.ql",
        "fn main() -> Int { return 0 }\n",
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test"])
        .arg(&project_root)
        .args(["--package", "app", "--list"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --package --list` skips unselected member target discovery",
    );
    let (stdout, stderr) = expect_success(
        "project-test-package-selector-unselected-source-root",
        "selected workspace package test listing with unbuildable unselected member",
        &output,
    )
    .expect(
        "workspace-path `ql test --package app --list` should not inspect unselected build targets",
    );
    expect_empty_stderr(
        "project-test-package-selector-unselected-source-root",
        "selected workspace package test listing with unbuildable unselected member",
        &stderr,
    )
    .expect("selected workspace package test listing should not print stderr");
    expect_stdout_contains_all(
        "project-test-package-selector-unselected-source-root",
        &stdout.replace('\\', "/"),
        &["packages/app/tests/smoke.ql", "test listing: 1 discovered"],
    )
    .expect("selected workspace package test listing should list only selected package tests");
}

#[test]
fn test_direct_source_file_rejects_package_selector_without_project_context() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-direct-source-package-selector");
    let source_path = temp.write("sample.ql", "fn main() -> Int { return 0 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test"])
        .arg(&source_path)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test` direct source package selector requires project context",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-direct-source-package-selector",
        "direct source package selector preflight failure",
        &output,
        1,
    )
    .expect("direct source file `ql test --package app` should exit with code 1");
    expect_empty_stdout(
        "project-test-direct-source-package-selector",
        "direct source package selector preflight failure",
        &stdout,
    )
    .expect("direct source package selector preflight failure should not print stdout");
    expect_stderr_contains(
        "project-test-direct-source-package-selector",
        "direct source package selector preflight failure",
        &stderr,
        "error: `ql test` package selectors require a package or workspace path",
    )
    .expect("direct source package selector preflight failure should explain the project-context requirement");
    expect_stderr_contains(
        "project-test-direct-source-package-selector",
        "direct source package selector preflight failure",
        &stderr,
        "note: selector: package `app`",
    )
    .expect("direct source package selector preflight failure should print the selector note");
}

#[test]
fn test_direct_source_file_json_rejects_package_selector_without_project_context() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-direct-source-package-selector-json");
    let source_path = temp.write("sample.ql", "fn main() -> Int { return 0 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test", "--json"])
        .arg(&source_path)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --json` direct source package selector requires project context",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-direct-source-package-selector-json",
        "direct source package selector json preflight failure",
        &output,
        1,
    )
    .expect("direct source file `ql test --json --package app` should exit with code 1");
    expect_empty_stderr(
        "project-test-direct-source-package-selector-json",
        "direct source package selector json preflight failure",
        &stderr,
    )
    .expect("direct source package selector json preflight failure should stay on stdout");

    let json = parse_json_output("project-test-direct-source-package-selector-json", &stdout);
    assert_eq!(json["schema"], "ql.test.v1");
    assert_eq!(
        json["path"],
        source_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["package_name"], "app");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["discovered_total"], 0);
    assert_eq!(json["selected_total"], 0);
    assert_eq!(json["targets"], serde_json::json!([]));
    assert_eq!(json["failures"], serde_json::json!([]));
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["error_kind"], "selector");
    assert_eq!(failure["stage"], "target-selection");
    assert_eq!(failure["selector"], "package `app`");
    assert_eq!(failure["target_count"], JsonValue::Null);
    assert!(
        failure["message"]
            .as_str()
            .expect("direct source package selector json failure should expose a message")
            .contains("package selectors require a package or workspace path"),
        "direct source package selector json failure should explain project context: {json}"
    );
}
