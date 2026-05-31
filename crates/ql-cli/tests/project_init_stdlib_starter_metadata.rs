mod support;

use support::project_init_stdlib::{
    assert_repo_stdlib_check_json, assert_repo_stdlib_starter_graph_json,
    assert_repo_stdlib_starter_status_json, assert_repo_stdlib_starter_targets_json, json_path,
    parse_json_output, repo_stdlib_written_interfaces, write_repo_stdlib_fixture,
};
use support::{
    TempDir, expect_empty_stderr, expect_stdout_contains_all, expect_success, ql_command,
    run_command_capture, workspace_root,
};

#[test]
fn repo_stdlib_fixture_starter_metadata_selectors_after_interface_sync() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-repo-stdlib-workspace-starter-metadata");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);

    let mut sync_check = ql_command(&workspace_root);
    sync_check
        .args(["check", "--sync-interfaces", "--json"])
        .arg(&stdlib_root);
    let output = run_command_capture(
        &mut sync_check,
        "`ql check --sync-interfaces --json` copied repo stdlib metadata fixture",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-metadata-fixture",
        "sync copied repo stdlib metadata fixture interfaces",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-metadata-fixture",
        "sync copied repo stdlib metadata fixture interfaces",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-metadata-fixture", &stdout);
    assert_repo_stdlib_check_json(
        "synced copied repo stdlib metadata fixture check json",
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
        "`ql project emit-interface` copied repo stdlib starter",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-metadata-fixture",
        "emit copied repo stdlib starter interface",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-metadata-fixture",
        "emit copied repo stdlib starter interface",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "repo-stdlib-workspace-starter-metadata-fixture",
        &stdout.replace('\\', "/"),
        &[&format!(
            "wrote interface: {}",
            json_path(&stdlib_root.join("examples/starter/stdlib.starter.qi"))
        )],
    )
    .unwrap();

    let mut graph = ql_command(&workspace_root);
    graph.args(["project", "graph"]).arg(&stdlib_root).args([
        "--package",
        "stdlib.starter",
        "--json",
    ]);
    let output = run_command_capture(
        &mut graph,
        "`ql project graph --package stdlib.starter --json` copied repo stdlib",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-metadata-fixture",
        "graph copied repo stdlib starter package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-metadata-fixture",
        "graph copied repo stdlib starter package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-metadata-fixture", &stdout);
    assert_repo_stdlib_starter_graph_json(
        "copied repo stdlib starter graph json",
        &actual,
        &stdlib_root,
    );

    let mut status = ql_command(&workspace_root);
    status.args(["project", "status"]).arg(&stdlib_root).args([
        "--package",
        "stdlib.starter",
        "--json",
    ]);
    let output = run_command_capture(
        &mut status,
        "`ql project status --package stdlib.starter --json` copied repo stdlib",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-metadata-fixture",
        "status copied repo stdlib starter package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-metadata-fixture",
        "status copied repo stdlib starter package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-metadata-fixture", &stdout);
    assert_repo_stdlib_starter_status_json(
        "copied repo stdlib starter status json",
        &actual,
        &stdlib_root,
    );

    let mut targets = ql_command(&workspace_root);
    targets
        .args(["project", "targets"])
        .arg(&stdlib_root)
        .args(["--package", "stdlib.starter", "--json"]);
    let output = run_command_capture(
        &mut targets,
        "`ql project targets --package stdlib.starter --json` copied repo stdlib",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-metadata-fixture",
        "targets copied repo stdlib starter package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-metadata-fixture",
        "targets copied repo stdlib starter package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-metadata-fixture", &stdout);
    assert_repo_stdlib_starter_targets_json(
        "copied repo stdlib starter targets json",
        &actual,
        &stdlib_root,
    );
}
