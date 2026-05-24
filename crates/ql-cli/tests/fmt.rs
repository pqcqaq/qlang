mod support;

use std::process::Stdio;

use support::{
    TempDir, assert_no_build_lock_directories, expect_empty_stderr, expect_empty_stdout,
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
}
