mod support;

use std::path::{Path, PathBuf};

use support::{
    TempDir, expect_empty_stderr, expect_snapshot_matches, expect_success, ql_command,
    run_command_capture, workspace_root,
};

struct WorkspaceGraphContextFixture {
    _temp: TempDir,
    project_root: PathBuf,
    manifest_path: PathBuf,
    source_path: PathBuf,
    member_dir: PathBuf,
}

fn write_workspace_graph_context_fixture(prefix: &str) -> WorkspaceGraphContextFixture {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("workspace");
    let member_dir = project_root.join("packages").join("app");
    let source_path = member_dir.join("src").join("lib.ql");
    std::fs::create_dir_all(source_path.parent().expect("workspace app source parent"))
        .expect("create workspace app source tree");
    std::fs::create_dir_all(project_root.join("packages").join("tool").join("src"))
        .expect("create workspace tool source tree");
    std::fs::create_dir_all(project_root.join("dep")).expect("create workspace dependency tree");

    let manifest_path = temp.write(
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

[references]
packages = ["../../dep"]
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
        "workspace/packages/app/src/lib.ql",
        r#"
pub fn run() -> Int {
    return 0
}
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

    WorkspaceGraphContextFixture {
        _temp: temp,
        project_root,
        manifest_path,
        source_path,
        member_dir,
    }
}

fn expected_workspace_graph_text(manifest_path: &Path) -> String {
    format!(
        "manifest: {}\npackage: <none>\nworkspace_members:\n  - packages/app\n  - packages/tool\nreferences: []\nworkspace_packages:\n  - member: packages/app\n    manifest: packages/app/qlang.toml\n    package: app\n    interface:\n      path: packages/app/app.qi\n      status: valid\n    references:\n      - ../../dep\n    reference_interfaces:\n      - reference: ../../dep\n        manifest: dep/qlang.toml\n        package: dep\n        path: dep/dep.qi\n        status: valid\n  - member: packages/tool\n    manifest: packages/tool/qlang.toml\n    package: tool\n    interface:\n      path: packages/tool/tool.qi\n      status: missing\n    references: []\n    reference_interfaces: []\n",
        manifest_path.to_string_lossy().replace('\\', "/")
    )
}

fn expected_workspace_graph_json(manifest_path: &Path) -> String {
    format!(
        "{{\n  \"interface\": null,\n  \"manifest_path\": \"{}\",\n  \"package_name\": null,\n  \"reference_interfaces\": [],\n  \"references\": [],\n  \"schema\": \"ql.project.graph.v1\",\n  \"workspace_members\": [\n    \"packages/app\",\n    \"packages/tool\"\n  ],\n  \"workspace_packages\": [\n    {{\n      \"interface\": {{\n        \"detail\": null,\n        \"path\": \"packages/app/app.qi\",\n        \"stale_reasons\": [],\n        \"status\": \"valid\"\n      }},\n      \"manifest_path\": \"packages/app/qlang.toml\",\n      \"member\": \"packages/app\",\n      \"member_error\": null,\n      \"member_status\": null,\n      \"package_name\": \"app\",\n      \"reference_interfaces\": [\n        {{\n          \"detail\": null,\n          \"manifest_path\": \"dep/qlang.toml\",\n          \"package_name\": \"dep\",\n          \"path\": \"dep/dep.qi\",\n          \"reference\": \"../../dep\",\n          \"stale_reasons\": [],\n          \"status\": \"valid\",\n          \"transitive_reference_failures\": {{\n            \"count\": 0,\n            \"first_failure\": null\n          }}\n        }}\n      ],\n      \"references\": [\n        \"../../dep\"\n      ]\n    }},\n    {{\n      \"interface\": {{\n        \"detail\": null,\n        \"path\": \"packages/tool/tool.qi\",\n        \"stale_reasons\": [],\n        \"status\": \"missing\"\n      }},\n      \"manifest_path\": \"packages/tool/qlang.toml\",\n      \"member\": \"packages/tool\",\n      \"member_error\": null,\n      \"member_status\": null,\n      \"package_name\": \"tool\",\n      \"reference_interfaces\": [],\n      \"references\": []\n    }}\n  ]\n}}\n",
        manifest_path.to_string_lossy().replace('\\', "/")
    )
}

#[test]
fn project_graph_expands_workspace_root_members() {
    let workspace_root = workspace_root();
    let fixture = write_workspace_graph_context_fixture("ql-project-graph-workspace-root");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "graph"])
        .arg(&fixture.project_root);
    let output = run_command_capture(&mut command, "`ql project graph` workspace root");
    let (stdout, stderr) = expect_success(
        "project-graph-workspace-root",
        "workspace root project graph rendering",
        &output,
    )
    .expect("workspace root project graph rendering should succeed");
    expect_empty_stderr(
        "project-graph-workspace-root",
        "workspace root project graph rendering",
        &stderr,
    )
    .expect("workspace root project graph rendering should stay silent on stderr");

    let expected = expected_workspace_graph_text(&fixture.manifest_path);
    expect_snapshot_matches(
        "project-graph-workspace-root",
        "workspace root project graph stdout",
        &expected,
        &stdout,
    )
    .expect("workspace root project graph stdout should match resolved member graph");
}

