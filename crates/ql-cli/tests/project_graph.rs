mod support;

use std::path::PathBuf;

use serde_json::Value as JsonValue;
use support::{
    TempDir, expect_empty_stderr, expect_snapshot_matches, expect_stdout_contains_all,
    expect_success, ql_command, run_command_capture, workspace_root,
};

fn normalize_output_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&normalize_output_text(stdout))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
}

struct StandaloneGraphFixture {
    manifest_path: PathBuf,
    source_path: PathBuf,
}

fn write_standalone_graph_fixture(prefix: &str) -> (TempDir, StandaloneGraphFixture) {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source directory for standalone graph fixture");
    std::fs::create_dir_all(temp.path().join("workspace").join("dep").join("src"))
        .expect("create dependency source directory for standalone graph fixture");

    let manifest_path = temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
pub fn run() -> Int {
    return 0
}
"#,
    );
    temp.write(
        "workspace/app/app.qi",
        r#"
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn run() -> Int
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
        "workspace/dep/src/lib.ql",
        r#"
pub fn exported() -> Int {
    return 1
}
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

    (
        temp,
        StandaloneGraphFixture {
            manifest_path,
            source_path,
        },
    )
}

#[test]
fn project_graph_prints_package_workspace_and_references() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create nested project directory for project graph test");
    std::fs::create_dir_all(temp.path().join("workspace").join("core"))
        .expect("create core directory for project graph test");
    std::fs::create_dir_all(temp.path().join("workspace").join("runtime"))
        .expect("create runtime directory for project graph test");
    let manifest_path = temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[workspace]
members = ["packages/app", "packages/core"]

[references]
packages = ["../core", "../runtime"]
"#,
    );
    temp.write(
        "workspace/core/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );
    temp.write(
        "workspace/runtime/qlang.toml",
        r#"
[package]
name = "runtime"
"#,
    );
    temp.write("workspace/app/app.qi", "broken interface\n");
    temp.write(
        "workspace/runtime/runtime.qi",
        r#"
// qlang interface v1
// package: runtime

// source: src/lib.ql
package demo.runtime

pub fn run() -> Int
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "graph"])
        .arg(project_root.join("src"));
    let output = run_command_capture(&mut command, "`ql project graph`");
    let (stdout, stderr) =
        expect_success("project-graph-success", "project graph rendering", &output)
            .expect("project graph rendering should succeed");
    expect_empty_stderr("project-graph-success", "project graph rendering", &stderr)
        .expect("successful project graph rendering should stay silent on stderr");

    let expected = format!(
        "manifest: {}\npackage: app\nworkspace_members:\n  - packages/app\n  - packages/core\nreferences:\n  - ../core\n  - ../runtime\ninterface:\n  path: app.qi\n  status: invalid\n  detail: expected `// qlang interface v1` header\nreference_interfaces:\n  - reference: ../core\n    manifest: ../core/qlang.toml\n    package: core\n    path: ../core/core.qi\n    status: missing\n  - reference: ../runtime\n    manifest: ../runtime/qlang.toml\n    package: runtime\n    path: ../runtime/runtime.qi\n    status: valid\n",
        manifest_path.to_string_lossy().replace('\\', "/")
    );
    expect_snapshot_matches(
        "project-graph-success",
        "project graph stdout",
        &expected,
        &stdout,
    )
    .expect("project graph stdout should match the resolved manifest graph");
}

#[test]
fn project_graph_supports_json_output_for_package_graph() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-json");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create nested project directory for project graph json test");
    std::fs::create_dir_all(temp.path().join("workspace").join("core"))
        .expect("create core directory for project graph json test");
    std::fs::create_dir_all(temp.path().join("workspace").join("runtime"))
        .expect("create runtime directory for project graph json test");
    let manifest_path = temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[workspace]
members = ["packages/app", "packages/core"]

