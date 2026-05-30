mod support;

use std::path::PathBuf;

use serde_json::Value as JsonValue;
use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_snapshot_matches,
    expect_stderr_contains, expect_success, ql_command, run_command_capture, workspace_root,
};

fn normalize_output_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&normalize_output_text(stdout))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
}

struct WorkspaceGraphSelectorFixture {
    _temp: TempDir,
    project_root: PathBuf,
    app_manifest_path: PathBuf,
    app_member_dir: PathBuf,
    app_source_path: PathBuf,
}

fn write_workspace_graph_selector_fixture(prefix: &str) -> WorkspaceGraphSelectorFixture {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages").join("app").join("src"))
        .expect("create workspace app directory");
    std::fs::create_dir_all(project_root.join("packages").join("tool").join("src"))
        .expect("create workspace tool directory");
    std::fs::create_dir_all(project_root.join("dep")).expect("create dependency directory");

    let app_manifest_path = temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../../dep"]
"#,
    );
    let app_source_path = temp.write(
        "workspace/packages/app/src/lib.ql",
        r#"
pub fn run() -> Int {
    return 0
}
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
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
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/packages/app/app.qi",
        r#"
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn run() -> Int
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported() -> Int
"#,
    );

    WorkspaceGraphSelectorFixture {
        _temp: temp,
        app_member_dir: project_root.join("packages").join("app"),
        app_source_path,
        project_root,
        app_manifest_path,
    }
}

fn expected_workspace_app_package_graph_json(fixture: &WorkspaceGraphSelectorFixture) -> String {
    format!(
        "{{\n  \"interface\": {{\n    \"detail\": null,\n    \"path\": \"app.qi\",\n    \"stale_reasons\": [],\n    \"status\": \"valid\"\n  }},\n  \"manifest_path\": \"{}\",\n  \"package_name\": \"app\",\n  \"reference_interfaces\": [\n    {{\n      \"detail\": null,\n      \"manifest_path\": \"dep/qlang.toml\",\n      \"package_name\": \"dep\",\n      \"path\": \"dep/dep.qi\",\n      \"reference\": \"../../dep\",\n      \"stale_reasons\": [],\n      \"status\": \"valid\",\n      \"transitive_reference_failures\": {{\n        \"count\": 0,\n        \"first_failure\": null\n      }}\n    }}\n  ],\n  \"references\": [\n    \"../../dep\"\n  ],\n  \"schema\": \"ql.project.graph.v1\",\n  \"workspace_members\": [],\n  \"workspace_packages\": []\n}}\n",
        fixture
            .app_manifest_path
            .to_string_lossy()
            .replace('\\', "/")
    )
}

#[test]
fn project_graph_supports_package_selector_for_workspace_root() {
    let workspace_root = workspace_root();
    let fixture =
        write_workspace_graph_selector_fixture("ql-project-graph-workspace-package-selector");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "graph", "--package", "app"])
        .arg(&fixture.project_root);
    let output = run_command_capture(&mut command, "`ql project graph --package` workspace root");
    let (stdout, stderr) = expect_success(
        "project-graph-workspace-package-selector",
        "workspace root package graph selector",
        &output,
    )
    .expect("workspace root package graph selector should succeed");
    expect_empty_stderr(
        "project-graph-workspace-package-selector",
        "workspace root package graph selector",
        &stderr,
    )
    .expect("workspace root package graph selector should stay silent on stderr");

    let expected = format!(
        "manifest: {}\npackage: app\nworkspace_members: []\nreferences:\n  - ../../dep\ninterface:\n  path: app.qi\n  status: valid\nreference_interfaces:\n  - reference: ../../dep\n    manifest: dep/qlang.toml\n    package: dep\n    path: dep/dep.qi\n    status: valid\n",
        fixture
            .app_manifest_path
            .to_string_lossy()
            .replace('\\', "/")
    );
    expect_snapshot_matches(
        "project-graph-workspace-package-selector",
        "workspace root package graph selector stdout",
        &expected,
        &stdout.replace('\\', "/"),
    )
    .expect("workspace root package graph selector should render the selected package graph");
}

