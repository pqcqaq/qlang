mod support;

use support::{
    TempDir, expect_empty_stdout, expect_exit_code, expect_stderr_contains,
    expect_stdout_contains_all, expect_success, ql_command, run_command_capture, workspace_root,
};

#[test]
fn analysis_commands_reject_unexpected_extra_arguments() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-analysis-command-args");
    let source = temp.write(
        "sample.ql",
        r#"
fn main() -> Int {
    return 0
}
"#,
    );

    for command_name in ["mir", "ownership", "runtime"] {
        let mut command = ql_command(&workspace_root);
        command.arg(command_name).arg(&source).arg("unexpected");
        let output = run_command_capture(
            &mut command,
            format!("`ql {command_name}` with unexpected extra argument"),
        );
        let (stdout, stderr) = expect_exit_code(
            "analysis-command-extra-argument",
            &format!("`ql {command_name}` with unexpected extra argument"),
            &output,
            1,
        )
        .unwrap_or_else(|error| panic!("{error}"));
        expect_empty_stdout(
            "analysis-command-extra-argument",
            &format!("failing `ql {command_name}`"),
            &stdout,
        )
        .unwrap_or_else(|error| panic!("{error}"));
        expect_stderr_contains(
            "analysis-command-extra-argument",
            &format!("failing `ql {command_name}`"),
            &stderr,
            &format!("error: unknown `ql {command_name}` argument `unexpected`"),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    }
}

#[test]
fn analysis_commands_accept_single_source_argument() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-analysis-command-success");
    let source = temp.write(
        "sample.ql",
        r#"
fn main() -> Int {
    return 0
}
"#,
    );

    let expectations = [
        ("mir", "body 0 main"),
        ("ownership", "ownership main"),
        ("runtime", "runtime requirements: none"),
    ];

    for (command_name, stdout_fragment) in expectations {
        let mut command = ql_command(&workspace_root);
        command.arg(command_name).arg(&source);
        let output = run_command_capture(&mut command, format!("`ql {command_name}`"));
        let (stdout, _) = expect_success(
            "analysis-command-single-source",
            &format!("`ql {command_name}`"),
            &output,
        )
        .unwrap_or_else(|error| panic!("{error}"));
        expect_stdout_contains_all(
            "analysis-command-single-source",
            &stdout,
            &[stdout_fragment],
        )
        .unwrap_or_else(|error| panic!("{error}"));
    }
}
