mod support;

use std::path::PathBuf;

use serde_json::Value as JsonValue;
use support::{
    TempDir, expect_empty_stderr, expect_snapshot_matches, expect_success, ql_command,
    run_command_capture, workspace_root,
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
