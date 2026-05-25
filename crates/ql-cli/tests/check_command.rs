mod support;

use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_stderr_contains,
    expect_stdout_contains_all, expect_success, ql_command, run_command_capture, workspace_root,
};

#[test]
fn check_accepts_single_source_file_argument() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-check-command-success");
    let source = temp.write(
        "src/main.ql",
        r#"
fn main() -> Int {
    return 0
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.arg("check").arg(&source);
    let output = run_command_capture(&mut command, "`ql check` single source file");
    let (stdout, stderr) = expect_success(
        "check-command-single-source",
        "`ql check` single source file",
        &output,
    )
    .expect("single-source ql check should succeed");
    expect_stdout_contains_all("check-command-single-source", &stdout, &["ok:", "main.ql"])
        .expect("single-source ql check should report the checked source");
    expect_empty_stderr(
        "check-command-single-source",
        "`ql check` single source file",
        &stderr,
    )
    .expect("single-source ql check should keep stderr empty");
}

#[test]
fn check_rejects_unexpected_arguments_and_options() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-check-command-args");
    let source = temp.write(
        "src/main.ql",
        r#"
fn main() -> Int {
    return 0
}
"#,
    );

    let mut extra_arg_command = ql_command(&workspace_root);
    extra_arg_command
        .arg("check")
        .arg(&source)
        .arg("unexpected.ql");
    let extra_arg_output = run_command_capture(
        &mut extra_arg_command,
        "`ql check` with unexpected extra argument",
    );
    let (stdout, stderr) = expect_exit_code(
        "check-command-argument-validation",
        "`ql check` with unexpected extra argument",
        &extra_arg_output,
        1,
    )
    .expect("extra argument should fail");
    expect_empty_stdout(
        "check-command-argument-validation",
        "`ql check` with unexpected extra argument",
        &stdout,
    )
    .expect("extra argument failure should not print stdout");
    expect_stderr_contains(
        "check-command-argument-validation",
        "`ql check` with unexpected extra argument",
        &stderr,
        "error: unknown `ql check` argument `unexpected.ql`",
    )
    .expect("extra argument failure should identify the rejected argument");

    let mut unknown_option_command = ql_command(&workspace_root);
    unknown_option_command
        .arg("check")
        .arg(&source)
        .arg("--unknown");
    let unknown_option_output = run_command_capture(
        &mut unknown_option_command,
        "`ql check` with unknown option",
    );
    let (stdout, stderr) = expect_exit_code(
        "check-command-argument-validation",
        "`ql check` with unknown option",
        &unknown_option_output,
        1,
    )
    .expect("unknown option should fail");
    expect_empty_stdout(
        "check-command-argument-validation",
        "`ql check` with unknown option",
        &stdout,
    )
    .expect("unknown option failure should not print stdout");
    expect_stderr_contains(
        "check-command-argument-validation",
        "`ql check` with unknown option",
        &stderr,
        "error: unknown `ql check` option `--unknown`",
    )
    .expect("unknown option failure should identify the rejected option");
}
