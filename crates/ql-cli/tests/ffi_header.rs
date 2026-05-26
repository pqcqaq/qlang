mod support;

use std::thread;

use support::{
    TempDir, assert_no_atomic_write_temp_files, assert_no_build_lock_directories,
    expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_file_exists,
    expect_snapshot_matches, expect_stderr_contains, expect_stderr_not_contains,
    expect_stdout_contains_all, expect_success, ql_command, read_normalized_file,
    run_command_capture, workspace_root,
};

#[test]
fn ffi_header_snapshot_matches() {
    assert_ffi_header_snapshot(
        "tests/ffi/pass/extern_c_export.ql",
        None,
        "extern_c_export.h",
        "tests/codegen/pass/extern_c_export.h",
    );
}

#[test]
fn ffi_header_import_snapshot_matches() {
    assert_ffi_header_snapshot(
        "tests/ffi/header/extern_c_surface.ql",
        Some("imports"),
        "extern_c_surface.imports.h",
        "tests/codegen/pass/extern_c_surface.imports.h",
    );
}

#[test]
fn ffi_header_combined_snapshot_matches() {
    assert_ffi_header_snapshot(
        "tests/ffi/header/extern_c_surface.ql",
        Some("both"),
        "extern_c_surface.ffi.h",
        "tests/codegen/pass/extern_c_surface.ffi.h",
    );
}

#[test]
fn ffi_header_rejects_unknown_surface() {
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.args([
        "ffi",
        "header",
        "tests/ffi/pass/extern_c_export.ql",
        "--surface",
        "invalid",
    ]);
    let output = run_command_capture(&mut command, "`ql ffi header --surface invalid`");
    let (stdout, stderr) = expect_exit_code(
        "ffi-header-invalid-surface",
        "invalid header generation",
        &output,
        1,
    )
    .expect("invalid-surface header generation should fail with exit code 1");
    expect_empty_stdout(
        "ffi-header-invalid-surface",
        "failing header generation",
        &stdout,
    )
    .expect("invalid-surface header generation should not print stdout");
    expect_stderr_contains(
        "ffi-header-invalid-surface",
        "invalid header generation",
        &stderr,
        "unsupported `ql ffi header` surface `invalid`",
    )
    .expect("invalid-surface diagnostic should mention unsupported surface");
}

#[test]
fn ffi_rejects_unknown_subcommand_and_prints_usage() {
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.args(["ffi", "unknown"]);
    let output = run_command_capture(&mut command, "`ql ffi unknown`");
    let (stdout, stderr) = expect_exit_code(
        "ffi-unknown-subcommand",
        "unknown ffi subcommand",
        &output,
        1,
    )
    .expect("unknown ffi subcommand should fail with exit code 1");
    expect_empty_stdout("ffi-unknown-subcommand", "unknown ffi subcommand", &stdout)
        .expect("unknown ffi subcommand should not print stdout");
    expect_stderr_contains(
        "ffi-unknown-subcommand",
        "unknown ffi subcommand",
        &stderr,
        "error: unknown `ql ffi` subcommand `unknown`",
    )
    .expect("unknown ffi subcommand diagnostic should mention the rejected subcommand");
    expect_stderr_contains(
        "ffi-unknown-subcommand",
        "unknown ffi subcommand",
        &stderr,
        "Qlang CLI",
    )
    .expect("unknown ffi subcommand should print usage header");
    expect_stderr_contains(
        "ffi-unknown-subcommand",
        "unknown ffi subcommand",
        &stderr,
        "ql ffi header <file> [--surface exports|imports|both] [-o <output>]",
    )
    .expect("unknown ffi subcommand should print ffi usage line");
}

