mod support;

use serde_json::Value as JsonValue;
use support::{
    TempDir, expect_empty_stderr, expect_exit_code, expect_file_exists, expect_stdout_contains_all,
    expect_success, ql_command, run_command_capture, workspace_root,
};

fn normalize_output_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&normalize_output_text(stdout))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
}

fn write_mock_clang_failure_script(temp: &TempDir) -> std::path::PathBuf {
    if cfg!(windows) {
        temp.write(
            "mock-clang-fail.cmd",
            "@echo off\r\necho mock clang failure 1>&2\r\nexit /b 9\r\n",
        )
    } else {
        let script = temp.write(
            "mock-clang-fail.sh",
            "#!/bin/sh\necho 'mock clang failure' 1>&2\nexit 9\n",
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = std::fs::metadata(&script)
                .expect("read mock clang failure script metadata")
                .permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&script, permissions)
                .expect("mark mock clang failure script executable");
        }
        script
    }
}

#[test]
fn build_single_file_supports_json_output() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-file-json");
    let source_path = temp.write("sample.ql", "fn main() -> Int { return 0 }\n");
    let artifact_path = temp.path().join("target/ql/debug/sample.ll");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&source_path).arg("--json");
    let output = run_command_capture(&mut command, "`ql build --json` single file");
    let (stdout, stderr) =
        expect_success("project-build-file-json", "single-file build json", &output)
            .expect("single-file `ql build --json` should succeed");
    expect_empty_stderr("project-build-file-json", "single-file build json", &stderr)
        .expect("single-file `ql build --json` should not print stderr");

    let json = parse_json_output("project-build-file-json", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.build.v1",
        "path": source_path.display().to_string().replace('\\', "/"),
        "scope": "file",
        "project_manifest_path": JsonValue::Null,
        "requested_emit": "llvm-ir",
        "requested_profile": "debug",
        "profile_overridden": false,
        "emit_interface": false,
        "status": "ok",
        "failure": JsonValue::Null,
        "built_targets": [
            {
                "manifest_path": JsonValue::Null,
                "package_name": JsonValue::Null,
                "selected": true,
                "dependency_only": false,
                "kind": "source",
                "path": source_path.display().to_string().replace('\\', "/"),
                "emit": "llvm-ir",
                "profile": "debug",
                "artifact_path": artifact_path.display().to_string().replace('\\', "/"),
                "c_header_path": JsonValue::Null,
            }
        ],
        "interfaces": [],
    });
    assert_eq!(
        json, expected,
        "single-file `ql build --json` should match the stable contract"
    );
    expect_file_exists(
        "project-build-file-json",
        &artifact_path,
        "single-file build artifact",
        "single-file build json",
    )
    .expect("single-file `ql build --json` should still write the artifact");
}