#[test]
fn project_graph_supports_json_package_selector_for_workspace_root() {
    let workspace_root = workspace_root();
    let fixture =
        write_workspace_graph_selector_fixture("ql-project-graph-workspace-package-selector-json");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "graph", "--package", "app"])
        .arg(&fixture.project_root)
        .arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql project graph --package --json` workspace root",
    );
    let (stdout, stderr) = expect_success(
        "project-graph-workspace-package-selector-json",
        "workspace root package graph selector json",
        &output,
    )
    .expect("workspace root package graph selector json should succeed");
    expect_empty_stderr(
        "project-graph-workspace-package-selector-json",
        "workspace root package graph selector json",
        &stderr,
    )
    .expect("workspace root package graph selector json should stay silent on stderr");

    let expected = expected_workspace_app_package_graph_json(&fixture);
    expect_snapshot_matches(
        "project-graph-workspace-package-selector-json",
        "workspace root package graph selector json stdout",
        &expected,
        &stdout.replace('\\', "/"),
    )
    .expect("workspace root package graph selector json should render the selected package graph");
}

#[test]
fn project_graph_json_package_selector_uses_workspace_member_source_context() {
    let workspace_root = workspace_root();
    let fixture = write_workspace_graph_selector_fixture(
        "ql-project-graph-member-source-package-selector-json",
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "graph", "--package", "app"])
        .arg(&fixture.app_source_path)
        .arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql project graph --package --json` workspace member source",
    );
    let (stdout, stderr) = expect_success(
        "project-graph-member-source-package-selector-json",
        "workspace member source package graph selector json",
        &output,
    )
    .expect("workspace member source package graph selector json should succeed");
    expect_empty_stderr(
        "project-graph-member-source-package-selector-json",
        "workspace member source package graph selector json",
        &stderr,
    )
    .expect("workspace member source package graph selector json should stay silent on stderr");

    let expected = expected_workspace_app_package_graph_json(&fixture);
    expect_snapshot_matches(
        "project-graph-member-source-package-selector-json",
        "workspace member source package graph selector json stdout",
        &expected,
        &stdout.replace('\\', "/"),
    )
    .expect("workspace member source package graph selector json should render selected package");
}

#[test]
fn project_graph_json_package_selector_uses_workspace_member_directory_context() {
    let workspace_root = workspace_root();
    let fixture = write_workspace_graph_selector_fixture(
        "ql-project-graph-member-directory-package-selector-json",
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "graph", "--package", "app"])
        .arg(&fixture.app_member_dir)
        .arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql project graph --package --json` workspace member directory",
    );
    let (stdout, stderr) = expect_success(
        "project-graph-member-directory-package-selector-json",
        "workspace member directory package graph selector json",
        &output,
    )
    .expect("workspace member directory package graph selector json should succeed");
    expect_empty_stderr(
        "project-graph-member-directory-package-selector-json",
        "workspace member directory package graph selector json",
        &stderr,
    )
    .expect("workspace member directory package graph selector json should stay silent on stderr");

    let expected = expected_workspace_app_package_graph_json(&fixture);
    expect_snapshot_matches(
        "project-graph-member-directory-package-selector-json",
        "workspace member directory package graph selector json stdout",
        &expected,
        &stdout.replace('\\', "/"),
    )
    .expect(
        "workspace member directory package graph selector json should render selected package",
    );
}

#[test]
fn project_graph_rejects_missing_workspace_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-workspace-package-missing");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages").join("app").join("src"))
        .expect("create workspace app directory");

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

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "graph", "--package", "missing"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project graph --package` missing workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-graph-workspace-package-missing",
        "workspace root package graph selector missing package",
        &output,
        1,
    )
    .expect("workspace root package graph selector should fail for missing packages");
    expect_empty_stdout(
        "project-graph-workspace-package-missing",
        "workspace root package graph selector missing package",
        &stdout,
    )
    .expect("workspace root package graph selector missing package should not print stdout");
    expect_stderr_contains(
        "project-graph-workspace-package-missing",
        "workspace root package graph selector missing package",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project graph` package selector matched no workspace members under `{}`",
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .expect(
        "workspace root package graph selector missing package should report the missing package",
    );
    expect_stderr_contains(
        "project-graph-workspace-package-missing",
        "workspace root package graph selector missing package",
        &stderr,
        "note: selector: package `missing`",
    )
    .expect("workspace root package graph selector missing package should report the selector");
    expect_stderr_contains(
        "project-graph-workspace-package-missing",
        "workspace root package graph selector missing package",
        &stderr.replace('\\', "/"),
        &format!(
            "hint: rerun `ql project graph {}` to inspect all workspace members, or adjust `--package`",
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .expect("workspace root package graph selector missing package should preserve the rerun hint");
}

#[test]
fn project_graph_json_reports_missing_workspace_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-workspace-package-missing-json");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages").join("app").join("src"))
        .expect("create workspace app directory");

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

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "graph", "--package", "missing", "--json"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project graph --package --json` missing workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-graph-workspace-package-missing-json",
        "workspace root package graph selector missing package json",
        &output,
        1,
    )
    .expect("workspace root package graph selector json should fail for missing packages");
    expect_empty_stderr(
        "project-graph-workspace-package-missing-json",
        "workspace root package graph selector missing package json",
        &stderr,
    )
    .expect("workspace root package graph selector missing package json should stay on stdout");

    let json = parse_json_output("project-graph-workspace-package-missing-json", &stdout);
    assert_eq!(json["schema"], "ql.project.graph.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["manifest_path"],
        project_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    assert_eq!(json["failure"]["kind"], "selection");
    let failure = &json["failure"]["selection_failure"];
    assert_eq!(failure["stage"], "package-selection");
    assert_eq!(failure["selector"], "package `missing`");
    assert_eq!(failure["target_count"], 0);
    assert!(
        failure["message"]
            .as_str()
            .expect("project graph json selector miss should expose a message")
            .contains("package selector matched no workspace members"),
        "project graph json selector miss should describe the missing package: {json}"
    );
}

