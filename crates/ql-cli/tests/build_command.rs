mod support;

use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_file_exists,
    expect_stderr_contains, expect_stdout_contains_all, expect_success, ql_command,
    run_command_capture, workspace_root,
};

#[test]
fn build_command_emits_single_source_llvm_ir() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-command-success");
    let source = temp.write(
        "src/main.ql",
        r#"
fn main() -> Int {
    return 0
}
"#,
    );
    let output_path = temp.path().join("artifacts/main.ll");

    let mut command = ql_command(&workspace_root);
    command
        .arg("build")
        .arg(&source)
        .args(["--emit", "llvm-ir"])
        .arg("-o")
        .arg(&output_path);
    let output = run_command_capture(&mut command, "`ql build --emit llvm-ir` single source");
    let (stdout, stderr) = expect_success(
        "build-command-single-source-llvm-ir",
        "`ql build --emit llvm-ir` single source",
        &output,
    )
    .expect("single-source llvm-ir build should succeed");
    expect_stdout_contains_all(
        "build-command-single-source-llvm-ir",
        &stdout,
        &["wrote llvm-ir:", "main.ll"],
    )
    .expect("single-source llvm-ir build should report the artifact");
    expect_empty_stderr(
        "build-command-single-source-llvm-ir",
        "`ql build --emit llvm-ir` single source",
        &stderr,
    )
    .expect("single-source llvm-ir build should keep stderr empty");
    expect_file_exists(
        "build-command-single-source-llvm-ir",
        &output_path,
        "llvm-ir artifact",
        "single-source llvm-ir build",
    )
    .expect("single-source llvm-ir build should write the requested artifact");
}

#[test]
fn build_command_rejects_invalid_arguments() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-command-args");
    let source = temp.write(
        "src/main.ql",
        r#"
fn main() -> Int {
    return 0
}
"#,
    );

    let cases = [
        (
            vec!["build", source.to_str().unwrap(), "--emit"],
            "error: `ql build --emit` expects a value",
        ),
        (
            vec!["build", source.to_str().unwrap(), "--emit", "bytecode"],
            "error: unsupported build emit target `bytecode`",
        ),
        (
            vec!["build", source.to_str().unwrap(), "--unknown"],
            "error: unknown `ql build` option `--unknown`",
        ),
    ];

    for (args, expected_error) in cases {
        let mut command = ql_command(&workspace_root);
        command.args(args);
        let output = run_command_capture(&mut command, "`ql build` with invalid arguments");
        let (stdout, stderr) = expect_exit_code(
            "build-command-argument-validation",
            "`ql build` with invalid arguments",
            &output,
            1,
        )
        .expect("invalid build arguments should fail");
        expect_empty_stdout(
            "build-command-argument-validation",
            "`ql build` with invalid arguments",
            &stdout,
        )
        .expect("invalid build arguments should not print stdout");
        expect_stderr_contains(
            "build-command-argument-validation",
            "`ql build` with invalid arguments",
            &stderr,
            expected_error,
        )
        .expect("invalid build arguments should identify the rejected input");
    }
}
