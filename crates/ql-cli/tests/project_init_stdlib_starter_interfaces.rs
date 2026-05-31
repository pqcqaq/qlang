mod support;

use support::project_init_stdlib::{
    assert_repo_stdlib_check_json, assert_repo_stdlib_starter_check_json, json_path,
    parse_json_output, repo_stdlib_written_interfaces, write_repo_stdlib_fixture,
};
use support::{
    TempDir, expect_empty_stderr, expect_stdout_contains_all, expect_success, ql_command,
    run_command_capture, workspace_root,
};

#[test]
fn repo_stdlib_fixture_checks_starter_package_and_interfaces() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-repo-stdlib-workspace-starter-interfaces");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);
    let starter_interface = stdlib_root.join("examples/starter/stdlib.starter.qi");

    let mut sync_check = ql_command(&workspace_root);
    sync_check
        .args(["check", "--sync-interfaces", "--json"])
        .arg(&stdlib_root);
    let output = run_command_capture(
        &mut sync_check,
        "`ql check --sync-interfaces --json` copied repo stdlib interface fixture",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-interface-fixture",
        "sync copied repo stdlib interface fixture interfaces",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-interface-fixture",
        "sync copied repo stdlib interface fixture interfaces",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-interface-fixture", &stdout);
    assert_repo_stdlib_check_json(
        "synced copied repo stdlib interface fixture check json",
        &actual,
        &stdlib_root,
        true,
        &repo_stdlib_written_interfaces(&stdlib_root),
    );

    let mut emit_starter = ql_command(&workspace_root);
    emit_starter
        .args(["project", "emit-interface"])
        .arg(&stdlib_root)
        .args(["--package", "stdlib.starter"]);
    let output = run_command_capture(
        &mut emit_starter,
        "`ql project emit-interface --package stdlib.starter` copied repo stdlib",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-interface-fixture",
        "emit copied repo stdlib starter interface",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-interface-fixture",
        "emit copied repo stdlib starter interface",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "repo-stdlib-workspace-starter-interface-fixture",
        &stdout.replace('\\', "/"),
        &[&format!(
            "wrote interface: {}",
            json_path(&starter_interface)
        )],
    )
    .unwrap();

    let mut package_check = ql_command(&workspace_root);
    package_check
        .args(["check"])
        .arg(&stdlib_root)
        .args(["--package", "stdlib.starter", "--json"]);
    let output = run_command_capture(
        &mut package_check,
        "`ql check --package stdlib.starter --json` copied repo stdlib",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-interface-fixture",
        "check copied repo stdlib starter package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-interface-fixture",
        "check copied repo stdlib starter package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-interface-fixture", &stdout);
    assert_repo_stdlib_starter_check_json(
        "copied repo stdlib starter check json",
        &actual,
        &stdlib_root,
    );

    let mut check_starter_interface = ql_command(&workspace_root);
    check_starter_interface
        .args(["project", "emit-interface", "--check"])
        .arg(&stdlib_root)
        .args(["--package", "stdlib.starter"]);
    let output = run_command_capture(
        &mut check_starter_interface,
        "`ql project emit-interface --check --package stdlib.starter` copied repo stdlib",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-interface-fixture",
        "check copied repo stdlib starter interface",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-interface-fixture",
        "check copied repo stdlib starter interface",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "repo-stdlib-workspace-starter-interface-fixture",
        &stdout.replace('\\', "/"),
        &[&format!("ok interface: {}", json_path(&starter_interface))],
    )
    .unwrap();

    let mut check_changed_starter_interface = ql_command(&workspace_root);
    check_changed_starter_interface
        .args(["project", "emit-interface", "--changed-only", "--check"])
        .arg(&stdlib_root)
        .args(["--package", "stdlib.starter"]);
    let output = run_command_capture(
        &mut check_changed_starter_interface,
        "`ql project emit-interface --changed-only --check --package stdlib.starter` copied repo stdlib",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-interface-fixture",
        "check changed-only copied repo stdlib starter interface",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-interface-fixture",
        "check changed-only copied repo stdlib starter interface",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "repo-stdlib-workspace-starter-interface-fixture",
        &stdout.replace('\\', "/"),
        &[&format!(
            "up-to-date interface: {}",
            json_path(&starter_interface)
        )],
    )
    .unwrap();
}
