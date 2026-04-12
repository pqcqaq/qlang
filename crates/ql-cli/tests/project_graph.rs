mod support;

use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_snapshot_matches,
    expect_stderr_contains, expect_stdout_contains_all, expect_success, ql_command,
    run_command_capture, workspace_root,
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
        "manifest: {normalized_manifest}\npackage: <none>\nworkspace_members:\n  - packages/app\n  - packages/broken\nreferences: []\nworkspace_packages:\n  - member: packages/app\n    manifest: packages/app/qlang.toml\n    package: app\n    interface:\n      path: packages/app/app.qi\n      status: valid\n    references: []\n    reference_interfaces: []\n  - member: packages/broken\n    manifest: packages/broken/qlang.toml\n    package: <unresolved>\n    member_error: invalid manifest `"
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
            "detail: manifest `",
            "does not declare `[package].name`",
            "reference: ../broken_ref",
            "manifest: ../broken_ref/qlang.toml",
            "status: unresolved-manifest",
            "detail: invalid manifest `",
        ],
    )
    .expect("project graph should explain unresolved reference manifest and package failures");
}
