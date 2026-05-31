mod support;

use std::path::Path;

use support::project_init_stdlib::{
    assert_repo_stdlib_starter_graph_json, assert_repo_stdlib_starter_status_json,
    assert_repo_stdlib_starter_targets_json, parse_json_output,
};
use support::{
    expect_empty_stderr, expect_success, ql_command, run_command_capture, workspace_root,
};

#[test]
fn repo_stdlib_workspace_starter_metadata_selectors_are_current() {
    let workspace_root = workspace_root();

    let mut graph = ql_command(&workspace_root);
    graph.args([
        "project",
        "graph",
        "stdlib",
        "--package",
        "stdlib.starter",
        "--json",
    ]);
    let output = run_command_capture(
        &mut graph,
        "`ql project graph stdlib --package stdlib.starter --json`",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-metadata",
        "graph repo stdlib starter package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-metadata",
        "graph repo stdlib starter package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-metadata", &stdout);
    assert_repo_stdlib_starter_graph_json(
        "repo stdlib starter graph json",
        &actual,
        Path::new("stdlib"),
    );

    let mut status = ql_command(&workspace_root);
    status.args([
        "project",
        "status",
        "stdlib",
        "--package",
        "stdlib.starter",
        "--json",
    ]);
    let output = run_command_capture(
        &mut status,
        "`ql project status stdlib --package stdlib.starter --json`",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-metadata",
        "status repo stdlib starter package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-metadata",
        "status repo stdlib starter package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-metadata", &stdout);
    assert_repo_stdlib_starter_status_json(
        "repo stdlib starter status json",
        &actual,
        Path::new("stdlib"),
    );

    let mut targets = ql_command(&workspace_root);
    targets.args([
        "project",
        "targets",
        "stdlib",
        "--package",
        "stdlib.starter",
        "--json",
    ]);
    let output = run_command_capture(
        &mut targets,
        "`ql project targets stdlib --package stdlib.starter --json`",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-metadata",
        "targets repo stdlib starter package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-metadata",
        "targets repo stdlib starter package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-metadata", &stdout);
    assert_repo_stdlib_starter_targets_json(
        "repo stdlib starter targets json",
        &actual,
        Path::new("stdlib"),
    );
}