#[test]
fn ffi_header_supports_string_export_signatures() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-ffi-header-string");
    let source = temp.write(
        "string_export.ql",
        r#"
extern "c" pub fn q_echo(message: String) -> String {
    return message
}
"#,
    );
    let header = temp.path().join("string_export.h");

    let mut command = ql_command(&workspace_root);
    command
        .args(["ffi", "header"])
        .arg(&source)
        .arg("-o")
        .arg(&header);
    let output = run_command_capture(&mut command, "`ql ffi header` on string export signature");
    let (stdout, stderr) = expect_success(
        "ffi-header-string-export-signature",
        "string header generation",
        &output,
    )
    .expect("string header generation should succeed");
    expect_stdout_contains_all(
        "ffi-header-string-export-signature",
        &stdout,
        &["wrote c-header:", "string_export.h"],
    )
    .expect("string header generation should report the header artifact path");
    expect_empty_stderr(
        "ffi-header-string-export-signature",
        "string header generation",
        &stderr,
    )
    .expect("string header generation should not print stderr");
    expect_file_exists(
        "ffi-header-string-export-signature",
        &header,
        "header artifact",
        "string header generation",
    )
    .expect("string header generation should produce a header artifact");

    let rendered = read_normalized_file(&header, "generated string ffi header");
    assert!(rendered.contains("typedef struct ql_string {"));
    assert!(rendered.contains("const uint8_t* ptr;"));
    assert!(rendered.contains("int64_t len;"));
    assert!(rendered.contains("ql_string q_echo(ql_string message);"));
}

#[test]
fn ffi_header_serializes_concurrent_output_writes() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-ffi-header-concurrent-write");
    let source = temp.write(
        "math_export.ql",
        r#"
extern "c" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}
"#,
    );
    let header = temp.path().join("math_export.h");

    let handles = (0..4)
        .map(|index| {
            let workspace_root = workspace_root.clone();
            let source = source.clone();
            let header = header.clone();
            thread::spawn(move || {
                let mut command = ql_command(&workspace_root);
                command
                    .args(["ffi", "header"])
                    .arg(&source)
                    .arg("-o")
                    .arg(&header);
                run_command_capture(&mut command, format!("concurrent ql ffi header #{index}"))
            })
        })
        .collect::<Vec<_>>();

    for (index, handle) in handles.into_iter().enumerate() {
        let output = handle
            .join()
            .unwrap_or_else(|_| panic!("wait for concurrent ql ffi header #{index}"));
        let (stdout, stderr) = expect_success(
            "ffi-header-concurrent-write",
            &format!("concurrent ql ffi header #{index}"),
            &output,
        )
        .unwrap_or_else(|error| panic!("{error}"));
        expect_stdout_contains_all(
            "ffi-header-concurrent-write",
            &stdout,
            &["wrote c-header:", "math_export.h"],
        )
        .unwrap_or_else(|error| panic!("{error}"));
        expect_empty_stderr(
            "ffi-header-concurrent-write",
            &format!("concurrent ql ffi header #{index}"),
            &stderr,
        )
        .unwrap_or_else(|error| panic!("{error}"));
    }

    let rendered = read_normalized_file(&header, "concurrently generated ffi header");
    assert!(rendered.contains("int64_t q_add(int64_t left, int64_t right);"));
    assert_no_build_lock_directories("ffi-header-concurrent-write", temp.path());
    assert_no_atomic_write_temp_files("ffi-header-concurrent-write", temp.path());
}

