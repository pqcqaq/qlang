mod support;

use std::path::Path;

use support::project_init_stdlib::{
    assert_repo_stdlib_dependents_json, assert_repo_stdlib_targets_json, parse_json_output,
};
use support::{
    expect_empty_stderr, expect_success, ql_command, run_command_capture, workspace_root,
};

#[test]
fn repo_stdlib_workspace_targets_and_dependents_are_current() {
    let workspace_root = workspace_root();

    let mut targets = ql_command(&workspace_root);
    targets.args(["project", "targets", "stdlib", "--json"]);
    let output = run_command_capture(&mut targets, "`ql project targets stdlib --json`");
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-targets",
        "targets repo stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-targets",
        "targets repo stdlib workspace",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-targets", &stdout);
    assert_repo_stdlib_targets_json(
        "repo stdlib workspace targets json",
        &actual,
        Path::new("stdlib"),
    );

    let mut option_dependents = ql_command(&workspace_root);
    option_dependents.args([
        "project",
        "dependents",
        "stdlib",
        "--name",
        "std.option",
        "--json",
    ]);
    let output = run_command_capture(
        &mut option_dependents,
        "`ql project dependents stdlib --name std.option --json`",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-targets",
        "std.option dependents in repo stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-targets",
        "std.option dependents in repo stdlib workspace",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-targets", &stdout);
    assert_repo_stdlib_dependents_json(
        "repo stdlib std.option dependents json",
        &actual,
        Path::new("stdlib"),
        "std.option",
        &[
            ("std.result", "packages/result"),
            ("std.test", "packages/test"),
            ("stdlib.starter", "examples/starter"),
        ],
    );

    let mut option_package_dependents = ql_command(&workspace_root);
    option_package_dependents.args([
        "project",
        "dependents",
        "stdlib",
        "--package",
        "std.option",
        "--json",
    ]);
    let output = run_command_capture(
        &mut option_package_dependents,
        "`ql project dependents stdlib --package std.option --json`",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-targets",
        "std.option dependents in repo stdlib workspace by package selector",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-targets",
        "std.option dependents in repo stdlib workspace by package selector",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-targets", &stdout);
    assert_repo_stdlib_dependents_json(
        "repo stdlib std.option dependents package selector json",
        &actual,
        Path::new("stdlib"),
        "std.option",
        &[
            ("std.result", "packages/result"),
            ("std.test", "packages/test"),
            ("stdlib.starter", "examples/starter"),
        ],
    );

    let mut core_dependents = ql_command(&workspace_root);
    core_dependents.args([
        "project",
        "dependents",
        "stdlib",
        "--name",
        "std.core",
        "--json",
    ]);
    let output = run_command_capture(
        &mut core_dependents,
        "`ql project dependents stdlib --name std.core --json`",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-targets",
        "std.core dependents in repo stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-targets",
        "std.core dependents in repo stdlib workspace",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-targets", &stdout);
    assert_repo_stdlib_dependents_json(
        "repo stdlib std.core dependents json",
        &actual,
        Path::new("stdlib"),
        "std.core",
        &[
            ("std.array", "packages/array"),
            ("std.test", "packages/test"),
            ("stdlib.starter", "examples/starter"),
        ],
    );
}
