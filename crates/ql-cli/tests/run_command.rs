mod support;

use support::{
    TempDir, executable_output_path, expect_empty_stderr, expect_empty_stdout, expect_exit_code,
    expect_stderr_contains, expect_stdout_contains_all, expect_success, ql_command,
    run_command_capture, workspace_root,
};

fn write_runnable_package(prefix: &str) -> TempDir {
    let temp = TempDir::new(prefix);
    temp.write(
        "qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "src/main.ql",
        r#"
fn main() -> Int {
    return 0
}
"#,
    );
    temp
}

#[test]
fn run_command_lists_package_targets_without_toolchain() {
    let workspace_root = workspace_root();
    let temp = write_runnable_package("ql-run-command-list");
    let executable_path = executable_output_path(&temp.path().join("target/ql/debug"), "main");

    let mut command = ql_command(&workspace_root);
    command
        .arg("run")
        .arg(temp.path())
        .arg("--list")
        .arg("--")
        .args(["--unknown", "other.ql"]);
    let output = run_command_capture(&mut command, "`ql run --list` package path");
    let (stdout, stderr) = expect_success(
        "run-command-list-package-targets",
        "`ql run --list` package path",
        &output,
    )
    .expect("package target listing should succeed without invoking the native toolchain");
    expect_stdout_contains_all(
        "run-command-list-package-targets",
        &stdout,
        &["package: app", "bin: src/main.ql"],
    )
    .expect("package target listing should include the runnable binary");
    expect_empty_stderr(
        "run-command-list-package-targets",
        "`ql run --list` package path",
        &stderr,
    )
    .expect("package target listing should keep stderr empty");
    assert!(
        !executable_path.exists(),
        "target listing should not build `{}`",
        executable_path.display()
    );
}

#[test]
fn run_command_rejects_invalid_arguments() {
    let workspace_root = workspace_root();
    let temp = write_runnable_package("ql-run-command-invalid-args");

    let cases = [
        (
            vec![
                "run".to_owned(),
                temp.path().display().to_string(),
                "--unknown".to_owned(),
            ],
            "error: unknown `ql run` option `--unknown`",
            None,
        ),
        (
            vec![
                "run".to_owned(),
                temp.path().display().to_string(),
                "other.ql".to_owned(),
            ],
            "error: unknown `ql run` argument `other.ql`",
            Some("hint: use `ql run <file-or-dir> -- <args...>`"),
        ),
        (
            vec!["run".to_owned()],
            "error: `ql run` expects a file or directory path",
            None,
        ),
        (
            vec![
                "run".to_owned(),
                temp.path().display().to_string(),
                "--profile".to_owned(),
            ],
            "error: `ql run --profile` expects `debug` or `release`",
            None,
        ),
        (
            vec![
                "run".to_owned(),
                temp.path().display().to_string(),
                "--profile".to_owned(),
                "fast".to_owned(),
            ],
            "error: `ql run` unsupported profile `fast`",
            Some("hint: supported profiles are `debug` and `release`"),
        ),
        (
            vec![
                "run".to_owned(),
                temp.path().display().to_string(),
                "--release".to_owned(),
                "--profile".to_owned(),
                "debug".to_owned(),
            ],
            "error: `ql run` received multiple profile selectors",
            None,
        ),
        (
            vec![
                "run".to_owned(),
                temp.path().display().to_string(),
                "--package".to_owned(),
            ],
            "error: `ql run` --package expects a package name",
            None,
        ),
        (
            vec![
                "run".to_owned(),
                temp.path().display().to_string(),
                "--bin".to_owned(),
            ],
            "error: `ql run` --bin expects a target name",
            None,
        ),
        (
            vec![
                "run".to_owned(),
                temp.path().display().to_string(),
                "--lib".to_owned(),
                "--bin".to_owned(),
                "main".to_owned(),
            ],
            "error: `ql run` does not support combining `--lib`, `--bin`, and `--target`",
            None,
        ),
    ];

    for (args, expected_error, expected_hint) in cases {
        let mut command = ql_command(&workspace_root);
        command.args(args);
        let output = run_command_capture(&mut command, "`ql run` with invalid arguments");
        let (stdout, stderr) = expect_exit_code(
            "run-command-argument-validation",
            "`ql run` with invalid arguments",
            &output,
            1,
        )
        .expect("invalid run arguments should fail");
        expect_empty_stdout(
            "run-command-argument-validation",
            "`ql run` with invalid arguments",
            &stdout,
        )
        .expect("invalid run arguments should not print stdout");
        expect_stderr_contains(
            "run-command-argument-validation",
            "`ql run` with invalid arguments",
            &stderr,
            expected_error,
        )
        .expect("invalid run arguments should identify the rejected input");
        if let Some(expected_hint) = expected_hint {
            expect_stderr_contains(
                "run-command-argument-validation",
                "`ql run` with invalid arguments",
                &stderr,
                expected_hint,
            )
            .expect("invalid extra run argument should explain passthrough syntax");
        }
    }
}