#[test]
fn project_graph_source_file_uses_workspace_root_context() {
    let workspace_root = workspace_root();
    let fixture = write_workspace_graph_context_fixture("ql-project-graph-workspace-source");

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&fixture.source_path);
    let output = run_command_capture(
        &mut command,
        "`ql project graph` workspace member source path",
    );
    let (stdout, stderr) = expect_success(
        "project-graph-workspace-source",
        "workspace member source project graph rendering",
        &output,
    )
    .expect("workspace member source project graph rendering should succeed");
    expect_empty_stderr(
        "project-graph-workspace-source",
        "workspace member source project graph rendering",
        &stderr,
    )
    .expect("workspace member source project graph rendering should stay silent on stderr");

    let expected = expected_workspace_graph_text(&fixture.manifest_path);
    expect_snapshot_matches(
        "project-graph-workspace-source",
        "workspace member source project graph stdout",
        &expected,
        &stdout,
    )
    .expect("workspace member source project graph stdout should match workspace root graph");
}

#[test]
fn project_graph_member_directory_uses_workspace_root_context() {
    let workspace_root = workspace_root();
    let fixture = write_workspace_graph_context_fixture("ql-project-graph-workspace-member-dir");

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&fixture.member_dir);
    let output = run_command_capture(
        &mut command,
        "`ql project graph` workspace member directory",
    );
    let (stdout, stderr) = expect_success(
        "project-graph-workspace-member-dir",
        "workspace member directory project graph rendering",
        &output,
    )
    .expect("workspace member directory project graph rendering should succeed");
    expect_empty_stderr(
        "project-graph-workspace-member-dir",
        "workspace member directory project graph rendering",
        &stderr,
    )
    .expect("workspace member directory project graph rendering should stay silent on stderr");

    let expected = expected_workspace_graph_text(&fixture.manifest_path);
    expect_snapshot_matches(
        "project-graph-workspace-member-dir",
        "workspace member directory project graph stdout",
        &expected,
        &stdout,
    )
    .expect("workspace member directory project graph stdout should match workspace root graph");
}

#[test]
fn project_graph_supports_json_output_for_workspace_root() {
    let workspace_root = workspace_root();
    let fixture = write_workspace_graph_context_fixture("ql-project-graph-workspace-root-json");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "graph"])
        .arg(&fixture.project_root)
        .arg("--json");
    let output = run_command_capture(&mut command, "`ql project graph --json` workspace root");
    let (stdout, stderr) = expect_success(
        "project-graph-workspace-root-json",
        "workspace root project graph json rendering",
        &output,
    )
    .expect("workspace root project graph json rendering should succeed");
    expect_empty_stderr(
        "project-graph-workspace-root-json",
        "workspace root project graph json rendering",
        &stderr,
    )
    .expect("workspace root project graph json rendering should stay silent on stderr");

    let expected = expected_workspace_graph_json(&fixture.manifest_path);
    expect_snapshot_matches(
        "project-graph-workspace-root-json",
        "workspace root project graph json stdout",
        &expected,
        &stdout.replace('\\', "/"),
    )
    .expect("workspace root project graph json stdout should match resolved member graph");
}
