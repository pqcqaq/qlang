mod support;

use std::path::Path;

use support::project_init_stdlib::{
    assert_repo_stdlib_graph_json, assert_repo_stdlib_starter_dependencies_json,
    assert_repo_stdlib_status_json, parse_json_output,
};
use support::{
    expect_empty_stderr, expect_stdout_contains_all, expect_success, ql_command,
    run_command_capture, workspace_root,
};

#[test]
fn repo_stdlib_workspace_interfaces_are_current() {
    let workspace_root = workspace_root();

    let mut check_interfaces = ql_command(&workspace_root);
    check_interfaces.args(["project", "emit-interface", "--check", "stdlib"]);
    let output = run_command_capture(
        &mut check_interfaces,
        "`ql project emit-interface --check stdlib`",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-interfaces",
        "check repo stdlib workspace interfaces",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-interfaces",
        "check repo stdlib workspace interfaces",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "repo-stdlib-workspace-interfaces",
        &stdout.replace('\\', "/"),
        &[
            "ok interface: stdlib/packages/core/std.core.qi",
            "ok interface: stdlib/packages/option/std.option.qi",
            "ok interface: stdlib/packages/result/std.result.qi",
            "ok interface: stdlib/packages/array/std.array.qi",
            "ok interface: stdlib/packages/test/std.test.qi",
            "ok interface: stdlib/examples/starter/stdlib.starter.qi",
        ],
    )
    .unwrap();

    let mut check_starter_interface = ql_command(&workspace_root);
    check_starter_interface.args([
        "project",
        "emit-interface",
        "--check",
        "stdlib",
        "--package",
        "stdlib.starter",
    ]);
    let output = run_command_capture(
        &mut check_starter_interface,
        "`ql project emit-interface --check stdlib --package stdlib.starter`",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-interfaces",
        "check repo stdlib starter interface",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-interfaces",
        "check repo stdlib starter interface",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "repo-stdlib-workspace-interfaces",
        &stdout.replace('\\', "/"),
        &["ok interface: stdlib/examples/starter/stdlib.starter.qi"],
    )
    .unwrap();

    let mut check_changed_starter_interface = ql_command(&workspace_root);
    check_changed_starter_interface.args([
        "project",
        "emit-interface",
        "--changed-only",
        "--check",
        "stdlib",
        "--package",
        "stdlib.starter",
    ]);
    let output = run_command_capture(
        &mut check_changed_starter_interface,
        "`ql project emit-interface --changed-only --check stdlib --package stdlib.starter`",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-interfaces",
        "check changed-only repo stdlib starter interface",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-interfaces",
        "check changed-only repo stdlib starter interface",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "repo-stdlib-workspace-interfaces",
        &stdout.replace('\\', "/"),
        &["up-to-date interface: stdlib/examples/starter/stdlib.starter.qi"],
    )
    .unwrap();

    let mut status = ql_command(&workspace_root);
    status.args(["project", "status", "stdlib", "--json"]);
    let output = run_command_capture(&mut status, "`ql project status stdlib --json`");
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-interfaces",
        "status repo stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-interfaces",
        "status repo stdlib workspace",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-interfaces", &stdout);
    assert_repo_stdlib_status_json(
        "repo stdlib workspace status json",
        &actual,
        Path::new("stdlib"),
    );
}

#[test]
fn repo_stdlib_workspace_graph_and_dependencies_are_current() {
    let workspace_root = workspace_root();

    let mut graph = ql_command(&workspace_root);
    graph.args(["project", "graph", "stdlib", "--json"]);
    let output = run_command_capture(&mut graph, "`ql project graph stdlib --json`");
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-graph",
        "graph repo stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-graph",
        "graph repo stdlib workspace",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-graph", &stdout);
    assert_repo_stdlib_graph_json(
        "repo stdlib workspace graph json",
        &actual,
        Path::new("stdlib"),
    );

    let mut dependencies = ql_command(&workspace_root);
    dependencies.args([
        "project",
        "dependencies",
        "stdlib",
        "--name",
        "stdlib.starter",
        "--json",
    ]);
    let output = run_command_capture(
        &mut dependencies,
        "`ql project dependencies stdlib --name stdlib.starter --json`",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-graph",
        "dependencies repo stdlib starter",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-graph",
        "dependencies repo stdlib starter",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-graph", &stdout);
    assert_repo_stdlib_starter_dependencies_json(
        "repo stdlib starter dependencies json",
        &actual,
        Path::new("stdlib"),
    );

    let mut package_dependencies = ql_command(&workspace_root);
    package_dependencies.args([
        "project",
        "dependencies",
        "stdlib",
        "--package",
        "stdlib.starter",
        "--json",
    ]);
    let output = run_command_capture(
        &mut package_dependencies,
        "`ql project dependencies stdlib --package stdlib.starter --json`",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-graph",
        "dependencies repo stdlib starter package selector",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-graph",
        "dependencies repo stdlib starter package selector",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-graph", &stdout);
    assert_repo_stdlib_starter_dependencies_json(
        "repo stdlib starter dependencies package selector json",
        &actual,
        Path::new("stdlib"),
    );
}
