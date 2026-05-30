mod support;

use std::path::PathBuf;

use serde_json::Value as JsonValue;
use support::{
    TempDir, expect_empty_stderr, expect_stdout_contains_all, expect_success, ql_command,
    run_command_capture, workspace_root,
};

fn normalize_output_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&normalize_output_text(stdout))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
}

struct BuildListWorkspaceFixture {
    temp: TempDir,
    project_root: PathBuf,
    app_root: PathBuf,
    app_source_path: PathBuf,
    app_manifest: PathBuf,
    tool_manifest: PathBuf,
}

fn write_build_list_workspace_fixture(prefix: &str) -> BuildListWorkspaceFixture {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("workspace");
    let app_root = project_root.join("packages").join("app");
    let tool_root = project_root.join("packages").join("tool");
    std::fs::create_dir_all(app_root.join("src/bin"))
        .expect("create app source tree for build list test");
    std::fs::create_dir_all(tool_root.join("src"))
        .expect("create tool source tree for build list test");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    let app_manifest = temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let app_source_path = temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn helper() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int { return 0 }\n",
    );
    temp.write(
        "workspace/packages/app/src/bin/admin.ql",
        "fn main() -> Int { return 2 }\n",
    );
    let tool_manifest = temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        "pub fn helper() -> Int { return 3 }\n",
    );

    BuildListWorkspaceFixture {
        temp,
        project_root,
        app_root,
        app_source_path,
        app_manifest,
        tool_manifest,
    }
}

fn expected_build_list_workspace_json(fixture: &BuildListWorkspaceFixture) -> JsonValue {
    serde_json::json!({
        "schema": "ql.project.targets.v1",
        "members": [
            {
                "manifest_path": fixture.app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "targets": [
                    {
                        "kind": "lib",
                        "path": "src/lib.ql",
                    },
                    {
                        "kind": "bin",
                        "path": "src/main.ql",
                    },
                    {
                        "kind": "bin",
                        "path": "src/bin/admin.ql",
                    }
                ],
            },
            {
                "manifest_path": fixture.tool_manifest.display().to_string().replace('\\', "/"),
                "package_name": "tool",
                "targets": [
                    {
                        "kind": "lib",
                        "path": "src/lib.ql",
                    }
                ],
            }
        ],
    })
}

#[test]
fn build_project_path_list_reports_discovered_targets_without_building() {
    let workspace_root = workspace_root();
    let fixture = write_build_list_workspace_fixture("ql-project-build-list");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["build"])
        .arg(&fixture.project_root)
        .arg("--list");
    let output = run_command_capture(&mut command, "`ql build --list` workspace path");
    let (stdout, stderr) = expect_success(
        "project-build-list",
        "workspace build target listing",
        &output,
    )
    .expect("workspace-path `ql build --list` should succeed");
    expect_empty_stderr(
        "project-build-list",
        "workspace build target listing",
        &stderr,
    )
    .expect("workspace-path `ql build --list` should not print stderr");
    expect_stdout_contains_all(
        "project-build-list",
        &stdout,
        &[
            &format!(
                "manifest: {}",
                fixture
                    .project_root
                    .join("packages/app/qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            "package: app",
            "  - lib: src/lib.ql",
            "  - bin: src/main.ql",
            "  - bin: src/bin/admin.ql",
            &format!(
                "manifest: {}",
                fixture
                    .project_root
                    .join("packages/tool/qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            "package: tool",
            "  - lib: src/lib.ql",
        ],
    )
    .expect("workspace-path `ql build --list` should report discovered targets");
    assert!(
        !fixture.project_root.join("packages/app/target").exists(),
        "`ql build --list` should not create build artifacts"
    );
}

#[test]
fn build_project_path_list_json_supports_target_selectors() {
    let workspace_root = workspace_root();
    let fixture = write_build_list_workspace_fixture("ql-project-build-list-json");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["build"]).arg(&fixture.project_root).args([
        "--list",
        "--json",
        "--package",
        "app",
        "--bin",
        "admin",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql build --list --json --package app --bin admin` workspace path",
    );
    let (stdout, stderr) = expect_success(
        "project-build-list-json",
        "workspace build target listing json",
        &output,
    )
    .expect("workspace-path `ql build --list --json` should succeed");
    expect_empty_stderr(
        "project-build-list-json",
        "workspace build target listing json",
        &stderr,
    )
    .expect("workspace-path `ql build --list --json` should not print stderr");

    let json = parse_json_output("project-build-list-json", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.project.targets.v1",
        "members": [
            {
                "manifest_path": fixture.app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "targets": [
                    {
                        "kind": "bin",
                        "path": "src/bin/admin.ql",
                    }
                ],
            }
        ],
    });
    assert_eq!(
        json, expected,
        "workspace-path `ql build --list --json` should reuse the stable target-listing schema"
    );
    assert!(
        !fixture.project_root.join("packages/app/target").exists(),
        "`ql build --list --json` should not create build artifacts"
    );
}

#[test]
fn build_project_member_source_list_json_uses_workspace_context() {
    let workspace_root = workspace_root();
    let fixture = write_build_list_workspace_fixture("ql-project-build-list-member-source");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["build"])
        .arg(&fixture.app_source_path)
        .args(["--list", "--json"]);
    let output = run_command_capture(
        &mut command,
        "`ql build --list --json` workspace member source path",
    );
    let (stdout, stderr) = expect_success(
        "project-build-list-member-source",
        "workspace member source build target listing",
        &output,
    )
    .expect("workspace member source `ql build --list --json` should succeed");
    expect_empty_stderr(
        "project-build-list-member-source",
        "workspace member source build target listing",
        &stderr,
    )
    .expect("workspace member source `ql build --list --json` should not print stderr");

    let json = parse_json_output("project-build-list-member-source", &stdout);
    let expected = expected_build_list_workspace_json(&fixture);
    assert_eq!(
        json, expected,
        "workspace member source `ql build --list --json` should resolve the outer workspace and report all discovered targets"
    );
    assert!(
        !fixture.project_root.join("packages/app/target").exists(),
        "`ql build --list --json` workspace member source should not create build artifacts"
    );
}

#[test]
fn build_project_member_directory_list_json_uses_workspace_context() {
    let workspace_root = workspace_root();
    let fixture = write_build_list_workspace_fixture("ql-project-build-list-member-dir");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["build"])
        .arg(&fixture.app_root)
        .args(["--list", "--json"]);
    let output = run_command_capture(
        &mut command,
        "`ql build --list --json` workspace member directory",
    );
    let (stdout, stderr) = expect_success(
        "project-build-list-member-dir",
        "workspace member directory build target listing",
        &output,
    )
    .expect("workspace member directory `ql build --list --json` should succeed");
    expect_empty_stderr(
        "project-build-list-member-dir",
        "workspace member directory build target listing",
        &stderr,
    )
    .expect("workspace member directory `ql build --list --json` should not print stderr");

    let json = parse_json_output("project-build-list-member-dir", &stdout);
    let expected = expected_build_list_workspace_json(&fixture);
    assert_eq!(
        json, expected,
        "workspace member directory `ql build --list --json` should resolve the outer workspace and report all discovered targets"
    );
    assert!(
        !fixture.project_root.join("packages/app/target").exists(),
        "`ql build --list --json` workspace member directory should not create build artifacts"
    );
}
