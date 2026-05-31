mod support;

use serde_json::Value as JsonValue;
use std::path::Path;
use support::project_init_stdlib::{
    assert_repo_stdlib_lock_json, json_path, parse_json_output, write_repo_stdlib_fixture,
};
use support::{
    TempDir, expect_empty_stderr, expect_exit_code, expect_file_exists, expect_success, ql_command,
    read_normalized_file, run_command_capture, workspace_root,
};

#[test]
fn repo_stdlib_fixture_writes_and_checks_workspace_lockfile() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-repo-stdlib-workspace-lock");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);
    let lockfile_path = stdlib_root.join("qlang.lock");

    assert!(
        !lockfile_path.exists(),
        "copied repo stdlib fixture should start without a generated lockfile"
    );

    let mut lock = ql_command(&workspace_root);
    lock.args(["project", "lock", "--json"]).arg(&stdlib_root);
    let output = run_command_capture(&mut lock, "`ql project lock --json` copied repo stdlib");
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-lock",
        "write copied repo stdlib workspace lockfile",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-lock",
        "write copied repo stdlib workspace lockfile",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-lock", &stdout);
    assert_repo_stdlib_lock_json(
        "copied repo stdlib workspace lock json",
        &actual,
        &stdlib_root,
        &stdlib_root,
        false,
        "wrote",
    );
    expect_file_exists(
        "repo-stdlib-workspace-lock",
        &lockfile_path,
        "copied repo stdlib workspace lockfile",
        "`ql project lock --json` copied repo stdlib",
    )
    .unwrap();

    let lockfile_source = read_normalized_file(&lockfile_path, "copied repo stdlib lockfile");
    let lockfile_json = serde_json::from_str::<JsonValue>(&lockfile_source)
        .expect("copied repo stdlib lockfile should remain valid json");
    assert_eq!(
        actual["lockfile"], lockfile_json,
        "copied repo stdlib lock write should report the written lockfile exactly"
    );

    let mut check = ql_command(&workspace_root);
    check
        .args(["project", "lock", "--check", "--json"])
        .arg(&stdlib_root);
    let output = run_command_capture(
        &mut check,
        "`ql project lock --check --json` copied repo stdlib",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-lock",
        "check copied repo stdlib workspace lockfile",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-lock",
        "check copied repo stdlib workspace lockfile",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-lock", &stdout);
    assert_repo_stdlib_lock_json(
        "copied repo stdlib workspace lock check json",
        &actual,
        &stdlib_root,
        &stdlib_root,
        true,
        "up-to-date",
    );
    assert_eq!(
        actual["lockfile"], lockfile_json,
        "copied repo stdlib lock check should report the written lockfile exactly"
    );
}

