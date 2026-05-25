mod support;

use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_stderr_contains,
    expect_stdout_contains_all, expect_success, ql_command, read_normalized_file,
    run_command_capture, workspace_root,
};

fn write_app_core_workspace(prefix: &str) -> TempDir {
    let temp = TempDir::new(prefix);
    temp.write(
        "qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\"]\n",
    );
    temp.write("packages/app/qlang.toml", "[package]\nname = \"app\"\n");
    temp.write("packages/core/qlang.toml", "[package]\nname = \"core\"\n");
    temp
}

#[test]
fn project_dependency_commands_use_current_directory_by_default() {
    let workspace_root = workspace_root();
    let temp = write_app_core_workspace("ql-project-dependency-default-cwd");
    let app_manifest = temp.path().join("packages/app/qlang.toml");

    let mut add = ql_command(&workspace_root);
    add.current_dir(temp.path());
    add.args([
        "project",
        "add-dependency",
        "--package",
        "app",
        "--name",
        "core",
    ]);
    let output = run_command_capture(&mut add, "`ql project add-dependency` default cwd");
    let (stdout, stderr) = expect_success(
        "project-dependency-default-cwd",
        "`ql project add-dependency` default cwd",
        &output,
    )
    .expect("project add-dependency should use the current directory by default");
    expect_empty_stderr(
        "project-dependency-default-cwd",
        "`ql project add-dependency` default cwd",
        &stderr,
    )
    .expect("project add-dependency default cwd should keep stderr empty");
    expect_stdout_contains_all(
        "project-dependency-default-cwd",
        &stdout.replace('\\', "/"),
        &[&format!(
            "updated: {}",
            app_manifest.to_string_lossy().replace('\\', "/")
        )],
    )
    .expect("project add-dependency default cwd should report the updated package manifest");
    let manifest = read_normalized_file(&app_manifest, "app manifest after add-dependency");
    assert!(
        manifest.contains("[dependencies]\ncore = \"../core\"\n"),
        "project add-dependency default cwd should add the selected dependency, got:\n{manifest}"
    );

    let mut remove = ql_command(&workspace_root);
    remove.current_dir(temp.path());
    remove.args([
        "project",
        "remove-dependency",
        "--package",
        "app",
        "--name",
        "core",
    ]);
    let output = run_command_capture(&mut remove, "`ql project remove-dependency` default cwd");
    let (stdout, stderr) = expect_success(
        "project-dependency-default-cwd",
        "`ql project remove-dependency` default cwd",
        &output,
    )
    .expect("project remove-dependency should use the current directory by default");
    expect_empty_stderr(
        "project-dependency-default-cwd",
        "`ql project remove-dependency` default cwd",
        &stderr,
    )
    .expect("project remove-dependency default cwd should keep stderr empty");
    expect_stdout_contains_all(
        "project-dependency-default-cwd",
        &stdout.replace('\\', "/"),
        &[&format!(
            "updated: {}",
            app_manifest.to_string_lossy().replace('\\', "/")
        )],
    )
    .expect("project remove-dependency default cwd should report the updated package manifest");
    let manifest = read_normalized_file(&app_manifest, "app manifest after remove-dependency");
    assert!(
        !manifest.contains("core = \"../core\""),
        "project remove-dependency default cwd should remove the selected dependency, got:\n{manifest}"
    );
}

