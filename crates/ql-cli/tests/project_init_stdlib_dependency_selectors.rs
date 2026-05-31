mod support;

use support::project_init_stdlib::{
    assert_repo_stdlib_dependents_json, assert_repo_stdlib_starter_dependencies_json,
    parse_json_output, write_repo_stdlib_fixture,
};
use support::{
    TempDir, expect_empty_stderr, expect_success, ql_command, run_command_capture, workspace_root,
};

#[test]
fn repo_stdlib_fixture_dependency_selectors_use_copied_workspace_paths() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-repo-stdlib-workspace-dependency-selectors");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);

    let mut package_dependencies = ql_command(&workspace_root);
    package_dependencies
        .args(["project", "dependencies"])
        .arg(&stdlib_root)
        .args(["--package", "stdlib.starter", "--json"]);
    let output = run_command_capture(
        &mut package_dependencies,
        "`ql project dependencies --package stdlib.starter --json` copied repo stdlib",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-dependency-selectors",
        "dependencies copied repo stdlib starter package selector",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-dependency-selectors",
        "dependencies copied repo stdlib starter package selector",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-dependency-selectors", &stdout);
    assert_repo_stdlib_starter_dependencies_json(
        "copied repo stdlib starter dependencies package selector json",
        &actual,
        &stdlib_root,
    );

    let mut option_dependents = ql_command(&workspace_root);
    option_dependents
        .args(["project", "dependents"])
        .arg(&stdlib_root)
        .args(["--package", "std.option", "--json"]);
    let output = run_command_capture(
        &mut option_dependents,
        "`ql project dependents --package std.option --json` copied repo stdlib",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-dependency-selectors",
        "std.option dependents in copied repo stdlib by package selector",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-dependency-selectors",
        "std.option dependents in copied repo stdlib by package selector",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-dependency-selectors", &stdout);
    assert_repo_stdlib_dependents_json(
        "copied repo stdlib std.option dependents package selector json",
        &actual,
        &stdlib_root,
        "std.option",
        &[
            ("std.result", "packages/result"),
            ("std.test", "packages/test"),
            ("stdlib.starter", "examples/starter"),
        ],
    );

    let mut core_dependents = ql_command(&workspace_root);
    core_dependents
        .args(["project", "dependents"])
        .arg(&stdlib_root)
        .args(["--name", "std.core", "--json"]);
    let output = run_command_capture(
        &mut core_dependents,
        "`ql project dependents --name std.core --json` copied repo stdlib",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-dependency-selectors",
        "std.core dependents in copied repo stdlib by name selector",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-dependency-selectors",
        "std.core dependents in copied repo stdlib by name selector",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-dependency-selectors", &stdout);
    assert_repo_stdlib_dependents_json(
        "copied repo stdlib std.core dependents json",
        &actual,
        &stdlib_root,
        "std.core",
        &[
            ("std.array", "packages/array"),
            ("std.test", "packages/test"),
            ("stdlib.starter", "examples/starter"),
        ],
    );
}
