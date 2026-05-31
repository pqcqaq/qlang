mod support;

use std::path::Path;

use support::project_init_stdlib::{
    assert_repo_stdlib_run_list_json, assert_repo_stdlib_targets_json,
    assert_repo_stdlib_test_list_json, parse_json_output,
};
use support::{
    expect_empty_stderr, expect_success, ql_command, run_command_capture, workspace_root,
};

#[test]
fn repo_stdlib_workspace_lists_build_run_and_tests() {
    let workspace_root = workspace_root();

    let mut build_list = ql_command(&workspace_root);
    build_list.args(["build", "stdlib", "--list", "--json"]);
    let output = run_command_capture(&mut build_list, "`ql build stdlib --list --json`");
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-list",
        "list build targets in repo stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-list",
        "list build targets in repo stdlib workspace",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-list", &stdout);
    assert_repo_stdlib_targets_json("repo stdlib build list json", &actual, Path::new("stdlib"));

    let mut run_list = ql_command(&workspace_root);
    run_list.args(["run", "stdlib", "--list", "--json"]);
    let output = run_command_capture(&mut run_list, "`ql run stdlib --list --json`");
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-list",
        "list runnable targets in repo stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-list",
        "list runnable targets in repo stdlib workspace",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-list", &stdout);
    assert_repo_stdlib_run_list_json("repo stdlib run list json", &actual, Path::new("stdlib"));

    let all_smoke_targets = [
        "packages/core/tests/smoke.ql",
        "packages/option/tests/smoke.ql",
        "packages/result/tests/smoke.ql",
        "packages/array/tests/smoke.ql",
        "packages/test/tests/smoke.ql",
        "examples/starter/tests/smoke.ql",
    ];

    let mut test_list = ql_command(&workspace_root);
    test_list.args(["test", "stdlib", "--list", "--json"]);
    let output = run_command_capture(&mut test_list, "`ql test stdlib --list --json`");
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-list",
        "list all smoke tests in repo stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-list",
        "list all smoke tests in repo stdlib workspace",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-list", &stdout);
    assert_repo_stdlib_test_list_json(
        "repo stdlib test list json",
        &actual,
        Path::new("stdlib"),
        None,
        &all_smoke_targets,
    );

    let mut starter_test_list = ql_command(&workspace_root);
    starter_test_list.args([
        "test",
        "stdlib",
        "--list",
        "--json",
        "--package",
        "stdlib.starter",
    ]);
    let output = run_command_capture(
        &mut starter_test_list,
        "`ql test stdlib --list --json --package stdlib.starter`",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-list",
        "list starter smoke tests in repo stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-list",
        "list starter smoke tests in repo stdlib workspace",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-list", &stdout);
    assert_repo_stdlib_test_list_json(
        "repo stdlib starter test list json",
        &actual,
        Path::new("stdlib"),
        Some("stdlib.starter"),
        &["examples/starter/tests/smoke.ql"],
    );
}
