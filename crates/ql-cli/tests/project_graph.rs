mod support;

use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_snapshot_matches,
    expect_stderr_contains, expect_success, ql_command, run_command_capture, workspace_root,
};

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
        "manifest: {}\npackage: app\nworkspace_members:\n  - packages/app\n  - packages/core\nreferences:\n  - ../core\n  - ../runtime\ninterface:\n  path: app.qi\n  status: invalid\nreference_interfaces:\n  - reference: ../core\n    package: core\n    path: ../core/core.qi\n    status: missing\n  - reference: ../runtime\n    package: runtime\n    path: ../runtime/runtime.qi\n    status: valid\n",
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
fn project_graph_rejects_manifest_without_package_or_workspace() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-invalid");
    let project_root = temp.path().join("invalid");
    std::fs::create_dir_all(&project_root).expect("create invalid project directory");
    temp.write(
        "invalid/qlang.toml",
        r#"
[references]
packages = ["../core"]
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql project graph` invalid manifest");
    let (stdout, stderr) = expect_exit_code(
        "project-graph-invalid",
        "invalid project graph rendering",
        &output,
        1,
    )
    .expect("invalid project graph rendering should fail with exit code 1");
    expect_empty_stdout(
        "project-graph-invalid",
        "invalid project graph rendering",
        &stdout,
    )
    .expect("invalid project graph rendering should not print stdout");
    expect_stderr_contains(
        "project-graph-invalid",
        "invalid project graph rendering",
        &stderr,
        "`qlang.toml` requires `[package]` or `[workspace]`",
    )
    .expect("invalid manifest diagnostic should mention the minimum section contract");
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
        "manifest: {}\npackage: app\nworkspace_members: []\nreferences:\n  - ../dep\ninterface:\n  path: app.qi\n  status: stale\n  stale_reasons:\n    - manifest: qlang.toml\nreference_interfaces:\n  - reference: ../dep\n    package: dep\n    path: ../dep/dep.qi\n    status: stale\n    stale_reasons:\n      - source: ../dep/src/lib.ql\n",
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
        "manifest: {}\npackage: <none>\nworkspace_members:\n  - packages/app\n  - packages/tool\nreferences: []\nworkspace_packages:\n  - member: packages/app\n    manifest: packages/app/qlang.toml\n    package: app\n    interface:\n      path: packages/app/app.qi\n      status: valid\n    references:\n      - ../../dep\n    reference_interfaces:\n      - reference: ../../dep\n        package: dep\n        path: dep/dep.qi\n        status: valid\n  - member: packages/tool\n    manifest: packages/tool/qlang.toml\n    package: tool\n    interface:\n      path: packages/tool/tool.qi\n      status: missing\n    references: []\n    reference_interfaces: []\n",
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
