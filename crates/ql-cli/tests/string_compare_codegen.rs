mod support;

use support::{
    TempDir, expect_empty_stderr, expect_file_exists, expect_stdout_contains_all, expect_success,
    run_ql_build_capture, workspace_root,
};

#[test]
fn builds_object_for_program_using_string_equality() {
    let workspace_root = workspace_root();
    let temp_dir = TempDir::new("ql-string-compare-codegen");
    let output_path = temp_dir.path().join("string_compare_build.obj");

    let output = run_ql_build_capture(
        &workspace_root,
        "fixtures/codegen/pass/string_compare_build.ql",
        "obj",
        &output_path,
        &[],
    );

    let (stdout, stderr) = expect_success(
        "string_compare_build_object",
        "string compare object build",
        &output,
    )
    .expect("string compare object build should succeed");
    expect_stdout_contains_all("string_compare_build_object", &stdout, &["wrote object:"])
        .expect("successful string compare object build should report the emitted object artifact");
    expect_empty_stderr(
        "string_compare_build_object",
        "string compare object build",
        &stderr,
    )
    .expect("successful string compare object build should stay silent on stderr");
    expect_file_exists(
        "string_compare_build_object",
        &output_path,
        "object artifact",
        "string compare object build",
    )
    .expect("string compare object build should produce an object artifact");
}

#[test]
fn builds_object_for_program_using_string_ordered_comparisons() {
    let workspace_root = workspace_root();
    let temp_dir = TempDir::new("ql-string-ordered-compare-codegen");
    let output_path = temp_dir.path().join("string_ordered_compare_build.obj");

    let output = run_ql_build_capture(
        &workspace_root,
        "fixtures/codegen/pass/string_ordered_compare_build.ql",
        "obj",
        &output_path,
        &[],
    );

    let (stdout, stderr) = expect_success(
        "string_ordered_compare_build_object",
        "string ordered compare object build",
        &output,
    )
    .expect("string ordered compare object build should succeed");
    expect_stdout_contains_all(
        "string_ordered_compare_build_object",
        &stdout,
        &["wrote object:"],
    )
    .expect(
        "successful string ordered compare object build should report the emitted object artifact",
    );
    expect_empty_stderr(
        "string_ordered_compare_build_object",
        "string ordered compare object build",
        &stderr,
    )
    .expect("successful string ordered compare object build should stay silent on stderr");
    expect_file_exists(
        "string_ordered_compare_build_object",
        &output_path,
        "object artifact",
        "string ordered compare object build",
    )
    .expect("string ordered compare object build should produce an object artifact");
}

#[test]
fn builds_object_for_program_using_string_literal_match() {
    let workspace_root = workspace_root();
    let temp_dir = TempDir::new("ql-string-match-codegen");
    let output_path = temp_dir.path().join("string_match_build.obj");

    let output = run_ql_build_capture(
        &workspace_root,
        "fixtures/codegen/pass/string_match_build.ql",
        "obj",
        &output_path,
        &[],
    );

    let (stdout, stderr) = expect_success(
        "string_match_build_object",
        "string literal match object build",
        &output,
    )
    .expect("string literal match object build should succeed");
    expect_stdout_contains_all("string_match_build_object", &stdout, &["wrote object:"]).expect(
        "successful string literal match object build should report the emitted object artifact",
    );
    expect_empty_stderr(
        "string_match_build_object",
        "string literal match object build",
        &stderr,
    )
    .expect("successful string literal match object build should stay silent on stderr");
    expect_file_exists(
        "string_match_build_object",
        &output_path,
        "object artifact",
        "string literal match object build",
    )
    .expect("string literal match object build should produce an object artifact");
}

#[test]
fn builds_object_for_program_using_string_path_match() {
    let workspace_root = workspace_root();
    let temp_dir = TempDir::new("ql-string-path-match-codegen");
    let output_path = temp_dir.path().join("string_path_match_build.obj");

    let output = run_ql_build_capture(
        &workspace_root,
        "fixtures/codegen/pass/string_path_match_build.ql",
        "obj",
        &output_path,
        &[],
    );

    let (stdout, stderr) = expect_success(
        "string_path_match_build_object",
        "string path match object build",
        &output,
    )
    .expect("string path match object build should succeed");
    expect_stdout_contains_all(
        "string_path_match_build_object",
        &stdout,
        &["wrote object:"],
    )
    .expect("successful string path match object build should report the emitted object artifact");
    expect_empty_stderr(
        "string_path_match_build_object",
        "string path match object build",
        &stderr,
    )
    .expect("successful string path match object build should stay silent on stderr");
    expect_file_exists(
        "string_path_match_build_object",
        &output_path,
        "object artifact",
        "string path match object build",
    )
    .expect("string path match object build should produce an object artifact");
}
