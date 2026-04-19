mod support;

use serde_json::Value as JsonValue;
use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_file_exists,
    expect_stdout_contains_all, expect_success, ql_command, run_command_capture,
    static_library_output_path, workspace_root,
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

#[test]
fn build_package_path_builds_all_discovered_targets() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src/bin/tools"))
        .expect("create package source tree for build test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn util() -> Int { return 1 }\n");
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");
    temp.write(
        "app/src/bin/tools/repl.ql",
        "fn main() -> Int { return 2 }\n",
    );

    let lib_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");
    let main_output = project_root.join("target/ql/debug/main.ll");
    let repl_output = project_root.join("target/ql/debug/bin/tools/repl.ll");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql build` package path");
    let (stdout, stderr) = expect_success("project-build-package", "package path build", &output)
        .expect("package path build should succeed");
    expect_empty_stderr("project-build-package", "package path build", &stderr)
        .expect("package path build should not print stderr");
    expect_stdout_contains_all(
        "project-build-package",
        &stdout.replace('\\', "/"),
        &[
            &format!("wrote staticlib: {}", lib_output.display()).replace('\\', "/"),
            &format!("wrote llvm-ir: {}", main_output.display()).replace('\\', "/"),
            &format!("wrote llvm-ir: {}", repl_output.display()).replace('\\', "/"),
        ],
    )
    .expect("package path build should report every discovered target artifact");

    expect_file_exists(
        "project-build-package",
        &lib_output,
        "package library artifact",
        "package path build",
    )
    .expect("package path build should emit the library artifact");
    expect_file_exists(
        "project-build-package",
        &main_output,
        "package binary artifact",
        "package path build",
    )
    .expect("package path build should emit the main artifact");
    expect_file_exists(
        "project-build-package",
        &repl_output,
        "package nested bin artifact",
        "package path build",
    )
    .expect("package path build should emit nested bin artifacts under a stable relative path");
}

#[test]
fn build_project_source_file_uses_project_aware_dependency_plan() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-source-file-project-aware");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source tree");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source tree");

    let dep_manifest = temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "dep/src/lib.ql",
        "extern \"c\" pub fn q_add(left: Int, right: Int) -> Int { return left + right }\n",
    );
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    let app_main = temp.write(
        "app/src/main.ql",
        "use dep.q_add as add\n\nfn main() -> Int { return add(6, 7) }\n",
    );

    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let app_output = project_root.join("target/ql/debug/main.ll");
    let interface_output = dep_root.join("dep.qi");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&app_main).arg("--json");
    let output = run_command_capture(&mut command, "`ql build --json` direct project source file");
    let (stdout, stderr) = expect_success(
        "project-build-source-file-project-aware",
        "direct project source file json build",
        &output,
    )
    .expect("direct project source file `ql build --json` should succeed");
    expect_empty_stderr(
        "project-build-source-file-project-aware",
        "direct project source file json build",
        &stderr,
    )
    .expect("direct project source file `ql build --json` should not print stderr");

    let json = parse_json_output("project-build-source-file-project-aware", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["path"],
        app_main.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "ok");
    assert_eq!(
        json["interfaces"],
        serde_json::json!([
            {
                "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "selected": true,
                "status": "wrote",
                "path": project_root.join("app.qi").display().to_string().replace('\\', "/"),
            }
        ])
    );
    let built_targets = json["built_targets"]
        .as_array()
        .expect("project-aware source build should expose built targets");
    assert_eq!(built_targets.len(), 2);
    assert_eq!(
        built_targets[0],
        serde_json::json!({
            "manifest_path": dep_manifest.display().to_string().replace('\\', "/"),
            "package_name": "dep",
            "selected": false,
            "dependency_only": true,
            "kind": "lib",
            "path": "src/lib.ql",
            "emit": "staticlib",
            "profile": "debug",
            "artifact_path": dep_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        })
    );
    assert_eq!(
        built_targets[1],
        serde_json::json!({
            "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
            "package_name": "app",
            "selected": true,
            "dependency_only": false,
            "kind": "bin",
            "path": "src/main.ql",
            "emit": "llvm-ir",
            "profile": "debug",
            "artifact_path": app_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        })
    );
    expect_file_exists(
        "project-build-source-file-project-aware",
        &dep_output,
        "dependency package artifact",
        "direct project source file json build",
    )
    .expect("direct project source build should emit dependency artifacts");
    expect_file_exists(
        "project-build-source-file-project-aware",
        &app_output,
        "selected package artifact",
        "direct project source file json build",
    )
    .expect("direct project source build should emit the selected package artifact");
    expect_file_exists(
        "project-build-source-file-project-aware",
        &interface_output,
        "synced dependency interface",
        "direct project source file json build",
    )
    .expect("direct project source build should keep dependency interface sync");
}

#[test]
fn build_project_source_file_supports_direct_dependency_public_functions() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-source-file-public-function");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source tree");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source tree");

    let dep_manifest = temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "dep/src/lib.ql",
        "pub fn add(left: Int, right: Int) -> Int { return left + right }\n",
    );
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    let app_main = temp.write(
        "app/src/main.ql",
        "use dep.add as sum\n\nfn main() -> Int { return sum(8, 5) }\n",
    );

    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let app_output = project_root.join("target/ql/debug/main.ll");
    let interface_output = dep_root.join("dep.qi");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&app_main).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` direct project source file dependency public function",
    );
    let (stdout, stderr) = expect_success(
        "project-build-source-file-public-function",
        "direct project source file dependency public function json build",
        &output,
    )
    .expect(
        "direct project source file `ql build --json` should support direct dependency public functions",
    );
    expect_empty_stderr(
        "project-build-source-file-public-function",
        "direct project source file dependency public function json build",
        &stderr,
    )
    .expect("direct dependency public function json build should not print stderr");

    let json = parse_json_output("project-build-source-file-public-function", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["path"],
        app_main.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "ok");
    let built_targets = json["built_targets"]
        .as_array()
        .expect("dependency public function json build should expose built targets");
    assert_eq!(built_targets.len(), 2);
    assert_eq!(
        built_targets[0],
        serde_json::json!({
            "manifest_path": dep_manifest.display().to_string().replace('\\', "/"),
            "package_name": "dep",
            "selected": false,
            "dependency_only": true,
            "kind": "lib",
            "path": "src/lib.ql",
            "emit": "staticlib",
            "profile": "debug",
            "artifact_path": dep_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        })
    );
    assert_eq!(
        built_targets[1],
        serde_json::json!({
            "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
            "package_name": "app",
            "selected": true,
            "dependency_only": false,
            "kind": "bin",
            "path": "src/main.ql",
            "emit": "llvm-ir",
            "profile": "debug",
            "artifact_path": app_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        })
    );
    expect_file_exists(
        "project-build-source-file-public-function",
        &dep_output,
        "dependency package artifact",
        "direct project source file dependency public function json build",
    )
    .expect("direct dependency public function json build should emit dependency artifacts");
    expect_file_exists(
        "project-build-source-file-public-function",
        &app_output,
        "selected package artifact",
        "direct project source file dependency public function json build",
    )
    .expect(
        "direct dependency public function json build should emit the selected package artifact",
    );
    expect_file_exists(
        "project-build-source-file-public-function",
        &interface_output,
        "synced dependency interface",
        "direct project source file dependency public function json build",
    )
    .expect("direct dependency public function json build should keep dependency interface sync");
}

