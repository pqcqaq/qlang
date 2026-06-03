mod support;

use serde_json::Value as JsonValue;
use support::{
    TempDir, expect_empty_stderr, expect_exit_code, expect_snapshot_matches, expect_success,
    ql_command, read_normalized_file, run_command_capture, workspace_root,
};

fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&stdout.replace("\r\n", "\n"))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
}

#[test]
fn project_lock_json_reports_invalid_manifest_load_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-lock-invalid-manifest-json");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(&project_root)
        .expect("create project directory for invalid lock manifest test");
    let manifest_path = temp.write(
        "app/qlang.toml",
        r#"
[package
name = "app"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "lock"])
        .arg(&project_root)
        .arg("--json");
    let output = run_command_capture(&mut command, "`ql project lock --json` invalid manifest");
    let (stdout, stderr) = expect_exit_code(
        "project-lock-invalid-manifest-json",
        "invalid manifest json lockfile generation",
        &output,
        1,
    )
    .expect("invalid manifest json lockfile generation should fail");
    expect_empty_stderr(
        "project-lock-invalid-manifest-json",
        "invalid manifest json lockfile generation",
        &stderr,
    )
    .expect("invalid manifest json lockfile generation should keep stderr empty");

    let json = parse_json_output("project-lock-invalid-manifest-json", &stdout);
    assert_eq!(json["schema"], "ql.project.lock.result.v1");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["check_only"], false);
    assert_eq!(json["lockfile"], JsonValue::Null);
    assert_eq!(json["lockfile_path"], JsonValue::Null);
    assert_eq!(
        json["project_manifest_path"],
        manifest_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["stage"], "manifest-load");
    assert_eq!(
        failure["manifest_path"],
        manifest_path.display().to_string().replace('\\', "/")
    );
    assert!(
        failure["message"]
            .as_str()
            .expect("invalid manifest json failure should expose a message")
            .contains("invalid manifest"),
        "invalid manifest json failure should describe the load failure: {json}"
    );
}

#[test]
fn project_lock_json_reports_lockfile_render_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-lock-render-failure-json");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(&project_root)
        .expect("create project directory for render failure lock test");
    let manifest_path = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let lockfile_path = project_root.join("qlang.lock");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "lock"])
        .arg(&project_root)
        .arg("--json");
    let output = run_command_capture(&mut command, "`ql project lock --json` render failure");
    let (stdout, stderr) = expect_exit_code(
        "project-lock-render-failure-json",
        "render failure json lockfile generation",
        &output,
        1,
    )
    .expect("render failure json lockfile generation should fail");
    expect_empty_stderr(
        "project-lock-render-failure-json",
        "render failure json lockfile generation",
        &stderr,
    )
    .expect("render failure json lockfile generation should keep stderr empty");
    assert!(
        !lockfile_path.exists(),
        "render failure json lockfile generation should not create a lockfile"
    );

    let json = parse_json_output("project-lock-render-failure-json", &stdout);
    assert_eq!(json["schema"], "ql.project.lock.result.v1");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["check_only"], false);
    assert_eq!(json["lockfile"], JsonValue::Null);
    assert_eq!(
        json["project_manifest_path"],
        manifest_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["lockfile_path"],
        lockfile_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["stage"], "lockfile-render");
    assert_eq!(
        failure["manifest_path"],
        manifest_path.display().to_string().replace('\\', "/")
    );
    assert!(
        failure["message"]
            .as_str()
            .expect("render failure json should expose a message")
            .contains("package source directory"),
        "render failure json should describe the missing package source root: {json}"
    );
}

#[test]
fn project_lock_check_json_reports_stale_lockfile() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-lock-stale-json");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for stale json lock test");

    let manifest_path = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[profile]
default = "release"
"#,
    );
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");

    let lockfile_path = project_root.join("qlang.lock");
    let mut write_command = ql_command(&workspace_root);
    write_command.args(["project", "lock"]).arg(&project_root);
    let write_output = run_command_capture(&mut write_command, "`ql project lock` package");
    let (_, write_stderr) = expect_success(
        "project-lock-stale-json",
        "initial json lockfile generation",
        &write_output,
    )
    .expect("initial json lockfile generation should succeed");
    expect_empty_stderr(
        "project-lock-stale-json",
        "initial json lockfile generation",
        &write_stderr,
    )
    .expect("initial json lockfile generation should not print stderr");
    let initial_lockfile = read_normalized_file(&lockfile_path, "initial json lockfile");

    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[profile]
default = "debug"
"#,
    );

    let mut check_command = ql_command(&workspace_root);
    check_command
        .args(["project", "lock", "--check"])
        .arg(&project_root)
        .arg("--json");
    let output = run_command_capture(&mut check_command, "`ql project lock --check --json` stale");
    let (stdout, stderr) = expect_exit_code(
        "project-lock-stale-json",
        "stale json lockfile check",
        &output,
        1,
    )
    .expect("stale json lockfile check should fail with exit code 1");
    expect_empty_stderr(
        "project-lock-stale-json",
        "stale json lockfile check",
        &stderr,
    )
    .expect("stale json lockfile check should keep stderr empty");

    let json = parse_json_output("project-lock-stale-json", &stdout);
    assert_eq!(json["schema"], "ql.project.lock.result.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["project_manifest_path"],
        manifest_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["lockfile_path"],
        lockfile_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["check_only"], true);
    assert_eq!(json["status"], "failed");
    assert_eq!(json["failure"]["kind"], "stale");
    assert_eq!(
        json["failure"]["message"],
        format!(
            "lockfile `{}` is stale",
            lockfile_path.display().to_string().replace('\\', "/")
        )
    );
    assert_eq!(
        json["failure"]["rerun_command"],
        format!(
            "ql project lock {}",
            manifest_path.display().to_string().replace('\\', "/")
        )
    );
    assert_eq!(json["lockfile"]["schema"], "ql.project.lock.v1");
    assert_eq!(json["lockfile"]["packages"][0]["default_profile"], "debug");

    let unchanged_lockfile = read_normalized_file(&lockfile_path, "stale json lockfile");
    expect_snapshot_matches(
        "project-lock-stale-json",
        "stale json lockfile contents",
        &initial_lockfile,
        &unchanged_lockfile,
    )
    .expect("stale json lockfile check should not rewrite the existing lockfile");
}
