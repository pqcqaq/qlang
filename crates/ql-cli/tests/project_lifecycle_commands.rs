mod support;

use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_file_exists,
    expect_stderr_contains, expect_stdout_contains_all, expect_success, ql_command,
    read_normalized_file, run_command_capture, workspace_root,
};

#[test]
fn project_lifecycle_commands_use_current_directory_by_default() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-lifecycle-default-cwd");

    let mut init = ql_command(&workspace_root);
    init.current_dir(temp.path());
    init.args(["project", "init", "--workspace", "--name", "app"]);
    let output = run_command_capture(&mut init, "`ql project init` default cwd");
    let (stdout, stderr) = expect_success(
        "project-lifecycle-default-cwd",
        "`ql project init` default cwd",
        &output,
    )
    .expect("project init should use the current directory by default");
    expect_empty_stderr(
        "project-lifecycle-default-cwd",
        "`ql project init` default cwd",
        &stderr,
    )
    .expect("project init default cwd should keep stderr empty");
    expect_stdout_contains_all(
        "project-lifecycle-default-cwd",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "created: {}",
                temp.path()
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                temp.path()
                    .join("packages/app/qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .expect("project init default cwd should report created workspace files");
    expect_file_exists(
        "project-lifecycle-default-cwd",
        &temp.path().join("qlang.toml"),
        "workspace manifest",
        "`ql project init` default cwd",
    )
    .expect("project init default cwd should create the workspace manifest");

    let mut add = ql_command(&workspace_root);
    add.current_dir(temp.path());
    add.args(["project", "add", "--name", "tools", "--dependency", "app"]);
    let output = run_command_capture(&mut add, "`ql project add` default cwd");
    let (stdout, stderr) = expect_success(
        "project-lifecycle-default-cwd",
        "`ql project add` default cwd",
        &output,
    )
    .expect("project add should use the current directory by default");
    expect_empty_stderr(
        "project-lifecycle-default-cwd",
        "`ql project add` default cwd",
        &stderr,
    )
    .expect("project add default cwd should keep stderr empty");
    expect_stdout_contains_all(
        "project-lifecycle-default-cwd",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "updated: {}",
                temp.path()
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                temp.path()
                    .join("packages/tools/qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .expect("project add default cwd should report updated and created files");
    let tools_manifest = read_normalized_file(
        &temp.path().join("packages/tools/qlang.toml"),
        "tools manifest",
    );
    assert!(
        tools_manifest.contains("[dependencies]\napp = \"../app\"\n"),
        "project add default cwd should write requested local dependency, got:\n{tools_manifest}"
    );

    let mut remove = ql_command(&workspace_root);
    remove.current_dir(temp.path());
    remove.args(["project", "remove", "--name", "tools"]);
    let output = run_command_capture(&mut remove, "`ql project remove` default cwd");
    let (stdout, stderr) = expect_success(
        "project-lifecycle-default-cwd",
        "`ql project remove` default cwd",
        &output,
    )
    .expect("project remove should use the current directory by default");
    expect_empty_stderr(
        "project-lifecycle-default-cwd",
        "`ql project remove` default cwd",
        &stderr,
    )
    .expect("project remove default cwd should keep stderr empty");
    expect_stdout_contains_all(
        "project-lifecycle-default-cwd",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "updated: {}",
                temp.path()
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "removed: {}",
                temp.path()
                    .join("packages/tools")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .expect("project remove default cwd should report updated workspace and removed member");
    let workspace_manifest =
        read_normalized_file(&temp.path().join("qlang.toml"), "workspace manifest");
    assert!(
        workspace_manifest.contains("packages/app") && !workspace_manifest.contains("tools"),
        "project remove default cwd should remove tools from workspace manifest, got:\n{workspace_manifest}"
    );
}

#[test]
fn project_init_cli_rejects_invalid_arguments() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-init-cli-invalid");
    let path = temp.path().display().to_string();

    let cases = [
        (
            vec![
                "project".to_owned(),
                "init".to_owned(),
                path.clone(),
                "--name".to_owned(),
            ],
            "error: `ql project init --name` expects a package name",
        ),
        (
            vec![
                "project".to_owned(),
                "init".to_owned(),
                path.clone(),
                "--name".to_owned(),
                "app".to_owned(),
                "--name".to_owned(),
                "core".to_owned(),
            ],
            "error: `ql project init` received `--name` more than once",
        ),
        (
            vec![
                "project".to_owned(),
                "init".to_owned(),
                path.clone(),
                "--stdlib".to_owned(),
            ],
            "error: `ql project init --stdlib` expects a stdlib workspace path",
        ),
        (
            vec![
                "project".to_owned(),
                "init".to_owned(),
                path.clone(),
                "--unknown".to_owned(),
            ],
            "error: unknown `ql project init` option `--unknown`",
        ),
        (
            vec![
                "project".to_owned(),
                "init".to_owned(),
                path,
                "extra".to_owned(),
            ],
            "error: unknown `ql project init` argument `extra`",
        ),
    ];

    for (args, expected_error) in cases {
        let mut command = ql_command(&workspace_root);
        command.args(args);
        let output = run_command_capture(&mut command, "`ql project init` invalid args");
        let (stdout, stderr) = expect_exit_code(
            "project-init-cli-invalid",
            "`ql project init` invalid args",
            &output,
            1,
        )
        .expect("project init should reject invalid arguments");
        expect_empty_stdout(
            "project-init-cli-invalid",
            "`ql project init` invalid args",
            &stdout,
        )
        .expect("project init invalid args should keep stdout empty");
        expect_stderr_contains(
            "project-init-cli-invalid",
            "`ql project init` invalid args",
            &stderr,
            expected_error,
        )
        .expect("project init invalid args should report the parser error");
    }
}

#[test]
fn project_add_cli_rejects_invalid_arguments() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-add-cli-invalid");
    let workspace = temp.path().join("workspace");
    std::fs::create_dir_all(&workspace).expect("create workspace root for add invalid tests");
    temp.write("workspace/qlang.toml", "[workspace]\nmembers = []\n");
    let path = workspace.display().to_string();

    let cases = [
        (
            vec![
                "project".to_owned(),
                "add".to_owned(),
                path.clone(),
                "--name".to_owned(),
            ],
            "error: `ql project add --name` expects a package name",
        ),
        (
            vec![
                "project".to_owned(),
                "add".to_owned(),
                path.clone(),
                "--name".to_owned(),
                "app".to_owned(),
                "--name".to_owned(),
                "core".to_owned(),
            ],
            "error: `ql project add` received `--name` more than once",
        ),
        (
            vec![
                "project".to_owned(),
                "add".to_owned(),
                path.clone(),
                "--existing".to_owned(),
            ],
            "error: `ql project add --existing` expects a file or directory",
        ),
        (
            vec![
                "project".to_owned(),
                "add".to_owned(),
                path.clone(),
                "--dependency".to_owned(),
            ],
            "error: `ql project add --dependency` expects a package name",
        ),
        (
            vec![
                "project".to_owned(),
                "add".to_owned(),
                path.clone(),
                "--unknown".to_owned(),
            ],
            "error: unknown `ql project add` option `--unknown`",
        ),
        (
            vec![
                "project".to_owned(),
                "add".to_owned(),
                path.clone(),
                "extra".to_owned(),
            ],
            "error: unknown `ql project add` argument `extra`",
        ),
        (
            vec!["project".to_owned(), "add".to_owned(), path],
            "error: `ql project add` requires `--name <package>`",
        ),
    ];

    for (args, expected_error) in cases {
        let mut command = ql_command(&workspace_root);
        command.args(args);
        let output = run_command_capture(&mut command, "`ql project add` invalid args");
        let (stdout, stderr) = expect_exit_code(
            "project-add-cli-invalid",
            "`ql project add` invalid args",
            &output,
            1,
        )
        .expect("project add should reject invalid arguments");
        expect_empty_stdout(
            "project-add-cli-invalid",
            "`ql project add` invalid args",
            &stdout,
        )
        .expect("project add invalid args should keep stdout empty");
        expect_stderr_contains(
            "project-add-cli-invalid",
            "`ql project add` invalid args",
            &stderr,
            expected_error,
        )
        .expect("project add invalid args should report the parser error");
    }
}

#[test]
fn project_remove_cli_rejects_invalid_arguments() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-remove-cli-invalid");
    let workspace = temp.path().join("workspace");
    std::fs::create_dir_all(&workspace).expect("create workspace root for remove invalid tests");
    temp.write("workspace/qlang.toml", "[workspace]\nmembers = []\n");
    let path = workspace.display().to_string();

    let cases = [
        (
            vec![
                "project".to_owned(),
                "remove".to_owned(),
                path.clone(),
                "--name".to_owned(),
            ],
            "error: `ql project remove --name` expects a package name",
        ),
        (
            vec![
                "project".to_owned(),
                "remove".to_owned(),
                path.clone(),
                "--name".to_owned(),
                "app".to_owned(),
                "--name".to_owned(),
                "core".to_owned(),
            ],
            "error: `ql project remove` received `--name` more than once",
        ),
        (
            vec![
                "project".to_owned(),
                "remove".to_owned(),
                path.clone(),
                "--unknown".to_owned(),
            ],
            "error: unknown `ql project remove` option `--unknown`",
        ),
        (
            vec![
                "project".to_owned(),
                "remove".to_owned(),
                path.clone(),
                "extra".to_owned(),
                "--name".to_owned(),
                "app".to_owned(),
            ],
            "error: unknown `ql project remove` argument `extra`",
        ),
        (
            vec!["project".to_owned(), "remove".to_owned(), path],
            "error: `ql project remove` requires `--name <package>`",
        ),
    ];

    for (args, expected_error) in cases {
        let mut command = ql_command(&workspace_root);
        command.args(args);
        let output = run_command_capture(&mut command, "`ql project remove` invalid args");
        let (stdout, stderr) = expect_exit_code(
            "project-remove-cli-invalid",
            "`ql project remove` invalid args",
            &output,
            1,
        )
        .expect("project remove should reject invalid arguments");
        expect_empty_stdout(
            "project-remove-cli-invalid",
            "`ql project remove` invalid args",
            &stdout,
        )
        .expect("project remove invalid args should keep stdout empty");
        expect_stderr_contains(
            "project-remove-cli-invalid",
            "`ql project remove` invalid args",
            &stderr,
            expected_error,
        )
        .expect("project remove invalid args should report the parser error");
    }
}