#[test]
fn build_single_file_json_supports_local_generic_function_instantiation() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-file-local-generic");
    let source_path = temp.write(
        "sample.ql",
        r#"
fn first[T, N](values: [T; N]) -> T {
    return values[0]
}

fn len[T, N](values: [T; N]) -> Int {
    return N
}

fn main() -> Int {
    return first([10, 20, 30]) + len([1, 2, 3, 4])
}
"#,
    );
    let artifact_path = temp.path().join("target/ql/debug/sample.ll");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&source_path).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` single file local generic function",
    );
    let (stdout, stderr) = expect_success(
        "project-build-file-local-generic",
        "single-file local generic function json build",
        &output,
    )
    .expect("single-file `ql build --json` should specialize local generic functions");
    expect_empty_stderr(
        "project-build-file-local-generic",
        "single-file local generic function json build",
        &stderr,
    )
    .expect("single-file local generic function json build should not print stderr");

    let json = parse_json_output("project-build-file-local-generic", &stdout);
    assert_eq!(json["status"], "ok");
    expect_file_exists(
        "project-build-file-local-generic",
        &artifact_path,
        "single-file local generic function build artifact",
        "single-file local generic function json build",
    )
    .expect("single-file local generic function build should write the artifact");
}

#[test]
fn build_single_file_json_rejects_target_selectors_without_project_context() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-file-json-selector-context");
    let source_path = temp.write("sample.ql", "fn main() -> Int { return 0 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["build"])
        .arg(&source_path)
        .args(["--json", "--bin", "admin"]);
    let output = run_command_capture(
        &mut command,
        "`ql build --json` single file selector requires project context",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-file-json-selector-context",
        "single-file build json selector preflight failure",
        &output,
        1,
    )
    .expect("single-file `ql build --json --bin admin` should exit with code 1");
    expect_empty_stderr(
        "project-build-file-json-selector-context",
        "single-file build json selector preflight failure",
        &stderr,
    )
    .expect("single-file `ql build --json --bin admin` should not print stderr");

    let json = parse_json_output("project-build-file-json-selector-context", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(json["scope"], "file");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["built_targets"], serde_json::json!([]));
    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(json["failure"]["error_kind"], "selector");
    assert_eq!(json["failure"]["stage"], "project-context");
    assert_eq!(json["failure"]["selector"], "binary `admin`");
    assert_eq!(
        json["failure"]["message"],
        "target selectors require a package or workspace path"
    );
}

#[test]
fn build_project_source_path_json_rejects_explicit_target_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-source-json-selector-conflict");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src/bin"))
        .expect("create package source tree for source selector conflict build json test");
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let main_path = temp.write("app/src/main.ql", "fn main() -> Int { return 1 }\n");
    temp.write("app/src/bin/admin.ql", "fn main() -> Int { return 2 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["build"])
        .arg(&main_path)
        .args(["--json", "--bin", "admin"]);
    let output = run_command_capture(
        &mut command,
        "`ql build --json` project source selector conflict",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-source-json-selector-conflict",
        "project source build json selector preflight failure",
        &output,
        1,
    )
    .expect("project source `ql build --json --bin admin` should exit with code 1");
    expect_empty_stderr(
        "project-build-source-json-selector-conflict",
        "project source build json selector preflight failure",
        &stderr,
    )
    .expect("project source `ql build --json --bin admin` should not print stderr");

    let json = parse_json_output("project-build-source-json-selector-conflict", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");
    assert_eq!(json["built_targets"], serde_json::json!([]));
    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(json["failure"]["error_kind"], "selector");
    assert_eq!(json["failure"]["stage"], "project-context");
    assert_eq!(json["failure"]["selector"], "binary `admin`");
    assert_eq!(
        json["failure"]["message"],
        "direct project source paths do not support target selectors"
    );
}

#[test]
fn build_single_file_supports_local_receiver_methods() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-file-local-receiver-method");
    let source_path = temp.write(
        "sample.ql",
        "struct Box { value: Int }\n\nimpl Box {\n    fn read(self) -> Int {\n        return self.value\n    }\n}\n\nfn main() -> Int {\n    let value = Box { value: 7 }\n    return value.read()\n}\n",
    );
    let artifact_path = temp.path().join("target/ql/debug/sample.ll");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&source_path);
    let output = run_command_capture(&mut command, "`ql build` local receiver method");
    let (stdout, stderr) = expect_success(
        "project-build-file-local-receiver-method",
        "single-file local receiver method build",
        &output,
    )
    .expect("single-file `ql build` should support local receiver methods");
    expect_empty_stderr(
        "project-build-file-local-receiver-method",
        "single-file local receiver method build",
        &stderr,
    )
    .expect("single-file `ql build` local receiver method should keep stderr empty");
    expect_stdout_contains_all(
        "project-build-file-local-receiver-method",
        &stdout.replace('\\', "/"),
        &[&format!("wrote llvm-ir: {}", artifact_path.display()).replace('\\', "/")],
    )
    .expect("single-file `ql build` local receiver method should report the LLVM IR artifact");
    expect_file_exists(
        "project-build-file-local-receiver-method",
        &artifact_path,
        "single-file local receiver method build artifact",
        "single-file local receiver method build",
    )
    .expect("single-file `ql build` local receiver method should write the LLVM IR artifact");
}

#[test]
fn build_single_file_json_reports_diagnostics_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-file-json-failure");
    let source_path = temp.write("sample.ql", "fn main() -> Int { return \"oops\" }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&source_path).arg("--json");
    let output = run_command_capture(&mut command, "`ql build --json` single file diagnostics");
    let (stdout, stderr) = expect_exit_code(
        "project-build-file-json-failure",
        "single-file build json diagnostics failure",
        &output,
        1,
    )
    .expect("single-file `ql build --json` diagnostics failure should exit with code 1");
    expect_empty_stderr(
        "project-build-file-json-failure",
        "single-file build json diagnostics failure",
        &stderr,
    )
    .expect("single-file `ql build --json` diagnostics failure should not print stderr");

    let json = parse_json_output("project-build-file-json-failure", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        source_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "file");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["requested_emit"], "llvm-ir");
    assert_eq!(json["requested_profile"], "debug");
    assert_eq!(json["profile_overridden"], false);
    assert_eq!(json["emit_interface"], false);
    assert_eq!(json["built_targets"], serde_json::json!([]));
    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(json["failure"]["manifest_path"], JsonValue::Null);
    assert_eq!(json["failure"]["package_name"], JsonValue::Null);
    assert_eq!(json["failure"]["selected"], true);
    assert_eq!(json["failure"]["dependency_only"], false);
    assert_eq!(json["failure"]["kind"], "source");
    assert_eq!(
        json["failure"]["path"],
        source_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["error_kind"], "diagnostics");
    assert_eq!(json["failure"]["message"], "build produced diagnostics");
    assert_eq!(
        json["failure"]["diagnostic_file"]["path"],
        source_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["failure"]["diagnostic_file"]["diagnostics"][0]["message"],
        "return value has type mismatch: expected `Int`, found `String`"
    );
}

#[test]
fn build_single_file_json_reports_toolchain_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-file-json-toolchain-failure");
    let source_path = temp.write("sample.ql", "fn main() -> Int { return 0 }\n");
    let clang_path = write_mock_clang_failure_script(&temp);

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .env("QLANG_CLANG", &clang_path)
        .args(["build"])
        .arg(&source_path)
        .args(["--emit", "obj", "--json"]);
    let output = run_command_capture(
        &mut command,
        "`ql build --json` single file toolchain failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-file-json-toolchain-failure",
        "single-file build json toolchain failure",
        &output,
        1,
    )
    .expect("single-file `ql build --json` toolchain failure should exit with code 1");
    expect_empty_stderr(
        "project-build-file-json-toolchain-failure",
        "single-file build json toolchain failure",
        &stderr,
    )
    .expect("single-file `ql build --json` toolchain failure should not print stderr");

    let json = parse_json_output("project-build-file-json-toolchain-failure", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        source_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "file");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["requested_emit"], "obj");
    assert_eq!(json["requested_profile"], "debug");
    assert_eq!(json["profile_overridden"], false);
    assert_eq!(json["emit_interface"], false);
    assert_eq!(json["built_targets"], serde_json::json!([]));
    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(json["failure"]["manifest_path"], JsonValue::Null);
    assert_eq!(json["failure"]["package_name"], JsonValue::Null);
    assert_eq!(json["failure"]["selected"], true);
    assert_eq!(json["failure"]["dependency_only"], false);
    assert_eq!(json["failure"]["kind"], "source");
    assert_eq!(
        json["failure"]["path"],
        source_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["error_kind"], "toolchain");
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("toolchain failure message should be a string")
            .contains("mock clang failure"),
        "toolchain failure json should preserve the toolchain stderr payload: {json}"
    );
    assert!(
        json["failure"]["preserved_artifacts"]
            .as_array()
            .expect("toolchain failure should expose preserved artifacts")
            .iter()
            .any(|value| value
                .as_str()
                .is_some_and(|path| path.ends_with(".codegen.ll"))),
        "toolchain failure json should preserve the intermediate LLVM IR path: {json}"
    );
    assert!(
        json["failure"]["intermediate_ir"]
            .as_str()
            .is_some_and(|path| path.ends_with(".codegen.ll")),
        "toolchain failure json should surface the primary intermediate IR path: {json}"
    );
}

#[test]
fn build_single_file_json_reports_emit_interface_package_context_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-file-json-emit-interface-context");
    let source_path = temp.write("sample.ql", "fn main() -> Int { return 0 }\n");
    let artifact_path = temp.path().join("target/ql/debug/sample.ll");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["build"])
        .arg(&source_path)
        .args(["--emit-interface", "--json"]);
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface --json` single file package context failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-file-json-emit-interface-context",
        "single-file build json emit-interface package context failure",
        &output,
        1,
    )
    .expect("single-file `ql build --emit-interface --json` should exit with code 1");
    expect_empty_stderr(
        "project-build-file-json-emit-interface-context",
        "single-file build json emit-interface package context failure",
        &stderr,
    )
    .expect("single-file `ql build --emit-interface --json` should not print stderr");

    let json = parse_json_output("project-build-file-json-emit-interface-context", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(json["scope"], "file");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["emit_interface"], true);
    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(json["failure"]["manifest_path"], JsonValue::Null);
    assert_eq!(json["failure"]["package_name"], JsonValue::Null);
    assert_eq!(json["failure"]["selected"], true);
    assert_eq!(json["failure"]["dependency_only"], false);
    assert_eq!(json["failure"]["kind"], "interface");
    assert_eq!(
        json["failure"]["path"],
        source_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["error_kind"], "project-context");
    assert_eq!(json["failure"]["stage"], "emit-interface");
    assert_eq!(json["failure"]["output_path"], JsonValue::Null);
    assert_eq!(json["failure"]["source_root"], JsonValue::Null);
    assert_eq!(json["failure"]["failing_source_count"], JsonValue::Null);
    assert_eq!(json["failure"]["first_failing_source"], JsonValue::Null);
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("emit-interface package context failure should expose a message")
            .contains("requires a package manifest"),
        "single-file emit-interface package context failure should explain the missing package context: {json}"
    );

    let built_targets = json["built_targets"]
        .as_array()
        .expect("emit-interface package context failure should expose built_targets");
    assert_eq!(built_targets.len(), 1);
    assert_eq!(
        built_targets[0]["artifact_path"],
        artifact_path.display().to_string().replace('\\', "/")
    );
    expect_file_exists(
        "project-build-file-json-emit-interface-context",
        &artifact_path,
        "single-file build artifact",
        "single-file build json emit-interface package context failure",
    )
    .expect("single-file `ql build --emit-interface --json` should preserve the built artifact");
}