#[test]
fn ffi_header_serializes_concurrent_default_output_writes() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-ffi-header-concurrent-default-write");
    let source = temp.write(
        "math_default.ql",
        r#"
extern "c" pub fn q_mul(left: Int, right: Int) -> Int {
    return left * right
}
"#,
    );
    let header = temp.path().join("target/ql/ffi/math_default.h");
    let working_dir = temp.path().to_path_buf();

    let handles = (0..4)
        .map(|index| {
            let workspace_root = workspace_root.clone();
            let source = source.clone();
            let working_dir = working_dir.clone();
            thread::spawn(move || {
                let mut command = ql_command(&workspace_root);
                command.current_dir(&working_dir);
                command.args(["ffi", "header"]).arg(&source);
                run_command_capture(
                    &mut command,
                    format!("concurrent default-output ql ffi header #{index}"),
                )
            })
        })
        .collect::<Vec<_>>();

    for (index, handle) in handles.into_iter().enumerate() {
        let output = handle.join().unwrap_or_else(|_| {
            panic!("wait for concurrent default-output ql ffi header #{index}")
        });
        let (stdout, stderr) = expect_success(
            "ffi-header-concurrent-default-write",
            &format!("concurrent default-output ql ffi header #{index}"),
            &output,
        )
        .unwrap_or_else(|error| panic!("{error}"));
        expect_stdout_contains_all(
            "ffi-header-concurrent-default-write",
            &stdout,
            &["wrote c-header:", "math_default.h"],
        )
        .unwrap_or_else(|error| panic!("{error}"));
        expect_empty_stderr(
            "ffi-header-concurrent-default-write",
            &format!("concurrent default-output ql ffi header #{index}"),
            &stderr,
        )
        .unwrap_or_else(|error| panic!("{error}"));
    }

    expect_file_exists(
        "ffi-header-concurrent-default-write",
        &header,
        "default-output header artifact",
        "concurrent default-output header generation",
    )
    .expect("concurrent default-output header generation should produce a header artifact");
    let rendered = read_normalized_file(&header, "concurrently generated default ffi header");
    assert!(rendered.contains("int64_t q_mul(int64_t left, int64_t right);"));
    assert_no_build_lock_directories("ffi-header-concurrent-default-write", temp.path());
    assert_no_atomic_write_temp_files("ffi-header-concurrent-default-write", temp.path());
}

#[test]
fn ffi_header_preserves_deferred_multi_segment_type_paths_in_unsupported_diagnostics() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-ffi-header-deferred-type");
    let source = temp.write(
        "unsupported_deferred_type.ql",
        r#"
use Command as Cmd

struct Command {
    value: Int,
}

extern "c" pub fn q_accept(value: Cmd.Scope.Config) -> Int {
    return 0
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["ffi", "header"]).arg(&source);
    let output = run_command_capture(
        &mut command,
        "`ql ffi header` on deferred multi-segment type signature",
    );
    let (stdout, stderr) = expect_exit_code(
        "ffi-header-deferred-type",
        "deferred-type header generation",
        &output,
        1,
    )
    .expect("deferred-type header generation should fail with exit code 1");
    expect_empty_stdout(
        "ffi-header-deferred-type",
        "failing header generation",
        &stdout,
    )
    .expect("deferred-type header generation should not print stdout");
    expect_stderr_contains(
        "ffi-header-deferred-type",
        "deferred-type header generation",
        &stderr,
        "C header generation does not support parameter type `Cmd.Scope.Config` yet",
    )
    .expect("deferred diagnostic should preserve source-backed path");
    expect_stderr_not_contains(
        "ffi-header-deferred-type",
        "deferred-type header generation",
        &stderr,
        "parameter type `Command`",
    )
    .expect("deferred diagnostic should not collapse to local type name");
}

fn assert_ffi_header_snapshot(
    source_path: &str,
    surface: Option<&str>,
    output_name: &str,
    expected_path: &str,
) {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-ffi-header");
    let output_path = temp.path().join(output_name);
    let expected_path = workspace_root.join(expected_path);
    let expected = read_normalized_file(&expected_path, "expected snapshot");

    let mut args = vec![
        "ffi".to_owned(),
        "header".to_owned(),
        source_path.to_owned(),
    ];
    if let Some(surface) = surface {
        args.push("--surface".to_owned());
        args.push(surface.to_owned());
    }
    args.push("--output".to_owned());
    args.push(output_path.to_string_lossy().into_owned());

    let mut command = ql_command(&workspace_root);
    command.args(&args);
    let output = run_command_capture(&mut command, format!("`ql {}`", args.join(" ")));
    let (_, stderr) = expect_success("ffi-header-snapshot", "header generation", &output)
        .expect("ffi header snapshot generation should succeed");
    expect_empty_stderr(
        "ffi-header-snapshot",
        "successful header generation",
        &stderr,
    )
    .expect("successful header generation should not print stderr");
    expect_file_exists(
        "ffi-header-snapshot",
        &output_path,
        "generated header",
        "header generation",
    )
    .expect("header generation should create an output file");

    let actual = read_normalized_file(&output_path, "generated header");
    expect_snapshot_matches(
        "ffi-header-snapshot",
        "generated header snapshot",
        &expected,
        &actual,
    )
    .expect("generated header snapshot should match");
}
