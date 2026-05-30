mod support;

use std::path::PathBuf;

use support::{
    TempDir, expect_stdout_contains_all, expect_success, ql_command, run_command_capture,
    workspace_root,
};

struct WorkspaceCheckProject {
    _temp: TempDir,
    workspace_dir: PathBuf,
    app_root: PathBuf,
    app_source: PathBuf,
    tool_source: PathBuf,
}

fn write_workspace_check_project(prefix: &str) -> WorkspaceCheckProject {
    let temp = TempDir::new(prefix);
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let workspace_dir = temp.path().join("workspace");
    let app_source = app_root.join("src").join("lib.ql");
    let tool_source = tool_root.join("src").join("lib.ql");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source directory");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(tool_root.join("src")).expect("create tool source directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
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
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../../dep"]
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
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
        r#"
package demo.tool

pub fn main() -> Int {
    return 2
}
"#,
    );

    WorkspaceCheckProject {
        _temp: temp,
        workspace_dir,
        app_root,
        app_source,
        tool_source,
    }
}

fn expect_workspace_check_output(
    snapshot_name: &str,
    label: &str,
    fixture: &WorkspaceCheckProject,
    stdout: &str,
    stderr: &str,
) {
    let normalized_stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        snapshot_name,
        &normalized_stdout,
        &[
            &format!(
                "ok: {}",
                fixture.app_source.display().to_string().replace('\\', "/")
            ),
            &format!(
                "ok: {}",
                fixture.tool_source.display().to_string().replace('\\', "/")
            ),
            "loaded interface: ",
            "dep.qi",
        ],
    )
    .expect("workspace ql check should report member sources and dependency interfaces");
    assert!(
        stderr.trim().is_empty(),
        "expected {label} stderr to stay empty, got:\n{stderr}"
    );
}

#[test]
fn check_workspace_root_runs_member_packages() {
    let workspace_root = workspace_root();
    let fixture = write_workspace_check_project("ql-project-check-workspace-root");

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&fixture.workspace_dir);
    let output = run_command_capture(&mut command, "`ql check` workspace root");
    let (stdout, stderr) = expect_success(
        "project-check-workspace-root",
        "workspace-root ql check",
        &output,
    )
    .expect("workspace-root ql check should succeed");
    expect_workspace_check_output(
        "project-check-workspace-root",
        "workspace-root ql check",
        &fixture,
        &stdout,
        &stderr,
    );
}

#[test]
fn check_workspace_member_source_file_uses_enclosing_workspace() {
    let workspace_root = workspace_root();
    let fixture = write_workspace_check_project("ql-project-check-workspace-member-source");

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&fixture.app_source);
    let output = run_command_capture(&mut command, "`ql check` workspace member source file");
    let (stdout, stderr) = expect_success(
        "project-check-workspace-member-source",
        "workspace member source ql check",
        &output,
    )
    .expect("workspace member source ql check should succeed");
    expect_workspace_check_output(
        "project-check-workspace-member-source",
        "workspace member source ql check",
        &fixture,
        &stdout,
        &stderr,
    );
}

#[test]
fn check_workspace_member_directory_uses_enclosing_workspace() {
    let workspace_root = workspace_root();
    let fixture = write_workspace_check_project("ql-project-check-workspace-member-dir");

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&fixture.app_root);
    let output = run_command_capture(&mut command, "`ql check` workspace member directory");
    let (stdout, stderr) = expect_success(
        "project-check-workspace-member-dir",
        "workspace member directory ql check",
        &output,
    )
    .expect("workspace member directory ql check should succeed");
    expect_workspace_check_output(
        "project-check-workspace-member-dir",
        "workspace member directory ql check",
        &fixture,
        &stdout,
        &stderr,
    );
}

#[test]
fn check_workspace_root_syncs_dependency_interfaces_once() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-sync");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let app_source = app_root.join("src").join("lib.ql");
    let tool_source = tool_root.join("src").join("lib.ql");
    let interface_path = dep_root.join("dep.qi");
    let workspace_manifest = temp.path().join("workspace");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source directory");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(tool_root.join("src")).expect("create tool source directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
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
package demo.dep

pub fn exported() -> Int {
    return 7
}
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
        "workspace/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"

[references]
packages = ["../../dep"]
"#,
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn main() -> Int {
    return 2
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["check", "--sync-interfaces"])
        .arg(&workspace_manifest);
    let output = run_command_capture(&mut command, "`ql check --sync-interfaces` workspace root");
    let (stdout, stderr) = expect_success(
        "project-check-workspace-sync",
        "workspace-root ql check with synced dependency interfaces",
        &output,
    )
    .expect("workspace-root ql check with synced dependency interfaces should succeed");
    let normalized_stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-sync",
        &normalized_stdout,
        &[
            "wrote interface: ",
            "dep.qi",
            &format!(
                "ok: {}",
                app_source.display().to_string().replace('\\', "/")
            ),
            &format!(
                "ok: {}",
                tool_source.display().to_string().replace('\\', "/")
            ),
            "loaded interface: ",
        ],
    )
    .expect("workspace-root sync path should report emitted and loaded interfaces");
    assert_eq!(
        normalized_stdout.matches("wrote interface: ").count(),
        1,
        "expected workspace-root sync path to emit one dependency interface, got:\n{stdout}"
    );
    assert!(
        interface_path.is_file(),
        "expected synced dependency interface at `{}`",
        interface_path.display()
    );
    assert!(
        stderr.trim().is_empty(),
        "expected workspace-root ql check stderr to stay empty, got:\n{stderr}"
    );
}