#[test]
fn project_add_dependency_cli_rejects_invalid_arguments() {
    let workspace_root = workspace_root();
    let temp = write_app_core_workspace("ql-project-add-dependency-cli-invalid");
    let path = temp.path().display().to_string();

    let cases = [
        (
            vec![
                "project".to_owned(),
                "add-dependency".to_owned(),
                path.clone(),
                "--package".to_owned(),
            ],
            "error: `ql project add-dependency --package` expects a package name",
        ),
        (
            vec![
                "project".to_owned(),
                "add-dependency".to_owned(),
                path.clone(),
                "--package".to_owned(),
                "app".to_owned(),
                "--package".to_owned(),
                "core".to_owned(),
            ],
            "error: `ql project add-dependency` received `--package` more than once",
        ),
        (
            vec![
                "project".to_owned(),
                "add-dependency".to_owned(),
                path.clone(),
                "--name".to_owned(),
            ],
            "error: `ql project add-dependency --name` expects a package name",
        ),
        (
            vec![
                "project".to_owned(),
                "add-dependency".to_owned(),
                path.clone(),
                "--name".to_owned(),
                "core".to_owned(),
                "--name".to_owned(),
                "util".to_owned(),
            ],
            "error: `ql project add-dependency` received `--name` more than once",
        ),
        (
            vec![
                "project".to_owned(),
                "add-dependency".to_owned(),
                path.clone(),
                "--path".to_owned(),
            ],
            "error: `ql project add-dependency --path` expects a package path",
        ),
        (
            vec![
                "project".to_owned(),
                "add-dependency".to_owned(),
                path.clone(),
                "--path".to_owned(),
                "dep-a".to_owned(),
                "--path".to_owned(),
                "dep-b".to_owned(),
            ],
            "error: `ql project add-dependency` received `--path` more than once",
        ),
        (
            vec![
                "project".to_owned(),
                "add-dependency".to_owned(),
                path.clone(),
                "--unknown".to_owned(),
            ],
            "error: unknown `ql project add-dependency` option `--unknown`",
        ),
        (
            vec![
                "project".to_owned(),
                "add-dependency".to_owned(),
                path.clone(),
                "extra".to_owned(),
                "--name".to_owned(),
                "core".to_owned(),
            ],
            "error: unknown `ql project add-dependency` argument `extra`",
        ),
        (
            vec![
                "project".to_owned(),
                "add-dependency".to_owned(),
                path.clone(),
                "--name".to_owned(),
                "core".to_owned(),
                "--path".to_owned(),
                "dep".to_owned(),
            ],
            "error: `ql project add-dependency` accepts either `--name <package>` or `--path <file-or-dir>`, not both",
        ),
        (
            vec!["project".to_owned(), "add-dependency".to_owned(), path],
            "error: `ql project add-dependency` requires `--name <package>` or `--path <file-or-dir>`",
        ),
    ];

    for (args, expected_error) in cases {
        let mut command = ql_command(&workspace_root);
        command.args(args);
        let output = run_command_capture(&mut command, "`ql project add-dependency` invalid args");
        let (stdout, stderr) = expect_exit_code(
            "project-add-dependency-cli-invalid",
            "`ql project add-dependency` invalid args",
            &output,
            1,
        )
        .expect("project add-dependency should reject invalid arguments");
        expect_empty_stdout(
            "project-add-dependency-cli-invalid",
            "`ql project add-dependency` invalid args",
            &stdout,
        )
        .expect("project add-dependency invalid args should keep stdout empty");
        expect_stderr_contains(
            "project-add-dependency-cli-invalid",
            "`ql project add-dependency` invalid args",
            &stderr,
            expected_error,
        )
        .expect("project add-dependency invalid args should report the parser error");
    }
}

#[test]
fn project_remove_dependency_cli_rejects_invalid_arguments() {
    let workspace_root = workspace_root();
    let temp = write_app_core_workspace("ql-project-remove-dependency-cli-invalid");
    let path = temp.path().display().to_string();

    let cases = [
        (
            vec![
                "project".to_owned(),
                "remove-dependency".to_owned(),
                path.clone(),
                "--package".to_owned(),
            ],
            "error: `ql project remove-dependency --package` expects a package name",
        ),
        (
            vec![
                "project".to_owned(),
                "remove-dependency".to_owned(),
                path.clone(),
                "--package".to_owned(),
                "app".to_owned(),
                "--package".to_owned(),
                "core".to_owned(),
            ],
            "error: `ql project remove-dependency` received `--package` more than once",
        ),
        (
            vec![
                "project".to_owned(),
                "remove-dependency".to_owned(),
                path.clone(),
                "--name".to_owned(),
            ],
            "error: `ql project remove-dependency --name` expects a package name",
        ),
        (
            vec![
                "project".to_owned(),
                "remove-dependency".to_owned(),
                path.clone(),
                "--name".to_owned(),
                "core".to_owned(),
                "--name".to_owned(),
                "util".to_owned(),
            ],
            "error: `ql project remove-dependency` received `--name` more than once",
        ),
        (
            vec![
                "project".to_owned(),
                "remove-dependency".to_owned(),
                path.clone(),
                "--unknown".to_owned(),
            ],
            "error: unknown `ql project remove-dependency` option `--unknown`",
        ),
        (
            vec![
                "project".to_owned(),
                "remove-dependency".to_owned(),
                path.clone(),
                "extra".to_owned(),
                "--name".to_owned(),
                "core".to_owned(),
            ],
            "error: unknown `ql project remove-dependency` argument `extra`",
        ),
        (
            vec![
                "project".to_owned(),
                "remove-dependency".to_owned(),
                path.clone(),
            ],
            "error: `ql project remove-dependency` requires `--name <package>`",
        ),
        (
            vec![
                "project".to_owned(),
                "remove-dependency".to_owned(),
                path,
                "--all".to_owned(),
                "--package".to_owned(),
                "app".to_owned(),
                "--name".to_owned(),
                "core".to_owned(),
            ],
            "error: `ql project remove-dependency --all` does not accept `--package`; bulk cleanup already targets all dependents of `--name`",
        ),
    ];

    for (args, expected_error) in cases {
        let mut command = ql_command(&workspace_root);
        command.args(args);
        let output =
            run_command_capture(&mut command, "`ql project remove-dependency` invalid args");
        let (stdout, stderr) = expect_exit_code(
            "project-remove-dependency-cli-invalid",
            "`ql project remove-dependency` invalid args",
            &output,
            1,
        )
        .expect("project remove-dependency should reject invalid arguments");
        expect_empty_stdout(
            "project-remove-dependency-cli-invalid",
            "`ql project remove-dependency` invalid args",
            &stdout,
        )
        .expect("project remove-dependency invalid args should keep stdout empty");
        expect_stderr_contains(
            "project-remove-dependency-cli-invalid",
            "`ql project remove-dependency` invalid args",
            &stderr,
            expected_error,
        )
        .expect("project remove-dependency invalid args should report the parser error");
    }
}