#[test]
fn project_graph_rejects_duplicate_workspace_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-workspace-package-duplicate");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages").join("a").join("src"))
        .expect("create first duplicate workspace directory");
    std::fs::create_dir_all(project_root.join("packages").join("b").join("src"))
        .expect("create second duplicate workspace directory");

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
        "workspace/packages/b/qlang.toml",
        r#"
[package]
name = "util"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "graph", "--package", "util"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project graph --package` duplicate workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-graph-workspace-package-duplicate",
        "workspace root package graph selector duplicate package",
        &output,
        1,
    )
    .expect("workspace root package graph selector should fail for duplicate package names");
    expect_empty_stdout(
        "project-graph-workspace-package-duplicate",
        "workspace root package graph selector duplicate package",
        &stdout,
    )
    .expect("workspace root package graph selector duplicate package should not print stdout");
    expect_stderr_contains(
        "project-graph-workspace-package-duplicate",
        "workspace root package graph selector duplicate package",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project graph` workspace manifest `{}` contains multiple members for package `util`: packages/a ({}/packages/a/qlang.toml), packages/b ({}/packages/b/qlang.toml)",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/"),
            project_root.to_string_lossy().replace('\\', "/"),
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .expect("workspace root package graph selector duplicate package should list matching members");
}

#[test]
fn project_graph_package_selector_surfaces_broken_workspace_member_metadata() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-workspace-package-broken-member");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages").join("app").join("src"))
        .expect("create healthy workspace member directory");
    std::fs::create_dir_all(project_root.join("packages").join("broken"))
        .expect("create broken workspace member directory");

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
        "workspace/packages/broken/qlang.toml",
        r#"
[package]
version = "0.1.0"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "graph", "--package", "app"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project graph --package` broken workspace member metadata",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-graph-workspace-package-broken-member",
        "workspace root package graph selector broken member metadata",
        &output,
        1,
    )
    .expect("workspace root package graph selector should fail when another member metadata is unresolved");
    expect_empty_stdout(
        "project-graph-workspace-package-broken-member",
        "workspace root package graph selector broken member metadata",
        &stdout,
    )
    .expect("workspace root package graph selector broken member metadata should not print stdout");
    expect_stderr_contains(
        "project-graph-workspace-package-broken-member",
        "workspace root package graph selector broken member metadata",
        &stderr.replace('\\', "/"),
        "error: `ql project graph` failed to inspect workspace member `packages/broken`: manifest",
    )
    .expect("workspace root package graph selector broken member metadata should surface the broken member error");
    expect_stderr_contains(
        "project-graph-workspace-package-broken-member",
        "workspace root package graph selector broken member metadata",
        &stderr,
        "does not declare `[package].name`",
    )
    .expect("workspace root package graph selector broken member metadata should preserve the missing-name detail");
}