[references]
packages = ["../core", "../runtime"]
"#,
    );
    temp.write(
        "workspace/core/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );
    temp.write(
        "workspace/runtime/qlang.toml",
        r#"
[package]
name = "runtime"
"#,
    );
    temp.write("workspace/app/app.qi", "broken interface\n");
    temp.write(
        "workspace/runtime/runtime.qi",
        r#"
// qlang interface v1
// package: runtime

// source: src/lib.ql
package demo.runtime

pub fn run() -> Int
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "graph", "--json"])
        .arg(project_root.join("src"));
    let output = run_command_capture(&mut command, "`ql project graph --json`");
    let (stdout, stderr) = expect_success(
        "project-graph-json-success",
        "project graph json rendering",
        &output,
    )
    .expect("project graph json rendering should succeed");
    expect_empty_stderr(
        "project-graph-json-success",
        "project graph json rendering",
        &stderr,
    )
    .expect("successful project graph json rendering should stay silent on stderr");

    let expected = format!(
        "{{\n  \"interface\": {{\n    \"detail\": \"expected `// qlang interface v1` header\",\n    \"path\": \"app.qi\",\n    \"stale_reasons\": [],\n    \"status\": \"invalid\"\n  }},\n  \"manifest_path\": \"{}\",\n  \"package_name\": \"app\",\n  \"reference_interfaces\": [\n    {{\n      \"detail\": null,\n      \"manifest_path\": \"../core/qlang.toml\",\n      \"package_name\": \"core\",\n      \"path\": \"../core/core.qi\",\n      \"reference\": \"../core\",\n      \"stale_reasons\": [],\n      \"status\": \"missing\",\n      \"transitive_reference_failures\": {{\n        \"count\": 0,\n        \"first_failure\": null\n      }}\n    }},\n    {{\n      \"detail\": null,\n      \"manifest_path\": \"../runtime/qlang.toml\",\n      \"package_name\": \"runtime\",\n      \"path\": \"../runtime/runtime.qi\",\n      \"reference\": \"../runtime\",\n      \"stale_reasons\": [],\n      \"status\": \"valid\",\n      \"transitive_reference_failures\": {{\n        \"count\": 0,\n        \"first_failure\": null\n      }}\n    }}\n  ],\n  \"references\": [\n    \"../core\",\n    \"../runtime\"\n  ],\n  \"schema\": \"ql.project.graph.v1\",\n  \"workspace_members\": [\n    \"packages/app\",\n    \"packages/core\"\n  ],\n  \"workspace_packages\": []\n}}\n",
        manifest_path.to_string_lossy().replace('\\', "/")
    );
    expect_snapshot_matches(
        "project-graph-json-success",
        "project graph json stdout",
        &expected,
        &stdout.replace('\\', "/"),
    )
    .expect("project graph json stdout should match the resolved manifest graph");
}

#[test]
fn project_graph_supports_json_output_for_standalone_package_source_path() {
    let workspace_root = workspace_root();
    let (_temp, fixture) = write_standalone_graph_fixture("ql-project-graph-package-source-json");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "graph", "--json"])
        .arg(&fixture.source_path);
    let output = run_command_capture(
        &mut command,
        "`ql project graph --json` standalone package source path",
    );
    let (stdout, stderr) = expect_success(
        "project-graph-package-source-json",
        "standalone package source graph json rendering",
        &output,
    )
    .expect("standalone package source graph json rendering should succeed");
    expect_empty_stderr(
        "project-graph-package-source-json",
        "standalone package source graph json rendering",
        &stderr,
    )
    .expect("standalone package source graph json rendering should stay silent on stderr");

    let actual = parse_json_output("project-graph-package-source-json", &stdout);
    assert_eq!(actual["schema"], "ql.project.graph.v1");
    assert_eq!(
        actual["manifest_path"],
        fixture.manifest_path.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(actual["package_name"], "app");
    assert_eq!(actual["workspace_members"], serde_json::json!([]));
    assert_eq!(actual["workspace_packages"], serde_json::json!([]));
    assert_eq!(actual["references"], serde_json::json!(["../dep"]));
    assert_eq!(actual["interface"]["path"], "app.qi");
    assert_eq!(actual["interface"]["status"], "valid");

    let reference = actual["reference_interfaces"]
        .as_array()
        .expect("reference interfaces should be an array")
        .first()
        .expect("standalone package graph should expose dependency interface");
    assert_eq!(reference["reference"], "../dep");
    assert_eq!(reference["package_name"], "dep");
    assert_eq!(reference["path"], "../dep/dep.qi");
    assert_eq!(reference["status"], "valid");
    assert_eq!(reference["manifest_path"], "../dep/qlang.toml");
}

