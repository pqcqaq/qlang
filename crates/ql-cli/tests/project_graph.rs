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
        "manifest: {}\npackage: app\nworkspace_members:\n  - packages/app\n  - packages/core\nreferences:\n  - ../core\n  - ../runtime\n",
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
