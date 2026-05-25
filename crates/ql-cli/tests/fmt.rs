mod support;

use std::process::Stdio;

use support::{
    TempDir, assert_no_atomic_write_temp_files, assert_no_build_lock_directories,
    expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_stderr_contains,
    expect_success, ql_command, read_normalized_file, run_command_capture, workspace_root,
};

#[test]
fn fmt_write_serializes_concurrent_source_writes() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-fmt-concurrent-write");
    let source_path = temp.write(
        "src/main.ql",
        r#"
fn main()->Int{
let value=1+2
return value
}
"#,
    );

    let mut expected_command = ql_command(&workspace_root);
    expected_command.arg("fmt").arg(&source_path);
    let expected_output = run_command_capture(&mut expected_command, "`ql fmt` expected output");
    let (expected, expected_stderr) = expect_success(
        "fmt-write-concurrent",
        "`ql fmt` expected output",
        &expected_output,
    )
    .expect("expected formatting command should succeed");
    expect_empty_stderr(
        "fmt-write-concurrent",
        "`ql fmt` expected output",
        &expected_stderr,
    )
    .expect("expected formatting command should keep stderr empty");

    let mut children = Vec::new();
    for index in 0..6 {
        let mut command = ql_command(&workspace_root);
        command.arg("fmt").arg(&source_path).arg("--write");
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let child = command
            .spawn()
            .unwrap_or_else(|error| panic!("spawn concurrent ql fmt --write #{index}: {error}"));
        children.push((index, child));
    }

    for (index, child) in children {
        let output = child
            .wait_with_output()
            .unwrap_or_else(|error| panic!("wait for concurrent ql fmt --write #{index}: {error}"));
        let (stdout, stderr) = expect_success(
            "fmt-write-concurrent",
            &format!("concurrent ql fmt --write #{index}"),
            &output,
        )
        .unwrap_or_else(|message| panic!("{message}"));
        expect_empty_stdout(
            "fmt-write-concurrent",
            &format!("concurrent ql fmt --write #{index}"),
            &stdout,
        )
        .unwrap_or_else(|message| panic!("{message}"));
        expect_empty_stderr(
            "fmt-write-concurrent",
            &format!("concurrent ql fmt --write #{index}"),
            &stderr,
        )
        .unwrap_or_else(|message| panic!("{message}"));
    }

    let actual = read_normalized_file(&source_path, "concurrently formatted source");
    assert_eq!(actual, expected);
    assert_no_build_lock_directories("fmt-write-concurrent", temp.path());
    assert_no_atomic_write_temp_files("fmt-write-concurrent", temp.path());
}

#[test]
fn fmt_rejects_unexpected_arguments_and_options() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-fmt-args");
    let source_path = temp.write(
        "src/main.ql",
        r#"
fn main()->Int{
return 0
}
"#,
    );

    let mut extra_arg_command = ql_command(&workspace_root);
    extra_arg_command
        .arg("fmt")
        .arg(&source_path)
        .arg("unexpected.ql");
    let extra_arg_output = run_command_capture(
        &mut extra_arg_command,
        "`ql fmt` with unexpected extra argument",
    );
    let (stdout, stderr) = expect_exit_code(
        "fmt-argument-validation",
        "`ql fmt` with unexpected extra argument",
        &extra_arg_output,
        1,
    )
    .expect("extra argument should fail");
    expect_empty_stdout(
        "fmt-argument-validation",
        "`ql fmt` with unexpected extra argument",
        &stdout,
    )
    .expect("extra argument failure should not print stdout");
    expect_stderr_contains(
        "fmt-argument-validation",
        "`ql fmt` with unexpected extra argument",
        &stderr,
        "error: unknown `ql fmt` argument `unexpected.ql`",
    )
    .expect("extra argument failure should identify the rejected argument");

    let mut unknown_option_command = ql_command(&workspace_root);
    unknown_option_command
        .arg("fmt")
        .arg(&source_path)
        .arg("--unknown");
    let unknown_option_output =
        run_command_capture(&mut unknown_option_command, "`ql fmt` with unknown option");
    let (stdout, stderr) = expect_exit_code(
        "fmt-argument-validation",
        "`ql fmt` with unknown option",
        &unknown_option_output,
        1,
    )
    .expect("unknown option should fail");
    expect_empty_stdout(
        "fmt-argument-validation",
        "`ql fmt` with unknown option",
        &stdout,
    )
    .expect("unknown option failure should not print stdout");
    expect_stderr_contains(
        "fmt-argument-validation",
        "`ql fmt` with unknown option",
        &stderr,
        "error: unknown `ql fmt` option `--unknown`",
    )
    .expect("unknown option failure should identify the rejected option");
}
