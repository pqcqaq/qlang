mod support;

use support::{
    TempDir, executable_output_path, expect_empty_stderr, expect_empty_stdout, expect_exit_code,
    expect_stderr_contains, expect_stdout_contains_all, expect_success, ql_command,
    run_command_capture, workspace_root,
};

fn write_test_package(prefix: &str) -> TempDir {
    let temp = TempDir::new(prefix);
    temp.write(
        "qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("tests/smoke.ql", "fn main() -> Int { return 0 }\n");
    temp.write("tests/api/extra.ql", "fn main() -> Int { return 0 }\n");
    temp
}

#[test]
fn test_command_lists_selected_package_tests_without_toolchain() {
    let workspace_root = workspace_root();
    let temp = write_test_package("ql-test-command-list");
    let smoke_output = executable_output_path(&temp.path().join("target/ql/debug/tests"), "smoke");

    let mut command = ql_command(&workspace_root);
    command.arg("test").arg(temp.path()).args([
        "--list",
        "--filter",
        "smoke",
        "--target",
        "tests/smoke.ql",
    ]);
    let output = run_command_capture(&mut command, "`ql test --list` selected package tests");
    let (stdout, stderr) = expect_success(
        "test-command-list-selected-package-tests",
        "`ql test --list` selected package tests",
        &output,
    )
    .expect("selected package test listing should succeed without invoking the native toolchain");
    let normalized_stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        "test-command-list-selected-package-tests",
        &normalized_stdout,
        &["tests/smoke.ql"],
    )
    .expect("selected package test listing should include the selected smoke test");
    expect_empty_stderr(
        "test-command-list-selected-package-tests",
        "`ql test --list` selected package tests",
        &stderr,
    )
    .expect("selected package test listing should keep stderr empty");
    assert!(
        !normalized_stdout.contains("tests/api/extra.ql"),
        "selected package test listing should exclude filtered tests: {stdout}"
    );
    assert!(
        !smoke_output.exists(),
        "test listing should not build `{}`",
        smoke_output.display()
    );
}

#[test]
fn test_command_rejects_invalid_arguments() {
    let workspace_root = workspace_root();
    let temp = write_test_package("ql-test-command-invalid-args");

    let cases = [
        (
            vec![
                "test".to_owned(),
                temp.path().display().to_string(),
                "--unknown".to_owned(),
            ],
            "error: unknown `ql test` option `--unknown`",
            None,
        ),
        (
            vec![
                "test".to_owned(),
                temp.path().display().to_string(),
                "other.ql".to_owned(),
            ],
            "error: unknown `ql test` argument `other.ql`",
            None,
        ),
        (
            vec!["test".to_owned()],
            "error: `ql test` expects a file or directory path",
            None,
        ),
        (
            vec![
                "test".to_owned(),
                temp.path().display().to_string(),
                "--profile".to_owned(),
            ],
            "error: `ql test --profile` expects `debug` or `release`",
            None,
        ),
        (
            vec![
                "test".to_owned(),
                temp.path().display().to_string(),
                "--profile".to_owned(),
                "fast".to_owned(),
            ],
            "error: `ql test` unsupported profile `fast`",
            Some("hint: supported profiles are `debug` and `release`"),
        ),
        (
            vec![
                "test".to_owned(),
                temp.path().display().to_string(),
                "--release".to_owned(),
                "--profile".to_owned(),
                "debug".to_owned(),
            ],
            "error: `ql test` received multiple profile selectors",
            None,
        ),
        (
            vec![
                "test".to_owned(),
                temp.path().display().to_string(),
                "--package".to_owned(),
            ],
            "error: `ql test --package` expects a package name",
            None,
        ),
        (
            vec![
                "test".to_owned(),
                temp.path().display().to_string(),
                "--package".to_owned(),
                "app".to_owned(),
                "--package".to_owned(),
                "tool".to_owned(),
            ],
            "error: `ql test` received multiple `--package` selectors",
            None,
        ),
        (
            vec![
                "test".to_owned(),
                temp.path().display().to_string(),
                "--filter".to_owned(),
            ],
            "error: `ql test --filter` expects a substring",
            None,
        ),
        (
            vec![
                "test".to_owned(),
                temp.path().display().to_string(),
                "--target".to_owned(),
            ],
            "error: `ql test --target` expects a test path",
            None,
        ),
        (
            vec![
                "test".to_owned(),
                temp.path().display().to_string(),
                "--target".to_owned(),
                "tests/smoke.ql".to_owned(),
                "--target".to_owned(),
                "tests/api/extra.ql".to_owned(),
            ],
            "error: `ql test` received multiple `--target` selectors",
            None,
        ),
    ];

    for (args, expected_error, expected_hint) in cases {
        let mut command = ql_command(&workspace_root);
        command.args(args);
        let output = run_command_capture(&mut command, "`ql test` with invalid arguments");
        let (stdout, stderr) = expect_exit_code(
            "test-command-argument-validation",
            "`ql test` with invalid arguments",
            &output,
            1,
        )
        .expect("invalid test arguments should fail");
        expect_empty_stdout(
            "test-command-argument-validation",
            "`ql test` with invalid arguments",
            &stdout,
        )
        .expect("invalid test arguments should not print stdout");
        expect_stderr_contains(
            "test-command-argument-validation",
            "`ql test` with invalid arguments",
            &stderr,
            expected_error,
        )
        .expect("invalid test arguments should identify the rejected input");
        if let Some(expected_hint) = expected_hint {
            expect_stderr_contains(
                "test-command-argument-validation",
                "`ql test` with invalid arguments",
                &stderr,
                expected_hint,
            )
            .expect("invalid test arguments should include the expected hint");
        }
    }
}
