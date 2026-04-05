mod support;

use support::{
    TempDir, expect_empty_stderr, expect_file_exists, expect_stdout_contains_all, expect_success,
    run_ql_build_capture, workspace_root,
};

#[test]
fn builds_object_for_program_using_string_literals() {
    let workspace_root = workspace_root();
    let temp_dir = TempDir::new("ql-string-codegen");
    let output_path = temp_dir.path().join("string_value_build.obj");

    let output = run_ql_build_capture(
        &workspace_root,
        "fixtures/codegen/pass/string_value_build.ql",
        "obj",
        &output_path,
        &[],
    );

    let (stdout, stderr) =
        expect_success("string_value_build_object", "string object build", &output)
            .expect("string object build should succeed");
    expect_stdout_contains_all("string_value_build_object", &stdout, &["wrote object:"])
        .expect("successful string object build should report the emitted object artifact");
    expect_empty_stderr("string_value_build_object", "string object build", &stderr)
        .expect("successful string object build should stay silent on stderr");
    expect_file_exists(
        "string_value_build_object",
        &output_path,
        "object artifact",
        "string object build",
    )
    .expect("string object build should produce an object artifact");
}