#[test]
fn build_project_source_file_supports_direct_dependency_public_values() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-source-file-public-values");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source tree");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source tree");

    let dep_manifest = temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "dep/src/lib.ql",
        "pub const VALUE: Int = 7\npub static READY: Bool = true\npub static VALUES: [Int; 3] = [1, 3, 5]\n",
    );
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    let app_main = temp.write(
        "app/src/main.ql",
        "use dep.VALUE as THRESHOLD\nuse dep.READY as ENABLED\nuse dep.VALUES as ITEMS\n\nfn main() -> Int {\n    if ENABLED {\n        return THRESHOLD + ITEMS[1]\n    }\n    return 0\n}\n",
    );

    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let app_output = project_root.join("target/ql/debug/main.ll");
    let interface_output = dep_root.join("dep.qi");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&app_main).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` direct project source file dependency public values",
    );
    let (stdout, stderr) = expect_success(
        "project-build-source-file-public-values",
        "direct project source file dependency public values json build",
        &output,
    )
    .expect(
        "direct project source file `ql build --json` should support direct dependency public values",
    );
    expect_empty_stderr(
        "project-build-source-file-public-values",
        "direct project source file dependency public values json build",
        &stderr,
    )
    .expect("direct dependency public values json build should not print stderr");

    let json = parse_json_output("project-build-source-file-public-values", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["path"],
        app_main.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "ok");
    let built_targets = json["built_targets"]
        .as_array()
        .expect("dependency public values json build should expose built targets");
    assert_eq!(built_targets.len(), 2);
    assert_eq!(
        built_targets[0],
        serde_json::json!({
            "manifest_path": dep_manifest.display().to_string().replace('\\', "/"),
            "package_name": "dep",
            "selected": false,
            "dependency_only": true,
            "kind": "lib",
            "path": "src/lib.ql",
            "emit": "staticlib",
            "profile": "debug",
            "artifact_path": dep_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        })
    );
    assert_eq!(
        built_targets[1],
        serde_json::json!({
            "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
            "package_name": "app",
            "selected": true,
            "dependency_only": false,
            "kind": "bin",
            "path": "src/main.ql",
            "emit": "llvm-ir",
            "profile": "debug",
            "artifact_path": app_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        })
    );
    expect_file_exists(
        "project-build-source-file-public-values",
        &dep_output,
        "dependency package artifact",
        "direct project source file dependency public values json build",
    )
    .expect("direct dependency public values json build should emit dependency artifacts");
    expect_file_exists(
        "project-build-source-file-public-values",
        &app_output,
        "selected package artifact",
        "direct project source file dependency public values json build",
    )
    .expect("direct dependency public values json build should emit the selected package artifact");
    expect_file_exists(
        "project-build-source-file-public-values",
        &interface_output,
        "synced dependency interface",
        "direct project source file dependency public values json build",
    )
    .expect("direct dependency public values json build should keep dependency interface sync");
}

#[test]
fn build_project_source_file_supports_dependency_public_values_with_function_initializers() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-source-file-public-value-function-initializers");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source tree");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source tree");

    let dep_manifest = temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "dep/src/lib.ql",
        "pub fn add_one(value: Int) -> Int { return value + 1 }\npub fn make_value() -> Int { return add_one(6) }\npub const VALUE: Int = make_value()\npub const APPLY: (Int) -> Int = add_one\n",
    );
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    let app_main = temp.write(
        "app/src/main.ql",
        "use dep.VALUE as VALUE_ALIAS\nuse dep.APPLY as RUN\n\nfn main() -> Int { return VALUE_ALIAS + RUN(3) }\n",
    );

    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let app_output = project_root.join("target/ql/debug/main.ll");
    let interface_output = dep_root.join("dep.qi");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&app_main).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` dependency public values with function initializers",
    );
    let (stdout, stderr) = expect_success(
        "project-build-source-file-public-value-function-initializers",
        "direct project source file dependency public values with function initializers json build",
        &output,
    )
    .expect(
        "direct project source file `ql build --json` should support dependency public values with function initializers",
    );
    expect_empty_stderr(
        "project-build-source-file-public-value-function-initializers",
        "direct project source file dependency public values with function initializers json build",
        &stderr,
    )
    .expect(
        "dependency public values with function initializers json build should not print stderr",
    );

    let json = parse_json_output(
        "project-build-source-file-public-value-function-initializers",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["path"],
        app_main.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "ok");
    let built_targets = json["built_targets"]
        .as_array()
        .expect("dependency public value initializer json build should expose built targets");
    assert_eq!(built_targets.len(), 2);
    assert_eq!(
        built_targets[0],
        serde_json::json!({
            "manifest_path": dep_manifest.display().to_string().replace('\\', "/"),
            "package_name": "dep",
            "selected": false,
            "dependency_only": true,
            "kind": "lib",
            "path": "src/lib.ql",
            "emit": "staticlib",
            "profile": "debug",
            "artifact_path": dep_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        })
    );
    assert_eq!(
        built_targets[1],
        serde_json::json!({
            "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
            "package_name": "app",
            "selected": true,
            "dependency_only": false,
            "kind": "bin",
            "path": "src/main.ql",
            "emit": "llvm-ir",
            "profile": "debug",
            "artifact_path": app_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        })
    );
    expect_file_exists(
        "project-build-source-file-public-value-function-initializers",
        &dep_output,
        "dependency package artifact",
        "direct project source file dependency public values with function initializers json build",
    )
    .expect("dependency public value initializer json build should emit dependency artifacts");
    expect_file_exists(
        "project-build-source-file-public-value-function-initializers",
        &app_output,
        "selected package artifact",
        "direct project source file dependency public values with function initializers json build",
    )
    .expect(
        "dependency public value initializer json build should emit the selected package artifact",
    );
    expect_file_exists(
        "project-build-source-file-public-value-function-initializers",
        &interface_output,
        "synced dependency interface",
        "direct project source file dependency public values with function initializers json build",
    )
    .expect("dependency public value initializer json build should keep dependency interface sync");
}

#[test]
fn build_package_path_supports_json_output_for_dependency_build_plan() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source tree");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source tree");

    let dep_manifest = temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write("dep/src/lib.ql", "pub fn exported() -> Int { return 1 }\n");
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");

    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let app_lib_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");
    let app_main_output = project_root.join("target/ql/debug/main.ll");
    let interface_output = project_root.join("app.qi");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["build"])
        .arg(&project_root)
        .args(["--emit-interface", "--json"]);
    let output = run_command_capture(&mut command, "`ql build --json` package dependency plan");
    let (stdout, stderr) = expect_success(
        "project-build-package-json",
        "package dependency-plan build json",
        &output,
    )
    .expect("package-path `ql build --json` should succeed");
    expect_empty_stderr(
        "project-build-package-json",
        "package dependency-plan build json",
        &stderr,
    )
    .expect("package-path `ql build --json` should not print stderr");

    let json = parse_json_output("project-build-package-json", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.build.v1",
        "path": project_root.display().to_string().replace('\\', "/"),
        "scope": "project",
        "project_manifest_path": app_manifest.display().to_string().replace('\\', "/"),
        "requested_emit": "llvm-ir",
        "requested_profile": "debug",
        "profile_overridden": false,
        "emit_interface": true,
        "status": "ok",
        "failure": JsonValue::Null,
        "built_targets": [
            {
                "manifest_path": dep_manifest.display().to_string().replace('\\', "/"),
                "package_name": "dep",
                "selected": false,
                "dependency_only": true,
                "kind": "lib",
                "path": "src/lib.ql",
                "emit": "staticlib",
                "profile": "debug",
                "artifact_path": dep_output.display().to_string().replace('\\', "/"),
                "c_header_path": JsonValue::Null,
            },
            {
                "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "selected": true,
                "dependency_only": false,
                "kind": "lib",
                "path": "src/lib.ql",
                "emit": "staticlib",
                "profile": "debug",
                "artifact_path": app_lib_output.display().to_string().replace('\\', "/"),
                "c_header_path": JsonValue::Null,
            },
            {
                "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "selected": true,
                "dependency_only": false,
                "kind": "bin",
                "path": "src/main.ql",
                "emit": "llvm-ir",
                "profile": "debug",
                "artifact_path": app_main_output.display().to_string().replace('\\', "/"),
                "c_header_path": JsonValue::Null,
            }
        ],
        "interfaces": [
            {
                "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "selected": true,
                "status": "wrote",
                "path": interface_output.display().to_string().replace('\\', "/"),
            }
        ],
    });
    assert_eq!(
        json, expected,
        "package-path `ql build --json` should match the stable contract"
    );

    expect_file_exists(
        "project-build-package-json",
        &dep_output,
        "dependency package artifact",
        "package dependency-plan build json",
    )
    .expect("package-path `ql build --json` should emit the dependency artifact");
    expect_file_exists(
        "project-build-package-json",
        &app_lib_output,
        "package library artifact",
        "package dependency-plan build json",
    )
    .expect("package-path `ql build --json` should emit the package library artifact");
    expect_file_exists(
        "project-build-package-json",
        &app_main_output,
        "package binary artifact",
        "package dependency-plan build json",
    )
    .expect("package-path `ql build --json` should emit the package binary artifact");
    expect_file_exists(
        "project-build-package-json",
        &interface_output,
        "package interface artifact",
        "package dependency-plan build json",
    )
    .expect("package-path `ql build --json` should emit the package interface");
}