#[test]
fn project_graph_reports_stale_package_and_reference_interfaces() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-stale-interface");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source directory for stale interface test");
    std::fs::create_dir_all(temp.path().join("workspace").join("dep").join("src"))
        .expect("create dep source directory for stale interface test");
    let manifest_path = temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
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
        "workspace/app/src/lib.ql",
        r#"
pub fn run() -> Int {
    1
}
"#,
    );
    temp.write(
        "workspace/dep/src/lib.ql",
        r#"
pub fn exported() -> Int {
    1
}
"#,
    );
    temp.write(
        "workspace/app/app.qi",
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

    std::thread::sleep(std::time::Duration::from_millis(1200));

    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write(
        "workspace/dep/src/lib.ql",
        r#"
pub fn exported() -> Int {
    2
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql project graph` stale interfaces");
    let (stdout, stderr) = expect_success(
        "project-graph-stale-interface",
        "project graph rendering with stale interfaces",
        &output,
    )
    .expect("project graph rendering with stale interfaces should succeed");
    expect_empty_stderr(
        "project-graph-stale-interface",
        "project graph rendering with stale interfaces",
        &stderr,
    )
    .expect("stale interface graph rendering should stay silent on stderr");

    let expected = format!(
        "manifest: {}\npackage: app\nworkspace_members: []\nreferences:\n  - ../dep\ninterface:\n  path: app.qi\n  status: stale\n  stale_reasons:\n    - manifest: qlang.toml\nreference_interfaces:\n  - reference: ../dep\n    manifest: ../dep/qlang.toml\n    package: dep\n    path: ../dep/dep.qi\n    status: stale\n    stale_reasons:\n      - source: ../dep/src/lib.ql\n",
        manifest_path.to_string_lossy().replace('\\', "/")
    );
    expect_snapshot_matches(
        "project-graph-stale-interface",
        "stale interface project graph stdout",
        &expected,
        &stdout,
    )
    .expect("project graph should report stale package and reference interfaces");
}

#[test]
fn project_graph_expands_workspace_root_members() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-workspace-root");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages").join("app").join("src"))
        .expect("create workspace app directory");
    std::fs::create_dir_all(project_root.join("packages").join("tool").join("src"))
        .expect("create workspace tool directory");
    std::fs::create_dir_all(project_root.join("dep")).expect("create dependency directory");

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

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&project_root);
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

    let expected = format!(
        "manifest: {}\npackage: <none>\nworkspace_members:\n  - packages/app\n  - packages/tool\nreferences: []\nworkspace_packages:\n  - member: packages/app\n    manifest: packages/app/qlang.toml\n    package: app\n    interface:\n      path: packages/app/app.qi\n      status: valid\n    references:\n      - ../../dep\n    reference_interfaces:\n      - reference: ../../dep\n        manifest: dep/qlang.toml\n        package: dep\n        path: dep/dep.qi\n        status: valid\n  - member: packages/tool\n    manifest: packages/tool/qlang.toml\n    package: tool\n    interface:\n      path: packages/tool/tool.qi\n      status: missing\n    references: []\n    reference_interfaces: []\n",
        manifest_path.to_string_lossy().replace('\\', "/")
    );
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
    let temp = TempDir::new("ql-project-graph-workspace-source");
    let source_path = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("app")
        .join("src")
        .join("lib.ql");
    std::fs::create_dir_all(
        source_path
            .parent()
            .expect("workspace app source parent should exist"),
    )
    .expect("create workspace app directory");
    std::fs::create_dir_all(
        temp.path()
            .join("workspace")
            .join("packages")
            .join("tool")
            .join("src"),
    )
    .expect("create workspace tool directory");
    std::fs::create_dir_all(temp.path().join("workspace").join("dep"))
        .expect("create dependency directory");

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

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&source_path);
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

    let expected = format!(
        "manifest: {}\npackage: <none>\nworkspace_members:\n  - packages/app\n  - packages/tool\nreferences: []\nworkspace_packages:\n  - member: packages/app\n    manifest: packages/app/qlang.toml\n    package: app\n    interface:\n      path: packages/app/app.qi\n      status: valid\n    references:\n      - ../../dep\n    reference_interfaces:\n      - reference: ../../dep\n        manifest: dep/qlang.toml\n        package: dep\n        path: dep/dep.qi\n        status: valid\n  - member: packages/tool\n    manifest: packages/tool/qlang.toml\n    package: tool\n    interface:\n      path: packages/tool/tool.qi\n      status: missing\n    references: []\n    reference_interfaces: []\n",
        manifest_path.to_string_lossy().replace('\\', "/")
    );
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
    let temp = TempDir::new("ql-project-graph-workspace-member-dir");
    let member_dir = temp.path().join("workspace").join("packages").join("app");
    std::fs::create_dir_all(member_dir.join("src")).expect("create workspace app directory");
    std::fs::create_dir_all(
        temp.path()
            .join("workspace")
            .join("packages")
            .join("tool")
            .join("src"),
    )
    .expect("create workspace tool directory");
    std::fs::create_dir_all(temp.path().join("workspace").join("dep"))
        .expect("create dependency directory");

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

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&member_dir);
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

    let expected = format!(
        "manifest: {}\npackage: <none>\nworkspace_members:\n  - packages/app\n  - packages/tool\nreferences: []\nworkspace_packages:\n  - member: packages/app\n    manifest: packages/app/qlang.toml\n    package: app\n    interface:\n      path: packages/app/app.qi\n      status: valid\n    references:\n      - ../../dep\n    reference_interfaces:\n      - reference: ../../dep\n        manifest: dep/qlang.toml\n        package: dep\n        path: dep/dep.qi\n        status: valid\n  - member: packages/tool\n    manifest: packages/tool/qlang.toml\n    package: tool\n    interface:\n      path: packages/tool/tool.qi\n      status: missing\n    references: []\n    reference_interfaces: []\n",
        manifest_path.to_string_lossy().replace('\\', "/")
    );
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
    let temp = TempDir::new("ql-project-graph-workspace-root-json");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages").join("app").join("src"))
        .expect("create workspace app source tree for workspace root json test");
    std::fs::create_dir_all(project_root.join("packages").join("tool").join("src"))
        .expect("create workspace tool source tree for workspace root json test");
    std::fs::create_dir_all(project_root.join("dep"))
        .expect("create workspace dependency tree for workspace root json test");

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

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "graph"])
        .arg(&project_root)
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

    let expected = format!(
        "{{\n  \"interface\": null,\n  \"manifest_path\": \"{}\",\n  \"package_name\": null,\n  \"reference_interfaces\": [],\n  \"references\": [],\n  \"schema\": \"ql.project.graph.v1\",\n  \"workspace_members\": [\n    \"packages/app\",\n    \"packages/tool\"\n  ],\n  \"workspace_packages\": [\n    {{\n      \"interface\": {{\n        \"detail\": null,\n        \"path\": \"packages/app/app.qi\",\n        \"stale_reasons\": [],\n        \"status\": \"valid\"\n      }},\n      \"manifest_path\": \"packages/app/qlang.toml\",\n      \"member\": \"packages/app\",\n      \"member_error\": null,\n      \"member_status\": null,\n      \"package_name\": \"app\",\n      \"reference_interfaces\": [\n        {{\n          \"detail\": null,\n          \"manifest_path\": \"dep/qlang.toml\",\n          \"package_name\": \"dep\",\n          \"path\": \"dep/dep.qi\",\n          \"reference\": \"../../dep\",\n          \"stale_reasons\": [],\n          \"status\": \"valid\",\n          \"transitive_reference_failures\": {{\n            \"count\": 0,\n            \"first_failure\": null\n          }}\n        }}\n      ],\n      \"references\": [\n        \"../../dep\"\n      ]\n    }},\n    {{\n      \"interface\": {{\n        \"detail\": null,\n        \"path\": \"packages/tool/tool.qi\",\n        \"stale_reasons\": [],\n        \"status\": \"missing\"\n      }},\n      \"manifest_path\": \"packages/tool/qlang.toml\",\n      \"member\": \"packages/tool\",\n      \"member_error\": null,\n      \"member_status\": null,\n      \"package_name\": \"tool\",\n      \"reference_interfaces\": [],\n      \"references\": []\n    }}\n  ]\n}}\n",
        manifest_path.to_string_lossy().replace('\\', "/")
    );
    expect_snapshot_matches(
        "project-graph-workspace-root-json",
        "workspace root project graph json stdout",
        &expected,
        &stdout.replace('\\', "/"),
    )
    .expect("workspace root project graph json stdout should match resolved member graph");
}

#[test]
fn project_graph_keeps_resolved_workspace_members_when_one_member_manifest_is_invalid() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-workspace-invalid-member");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages").join("app").join("src"))
        .expect("create workspace app directory for invalid member graph test");
    std::fs::create_dir_all(project_root.join("packages").join("broken"))
        .expect("create workspace broken directory for invalid member graph test");

    let manifest_path = temp.write(
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
        "workspace/packages/broken/qlang.toml",
        r#"
[package
name = "broken"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project graph` workspace root with invalid member manifest",
    );
    let (stdout, stderr) = expect_success(
        "project-graph-workspace-invalid-member",
        "workspace root project graph rendering with invalid member manifest",
        &output,
    )
    .expect("workspace root graph should still render when one member manifest is invalid");
    expect_empty_stderr(
        "project-graph-workspace-invalid-member",
        "workspace root project graph rendering with invalid member manifest",
        &stderr,
    )
    .expect("workspace root graph with invalid member manifest should stay silent on stderr");

    let normalized_manifest = manifest_path.to_string_lossy().replace('\\', "/");
    let expected_prefix = format!(
        "manifest: {normalized_manifest}\npackage: <none>\nworkspace_members:\n  - packages/app\n  - packages/broken\nreferences: []\nworkspace_packages:\n  - member: packages/app\n    manifest: packages/app/qlang.toml\n    package: app\n    interface:\n      path: packages/app/app.qi\n      status: valid\n    references: []\n    reference_interfaces: []\n  - member: packages/broken\n    manifest: packages/broken/qlang.toml\n    package: <unresolved>\n    member_status: unresolved-manifest\n    member_error: invalid manifest `"
    );
    assert!(
        stdout.replace('\\', "/").starts_with(&expected_prefix),
        "expected workspace graph to keep resolved members and surface invalid member error, got:\n{stdout}"
    );
    assert!(
        stdout.contains("packages/broken/qlang.toml")
            || stdout.contains("packages\\broken\\qlang.toml"),
        "expected workspace graph to mention the broken member manifest path, got:\n{stdout}"
    );
}

#[test]
fn project_graph_keeps_resolved_workspace_members_when_one_member_manifest_is_missing_package_name()
{
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-workspace-missing-package-name-member");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages").join("app").join("src"))
        .expect("create workspace app directory for missing package name member graph test");
    std::fs::create_dir_all(project_root.join("packages").join("broken"))
        .expect("create workspace broken directory for missing package name member graph test");

    let manifest_path = temp.write(
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
        "workspace/packages/broken/qlang.toml",
        r#"
[package]
version = "0.1.0"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project graph` workspace root with missing package-name member",
    );
    let (stdout, stderr) = expect_success(
        "project-graph-workspace-missing-package-name-member",
        "workspace root project graph rendering with missing package name member",
        &output,
    )
    .expect(
        "workspace root graph should still render when one member manifest is missing `[package].name`",
    );
    expect_empty_stderr(
        "project-graph-workspace-missing-package-name-member",
        "workspace root project graph rendering with missing package name member",
        &stderr,
    )
    .expect("workspace root graph with missing package name member should stay silent on stderr");

    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_manifest = manifest_path.to_string_lossy().replace('\\', "/");
    expect_stdout_contains_all(
        "project-graph-workspace-missing-package-name-member",
        &normalized_stdout,
        &[
            &format!("manifest: {normalized_manifest}"),
            "  - member: packages/app",
            "    package: app",
            "  - member: packages/broken",
            "    package: <unresolved>",
            "    member_status: unresolved-package",
            "    member_error: manifest `packages/broken/qlang.toml` does not declare `[package].name`",
        ],
    )
    .expect(
        "workspace graph should keep resolved members and normalize missing package-name member errors",
    );
    assert!(
        !normalized_stdout.contains("`[package].name` must be present"),
        "expected workspace graph to normalize missing package-name member errors, got:\n{stdout}"
    );
}

#[test]
fn project_graph_explains_unresolved_reference_manifests_and_packages() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-unresolved-reference-detail");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source directory for unresolved reference graph test");
    std::fs::create_dir_all(temp.path().join("workspace").join("dep"))
        .expect("create valid dependency directory for unresolved reference graph test");
    std::fs::create_dir_all(temp.path().join("workspace").join("workspace_ref"))
        .expect("create workspace-only reference directory for unresolved reference graph test");
    std::fs::create_dir_all(temp.path().join("workspace").join("broken_ref"))
        .expect("create broken reference directory for unresolved reference graph test");

    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep", "../workspace_ref", "../broken_ref"]
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
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported() -> Int
"#,
    );
    temp.write(
        "workspace/workspace_ref/qlang.toml",
        r#"
[workspace]
members = ["packages/demo"]
"#,
    );
    temp.write(
        "workspace/broken_ref/qlang.toml",
        r#"
[package
name = "broken_ref"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project graph` unresolved reference details",
    );
    let (stdout, stderr) = expect_success(
        "project-graph-unresolved-reference-detail",
        "project graph rendering with unresolved reference details",
        &output,
    )
    .expect("project graph with unresolved reference details should succeed");
    expect_empty_stderr(
        "project-graph-unresolved-reference-detail",
        "project graph rendering with unresolved reference details",
        &stderr,
    )
    .expect("project graph unresolved reference detail rendering should stay silent on stderr");
    let normalized_stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        "project-graph-unresolved-reference-detail",
        &normalized_stdout,
        &[
            "reference: ../dep",
            "manifest: ../dep/qlang.toml",
            "status: valid",
            "reference: ../workspace_ref",
            "manifest: ../workspace_ref/qlang.toml",
            "status: unresolved-package",
            "detail: manifest `../workspace_ref/qlang.toml` does not declare `[package].name`",
            "reference: ../broken_ref",
            "manifest: ../broken_ref/qlang.toml",
            "status: unresolved-manifest",
            "detail: invalid manifest `../broken_ref/qlang.toml`:",
        ],
    )
    .expect("project graph should explain unresolved reference manifest and package failures");
}

