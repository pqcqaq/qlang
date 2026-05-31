mod support;

use serde_json::Value as JsonValue;
use support::{
    TempDir, expect_empty_stderr, expect_file_exists, expect_success, ql_command,
    read_normalized_file, run_command_capture, static_library_output_path, workspace_root,
};

fn normalize_output_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&normalize_output_text(stdout))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
}

#[test]
fn build_project_source_file_supports_direct_dependency_generic_public_types() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-source-file-generic-public-types");
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
        r#"
pub struct Box[T] {
    value: T,
}

pub enum Maybe[T] {
    Some(T),
    None,
}

pub fn identity[T](value: T) -> T {
    return value
}

pub fn inspect_box(value: Box[Int]) -> Int {
    return 7
}

pub fn classify(value: Maybe[Int]) -> Int {
    return 11
}
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
    let app_main = temp.write(
        "app/src/main.ql",
        r#"
use dep.Box as Box
use dep.Maybe as Maybe
use dep.inspect_box as inspect_box
use dep.classify as classify

fn consume_box(value: Box[Int]) -> Int {
    let local: Box[Int] = Box { value: 7 }
    return inspect_box(value) + value.value + inspect_box(local) + local.value
}

fn consume_maybe(value: Maybe[Int]) -> Int {
    return classify(value)
}

fn main() -> Int {
    let value: Box[Int] = Box { value: 5 }
    return consume_box(value)
}
"#,
    );

    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let app_output = project_root.join("target/ql/debug/main.ll");
    let interface_output = dep_root.join("dep.qi");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&app_main).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` direct project source file dependency generic public types",
    );
    let (stdout, stderr) = expect_success(
        "project-build-source-file-generic-public-types",
        "direct project source file dependency generic public types json build",
        &output,
    )
    .expect(
        "direct project source file `ql build --json` should support direct dependency generic public types in concrete function signatures",
    );
    expect_empty_stderr(
        "project-build-source-file-generic-public-types",
        "direct project source file dependency generic public types json build",
        &stderr,
    )
    .expect("direct dependency generic public types json build should not print stderr");

    let json = parse_json_output("project-build-source-file-generic-public-types", &stdout);
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
        .expect("dependency generic public types json build should expose built targets");
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
        "project-build-source-file-generic-public-types",
        &dep_output,
        "dependency package artifact",
        "direct project source file dependency generic public types json build",
    )
    .expect("direct dependency generic public types json build should emit dependency artifacts");
    expect_file_exists(
        "project-build-source-file-generic-public-types",
        &app_output,
        "selected package artifact",
        "direct project source file dependency generic public types json build",
    )
    .expect(
        "direct dependency generic public types json build should emit the selected package artifact",
    );
    expect_file_exists(
        "project-build-source-file-generic-public-types",
        &interface_output,
        "synced dependency interface",
        "direct project source file dependency generic public types json build",
    )
    .expect(
        "direct dependency generic public types json build should keep dependency interface sync",
    );

    let interface = read_normalized_file(&interface_output, "synced dependency interface");
    assert!(
        interface.contains("pub struct Box[T]"),
        "synced dependency interface should preserve generic public struct declarations:\n{interface}"
    );
    assert!(
        interface.contains("pub enum Maybe[T]"),
        "synced dependency interface should preserve generic public enum declarations:\n{interface}"
    );
    assert!(
        interface.contains("pub fn identity[T](value: T) -> T"),
        "synced dependency interface should preserve uninstantiated generic public function declarations:\n{interface}"
    );
}

#[test]
fn build_package_path_json_supports_direct_dependency_generic_public_function_single_instantiation()
{
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-generic-public-function-instantiation");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dep source tree for dependency generic function instantiation");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for dependency generic function instantiation");

    let dep_manifest = temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "dep/src/lib.ql",
        r#"
pub fn identity[T](value: T) -> T {
    return value
}

pub fn concrete(value: Int) -> Int {
    return value
}
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
    temp.write(
        "app/src/main.ql",
        r#"
use dep.identity as identity

fn main() -> Int {
    return identity(7)
}
"#,
    );

    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let app_output = project_root.join("target/ql/debug/main.ll");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` direct dependency generic public function instantiation",
    );
    let (stdout, stderr) = expect_success(
        "project-build-package-json-generic-public-function-instantiation",
        "package build json dependency generic public function instantiation",
        &output,
    )
    .expect("package-path `ql build --json` should support a single direct dependency generic function instantiation");
    expect_empty_stderr(
        "project-build-package-json-generic-public-function-instantiation",
        "package build json dependency generic public function instantiation",
        &stderr,
    )
    .expect("generic function instantiation json build should not print stderr");

    let json = parse_json_output(
        "project-build-package-json-generic-public-function-instantiation",
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
    assert_eq!(json["status"], "ok");

    let built_targets = json["built_targets"]
        .as_array()
        .expect("generic function instantiation json should expose built_targets");
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
        "project-build-package-json-generic-public-function-instantiation",
        &dep_output,
        "dependency package artifact",
        "package build json dependency generic public function instantiation",
    )
    .expect("generic function instantiation should preserve the dependency package artifact");
    expect_file_exists(
        "project-build-package-json-generic-public-function-instantiation",
        &app_output,
        "selected package artifact",
        "package build json dependency generic public function instantiation",
    )
    .expect("generic function instantiation should emit the selected package artifact");
}
