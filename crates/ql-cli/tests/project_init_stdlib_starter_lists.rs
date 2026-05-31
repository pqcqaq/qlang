mod support;

use support::project_init_stdlib::{
    assert_repo_stdlib_starter_run_list_json, assert_repo_stdlib_starter_targets_json,
    assert_repo_stdlib_test_list_json, parse_json_output, write_repo_stdlib_fixture,
};
use support::{
    TempDir, expect_empty_stderr, expect_success, ql_command, run_command_capture, workspace_root,
};

#[test]
fn repo_stdlib_fixture_lists_starter_package_targets() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-repo-stdlib-workspace-starter-lists");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);

    let mut build_list = ql_command(&workspace_root);
    build_list.args(["build"]).arg(&stdlib_root).args([
        "--list",
        "--json",
        "--package",
        "stdlib.starter",
    ]);
    let output = run_command_capture(
        &mut build_list,
        "`ql build --list --json --package stdlib.starter` copied repo stdlib",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-lists",
        "list copied repo stdlib starter build targets",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-lists",
        "list copied repo stdlib starter build targets",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-lists", &stdout);
    assert_repo_stdlib_starter_targets_json(
        "copied repo stdlib starter build list json",
        &actual,
        &stdlib_root,
    );

    let mut run_list = ql_command(&workspace_root);
    run_list.args(["run"]).arg(&stdlib_root).args([
        "--list",
        "--json",
        "--package",
        "stdlib.starter",
    ]);
    let output = run_command_capture(
        &mut run_list,
        "`ql run --list --json --package stdlib.starter` copied repo stdlib",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-lists",
        "list copied repo stdlib starter run targets",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-lists",
        "list copied repo stdlib starter run targets",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-lists", &stdout);
    assert_repo_stdlib_starter_run_list_json(
        "copied repo stdlib starter run list json",
        &actual,
        &stdlib_root,
    );

    let mut test_list = ql_command(&workspace_root);
    test_list.args(["test"]).arg(&stdlib_root).args([
        "--list",
        "--json",
        "--package",
        "stdlib.starter",
    ]);
    let output = run_command_capture(
        &mut test_list,
        "`ql test --list --json --package stdlib.starter` copied repo stdlib",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-lists",
        "list copied repo stdlib starter tests",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-lists",
        "list copied repo stdlib starter tests",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-lists", &stdout);
    assert_repo_stdlib_test_list_json(
        "copied repo stdlib starter test list json",
        &actual,
        &stdlib_root,
        Some("stdlib.starter"),
        &["examples/starter/tests/smoke.ql"],
    );
}
