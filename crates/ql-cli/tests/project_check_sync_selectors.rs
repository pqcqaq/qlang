mod support;

use std::path::PathBuf;

use support::{
    TempDir, expect_stdout_contains_all, expect_success, ql_command, run_command_capture,
    workspace_root,
};

struct WorkspaceCheckSyncPackageSelectorProject {
    temp: TempDir,
    workspace_root: PathBuf,
    app_root: PathBuf,
    app_source: PathBuf,
    tool_source: PathBuf,
    app_interface: PathBuf,
    tool_interface: PathBuf,
}

fn write_workspace_check_sync_package_selector_project(
    prefix: &str,
) -> WorkspaceCheckSyncPackageSelectorProject {
    let temp = TempDir::new(prefix);
    let dep_app_root = temp.path().join("workspace").join("dep_app");
    let dep_tool_root = temp.path().join("workspace").join("dep_tool");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let workspace_root = temp.path().join("workspace");
    let app_source = app_root.join("src").join("lib.ql");
    let tool_source = tool_root.join("src").join("lib.ql");
    let app_interface = dep_app_root.join("dep_app.qi");
    let tool_interface = dep_tool_root.join("dep_tool.qi");
    std::fs::create_dir_all(dep_app_root.join("src")).expect("create app dependency source");
    std::fs::create_dir_all(dep_tool_root.join("src")).expect("create tool dependency source");
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
        "workspace/dep_app/qlang.toml",
        r#"
[package]
name = "dep_app"
"#,
    );
    temp.write(
        "workspace/dep_app/src/lib.ql",
        r#"
package demo.dep_app

pub fn exported() -> Int {
    return 7
}
"#,
    );
    temp.write(
        "workspace/dep_tool/qlang.toml",
        r#"
[package]
name = "dep_tool"
"#,
    );
    temp.write(
        "workspace/dep_tool/src/lib.ql",
        r#"
package demo.dep_tool

pub fn exported() -> Int {
    return 9
}
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../../dep_app"]
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
packages = ["../../dep_tool"]
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

    WorkspaceCheckSyncPackageSelectorProject {
        temp,
        workspace_root,
        app_root,
        app_source,
        tool_source,
        app_interface,
        tool_interface,
    }
}

fn expect_workspace_check_sync_package_selector_output(
    snapshot_name: &str,
    label: &str,
    fixture: &WorkspaceCheckSyncPackageSelectorProject,
    stdout: &str,
    stderr: &str,
) {
    let normalized_stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        snapshot_name,
        &normalized_stdout,
        &[
            "wrote interface: ",
            "dep_app.qi",
            &format!(
                "ok: {}",
                fixture.app_source.display().to_string().replace('\\', "/")
            ),
            "loaded interface: ",
        ],
    )
    .expect("selected workspace package sync should report emitted and loaded interfaces");
    assert_eq!(
        normalized_stdout.matches("wrote interface: ").count(),
        1,
        "expected {label} to emit one selected dependency interface, got:\n{stdout}"
    );
    assert!(
        !normalized_stdout.contains(&fixture.tool_source.display().to_string().replace('\\', "/")),
        "expected {label} to skip the unselected tool member, got:\n{stdout}"
    );
    assert!(
        !normalized_stdout.contains("dep_tool.qi"),
        "expected {label} to skip the unselected tool dependency, got:\n{stdout}"
    );
    assert!(
        fixture.app_interface.is_file(),
        "expected selected dependency interface at `{}`",
        fixture.app_interface.display()
    );
    assert!(
        !fixture.tool_interface.exists(),
        "expected unselected dependency interface to stay absent at `{}`",
        fixture.tool_interface.display()
    );
    assert!(
        stderr.trim().is_empty(),
        "expected {label} stderr to stay empty, got:\n{stderr}"
    );
}

#[test]
fn check_workspace_root_sync_package_selector_only_syncs_selected_member() {
    let workspace_root = workspace_root();
    let fixture = write_workspace_check_sync_package_selector_project(
        "ql-project-check-workspace-sync-package-selector",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["check", "--sync-interfaces"])
        .arg(&fixture.workspace_root)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` workspace root package selector",
    );
    let (stdout, stderr) = expect_success(
        "project-check-workspace-sync-package-selector",
        "workspace-root ql check sync package selector",
        &output,
    )
    .expect("workspace-root ql check sync package selector should succeed");
    expect_workspace_check_sync_package_selector_output(
        "project-check-workspace-sync-package-selector",
        "workspace-root ql check sync package selector",
        &fixture,
        &stdout,
        &stderr,
    );
}

#[test]
fn check_workspace_member_source_sync_package_selector_only_syncs_selected_member() {
    let workspace_root = workspace_root();
    let fixture = write_workspace_check_sync_package_selector_project(
        "ql-project-check-workspace-member-source-sync-package-selector",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["check", "--sync-interfaces"])
        .arg(&fixture.app_source)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` workspace member source package selector",
    );
    let (stdout, stderr) = expect_success(
        "project-check-workspace-member-source-sync-package-selector",
        "workspace member source ql check sync package selector",
        &output,
    )
    .expect("workspace member source ql check sync package selector should succeed");
    expect_workspace_check_sync_package_selector_output(
        "project-check-workspace-member-source-sync-package-selector",
        "workspace member source ql check sync package selector",
        &fixture,
        &stdout,
        &stderr,
    );
}

#[test]
fn check_workspace_member_directory_sync_package_selector_only_syncs_selected_member() {
    let workspace_root = workspace_root();
    let fixture = write_workspace_check_sync_package_selector_project(
        "ql-project-check-workspace-member-directory-sync-package-selector",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["check", "--sync-interfaces"])
        .arg(&fixture.app_root)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` workspace member directory package selector",
    );
    let (stdout, stderr) = expect_success(
        "project-check-workspace-member-directory-sync-package-selector",
        "workspace member directory ql check sync package selector",
        &output,
    )
    .expect("workspace member directory ql check sync package selector should succeed");
    expect_workspace_check_sync_package_selector_output(
        "project-check-workspace-member-directory-sync-package-selector",
        "workspace member directory ql check sync package selector",
        &fixture,
        &stdout,
        &stderr,
    );
}
