mod support;

use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_stderr_contains,
    expect_stdout_contains_all, expect_success, ql_command, run_command_capture, workspace_root,
};

fn write_query_package(prefix: &str) -> TempDir {
    let temp = TempDir::new(prefix);
    temp.write(
        "qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("src/main.ql", "fn main() -> Int { return 0 }\n");
    temp
}

fn write_dependency_workspace(prefix: &str) -> TempDir {
    let temp = TempDir::new(prefix);
    temp.write(
        "qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
    );
    temp.write(
        "packages/app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
core = "../core"
"#,
    );
    temp.write(
        "packages/core/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );
    temp
}

#[test]
fn project_query_commands_use_current_directory_by_default() {
    let workspace_root = workspace_root();
    let temp = write_query_package("ql-project-query-default-cwd");

    let mut status = ql_command(&workspace_root);
    status.current_dir(temp.path());
    status.args(["project", "status"]);
    let output = run_command_capture(&mut status, "`ql project status` default cwd");
    let (stdout, stderr) = expect_success(
        "project-query-status-default-cwd",
        "`ql project status` default cwd",
        &output,
    )
    .expect("project status should inspect the current directory by default");
    expect_empty_stderr(
        "project-query-status-default-cwd",
        "`ql project status` default cwd",
        &stderr,
    )
    .expect("project status default cwd should keep stderr empty");
    expect_stdout_contains_all(
        "project-query-status-default-cwd",
        &stdout.replace('\\', "/"),
        &["kind: package", "  - app", "targets:", "lib: src/lib.ql"],
    )
    .expect("project status default cwd should render the package status");

    let mut graph = ql_command(&workspace_root);
    graph.current_dir(temp.path());
    graph.args(["project", "graph"]);
    let output = run_command_capture(&mut graph, "`ql project graph` default cwd");
    let (stdout, stderr) = expect_success(
        "project-query-graph-default-cwd",
        "`ql project graph` default cwd",
        &output,
    )
    .expect("project graph should inspect the current directory by default");
    expect_empty_stderr(
        "project-query-graph-default-cwd",
        "`ql project graph` default cwd",
        &stderr,
    )
    .expect("project graph default cwd should keep stderr empty");
    expect_stdout_contains_all(
        "project-query-graph-default-cwd",
        &stdout.replace('\\', "/"),
        &["package: app", "references: []"],
    )
    .expect("project graph default cwd should render the package graph");
}

#[test]
fn project_targets_command_applies_target_selectors() {
    let workspace_root = workspace_root();
    let temp = write_query_package("ql-project-query-target-selector");

    let mut command = ql_command(&workspace_root);
    command
        .arg("project")
        .arg("targets")
        .arg(temp.path())
        .args(["--bin", "main"]);
    let output = run_command_capture(&mut command, "`ql project targets --bin main`");
    let (stdout, stderr) = expect_success(
        "project-query-target-selector",
        "`ql project targets --bin main`",
        &output,
    )
    .expect("project targets should apply the binary selector");
    expect_empty_stderr(
        "project-query-target-selector",
        "`ql project targets --bin main`",
        &stderr,
    )
    .expect("project targets selector should keep stderr empty");
    let stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        "project-query-target-selector",
        &stdout,
        &["package: app", "bin: src/main.ql"],
    )
    .expect("project targets selector should render the selected binary");
    assert!(
        !stdout.contains("lib: src/lib.ql"),
        "binary selector should exclude the library target, got:\n{stdout}"
    );
}

#[test]
fn project_dependency_query_commands_apply_package_selectors() {
    let workspace_root = workspace_root();
    let temp = write_dependency_workspace("ql-project-query-dependency-selector");

    let mut dependencies = ql_command(&workspace_root);
    dependencies
        .arg("project")
        .arg("dependencies")
        .arg(temp.path())
        .args(["--name", "app"]);
    let output = run_command_capture(&mut dependencies, "`ql project dependencies --name app`");
    let (stdout, stderr) = expect_success(
        "project-query-dependencies-selector",
        "`ql project dependencies --name app`",
        &output,
    )
    .expect("project dependencies should apply the package selector");
    expect_empty_stderr(
        "project-query-dependencies-selector",
        "`ql project dependencies --name app`",
        &stderr,
    )
    .expect("project dependencies selector should keep stderr empty");
    expect_stdout_contains_all(
        "project-query-dependencies-selector",
        &stdout.replace('\\', "/"),
        &["package: app", "packages/core (core)"],
    )
    .expect("project dependencies selector should render selected package dependencies");

    let mut dependents = ql_command(&workspace_root);
    dependents
        .arg("project")
        .arg("dependents")
        .arg(temp.path())
        .args(["--package", "core"]);
    let output = run_command_capture(&mut dependents, "`ql project dependents --package core`");
    let (stdout, stderr) = expect_success(
        "project-query-dependents-selector",
        "`ql project dependents --package core`",
        &output,
    )
    .expect("project dependents should apply the package selector alias");
    expect_empty_stderr(
        "project-query-dependents-selector",
        "`ql project dependents --package core`",
        &stderr,
    )
    .expect("project dependents selector should keep stderr empty");
    expect_stdout_contains_all(
        "project-query-dependents-selector",
        &stdout.replace('\\', "/"),
        &["package: core", "packages/app (app)"],
    )
    .expect("project dependents selector should render selected package dependents");
}

#[test]
fn project_query_commands_reject_invalid_arguments() {
    let workspace_root = workspace_root();
    let temp = write_query_package("ql-project-query-invalid-args");

    let cases = [
        (
            vec![
                "project".to_owned(),
                "status".to_owned(),
                temp.path().display().to_string(),
                "--unknown".to_owned(),
            ],
            "error: unknown `ql project status` option `--unknown`",
        ),
        (
            vec![
                "project".to_owned(),
                "status".to_owned(),
                temp.path().display().to_string(),
                "extra".to_owned(),
            ],
            "error: unknown `ql project status` argument `extra`",
        ),
        (
            vec![
                "project".to_owned(),
                "status".to_owned(),
                temp.path().display().to_string(),
                "--package".to_owned(),
            ],
            "error: `ql project status` --package expects a package name",
        ),
        (
            vec![
                "project".to_owned(),
                "status".to_owned(),
                temp.path().display().to_string(),
                "--package".to_owned(),
                "app".to_owned(),
                "--package".to_owned(),
                "core".to_owned(),
            ],
            "error: `ql project status` received `--package` more than once",
        ),
        (
            vec![
                "project".to_owned(),
                "graph".to_owned(),
                temp.path().display().to_string(),
                "--unknown".to_owned(),
            ],
            "error: unknown `ql project graph` option `--unknown`",
        ),
        (
            vec![
                "project".to_owned(),
                "graph".to_owned(),
                temp.path().display().to_string(),
                "extra".to_owned(),
            ],
            "error: unknown `ql project graph` argument `extra`",
        ),
        (
            vec![
                "project".to_owned(),
                "graph".to_owned(),
                temp.path().display().to_string(),
                "--package".to_owned(),
            ],
            "error: `ql project graph` --package expects a package name",
        ),
        (
            vec![
                "project".to_owned(),
                "targets".to_owned(),
                temp.path().display().to_string(),
                "--unknown".to_owned(),
            ],
            "error: unknown `ql project targets` option `--unknown`",
        ),
        (
            vec![
                "project".to_owned(),
                "targets".to_owned(),
                temp.path().display().to_string(),
                "extra".to_owned(),
            ],
            "error: unknown `ql project targets` argument `extra`",
        ),
        (
            vec![
                "project".to_owned(),
                "targets".to_owned(),
                temp.path().display().to_string(),
                "--bin".to_owned(),
            ],
            "error: `ql project targets` --bin expects a target name",
        ),
        (
            vec![
                "project".to_owned(),
                "dependencies".to_owned(),
                temp.path().display().to_string(),
                "--unknown".to_owned(),
            ],
            "error: unknown `ql project dependencies` option `--unknown`",
        ),
        (
            vec![
                "project".to_owned(),
                "dependencies".to_owned(),
                temp.path().display().to_string(),
                "extra".to_owned(),
            ],
            "error: unknown `ql project dependencies` argument `extra`",
        ),
        (
            vec![
                "project".to_owned(),
                "dependencies".to_owned(),
                temp.path().display().to_string(),
                "--name".to_owned(),
            ],
            "error: `ql project dependencies` --name expects a package name",
        ),
        (
            vec![
                "project".to_owned(),
                "dependencies".to_owned(),
                temp.path().display().to_string(),
                "--name".to_owned(),
                "app".to_owned(),
                "--package".to_owned(),
                "core".to_owned(),
            ],
            "error: `ql project dependencies` received package selector more than once",
        ),
        (
            vec![
                "project".to_owned(),
                "dependents".to_owned(),
                temp.path().display().to_string(),
                "--unknown".to_owned(),
            ],
            "error: unknown `ql project dependents` option `--unknown`",
        ),
        (
            vec![
                "project".to_owned(),
                "dependents".to_owned(),
                temp.path().display().to_string(),
                "extra".to_owned(),
            ],
            "error: unknown `ql project dependents` argument `extra`",
        ),
        (
            vec![
                "project".to_owned(),
                "dependents".to_owned(),
                temp.path().display().to_string(),
                "--package".to_owned(),
            ],
            "error: `ql project dependents` --package expects a package name",
        ),
        (
            vec![
                "project".to_owned(),
                "dependents".to_owned(),
                temp.path().display().to_string(),
                "--package".to_owned(),
                "app".to_owned(),
                "--name".to_owned(),
                "core".to_owned(),
            ],
            "error: `ql project dependents` received package selector more than once",
        ),
    ];

    for (args, expected_error) in cases {
        let mut command = ql_command(&workspace_root);
        command.args(args);
        let output = run_command_capture(&mut command, "`ql project` query with invalid args");
        let (stdout, stderr) = expect_exit_code(
            "project-query-command-argument-validation",
            "`ql project` query with invalid args",
            &output,
            1,
        )
        .expect("invalid project query arguments should fail");
        expect_empty_stdout(
            "project-query-command-argument-validation",
            "`ql project` query with invalid args",
            &stdout,
        )
        .expect("invalid project query arguments should not print stdout");
        expect_stderr_contains(
            "project-query-command-argument-validation",
            "`ql project` query with invalid args",
            &stderr,
            expected_error,
        )
        .expect("invalid project query arguments should identify the rejected input");
    }
}
