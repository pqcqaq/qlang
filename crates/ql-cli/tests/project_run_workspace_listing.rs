mod support;

use serde_json::Value as JsonValue;
use support::{
    TempDir, expect_empty_stderr, expect_success, ql_command, run_command_capture, workspace_root,
};

fn normalize_output_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&normalize_output_text(stdout))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
}

#[test]
fn run_project_source_file_list_uses_workspace_context_and_only_reports_runnable_targets() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-list-workspace-source");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app source tree for run list test");
    std::fs::create_dir_all(project_root.join("packages/tool/src"))
        .expect("create tool source tree for run list test");
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
    let app_main = temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn helper() -> Int { return 2 }\n",
    );
    let tool_manifest = temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/tool/src/main.ql",
        "fn main() -> Int { return 3 }\n",
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        "pub fn helper() -> Int { return 4 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["run"])
        .arg(&app_main)
        .args(["--list", "--json"]);
    let output = run_command_capture(
        &mut command,
        "`ql run --list --json` workspace member source path",
    );
    let (stdout, stderr) = expect_success(
        "project-run-list-workspace-source",
        "workspace member source runnable target listing",
        &output,
    )
    .expect("workspace member source `ql run --list --json` should succeed");
    expect_empty_stderr(
        "project-run-list-workspace-source",
        "workspace member source runnable target listing",
        &stderr,
    )
    .expect("workspace member source `ql run --list --json` should not print stderr");

    let json = parse_json_output("project-run-list-workspace-source", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.project.targets.v1",
        "members": [
            {
                "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "targets": [
                    {
                        "kind": "bin",
                        "path": "src/main.ql",
                    }
                ],
            },
            {
                "manifest_path": tool_manifest.display().to_string().replace('\\', "/"),
                "package_name": "tool",
                "targets": [
                    {
                        "kind": "bin",
                        "path": "src/main.ql",
                    }
                ],
            }
        ],
    });
    assert_eq!(
        json, expected,
        "workspace member source `ql run --list --json` should resolve the outer workspace and only report runnable targets"
    );
    assert!(
        !project_root.join("packages/app/target").exists(),
        "`ql run --list --json` should not build the selected source"
    );
}

#[test]
fn run_project_member_directory_list_uses_workspace_context_and_only_reports_runnable_targets() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-list-workspace-member-dir");
    let project_root = temp.path().join("workspace");
    let app_root = project_root.join("packages").join("app");
    let tool_root = project_root.join("packages").join("tool");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app source tree for run list workspace member directory test");
    std::fs::create_dir_all(tool_root.join("src"))
        .expect("create tool source tree for run list workspace member directory test");
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
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn helper() -> Int { return 2 }\n",
    );
    let tool_manifest = temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/tool/src/main.ql",
        "fn main() -> Int { return 3 }\n",
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        "pub fn helper() -> Int { return 4 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["run"])
        .arg(&app_root)
        .args(["--list", "--json"]);
    let output = run_command_capture(
        &mut command,
        "`ql run --list --json` workspace member directory",
    );
    let (stdout, stderr) = expect_success(
        "project-run-list-workspace-member-dir",
        "workspace member directory runnable target listing",
        &output,
    )
    .expect("workspace member directory `ql run --list --json` should succeed");
    expect_empty_stderr(
        "project-run-list-workspace-member-dir",
        "workspace member directory runnable target listing",
        &stderr,
    )
    .expect("workspace member directory `ql run --list --json` should not print stderr");

    let json = parse_json_output("project-run-list-workspace-member-dir", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.project.targets.v1",
        "members": [
            {
                "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "targets": [
                    {
                        "kind": "bin",
                        "path": "src/main.ql",
                    }
                ],
            },
            {
                "manifest_path": tool_manifest.display().to_string().replace('\\', "/"),
                "package_name": "tool",
                "targets": [
                    {
                        "kind": "bin",
                        "path": "src/main.ql",
                    }
                ],
            }
        ],
    });
    assert_eq!(
        json, expected,
        "workspace member directory `ql run --list --json` should resolve the outer workspace and only report runnable targets"
    );
    assert!(
        !project_root.join("packages/app/target").exists(),
        "`ql run --list --json` workspace member directory should not build the selected directory"
    );
}
