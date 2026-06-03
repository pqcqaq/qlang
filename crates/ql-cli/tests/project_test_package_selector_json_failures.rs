mod support;

use std::fs;

use serde_json::Value as JsonValue;
use support::project_test_package_selector::{
    parse_json_output, write_workspace_test_package_selector_project,
};
use support::{
    TempDir, expect_empty_stderr, expect_exit_code, ql_command, run_command_capture, workspace_root,
};

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
