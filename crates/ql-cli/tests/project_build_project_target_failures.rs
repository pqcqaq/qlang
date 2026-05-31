mod support;

use serde_json::Value as JsonValue;
use support::{
    TempDir, expect_empty_stderr, expect_exit_code, expect_file_exists, ql_command,
    run_command_capture, static_library_output_path, workspace_root,
};

fn normalize_output_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&normalize_output_text(stdout))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
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