#[test]
fn repo_stdlib_fixture_lock_check_reports_stale_workspace_lockfile() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-repo-stdlib-workspace-lock-stale");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);
    let manifest_path = stdlib_root.join("qlang.toml");
    let lockfile_path = stdlib_root.join("qlang.lock");

    let mut lock = ql_command(&workspace_root);
    lock.args(["project", "lock", "--json"]).arg(&stdlib_root);
    let output = run_command_capture(&mut lock, "`ql project lock --json` copied repo stdlib");
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-lock-stale",
        "write copied repo stdlib workspace lockfile",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-lock-stale",
        "write copied repo stdlib workspace lockfile",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-lock-stale", &stdout);
    assert_repo_stdlib_lock_json(
        "copied repo stdlib workspace initial lock json",
        &actual,
        &stdlib_root,
        &stdlib_root,
        false,
        "wrote",
    );
    let initial_lockfile =
        read_normalized_file(&lockfile_path, "initial copied repo stdlib lockfile");

    temp.write(
        "stdlib/packages/core/qlang.toml",
        r#"[package]
name = "std.core"

[profile]
default = "debug"
"#,
    );

    let mut check = ql_command(&workspace_root);
    check
        .args(["project", "lock", "--check", "--json"])
        .arg(&stdlib_root);
    let output = run_command_capture(
        &mut check,
        "`ql project lock --check --json` stale copied repo stdlib",
    );
    let (stdout, stderr) = expect_exit_code(
        "repo-stdlib-workspace-lock-stale",
        "check stale copied repo stdlib workspace lockfile",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-lock-stale",
        "check stale copied repo stdlib workspace lockfile",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-lock-stale", &stdout);

    assert_eq!(actual["schema"], "ql.project.lock.result.v1");
    assert_eq!(actual["path"], json_path(&stdlib_root));
    assert_eq!(actual["project_manifest_path"], json_path(&manifest_path));
    assert_eq!(actual["lockfile_path"], json_path(&lockfile_path));
    assert_eq!(actual["check_only"], true);
    assert_eq!(actual["status"], "failed");
    assert_eq!(actual["failure"]["kind"], "stale");
    assert_eq!(
        actual["failure"]["message"],
        format!("lockfile `{}` is stale", json_path(&lockfile_path))
    );
    assert_eq!(
        actual["failure"]["rerun_command"],
        format!("ql project lock {}", json_path(&manifest_path))
    );
    assert_eq!(actual["lockfile"]["schema"], "ql.project.lock.v1");
    assert_eq!(actual["lockfile"]["root"]["kind"], "workspace");
    assert_eq!(actual["lockfile"]["root"]["manifest_path"], "qlang.toml");
    assert_eq!(
        actual["lockfile"]["workspace_members"],
        serde_json::json!([
            "packages/core/qlang.toml",
            "packages/option/qlang.toml",
            "packages/result/qlang.toml",
            "packages/array/qlang.toml",
            "packages/test/qlang.toml",
            "examples/starter/qlang.toml",
        ])
    );
    let core_package = actual["lockfile"]["packages"]
        .as_array()
        .unwrap_or_else(|| {
            panic!("stale copied repo stdlib check should report rendered packages: {actual}")
        })
        .iter()
        .find(|package| package["package_name"] == "std.core")
        .unwrap_or_else(|| {
            panic!("stale copied repo stdlib check should report std.core package: {actual}")
        });
    assert_eq!(core_package["manifest_path"], "packages/core/qlang.toml");
    assert_eq!(core_package["default_profile"], "debug");

    let unchanged_lockfile =
        read_normalized_file(&lockfile_path, "stale copied repo stdlib lockfile");
    assert_eq!(
        initial_lockfile, unchanged_lockfile,
        "stale copied repo stdlib lock check should not rewrite qlang.lock"
    );
    let on_disk_lockfile: JsonValue = serde_json::from_str(&unchanged_lockfile)
        .expect("unchanged copied repo stdlib lockfile should remain valid json");
    assert_eq!(
        on_disk_lockfile["packages"][0]["default_profile"],
        JsonValue::Null,
        "unchanged lockfile should keep the pre-drift std.core profile"
    );
}

#[test]
fn repo_stdlib_workspace_lockfile_is_current() {
    let workspace_root = workspace_root();

    let mut lock_check = ql_command(&workspace_root);
    lock_check.args(["project", "lock", "stdlib", "--check", "--json"]);
    let output = run_command_capture(&mut lock_check, "`ql project lock stdlib --check --json`");
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-lock",
        "check repo stdlib workspace lockfile",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-lock",
        "check repo stdlib workspace lockfile",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-lock", &stdout);
    assert_repo_stdlib_lock_json(
        "repo stdlib workspace lock check json",
        &actual,
        Path::new("stdlib"),
        Path::new("stdlib"),
        true,
        "up-to-date",
    );

    let lockfile_source = read_normalized_file(
        &workspace_root.join("stdlib/qlang.lock"),
        "repo stdlib lockfile",
    );
    let lockfile_json = serde_json::from_str::<JsonValue>(&lockfile_source)
        .expect("repo stdlib lockfile should remain valid json");
    assert_eq!(
        actual["lockfile"], lockfile_json,
        "repo stdlib lock check should report the tracked lockfile exactly"
    );
}