#[test]
fn build_project_path_list_reports_discovered_targets_without_building() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-list");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src/bin"))
        .expect("create app source tree for build list test");
    std::fs::create_dir_all(project_root.join("packages/tool/src"))
        .expect("create tool source tree for build list test");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn helper() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int { return 0 }\n",
    );
    temp.write(
        "workspace/packages/app/src/bin/admin.ql",
        "fn main() -> Int { return 2 }\n",
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        "pub fn helper() -> Int { return 3 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--list");
    let output = run_command_capture(&mut command, "`ql build --list` workspace path");
    let (stdout, stderr) = expect_success(
        "project-build-list",
        "workspace build target listing",
        &output,
    )
    .expect("workspace-path `ql build --list` should succeed");
    expect_empty_stderr(
        "project-build-list",
        "workspace build target listing",
        &stderr,
    )
    .expect("workspace-path `ql build --list` should not print stderr");
    expect_stdout_contains_all(
        "project-build-list",
        &stdout,
        &[
            &format!(
                "manifest: {}",
                project_root
                    .join("packages/app/qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            "package: app",
            "  - lib: src/lib.ql",
            "  - bin: src/main.ql",
            "  - bin: src/bin/admin.ql",
            &format!(
                "manifest: {}",
                project_root
                    .join("packages/tool/qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            "package: tool",
            "  - lib: src/lib.ql",
        ],
    )
    .expect("workspace-path `ql build --list` should report discovered targets");
    assert!(
        !project_root.join("packages/app/target").exists(),
        "`ql build --list` should not create build artifacts"
    );
}

#[test]
fn build_project_path_list_json_supports_target_selectors() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-list-json");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src/bin"))
        .expect("create app source tree for build list json test");
    std::fs::create_dir_all(project_root.join("packages/tool/src"))
        .expect("create tool source tree for build list json test");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    let app_manifest = temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int { return 0 }\n",
    );
    temp.write(
        "workspace/packages/app/src/bin/admin.ql",
        "fn main() -> Int { return 2 }\n",
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        "pub fn helper() -> Int { return 3 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).args([
        "--list",
        "--json",
        "--package",
        "app",
        "--bin",
        "admin",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql build --list --json --package app --bin admin` workspace path",
    );
    let (stdout, stderr) = expect_success(
        "project-build-list-json",
        "workspace build target listing json",
        &output,
    )
    .expect("workspace-path `ql build --list --json` should succeed");
    expect_empty_stderr(
        "project-build-list-json",
        "workspace build target listing json",
        &stderr,
    )
    .expect("workspace-path `ql build --list --json` should not print stderr");

    let json = parse_json_output("project-build-list-json", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.project.targets.v1",
        "members": [
            {
                "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "targets": [
                    {
                        "kind": "bin",
                        "path": "src/bin/admin.ql",
                    }
                ],
            }
        ],
    });
    assert_eq!(
        json, expected,
        "workspace-path `ql build --list --json` should reuse the stable target-listing schema"
    );
    assert!(
        !project_root.join("packages/app/target").exists(),
        "`ql build --list --json` should not create build artifacts"
    );
}

#[test]
fn build_project_member_directory_list_json_uses_workspace_context() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-list-member-dir");
    let project_root = temp.path().join("workspace");
    let app_root = project_root.join("packages").join("app");
    let tool_root = project_root.join("packages").join("tool");
    std::fs::create_dir_all(app_root.join("src/bin"))
        .expect("create app source tree for build list member directory test");
    std::fs::create_dir_all(tool_root.join("src"))
        .expect("create tool source tree for build list member directory test");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    let app_manifest = temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn helper() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int { return 0 }\n",
    );
    temp.write(
        "workspace/packages/app/src/bin/admin.ql",
        "fn main() -> Int { return 2 }\n",
    );
    let tool_manifest = temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        "pub fn helper() -> Int { return 3 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["build"])
        .arg(&app_root)
        .args(["--list", "--json"]);
    let output = run_command_capture(
        &mut command,
        "`ql build --list --json` workspace member directory",
    );
    let (stdout, stderr) = expect_success(
        "project-build-list-member-dir",
        "workspace member directory build target listing",
        &output,
    )
    .expect("workspace member directory `ql build --list --json` should succeed");
    expect_empty_stderr(
        "project-build-list-member-dir",
        "workspace member directory build target listing",
        &stderr,
    )
    .expect("workspace member directory `ql build --list --json` should not print stderr");

    let json = parse_json_output("project-build-list-member-dir", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.project.targets.v1",
        "members": [
            {
                "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "targets": [
                    {
                        "kind": "lib",
                        "path": "src/lib.ql",
                    },
                    {
                        "kind": "bin",
                        "path": "src/main.ql",
                    },
                    {
                        "kind": "bin",
                        "path": "src/bin/admin.ql",
                    }
                ],
            },
            {
                "manifest_path": tool_manifest.display().to_string().replace('\\', "/"),
                "package_name": "tool",
                "targets": [
                    {
                        "kind": "lib",
                        "path": "src/lib.ql",
                    }
                ],
            }
        ],
    });
    assert_eq!(
        json, expected,
        "workspace member directory `ql build --list --json` should resolve the outer workspace and report all discovered targets"
    );
    assert!(
        !project_root.join("packages/app/target").exists(),
        "`ql build --list --json` workspace member directory should not create build artifacts"
    );
}

#[test]
fn build_package_path_json_reports_dependency_interface_prep_manifest_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-dependency-prep-manifest");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(&dep_root).expect("create dependency root for prep manifest failure");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for prep manifest failure");

    let dep_manifest = temp.write(
        "dep/qlang.toml",
        r#"
[package]
"#,
    );
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` dependency interface prep manifest failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-json-dependency-prep-manifest",
        "package build json dependency interface prep manifest failure",
        &output,
        1,
    )
    .expect("package-path `ql build --json` should fail on dependency prep manifest failures");
    expect_empty_stderr(
        "project-build-package-json-dependency-prep-manifest",
        "package build json dependency interface prep manifest failure",
        &stderr,
    )
    .expect("dependency prep manifest failures should stay on stdout in json mode");

    let json = parse_json_output(
        "project-build-package-json-dependency-prep-manifest",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");
    assert_eq!(json["built_targets"], serde_json::json!([]));
    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(
        json["failure"]["manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], JsonValue::Null);
    assert_eq!(json["failure"]["selected"], false);
    assert_eq!(json["failure"]["dependency_only"], true);
    assert_eq!(json["failure"]["kind"], "interface");
    assert_eq!(
        json["failure"]["path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["error_kind"], "manifest");
    assert_eq!(json["failure"]["stage"], "dependency-interface-prep");
    assert_eq!(
        json["failure"]["owner_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["failure"]["reference_manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["reference"], "../dep");
    assert_eq!(json["failure"]["failing_dependency_count"], 1);
    assert_eq!(
        json["failure"]["first_failing_dependency_manifest"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("dependency prep manifest failure should expose a message")
            .contains("does not declare `[package].name`"),
        "dependency prep manifest failure should preserve the broken dependency manifest detail: {json}"
    );
}

#[test]
fn build_package_path_json_reports_dependency_interface_prep_output_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-dependency-prep-output");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dependency source tree for prep output failure");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for prep output failure");

    let dep_manifest = temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write("dep/src/lib.ql", "pub fn exported() -> Int { return 1 }\n");
    let interface_output = dep_root.join("dep.qi");
    std::fs::create_dir_all(&interface_output)
        .expect("create blocking dependency interface directory for prep output failure");
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` dependency interface prep output failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-json-dependency-prep-output",
        "package build json dependency interface prep output failure",
        &output,
        1,
    )
    .expect("package-path `ql build --json` should fail on dependency prep output failures");
    expect_empty_stderr(
        "project-build-package-json-dependency-prep-output",
        "package build json dependency interface prep output failure",
        &stderr,
    )
    .expect("dependency prep output failures should stay on stdout in json mode");

    let json = parse_json_output("project-build-package-json-dependency-prep-output", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");
    assert_eq!(json["built_targets"], serde_json::json!([]));
    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(
        json["failure"]["manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], JsonValue::Null);
    assert_eq!(json["failure"]["selected"], false);
    assert_eq!(json["failure"]["dependency_only"], true);
    assert_eq!(json["failure"]["kind"], "interface");
    assert_eq!(
        json["failure"]["path"],
        interface_output.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["error_kind"], "interface-output");
    assert_eq!(json["failure"]["stage"], "dependency-interface-prep");
    assert_eq!(
        json["failure"]["output_path"],
        interface_output.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["failure"]["owner_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["failure"]["reference_manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["reference"], "../dep");
    assert_eq!(json["failure"]["failing_dependency_count"], 1);
    assert_eq!(
        json["failure"]["first_failing_dependency_manifest"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("dependency prep output failure should expose a message")
            .contains("failed to write interface"),
        "dependency prep output failure should preserve the blocked interface write detail: {json}"
    );
    assert!(
        interface_output.is_dir(),
        "dependency prep output failure should preserve `{}` as a directory",
        interface_output.display()
    );
}

#[test]
fn build_package_path_json_reports_build_plan_dependency_cycle() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-build-plan-cycle");
    let app_root = temp.path().join("app");
    let core_root = temp.path().join("core");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app source tree for build plan cycle test");
    std::fs::create_dir_all(core_root.join("src"))
        .expect("create core source tree for build plan cycle test");

    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
core = "../core"
"#,
    );
    let core_manifest = temp.write(
        "core/qlang.toml",
        r#"
[package]
name = "core"

[dependencies]
app = "../app"
"#,
    );
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");
    temp.write("core/src/lib.ql", "pub fn answer() -> Int { return 42 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&app_root).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` package build-plan dependency cycle",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-json-build-plan-cycle",
        "package build json build-plan dependency cycle failure",
        &output,
        1,
    )
    .expect("package-path `ql build --json` should fail on build-plan dependency cycles");
    expect_empty_stderr(
        "project-build-package-json-build-plan-cycle",
        "package build json build-plan dependency cycle failure",
        &stderr,
    )
    .expect("build-plan dependency cycle failures should stay on stdout in json mode");

    let json = parse_json_output("project-build-package-json-build-plan-cycle", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        app_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");
    assert_eq!(json["built_targets"], serde_json::json!([]));
    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(
        json["failure"]["manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], JsonValue::Null);
    assert_eq!(json["failure"]["selected"], JsonValue::Null);
    assert_eq!(json["failure"]["dependency_only"], JsonValue::Null);
    assert_eq!(json["failure"]["kind"], JsonValue::Null);
    assert_eq!(
        json["failure"]["path"],
        app_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["error_kind"], "cycle");
    assert_eq!(json["failure"]["stage"], "build-plan");
    assert_eq!(
        json["failure"]["message"],
        "local package build dependencies contain a cycle"
    );
    assert_eq!(json["failure"]["owner_manifest_path"], JsonValue::Null);
    assert_eq!(
        json["failure"]["dependency_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["failure"]["cycle_manifests"],
        serde_json::json!([
            app_manifest.display().to_string().replace('\\', "/"),
            core_manifest.display().to_string().replace('\\', "/"),
            app_manifest.display().to_string().replace('\\', "/"),
        ])
    );
}

#[test]
fn build_package_path_reports_build_plan_dependency_cycle_without_stack_overflow() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-build-plan-cycle");
    let app_root = temp.path().join("app");
    let core_root = temp.path().join("core");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app source tree for non-json build plan cycle test");
    std::fs::create_dir_all(core_root.join("src"))
        .expect("create core source tree for non-json build plan cycle test");

    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
core = "../core"
"#,
    );
    temp.write(
        "core/qlang.toml",
        r#"
[package]
name = "core"

[dependencies]
app = "../app"
"#,
    );
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");
    temp.write("core/src/lib.ql", "pub fn answer() -> Int { return 42 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&app_root);
    let output = run_command_capture(
        &mut command,
        "`ql build` package build-plan dependency cycle",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-build-plan-cycle",
        "package build build-plan dependency cycle failure",
        &output,
        1,
    )
    .expect("package-path `ql build` should fail on build-plan dependency cycles");
    assert!(
        !stdout.contains("has overflowed its stack"),
        "non-json build-plan dependency cycle should not crash, got stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("error: `ql build` local package build dependencies contain a cycle"),
        "non-json build-plan dependency cycle should report the cycle instead of crashing, got:\n{stderr}"
    );
    assert!(
        stderr.contains("note: cycle manifests:"),
        "non-json build-plan dependency cycle should print cycle manifest detail, got:\n{stderr}"
    );
}

#[test]
fn build_package_path_json_reports_build_plan_dependency_target_discovery_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-build-plan-target-discovery");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dependency source tree for build plan target discovery test");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for build plan target discovery test");

    let dep_manifest = temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"

[lib]
path = "src/missing.ql"
"#,
    );
    temp.write("dep/src/api.ql", "pub fn exported() -> Int { return 1 }\n");
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` dependency target discovery failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-json-build-plan-target-discovery",
        "package build json build-plan dependency target discovery failure",
        &output,
        1,
    )
    .expect("package-path `ql build --json` should fail on build-plan dependency target discovery failures");
    expect_empty_stderr(
        "project-build-package-json-build-plan-target-discovery",
        "package build json build-plan dependency target discovery failure",
        &stderr,
    )
    .expect("build-plan dependency target discovery failures should stay on stdout in json mode");

    let json = parse_json_output(
        "project-build-package-json-build-plan-target-discovery",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");
    assert_eq!(json["built_targets"], serde_json::json!([]));
    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(
        json["failure"]["manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], JsonValue::Null);
    assert_eq!(json["failure"]["selected"], JsonValue::Null);
    assert_eq!(json["failure"]["dependency_only"], JsonValue::Null);
    assert_eq!(json["failure"]["kind"], JsonValue::Null);
    assert_eq!(
        json["failure"]["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["error_kind"], "dependency");
    assert_eq!(json["failure"]["stage"], "build-plan");
    assert_eq!(
        json["failure"]["owner_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["failure"]["dependency_manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["cycle_manifests"], JsonValue::Null);
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("build-plan dependency failure should expose a message")
            .contains("`[lib].path` declares missing target"),
        "build-plan dependency failure should preserve the dependency target discovery error: {json}"
    );
}

#[test]
fn build_package_path_json_reports_emit_interface_source_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-emit-interface-source-failure");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for emit-interface source failure test");

    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    let extra_source = temp.write(
        "app/src/extra.ql",
        "fn broken() -> Int { return \"oops\" }\n",
    );

    let app_lib_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["build"])
        .arg(&project_root)
        .args(["--emit-interface", "--json"]);
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface --json` package interface source failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-json-emit-interface-source-failure",
        "package interface source failure build json",
        &output,
        1,
    )
    .expect("package-path `ql build --emit-interface --json` should exit with code 1");
    expect_empty_stderr(
        "project-build-package-json-emit-interface-source-failure",
        "package interface source failure build json",
        &stderr,
    )
    .expect("package-path `ql build --emit-interface --json` should not print stderr");

    let json = parse_json_output(
        "project-build-package-json-emit-interface-source-failure",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");
    assert_eq!(json["emit_interface"], true);
    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(json["built_targets"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        json["failure"]["manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], "app");
    assert_eq!(json["failure"]["selected"], true);
    assert_eq!(json["failure"]["dependency_only"], false);
    assert_eq!(json["failure"]["kind"], "interface");
    assert_eq!(
        json["failure"]["path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["error_kind"], "package-sources");
    assert_eq!(json["failure"]["stage"], "emit-interface");
    assert_eq!(
        json["failure"]["message"],
        "package interface emission found 1 failing source file(s)"
    );
    assert_eq!(json["failure"]["output_path"], JsonValue::Null);
    assert_eq!(json["failure"]["source_root"], JsonValue::Null);
    assert_eq!(json["failure"]["failing_source_count"], 1);
    assert_eq!(
        json["failure"]["first_failing_source"],
        extra_source.display().to_string().replace('\\', "/")
    );

    expect_file_exists(
        "project-build-package-json-emit-interface-source-failure",
        &app_lib_output,
        "package library artifact",
        "package interface source failure build json",
    )
    .expect("package-path `ql build --emit-interface --json` should preserve the build artifact");
}

#[test]
fn build_package_path_json_reports_emit_interface_output_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-emit-interface-output-failure");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for emit-interface output failure test");

    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    let interface_output = project_root.join("app.qi");
    std::fs::create_dir_all(&interface_output)
        .expect("occupy the default interface output path with a directory");
    let app_lib_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["build"])
        .arg(&project_root)
        .args(["--emit-interface", "--json"]);
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface --json` package interface output failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-json-emit-interface-output-failure",
        "package interface output failure build json",
        &output,
        1,
    )
    .expect("package-path `ql build --emit-interface --json` should exit with code 1");
    expect_empty_stderr(
        "project-build-package-json-emit-interface-output-failure",
        "package interface output failure build json",
        &stderr,
    )
    .expect("package-path `ql build --emit-interface --json` should not print stderr");

    let json = parse_json_output(
        "project-build-package-json-emit-interface-output-failure",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");
    assert_eq!(json["emit_interface"], true);
    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(json["built_targets"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        json["failure"]["manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], "app");
    assert_eq!(json["failure"]["selected"], true);
    assert_eq!(json["failure"]["dependency_only"], false);
    assert_eq!(json["failure"]["kind"], "interface");
    assert_eq!(
        json["failure"]["path"],
        interface_output.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["error_kind"], "interface-output");
    assert_eq!(json["failure"]["stage"], "emit-interface");
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("interface output failure json should expose a message")
            .contains(&format!(
                "failed to write interface `{}`",
                interface_output.display().to_string().replace('\\', "/")
            )),
        "interface output failure json should preserve the write failure context: {json}"
    );
    assert_eq!(
        json["failure"]["output_path"],
        interface_output.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["source_root"], JsonValue::Null);
    assert_eq!(json["failure"]["failing_source_count"], JsonValue::Null);
    assert_eq!(json["failure"]["first_failing_source"], JsonValue::Null);

    expect_file_exists(
        "project-build-package-json-emit-interface-output-failure",
        &app_lib_output,
        "package library artifact",
        "package interface output failure build json",
    )
    .expect("package-path `ql build --emit-interface --json` should preserve the build artifact");
}

#[test]
fn build_project_json_reports_invalid_manifest_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-json-invalid-manifest");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(&project_root)
        .expect("create package root for invalid manifest json test");
    let manifest_path = temp.write(
        "app/qlang.toml",
        r#"
[package
name = "app"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(&mut command, "`ql build --json` invalid manifest");
    let (stdout, stderr) = expect_exit_code(
        "project-build-json-invalid-manifest",
        "invalid manifest build json failure",
        &output,
        1,
    )
    .expect("project-path `ql build --json` invalid manifest should exit with code 1");
    expect_empty_stderr(
        "project-build-json-invalid-manifest",
        "invalid manifest build json failure",
        &stderr,
    )
    .expect("project-path `ql build --json` invalid manifest should not print stderr");

    let json = parse_json_output("project-build-json-invalid-manifest", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(json["project_manifest_path"], JsonValue::Null);
    assert_eq!(json["status"], "failed");
    assert_eq!(json["built_targets"], serde_json::json!([]));
    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(
        json["failure"]["manifest_path"],
        manifest_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], JsonValue::Null);
    assert_eq!(json["failure"]["selected"], JsonValue::Null);
    assert_eq!(json["failure"]["dependency_only"], JsonValue::Null);
    assert_eq!(json["failure"]["kind"], JsonValue::Null);
    assert_eq!(
        json["failure"]["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["error_kind"], "manifest");
    assert_eq!(json["failure"]["stage"], "manifest-load");
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("invalid manifest json failure should expose a message")
            .contains("invalid manifest"),
        "invalid manifest json failure should preserve the manifest parse failure: {json}"
    );
}

#[test]
fn build_project_json_reports_selector_miss_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-json-selector-miss");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app package source tree");
    std::fs::create_dir_all(project_root.join("packages/tool/src"))
        .expect("create tool package source tree");

    let workspace_manifest = temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn app_value() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        "pub fn tool_value() -> Int { return 2 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["build"])
        .arg(&project_root)
        .args(["--package", "missing", "--json"]);
    let output = run_command_capture(&mut command, "`ql build --json --package missing`");
    let (stdout, stderr) = expect_exit_code(
        "project-build-json-selector-miss",
        "selector miss build json failure",
        &output,
        1,
    )
    .expect("project-path `ql build --json --package missing` should exit with code 1");
    expect_empty_stderr(
        "project-build-json-selector-miss",
        "selector miss build json failure",
        &stderr,
    )
    .expect("project-path `ql build --json --package missing` should not print stderr");

    let json = parse_json_output("project-build-json-selector-miss", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        workspace_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");
    assert_eq!(json["built_targets"], serde_json::json!([]));
    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(json["failure"]["manifest_path"], JsonValue::Null);
    assert_eq!(json["failure"]["package_name"], JsonValue::Null);
    assert_eq!(json["failure"]["selected"], JsonValue::Null);
    assert_eq!(json["failure"]["dependency_only"], JsonValue::Null);
    assert_eq!(json["failure"]["kind"], JsonValue::Null);
    assert_eq!(
        json["failure"]["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["error_kind"], "selector");
    assert_eq!(json["failure"]["stage"], "target-selection");
    assert_eq!(json["failure"]["selector"], "package `missing`");
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("selector miss json failure should expose a message")
            .contains("target selector matched no build targets"),
        "selector miss json failure should describe the selector mismatch: {json}"
    );
}

#[test]
fn build_package_path_json_reports_target_failure_after_partial_build() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-failure");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source tree");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source tree");

    let dep_manifest = temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write("dep/src/lib.ql", "pub fn exported() -> Int { return 1 }\n");
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    let app_main_path = temp.write("app/src/main.ql", "fn main() -> Int { return \"oops\" }\n");

    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let app_lib_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` package target failure after partial build",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-json-failure",
        "package target failure after partial build json",
        &output,
        1,
    )
    .expect("package-path `ql build --json` target failure should exit with code 1");
    expect_empty_stderr(
        "project-build-package-json-failure",
        "package target failure after partial build json",
        &stderr,
    )
    .expect("package-path `ql build --json` target failure should not print stderr");

    let json = parse_json_output("project-build-package-json-failure", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");
    assert_eq!(json["requested_emit"], "llvm-ir");
    assert_eq!(json["requested_profile"], "debug");
    assert_eq!(json["profile_overridden"], false);
    assert_eq!(json["emit_interface"], false);

    let built_targets = json["built_targets"]
        .as_array()
        .expect("build failure json should expose built_targets");
    assert_eq!(built_targets.len(), 2);
    assert_eq!(
        built_targets[0]["manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(built_targets[0]["package_name"], "dep");
    assert_eq!(built_targets[0]["selected"], false);
    assert_eq!(built_targets[0]["dependency_only"], true);
    assert_eq!(built_targets[0]["kind"], "lib");
    assert_eq!(built_targets[0]["path"], "src/lib.ql");
    assert_eq!(built_targets[0]["emit"], "staticlib");
    assert_eq!(
        built_targets[0]["artifact_path"],
        dep_output.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        built_targets[1]["manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(built_targets[1]["package_name"], "app");
    assert_eq!(built_targets[1]["selected"], true);
    assert_eq!(built_targets[1]["dependency_only"], false);
    assert_eq!(built_targets[1]["kind"], "lib");
    assert_eq!(built_targets[1]["path"], "src/lib.ql");
    assert_eq!(built_targets[1]["emit"], "staticlib");
    assert_eq!(
        built_targets[1]["artifact_path"],
        app_lib_output.display().to_string().replace('\\', "/")
    );

    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(
        json["failure"]["manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], "app");
    assert_eq!(json["failure"]["selected"], true);
    assert_eq!(json["failure"]["dependency_only"], false);
    assert_eq!(json["failure"]["kind"], "bin");
    assert_eq!(json["failure"]["path"], "src/main.ql");
    assert_eq!(json["failure"]["error_kind"], "diagnostics");
    assert_eq!(json["failure"]["message"], "build produced diagnostics");
    assert_eq!(
        json["failure"]["diagnostic_file"]["path"],
        app_main_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["failure"]["diagnostic_file"]["diagnostics"][0]["message"],
        "return value has type mismatch: expected `Int`, found `String`"
    );

    expect_file_exists(
        "project-build-package-json-failure",
        &dep_output,
        "dependency package artifact",
        "package target failure after partial build json",
    )
    .expect("package-path `ql build --json` should preserve the dependency artifact");
    expect_file_exists(
        "project-build-package-json-failure",
        &app_lib_output,
        "package library artifact",
        "package target failure after partial build json",
    )
    .expect(
        "package-path `ql build --json` should preserve the already-built package library artifact",
    );
}

#[test]
fn build_package_path_json_reports_target_prep_dependency_extern_conflict() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-target-prep-extern-conflict");
    let dep_a_root = temp.path().join("dep-a");
    let dep_b_root = temp.path().join("dep-b");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_a_root.join("src"))
        .expect("create dep-a source tree for target-prep extern conflict");
    std::fs::create_dir_all(dep_b_root.join("src"))
        .expect("create dep-b source tree for target-prep extern conflict");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for target-prep extern conflict");

    let dep_a_manifest = temp.write(
        "dep-a/qlang.toml",
        r#"
[package]
name = "demo.shared.alpha"
"#,
    );
    temp.write(
        "dep-a/src/lib.ql",
        "extern \"c\" pub fn q_shared() -> Int { return 1 }\n",
    );
    let dep_b_manifest = temp.write(
        "dep-b/qlang.toml",
        r#"
[package]
name = "demo.shared.beta"
"#,
    );
    temp.write(
        "dep-b/src/lib.ql",
        "extern \"c\" pub fn q_shared() -> Int { return 2 }\n",
    );
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
alpha = "../dep-a"
beta = "../dep-b"
"#,
    );
    temp.write(
        "app/src/main.ql",
        "use demo.shared.alpha.q_shared as alpha_shared\nuse demo.shared.beta.q_shared as beta_shared\n\nfn main() -> Int { return alpha_shared() + beta_shared() }\n",
    );

    let dep_a_output = static_library_output_path(&dep_a_root.join("target/ql/debug"), "lib");
    let dep_b_output = static_library_output_path(&dep_b_root.join("target/ql/debug"), "lib");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` target-prep dependency extern conflict",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-json-target-prep-extern-conflict",
        "package build json target-prep dependency extern conflict",
        &output,
        1,
    )
    .expect(
        "package-path `ql build --json` should fail on target-prep dependency extern conflicts",
    );
    expect_empty_stderr(
        "project-build-package-json-target-prep-extern-conflict",
        "package build json target-prep dependency extern conflict",
        &stderr,
    )
    .expect("target-prep dependency extern conflicts should stay on stdout in json mode");

    let json = parse_json_output(
        "project-build-package-json-target-prep-extern-conflict",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");

    let built_targets = json["built_targets"]
        .as_array()
        .expect("target-prep conflict json should expose built_targets");
    assert_eq!(built_targets.len(), 2);
    assert_eq!(
        built_targets[0]["manifest_path"],
        dep_a_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(built_targets[0]["package_name"], "demo.shared.alpha");
    assert_eq!(built_targets[0]["selected"], false);
    assert_eq!(built_targets[0]["dependency_only"], true);
    assert_eq!(built_targets[0]["kind"], "lib");
    assert_eq!(
        built_targets[0]["artifact_path"],
        dep_a_output.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        built_targets[1]["manifest_path"],
        dep_b_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(built_targets[1]["package_name"], "demo.shared.beta");
    assert_eq!(built_targets[1]["selected"], false);
    assert_eq!(built_targets[1]["dependency_only"], true);
    assert_eq!(built_targets[1]["kind"], "lib");
    assert_eq!(
        built_targets[1]["artifact_path"],
        dep_b_output.display().to_string().replace('\\', "/")
    );

    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(
        json["failure"]["manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], "app");
    assert_eq!(json["failure"]["selected"], true);
    assert_eq!(json["failure"]["dependency_only"], false);
    assert_eq!(json["failure"]["kind"], "bin");
    assert_eq!(json["failure"]["path"], "src/main.ql");
    assert_eq!(json["failure"]["stage"], "target-prep");
    assert_eq!(json["failure"]["error_kind"], "dependency-extern-conflict");
    assert_eq!(json["failure"]["symbol"], "q_shared");
    assert_eq!(
        json["failure"]["first_dependency_package"],
        "demo.shared.alpha"
    );
    assert_eq!(
        json["failure"]["first_dependency_manifest_path"],
        dep_a_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["failure"]["conflicting_dependency_package"],
        "demo.shared.beta"
    );
    assert_eq!(
        json["failure"]["conflicting_dependency_manifest_path"],
        dep_b_manifest.display().to_string().replace('\\', "/")
    );
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("target-prep conflict json should expose a message")
            .contains("conflicting direct dependency extern imports"),
        "target-prep conflict json should preserve the extern collision detail: {json}"
    );

    expect_file_exists(
        "project-build-package-json-target-prep-extern-conflict",
        &dep_a_output,
        "dep-a artifact",
        "package build json target-prep dependency extern conflict",
    )
    .expect("target-prep conflict should preserve dep-a artifact");
    expect_file_exists(
        "project-build-package-json-target-prep-extern-conflict",
        &dep_b_output,
        "dep-b artifact",
        "package build json target-prep dependency extern conflict",
    )
    .expect("target-prep conflict should preserve dep-b artifact");
}

#[test]
fn build_package_path_json_reports_dependency_public_function_local_conflict() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-public-function-local-conflict");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dep source tree for dependency public function local conflict");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for dependency public function local conflict");

    let dep_manifest = temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "dep/src/lib.ql",
        "pub fn add(left: Int, right: Int) -> Int { return left + right }\n",
    );
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    temp.write(
        "app/src/main.ql",
        "use dep.add as sum\n\nfn add(left: Int, right: Int) -> Int { return left - right }\n\nfn main() -> Int { return sum(8, 5) }\n",
    );

    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` dependency public function local conflict",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-json-public-function-local-conflict",
        "package build json dependency public function local conflict",
        &output,
        1,
    )
    .expect(
        "package-path `ql build --json` should fail when the root source already defines the dependency bridge symbol",
    );
    expect_empty_stderr(
        "project-build-package-json-public-function-local-conflict",
        "package build json dependency public function local conflict",
        &stderr,
    )
    .expect("dependency public function local conflicts should stay on stdout in json mode");

    let json = parse_json_output(
        "project-build-package-json-public-function-local-conflict",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");

    let built_targets = json["built_targets"]
        .as_array()
        .expect("local conflict json should expose built_targets");
    assert_eq!(built_targets.len(), 1);
    assert_eq!(
        built_targets[0]["manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(built_targets[0]["package_name"], "dep");
    assert_eq!(built_targets[0]["selected"], false);
    assert_eq!(built_targets[0]["dependency_only"], true);
    assert_eq!(built_targets[0]["kind"], "lib");
    assert_eq!(
        built_targets[0]["artifact_path"],
        dep_output.display().to_string().replace('\\', "/")
    );

    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(
        json["failure"]["manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], "app");
    assert_eq!(json["failure"]["selected"], true);
    assert_eq!(json["failure"]["dependency_only"], false);
    assert_eq!(json["failure"]["kind"], "bin");
    assert_eq!(json["failure"]["path"], "src/main.ql");
    assert_eq!(json["failure"]["stage"], "target-prep");
    assert_eq!(
        json["failure"]["error_kind"],
        "dependency-function-local-conflict"
    );
    assert_eq!(json["failure"]["symbol"], "add");
    assert_eq!(json["failure"]["dependency_package"], "dep");
    assert_eq!(
        json["failure"]["dependency_manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("local conflict json should expose a message")
            .contains("already defines the same top-level name"),
        "local conflict json should preserve the bridge-name collision detail: {json}"
    );

    expect_file_exists(
        "project-build-package-json-public-function-local-conflict",
        &dep_output,
        "dependency package artifact",
        "package build json dependency public function local conflict",
    )
    .expect("local conflict should preserve the dependency package artifact");
}

#[test]
fn build_package_path_json_reports_dependency_public_value_local_conflict() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-public-value-local-conflict");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dep source tree for dependency public value local conflict");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for dependency public value local conflict");

    let dep_manifest = temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write("dep/src/lib.ql", "pub const VALUE: Int = 7\n");
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    temp.write(
        "app/src/main.ql",
        "use dep.VALUE as VALUE_ALIAS\n\nconst VALUE: Int = 2\n\nfn main() -> Int { return VALUE_ALIAS }\n",
    );

    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` dependency public value local conflict",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-json-public-value-local-conflict",
        "package build json dependency public value local conflict",
        &output,
        1,
    )
    .expect(
        "package-path `ql build --json` should fail when the root source already defines the dependency public value bridge symbol",
    );
    expect_empty_stderr(
        "project-build-package-json-public-value-local-conflict",
        "package build json dependency public value local conflict",
        &stderr,
    )
    .expect("dependency public value local conflicts should stay on stdout in json mode");

    let json = parse_json_output(
        "project-build-package-json-public-value-local-conflict",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");

    let built_targets = json["built_targets"]
        .as_array()
        .expect("local conflict json should expose built_targets");
    assert_eq!(built_targets.len(), 1);
    assert_eq!(
        built_targets[0]["manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(built_targets[0]["package_name"], "dep");
    assert_eq!(built_targets[0]["selected"], false);
    assert_eq!(built_targets[0]["dependency_only"], true);
    assert_eq!(built_targets[0]["kind"], "lib");
    assert_eq!(
        built_targets[0]["artifact_path"],
        dep_output.display().to_string().replace('\\', "/")
    );

    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(
        json["failure"]["manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], "app");
    assert_eq!(json["failure"]["selected"], true);
    assert_eq!(json["failure"]["dependency_only"], false);
    assert_eq!(json["failure"]["kind"], "bin");
    assert_eq!(json["failure"]["path"], "src/main.ql");
    assert_eq!(json["failure"]["stage"], "target-prep");
    assert_eq!(
        json["failure"]["error_kind"],
        "dependency-value-local-conflict"
    );
    assert_eq!(json["failure"]["symbol"], "VALUE");
    assert_eq!(json["failure"]["dependency_package"], "dep");
    assert_eq!(
        json["failure"]["dependency_manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("local conflict json should expose a message")
            .contains("already defines the same top-level name"),
        "local conflict json should preserve the bridge-name collision detail: {json}"
    );

    expect_file_exists(
        "project-build-package-json-public-value-local-conflict",
        &dep_output,
        "dependency package artifact",
        "package build json dependency public value local conflict",
    )
    .expect("local conflict should preserve the dependency package artifact");
}

#[test]
fn build_package_path_json_reports_implicit_dependency_public_function_local_conflict() {
    let workspace_root = workspace_root();
    let temp =
        TempDir::new("ql-project-build-package-json-implicit-public-function-local-conflict");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dep source tree for implicit dependency public function local conflict");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for implicit dependency public function local conflict");

    let dep_manifest = temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "dep/src/lib.ql",
        "pub fn add_one(value: Int) -> Int { return value + 1 }\npub const APPLY: (Int) -> Int = add_one\n",
    );
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    temp.write(
        "app/src/main.ql",
        "use dep.APPLY as RUN\n\nfn add_one(value: Int) -> Int { return value - 1 }\n\nfn main() -> Int { return RUN(8) }\n",
    );

    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` implicit dependency public function local conflict",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-json-implicit-public-function-local-conflict",
        "package build json implicit dependency public function local conflict",
        &output,
        1,
    )
    .expect(
        "package-path `ql build --json` should fail when an implicit dependency function bridge collides with a local top-level function",
    );
    expect_empty_stderr(
        "project-build-package-json-implicit-public-function-local-conflict",
        "package build json implicit dependency public function local conflict",
        &stderr,
    )
    .expect(
        "implicit dependency public function local conflicts should stay on stdout in json mode",
    );

    let json = parse_json_output(
        "project-build-package-json-implicit-public-function-local-conflict",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");

    let built_targets = json["built_targets"]
        .as_array()
        .expect("implicit local conflict json should expose built_targets");
    assert_eq!(built_targets.len(), 1);
    assert_eq!(
        built_targets[0]["manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(built_targets[0]["package_name"], "dep");
    assert_eq!(built_targets[0]["selected"], false);
    assert_eq!(built_targets[0]["dependency_only"], true);
    assert_eq!(built_targets[0]["kind"], "lib");
    assert_eq!(
        built_targets[0]["artifact_path"],
        dep_output.display().to_string().replace('\\', "/")
    );

    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(
        json["failure"]["manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], "app");
    assert_eq!(json["failure"]["selected"], true);
    assert_eq!(json["failure"]["dependency_only"], false);
    assert_eq!(json["failure"]["kind"], "bin");
    assert_eq!(json["failure"]["path"], "src/main.ql");
    assert_eq!(json["failure"]["stage"], "target-prep");
    assert_eq!(
        json["failure"]["error_kind"],
        "dependency-function-local-conflict"
    );
    assert_eq!(json["failure"]["symbol"], "add_one");
    assert_eq!(json["failure"]["dependency_package"], "dep");
    assert_eq!(
        json["failure"]["dependency_manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("implicit local conflict json should expose a message")
            .contains("already defines the same top-level name"),
        "implicit local conflict json should preserve the bridge-name collision detail: {json}"
    );

    expect_file_exists(
        "project-build-package-json-implicit-public-function-local-conflict",
        &dep_output,
        "dependency package artifact",
        "package build json implicit dependency public function local conflict",
    )
    .expect("implicit local conflict should preserve the dependency package artifact");
}

#[test]
fn build_package_path_json_ignores_unused_dependency_extern_conflicts() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-unused-extern-conflict");
    let dep_a_root = temp.path().join("dep-a");
    let dep_b_root = temp.path().join("dep-b");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_a_root.join("src"))
        .expect("create dep-a source tree for unused extern conflict");
    std::fs::create_dir_all(dep_b_root.join("src"))
        .expect("create dep-b source tree for unused extern conflict");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for unused extern conflict");

    let dep_a_manifest = temp.write(
        "dep-a/qlang.toml",
        r#"
[package]
name = "demo.shared.alpha"
"#,
    );
    temp.write(
        "dep-a/src/lib.ql",
        "extern \"c\" pub fn q_shared() -> Int { return 1 }\n",
    );
    let dep_b_manifest = temp.write(
        "dep-b/qlang.toml",
        r#"
[package]
name = "demo.shared.beta"
"#,
    );
    temp.write(
        "dep-b/src/lib.ql",
        "extern \"c\" pub fn q_shared() -> Int { return 2 }\n",
    );
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
alpha = "../dep-a"
beta = "../dep-b"
"#,
    );
    temp.write(
        "app/src/main.ql",
        "use demo.shared.alpha.q_shared as shared\n\nfn main() -> Int { return shared() }\n",
    );

    let dep_a_output = static_library_output_path(&dep_a_root.join("target/ql/debug"), "lib");
    let dep_b_output = static_library_output_path(&dep_b_root.join("target/ql/debug"), "lib");
    let app_output = project_root.join("target/ql/debug/main.ll");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` ignores unused dependency extern conflicts",
    );
    let (stdout, stderr) = expect_success(
        "project-build-package-json-unused-extern-conflict",
        "package build json unused dependency extern conflict",
        &output,
    )
    .expect("package-path `ql build --json` should ignore unused dependency extern conflicts");
    expect_empty_stderr(
        "project-build-package-json-unused-extern-conflict",
        "package build json unused dependency extern conflict",
        &stderr,
    )
    .expect("unused dependency extern conflict build should not print stderr");

    let json = parse_json_output("project-build-package-json-unused-extern-conflict", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "ok");
    assert_eq!(json["failure"], JsonValue::Null);
    let built_targets = json["built_targets"]
        .as_array()
        .expect("unused dependency extern conflict json should expose built_targets");
    assert_eq!(built_targets.len(), 3);
    assert_eq!(
        built_targets[0],
        serde_json::json!({
            "manifest_path": dep_a_manifest.display().to_string().replace('\\', "/"),
            "package_name": "demo.shared.alpha",
            "selected": false,
            "dependency_only": true,
            "kind": "lib",
            "path": "src/lib.ql",
            "emit": "staticlib",
            "profile": "debug",
            "artifact_path": dep_a_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        })
    );
    assert_eq!(
        built_targets[1],
        serde_json::json!({
            "manifest_path": dep_b_manifest.display().to_string().replace('\\', "/"),
            "package_name": "demo.shared.beta",
            "selected": false,
            "dependency_only": true,
            "kind": "lib",
            "path": "src/lib.ql",
            "emit": "staticlib",
            "profile": "debug",
            "artifact_path": dep_b_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        })
    );
    assert_eq!(
        built_targets[2],
        serde_json::json!({
            "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
            "package_name": "app",
            "selected": true,
            "dependency_only": false,
            "kind": "bin",
            "path": "src/main.ql",
            "emit": "llvm-ir",
            "profile": "debug",
            "artifact_path": app_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        })
    );

    expect_file_exists(
        "project-build-package-json-unused-extern-conflict",
        &dep_a_output,
        "dep-a artifact",
        "package build json unused dependency extern conflict",
    )
    .expect("unused dependency extern conflict build should preserve dep-a artifact");
    expect_file_exists(
        "project-build-package-json-unused-extern-conflict",
        &dep_b_output,
        "dep-b artifact",
        "package build json unused dependency extern conflict",
    )
    .expect("unused dependency extern conflict build should preserve dep-b artifact");
    expect_file_exists(
        "project-build-package-json-unused-extern-conflict",
        &app_output,
        "app artifact",
        "package build json unused dependency extern conflict",
    )
    .expect("unused dependency extern conflict build should emit the selected artifact");
}

#[test]
fn build_project_json_reports_emit_interface_output_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-json-emit-interface-output-failure");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for json emit-interface output failure test");
    let manifest_path = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn util() -> Int { return 1 }\n");
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");

    let lib_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");
    let main_output = project_root.join("target/ql/debug/main.ll");
    let interface_output = project_root.join("app.qi");
    std::fs::create_dir_all(&interface_output)
        .expect("create blocking interface directory for json emit-interface output failure test");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["build"])
        .arg(&project_root)
        .args(["--emit-interface", "--json"]);
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface --json` blocked interface output path",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-json-emit-interface-output-failure",
        "project build json emit-interface output failure",
        &output,
        1,
    )
    .expect("project-path `ql build --emit-interface --json` should exit with code 1");
    expect_empty_stderr(
        "project-build-json-emit-interface-output-failure",
        "project build json emit-interface output failure",
        &stderr,
    )
    .expect("project-path `ql build --emit-interface --json` should not print stderr");

    let json = parse_json_output("project-build-json-emit-interface-output-failure", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        manifest_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");
    assert_eq!(json["emit_interface"], true);
    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(
        json["failure"]["manifest_path"],
        manifest_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], "app");
    assert_eq!(json["failure"]["selected"], true);
    assert_eq!(json["failure"]["dependency_only"], false);
    assert_eq!(json["failure"]["kind"], "interface");
    assert_eq!(
        json["failure"]["path"],
        interface_output.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["error_kind"], "interface-output");
    assert_eq!(json["failure"]["stage"], "emit-interface");
    assert_eq!(
        json["failure"]["output_path"],
        interface_output.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["source_root"], JsonValue::Null);
    assert_eq!(json["failure"]["failing_source_count"], JsonValue::Null);
    assert_eq!(json["failure"]["first_failing_source"], JsonValue::Null);
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("emit-interface output failure should expose a message")
            .contains("failed to write interface"),
        "emit-interface output failure should preserve the interface write failure: {json}"
    );

    let built_targets = json["built_targets"]
        .as_array()
        .expect("emit-interface output failure should expose built_targets");
    assert_eq!(built_targets.len(), 2);
    assert_eq!(
        built_targets[0]["artifact_path"],
        lib_output.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        built_targets[1]["artifact_path"],
        main_output.display().to_string().replace('\\', "/")
    );
    expect_file_exists(
        "project-build-json-emit-interface-output-failure",
        &lib_output,
        "package library artifact",
        "project build json emit-interface output failure",
    )
    .expect("project-path `ql build --emit-interface --json` should preserve the library artifact");
    expect_file_exists(
        "project-build-json-emit-interface-output-failure",
        &main_output,
        "package binary artifact",
        "project build json emit-interface output failure",
    )
    .expect("project-path `ql build --emit-interface --json` should preserve the binary artifact");
    assert!(
        interface_output.is_dir(),
        "project-path `ql build --emit-interface --json` should not replace the blocked interface output directory"
    );
}

#[test]
fn build_package_path_uses_manifest_default_release_profile() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-manifest-profile");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for manifest profile build test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[profile]
default = "release"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn util() -> Int { return 1 }\n");

    let output_path = static_library_output_path(&project_root.join("target/ql/release"), "lib");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql build` manifest default profile");
    let (stdout, stderr) = expect_success(
        "project-build-manifest-profile",
        "manifest default profile build",
        &output,
    )
    .expect("package-path `ql build` should honor the manifest default profile");
    expect_empty_stderr(
        "project-build-manifest-profile",
        "manifest default profile build",
        &stderr,
    )
    .expect("manifest default profile build should not print stderr");
    expect_stdout_contains_all(
        "project-build-manifest-profile",
        &stdout.replace('\\', "/"),
        &[&format!("wrote staticlib: {}", output_path.display()).replace('\\', "/")],
    )
    .expect("package-path `ql build` should write artifacts under the manifest-selected profile");
    expect_file_exists(
        "project-build-manifest-profile",
        &output_path,
        "manifest default profile artifact",
        "manifest default profile build",
    )
    .expect("manifest default profile build should emit the release artifact");
}

#[test]
fn build_package_path_profile_flag_overrides_manifest_default_profile() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-profile-override");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for profile override build test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[profile]
default = "release"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn util() -> Int { return 1 }\n");

    let debug_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");
    let release_output = static_library_output_path(&project_root.join("target/ql/release"), "lib");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["build"])
        .arg(&project_root)
        .args(["--profile", "debug"]);
    let output = run_command_capture(&mut command, "`ql build --profile` manifest override");
    let (stdout, stderr) = expect_success(
        "project-build-profile-override",
        "manifest profile override build",
        &output,
    )
    .expect("package-path `ql build --profile` should override the manifest default");
    expect_empty_stderr(
        "project-build-profile-override",
        "manifest profile override build",
        &stderr,
    )
    .expect("manifest profile override build should not print stderr");
    expect_stdout_contains_all(
        "project-build-profile-override",
        &stdout.replace('\\', "/"),
        &[&format!("wrote staticlib: {}", debug_output.display()).replace('\\', "/")],
    )
    .expect("`--profile debug` should write artifacts under the debug profile");
    expect_file_exists(
        "project-build-profile-override",
        &debug_output,
        "manifest profile override artifact",
        "manifest profile override build",
    )
    .expect("profile override build should emit the debug artifact");
    assert!(
        !release_output.exists(),
        "`ql build --profile debug` should not silently fall back to the manifest-selected release profile"
    );
}

#[test]
fn build_workspace_path_uses_workspace_default_profile() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-workspace-profile");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create workspace package source tree for workspace profile build test");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]

[profile]
default = "release"
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn util() -> Int { return 1 }\n",
    );

    let output_path =
        static_library_output_path(&project_root.join("packages/app/target/ql/release"), "lib");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql build` workspace default profile");
    let (stdout, stderr) = expect_success(
        "project-build-workspace-profile",
        "workspace default profile build",
        &output,
    )
    .expect("workspace-path `ql build` should honor the workspace default profile");
    expect_empty_stderr(
        "project-build-workspace-profile",
        "workspace default profile build",
        &stderr,
    )
    .expect("workspace default profile build should not print stderr");
    expect_stdout_contains_all(
        "project-build-workspace-profile",
        &stdout.replace('\\', "/"),
        &[&format!("wrote staticlib: {}", output_path.display()).replace('\\', "/")],
    )
    .expect(
        "workspace-path `ql build` should write artifacts under the workspace-selected profile",
    );
    expect_file_exists(
        "project-build-workspace-profile",
        &output_path,
        "workspace default profile artifact",
        "workspace default profile build",
    )
    .expect("workspace default profile build should emit the release artifact");
}

#[test]
fn build_workspace_member_source_file_uses_workspace_default_profile() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-workspace-source-profile");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create workspace package source tree for workspace source profile build test");

    let workspace_manifest = temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]

[profile]
default = "release"
"#,
    );
    let app_manifest = temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let lib_path = temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn util() -> Int { return 1 }\n",
    );

    let output_path =
        static_library_output_path(&project_root.join("packages/app/target/ql/release"), "lib");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&lib_path).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` workspace member source default profile",
    );
    let (stdout, stderr) = expect_success(
        "project-build-workspace-source-profile",
        "workspace member source default profile build",
        &output,
    )
    .expect(
        "workspace member source path `ql build --json` should honor the workspace default profile",
    );
    expect_empty_stderr(
        "project-build-workspace-source-profile",
        "workspace member source default profile build",
        &stderr,
    )
    .expect("workspace member source default profile build should not print stderr");

    let json = parse_json_output("project-build-workspace-source-profile", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        lib_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        workspace_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["requested_emit"], "llvm-ir");
    assert_eq!(json["requested_profile"], "debug");
    assert_eq!(json["profile_overridden"], false);
    assert_eq!(json["emit_interface"], false);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["failure"], JsonValue::Null);
    assert_eq!(
        json["interfaces"],
        serde_json::json!([
            {
                "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "selected": true,
                "status": "wrote",
                "path": project_root.join("packages/app/app.qi").display().to_string().replace('\\', "/"),
            }
        ])
    );
    assert_eq!(
        json["built_targets"],
        serde_json::json!([
            {
                "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "selected": true,
                "dependency_only": false,
                "kind": "lib",
                "path": "src/lib.ql",
                "emit": "staticlib",
                "profile": "release",
                "artifact_path": output_path.display().to_string().replace('\\', "/"),
                "c_header_path": JsonValue::Null,
            }
        ])
    );
    expect_file_exists(
        "project-build-workspace-source-profile",
        &output_path,
        "workspace member source default profile artifact",
        "workspace member source default profile build",
    )
    .expect("workspace member source default profile build should emit the release artifact");
}

#[test]
fn build_workspace_path_builds_each_member_target() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-workspace");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app package source tree");
    std::fs::create_dir_all(project_root.join("packages/tool/src"))
        .expect("create tool package source tree");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn app_value() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        "pub fn tool_value() -> Int { return 2 }\n",
    );

    let app_output =
        static_library_output_path(&project_root.join("packages/app/target/ql/debug"), "lib");
    let tool_output =
        static_library_output_path(&project_root.join("packages/tool/target/ql/debug"), "lib");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql build` workspace path");
    let (stdout, stderr) =
        expect_success("project-build-workspace", "workspace path build", &output)
            .expect("workspace path build should succeed");
    expect_empty_stderr("project-build-workspace", "workspace path build", &stderr)
        .expect("workspace path build should not print stderr");
    expect_stdout_contains_all(
        "project-build-workspace",
        &stdout.replace('\\', "/"),
        &[
            &format!("wrote staticlib: {}", app_output.display()).replace('\\', "/"),
            &format!("wrote staticlib: {}", tool_output.display()).replace('\\', "/"),
        ],
    )
    .expect("workspace path build should report each member artifact");

    expect_file_exists(
        "project-build-workspace",
        &app_output,
        "workspace app artifact",
        "workspace path build",
    )
    .expect("workspace path build should emit the app artifact");
    expect_file_exists(
        "project-build-workspace",
        &tool_output,
        "workspace tool artifact",
        "workspace path build",
    )
    .expect("workspace path build should emit the tool artifact");
}

#[test]
fn build_workspace_path_selects_requested_package_targets() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-selector");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app package source tree");
    std::fs::create_dir_all(project_root.join("packages/tool/src"))
        .expect("create tool package source tree");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn app_value() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        "pub fn tool_value() -> Int { return 2 }\n",
    );

    let app_output =
        static_library_output_path(&project_root.join("packages/app/target/ql/debug"), "lib");
    let tool_output =
        static_library_output_path(&project_root.join("packages/tool/target/ql/debug"), "lib");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["build"])
        .arg(&project_root)
        .args(["--package", "app"]);
    let output = run_command_capture(&mut command, "`ql build --package` workspace path");
    let (stdout, stderr) = expect_success(
        "project-build-package-selector",
        "workspace package selector build",
        &output,
    )
    .expect("workspace-path `ql build --package` should succeed");
    expect_empty_stderr(
        "project-build-package-selector",
        "workspace package selector build",
        &stderr,
    )
    .expect("workspace-path `ql build --package` should not print stderr");
    expect_stdout_contains_all(
        "project-build-package-selector",
        &stdout.replace('\\', "/"),
        &[&format!("wrote staticlib: {}", app_output.display()).replace('\\', "/")],
    )
    .expect("workspace-path `ql build --package` should report only the selected package artifact");
    expect_file_exists(
        "project-build-package-selector",
        &app_output,
        "selected package artifact",
        "workspace package selector build",
    )
    .expect("workspace-path `ql build --package` should emit the selected package artifact");
    assert!(
        !tool_output.exists(),
        "workspace-path `ql build --package` should not build unselected package artifacts"
    );
}

