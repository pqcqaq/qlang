mod support;

use support::{
    TempDir, expect_empty_stderr, expect_stdout_contains_all, expect_success, ql_command,
    run_command_capture, workspace_root,
};

struct WorkspaceMemberErrorFixture {
    _temp: TempDir,
    project_root: std::path::PathBuf,
    manifest_path: std::path::PathBuf,
}

fn write_workspace_member_error_fixture(
    prefix: &str,
    broken_manifest: &str,
) -> WorkspaceMemberErrorFixture {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages").join("app").join("src"))
        .expect("create workspace app directory for member error graph test");
    std::fs::create_dir_all(project_root.join("packages").join("broken"))
        .expect("create workspace broken directory for member error graph test");

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
    temp.write("workspace/packages/broken/qlang.toml", broken_manifest);

    WorkspaceMemberErrorFixture {
        _temp: temp,
        project_root,
        manifest_path,
    }
}

#[test]
fn project_graph_keeps_resolved_workspace_members_when_one_member_manifest_is_invalid() {
    let workspace_root = workspace_root();
    let fixture = write_workspace_member_error_fixture(
        "ql-project-graph-workspace-invalid-member",
        r#"
[package
name = "broken"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "graph"])
        .arg(&fixture.project_root);
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

    let normalized_manifest = fixture.manifest_path.to_string_lossy().replace('\\', "/");
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
    let fixture = write_workspace_member_error_fixture(
        "ql-project-graph-workspace-missing-package-name-member",
        r#"
[package]
version = "0.1.0"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "graph"])
        .arg(&fixture.project_root);
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
    let normalized_manifest = fixture.manifest_path.to_string_lossy().replace('\\', "/");
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