#[test]
fn project_graph_reports_transitive_reference_failures_for_direct_dependencies() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-transitive-reference-failures");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source directory for transitive reference graph test");
    std::fs::create_dir_all(temp.path().join("workspace").join("dep").join("src"))
        .expect("create dependency source directory for transitive reference graph test");
    std::fs::create_dir_all(temp.path().join("workspace").join("broken_ref"))
        .expect("create broken reference directory for transitive reference graph test");

    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"

[references]
packages = ["../broken_ref"]
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
    temp.write(
        "workspace/broken_ref/qlang.toml",
        r#"
[package
name = "broken_ref"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project graph` transitive reference failures",
    );
    let (stdout, stderr) = expect_success(
        "project-graph-transitive-reference-failures",
        "project graph rendering with transitive reference failures",
        &output,
    )
    .expect("project graph with transitive reference failures should succeed");
    expect_empty_stderr(
        "project-graph-transitive-reference-failures",
        "project graph rendering with transitive reference failures",
        &stderr,
    )
    .expect("project graph with transitive reference failures should stay silent on stderr");
    let normalized_stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        "project-graph-transitive-reference-failures",
        &normalized_stdout,
        &[
            "reference: ../dep",
            "manifest: ../dep/qlang.toml",
            "status: valid",
            "transitive_reference_failures: 1",
            "first_transitive_failure_manifest: ../broken_ref/qlang.toml",
            "first_transitive_failure_status: unresolved-manifest",
            "first_transitive_failure_detail: invalid manifest `../broken_ref/qlang.toml`:",
        ],
    )
    .expect("project graph should summarize transitive reference failures on direct dependencies");
}

#[test]
fn project_graph_reports_transitive_stale_reference_reasons_for_direct_dependencies() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-transitive-stale-reference-failures");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source directory for transitive stale graph test");
    std::fs::create_dir_all(temp.path().join("workspace").join("dep").join("src"))
        .expect("create dependency source directory for transitive stale graph test");
    std::fs::create_dir_all(temp.path().join("workspace").join("leaf").join("src"))
        .expect("create leaf source directory for transitive stale graph test");

    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"

[references]
packages = ["../leaf"]
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
    temp.write(
        "workspace/leaf/qlang.toml",
        r#"
[package]
name = "leaf"
"#,
    );
    temp.write(
        "workspace/leaf/src/lib.ql",
        r#"
pub fn exported() -> Int {
    1
}
"#,
    );
    temp.write(
        "workspace/leaf/leaf.qi",
        r#"
// qlang interface v1
// package: leaf

// source: src/lib.ql
package demo.leaf

pub fn exported() -> Int
"#,
    );

    std::thread::sleep(std::time::Duration::from_millis(1200));

    temp.write(
        "workspace/leaf/src/lib.ql",
        r#"
pub fn exported() -> Int {
    2
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project graph` transitive stale reference failures",
    );
    let (stdout, stderr) = expect_success(
        "project-graph-transitive-stale-reference-failures",
        "project graph rendering with transitive stale reference failures",
        &output,
    )
    .expect("project graph with transitive stale reference failures should succeed");
    expect_empty_stderr(
        "project-graph-transitive-stale-reference-failures",
        "project graph rendering with transitive stale reference failures",
        &stderr,
    )
    .expect("project graph with transitive stale reference failures should stay silent on stderr");
    let normalized_stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        "project-graph-transitive-stale-reference-failures",
        &normalized_stdout,
        &[
            "reference: ../dep",
            "manifest: ../dep/qlang.toml",
            "status: valid",
            "transitive_reference_failures: 1",
            "first_transitive_failure_manifest: ../leaf/qlang.toml",
            "first_transitive_failure_path: ../leaf/leaf.qi",
            "first_transitive_failure_status: stale",
            "first_transitive_failure_stale_reasons:",
            "- source: ../leaf/src/lib.ql",
        ],
    )
    .expect("project graph should summarize the first transitive stale reference reason");
}
