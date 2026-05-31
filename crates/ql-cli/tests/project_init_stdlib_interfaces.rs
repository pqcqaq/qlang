mod support;

use support::project_init_stdlib::{
    assert_repo_stdlib_check_json, parse_json_output, repo_stdlib_written_interfaces,
    write_repo_stdlib_fixture,
};
use support::{
    TempDir, expect_empty_stderr, expect_file_exists, expect_success, ql_command,
    run_command_capture, workspace_root,
};

#[test]
fn repo_stdlib_fixture_syncs_interfaces_and_checks_workspace() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-repo-stdlib-workspace-check");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);
    let written_interfaces = repo_stdlib_written_interfaces(&stdlib_root);

    for interface_path in &written_interfaces {
        assert!(
            !interface_path.exists(),
            "repo stdlib fixture should start from source-only packages: {}",
            interface_path.display()
        );
    }

    let mut sync_check = ql_command(&workspace_root);
    sync_check
        .args(["check", "--sync-interfaces", "--json"])
        .arg(&stdlib_root);
    let output = run_command_capture(
        &mut sync_check,
        "`ql check --sync-interfaces --json` copied repo stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-check",
        "sync copied repo stdlib workspace interfaces",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-check",
        "sync copied repo stdlib workspace interfaces",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-check", &stdout);
    assert_repo_stdlib_check_json(
        "synced copied repo stdlib workspace check json",
        &actual,
        &stdlib_root,
        true,
        &written_interfaces,
    );
    for interface_path in &written_interfaces {
        expect_file_exists(
            "repo-stdlib-workspace-check",
            interface_path,
            "synced stdlib interface artifact",
            "`ql check --sync-interfaces --json` copied repo stdlib workspace",
        )
        .unwrap();
    }

    let mut check = ql_command(&workspace_root);
    check.args(["check", "--json"]).arg(&stdlib_root);
    let output = run_command_capture(&mut check, "`ql check --json` synced repo stdlib workspace");
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-check",
        "check synced repo stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-check",
        "check synced repo stdlib workspace",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-check", &stdout);
    assert_repo_stdlib_check_json(
        "synced copied repo stdlib workspace follow-up check json",
        &actual,
        &stdlib_root,
        false,
        &[],
    );
}