#[test]
fn build_workspace_package_selector_builds_local_dependency_packages_first() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-workspace-dependency-closure");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app package source tree");
    std::fs::create_dir_all(project_root.join("packages/core/src"))
        .expect("create core package source tree");
    std::fs::create_dir_all(project_root.join("packages/tool/src"))
        .expect("create tool package source tree");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/core", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
core = "../core"
"#,
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn app_value() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/core/src/lib.ql",
        "pub fn core_value() -> Int { return 2 }\n",
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        "pub fn tool_value() -> Int { return 3 }\n",
    );

    let app_output =
        static_library_output_path(&project_root.join("packages/app/target/ql/debug"), "lib");
    let core_output =
        static_library_output_path(&project_root.join("packages/core/target/ql/debug"), "lib");
    let tool_output =
        static_library_output_path(&project_root.join("packages/tool/target/ql/debug"), "lib");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["build"])
        .arg(&project_root)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql build --package` workspace dependency closure",
    );
    let (stdout, stderr) = expect_success(
        "project-build-workspace-dependency-closure",
        "workspace package selector dependency closure build",
        &output,
    )
    .expect("workspace-path `ql build --package` should build local dependency packages first");
    expect_empty_stderr(
        "project-build-workspace-dependency-closure",
        "workspace package selector dependency closure build",
        &stderr,
    )
    .expect("workspace package selector dependency closure build should not print stderr");

    let normalized_stdout = stdout.replace('\\', "/");
    let core_fragment = format!("wrote staticlib: {}", core_output.display()).replace('\\', "/");
    let app_fragment = format!("wrote staticlib: {}", app_output.display()).replace('\\', "/");
    expect_stdout_contains_all(
        "project-build-workspace-dependency-closure",
        &normalized_stdout,
        &[&core_fragment, &app_fragment],
    )
    .expect("workspace package selector dependency closure build should report dependency and root artifacts");

    let core_index = normalized_stdout
        .find(&core_fragment)
        .expect("workspace dependency artifact should be present in stdout");
    let app_index = normalized_stdout
        .find(&app_fragment)
        .expect("workspace root artifact should be present in stdout");
    assert!(
        core_index < app_index,
        "workspace dependency package should be built before the selected root package, got:\n{stdout}"
    );

    expect_file_exists(
        "project-build-workspace-dependency-closure",
        &core_output,
        "workspace dependency artifact",
        "workspace package selector dependency closure build",
    )
    .expect(
        "workspace package selector dependency closure build should emit the dependency artifact",
    );
    expect_file_exists(
        "project-build-workspace-dependency-closure",
        &app_output,
        "workspace selected package artifact",
        "workspace package selector dependency closure build",
    )
    .expect("workspace package selector dependency closure build should emit the selected package artifact");
    assert!(
        !tool_output.exists(),
        "workspace package selector dependency closure build should not build unrelated package artifacts"
    );
}

#[test]
fn build_project_path_rejects_output_for_multiple_targets() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-output");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for output rejection test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn util() -> Int { return 1 }\n");
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");

    let output_path = project_root.join("custom.ll");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["build"])
        .arg(&project_root)
        .args(["--output"])
        .arg(&output_path);
    let output = run_command_capture(&mut command, "`ql build --output` multiple targets");
    let (stdout, stderr) = expect_exit_code(
        "project-build-output",
        "multiple target output rejection",
        &output,
        1,
    )
    .expect("project build should reject `--output` when multiple targets are discovered");
    expect_empty_stdout(
        "project-build-output",
        "multiple target output rejection",
        &stdout,
    )
    .expect("output rejection should not print stdout");
    assert!(
        stderr
            .contains("error: `ql build --output` only supports a single discovered build target"),
        "expected multi-target output rejection, got:\n{stderr}"
    );
}

#[test]
fn build_package_path_selects_requested_target_for_custom_output() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-target-selector");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src/bin/tools"))
        .expect("create package source tree for target selector build test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn util() -> Int { return 1 }\n");
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");
    temp.write(
        "app/src/bin/tools/repl.ql",
        "fn main() -> Int { return 2 }\n",
    );

    let output_path = project_root.join("custom.ll");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["build"])
        .arg(&project_root)
        .args(["--target", "src/bin/tools/repl.ql", "--output"])
        .arg(&output_path);
    let output = run_command_capture(&mut command, "`ql build --target --output` package path");
    let (stdout, stderr) = expect_success(
        "project-build-target-selector",
        "selected target custom output build",
        &output,
    )
    .expect("package-path `ql build --target --output` should succeed for one selected target");
    expect_empty_stderr(
        "project-build-target-selector",
        "selected target custom output build",
        &stderr,
    )
    .expect("package-path `ql build --target --output` should not print stderr");
    expect_stdout_contains_all(
        "project-build-target-selector",
        &stdout.replace('\\', "/"),
        &[&format!("wrote llvm-ir: {}", output_path.display()).replace('\\', "/")],
    )
    .expect("package-path `ql build --target --output` should report the selected artifact");
    expect_file_exists(
        "project-build-target-selector",
        &output_path,
        "selected target custom output artifact",
        "selected target custom output build",
    )
    .expect("package-path `ql build --target --output` should write the selected artifact");
}

#[test]
fn build_project_path_emits_interface_once_for_multiple_targets() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-emit-interface");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for emit-interface build test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn util() -> Int { return 1 }\n");
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");

    let lib_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");
    let main_output = project_root.join("target/ql/debug/main.ll");
    let interface_output = project_root.join("app.qi");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["build"])
        .arg(&project_root)
        .arg("--emit-interface");
    let output = run_command_capture(&mut command, "`ql build --emit-interface` package path");
    let (stdout, stderr) = expect_success(
        "project-build-emit-interface",
        "package path build with interface emission",
        &output,
    )
    .expect("package path build with interface emission should succeed");
    expect_empty_stderr(
        "project-build-emit-interface",
        "package path build with interface emission",
        &stderr,
    )
    .expect("package path build with interface emission should not print stderr");
    expect_stdout_contains_all(
        "project-build-emit-interface",
        &stdout.replace('\\', "/"),
        &[
            &format!("wrote staticlib: {}", lib_output.display()).replace('\\', "/"),
            &format!("wrote llvm-ir: {}", main_output.display()).replace('\\', "/"),
            &format!("wrote interface: {}", interface_output.display()).replace('\\', "/"),
        ],
    )
    .expect("package path build with interface emission should report artifacts and interface");
    expect_file_exists(
        "project-build-emit-interface",
        &interface_output,
        "package interface artifact",
        "package path build with interface emission",
    )
    .expect("package path build with interface emission should write the package interface");
}

#[test]
fn build_package_path_syncs_dependency_interfaces_from_manifest_dependencies() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-dependency-sync");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source tree");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source tree");
    temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "dep/src/lib.ql",
        "extern \"c\" pub fn q_add(left: Int, right: Int) -> Int { return left + right }\n",
    );
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    temp.write(
        "app/src/main.ql",
        "use dep.q_add as add\n\nfn main() -> Int { return add(2, 3) }\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let dep_output_suffix = "dep/target/ql/debug/lib.lib";
    let app_output = project_root.join("target/ql/debug/main.ll");
    assert!(
        !interface_output.exists(),
        "dependency interface should start missing for sync test"
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql build` dependency sync");
    let (stdout, stderr) = expect_success(
        "project-build-dependency-sync",
        "package path build with dependency sync",
        &output,
    )
    .expect("package-path `ql build` should sync dependency interfaces before building");
    expect_empty_stderr(
        "project-build-dependency-sync",
        "package path build with dependency sync",
        &stderr,
    )
    .expect("dependency-sync build should not print stderr");
    expect_stdout_contains_all(
        "project-build-dependency-sync",
        &stdout.replace('\\', "/"),
        &[
            "wrote interface: ",
            "dep.qi",
            dep_output_suffix,
            &format!("wrote llvm-ir: {}", app_output.display()).replace('\\', "/"),
        ],
    )
    .expect(
        "dependency-sync build should report the synced interface plus dependency and root artifacts",
    );
    let normalized_stdout = stdout.replace('\\', "/");
    let dep_fragment = dep_output_suffix.to_owned();
    let app_fragment = format!("wrote llvm-ir: {}", app_output.display()).replace('\\', "/");
    let dep_index = normalized_stdout
        .find(&dep_fragment)
        .expect("dependency artifact should be present in stdout");
    let app_index = normalized_stdout
        .find(&app_fragment)
        .expect("root artifact should be present in stdout");
    assert!(
        dep_index < app_index,
        "dependency package should be built before the root package, got:\n{stdout}"
    );
    expect_file_exists(
        "project-build-dependency-sync",
        &interface_output,
        "synced dependency interface",
        "package path build with dependency sync",
    )
    .expect("dependency-sync build should emit the dependency interface");
    expect_file_exists(
        "project-build-dependency-sync",
        &dep_output,
        "dependency package build artifact",
        "package path build with dependency sync",
    )
    .expect("dependency-sync build should emit the dependency package artifact");
    expect_file_exists(
        "project-build-dependency-sync",
        &app_output,
        "package build artifact",
        "package path build with dependency sync",
    )
    .expect("dependency-sync build should still emit the package artifact");
}
