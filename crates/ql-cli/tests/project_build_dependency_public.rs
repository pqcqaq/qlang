mod support;

use serde_json::Value as JsonValue;
use support::{
    TempDir, expect_empty_stderr, expect_file_exists, expect_success, ql_command,
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
fn build_project_source_file_supports_direct_dependency_public_struct_functions() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-source-file-public-struct-function");
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
        "pub struct Box { value: Int }\npub fn make_box() -> Box { return Box { value: 7 } }\n",
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
        "use dep.Box as Box\nuse dep.make_box as make\n\nfn main() -> Int {\n    let value: Box = make()\n    return value.value\n}\n",
    );

    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let app_output = project_root.join("target/ql/debug/main.ll");
    let interface_output = dep_root.join("dep.qi");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&app_main).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` direct project source file dependency public struct function",
    );
    let (stdout, stderr) = expect_success(
        "project-build-source-file-public-struct-function",
        "direct project source file dependency public struct function json build",
        &output,
    )
    .expect(
        "direct project source file `ql build --json` should support direct dependency public struct functions",
    );
    expect_empty_stderr(
        "project-build-source-file-public-struct-function",
        "direct project source file dependency public struct function json build",
        &stderr,
    )
    .expect("direct dependency public struct function json build should not print stderr");

    let json = parse_json_output("project-build-source-file-public-struct-function", &stdout);
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
        .expect("dependency public struct function json build should expose built targets");
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
        "project-build-source-file-public-struct-function",
        &dep_output,
        "dependency package artifact",
        "direct project source file dependency public struct function json build",
    )
    .expect("direct dependency public struct function json build should emit dependency artifacts");
    expect_file_exists(
        "project-build-source-file-public-struct-function",
        &app_output,
        "selected package artifact",
        "direct project source file dependency public struct function json build",
    )
    .expect(
        "direct dependency public struct function json build should emit the selected package artifact",
    );
    expect_file_exists(
        "project-build-source-file-public-struct-function",
        &interface_output,
        "synced dependency interface",
        "direct project source file dependency public struct function json build",
    )
    .expect(
        "direct dependency public struct function json build should keep dependency interface sync",
    );
}

#[test]
fn build_project_source_file_supports_direct_dependency_public_type_alias_functions() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-source-file-public-type-alias-function");
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
        "pub type Count = Int\npub type Score = Count\npub fn make_score(value: Count) -> Score { return value + 2 }\npub fn unwrap_score(value: Score) -> Int { return value }\n",
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
        "use dep.make_score as make_score\nuse dep.unwrap_score as unwrap_score\n\nfn main() -> Int {\n    return unwrap_score(make_score(5))\n}\n",
    );

    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let app_output = project_root.join("target/ql/debug/main.ll");
    let interface_output = dep_root.join("dep.qi");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&app_main).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` direct project source file dependency public type alias function",
    );
    let (stdout, stderr) = expect_success(
        "project-build-source-file-public-type-alias-function",
        "direct project source file dependency public type alias function json build",
        &output,
    )
    .expect(
        "direct project source file `ql build --json` should support direct dependency public type alias functions",
    );
    expect_empty_stderr(
        "project-build-source-file-public-type-alias-function",
        "direct project source file dependency public type alias function json build",
        &stderr,
    )
    .expect("direct dependency public type alias function json build should not print stderr");

    let json = parse_json_output(
        "project-build-source-file-public-type-alias-function",
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
        .expect("dependency public type alias function json build should expose built targets");
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
        "project-build-source-file-public-type-alias-function",
        &dep_output,
        "dependency package artifact",
        "direct project source file dependency public type alias function json build",
    )
    .expect(
        "direct dependency public type alias function json build should emit dependency artifacts",
    );
    expect_file_exists(
        "project-build-source-file-public-type-alias-function",
        &app_output,
        "selected package artifact",
        "direct project source file dependency public type alias function json build",
    )
    .expect(
        "direct dependency public type alias function json build should emit the selected package artifact",
    );
    expect_file_exists(
        "project-build-source-file-public-type-alias-function",
        &interface_output,
        "synced dependency interface",
        "direct project source file dependency public type alias function json build",
    )
    .expect(
        "direct dependency public type alias function json build should keep dependency interface sync",
    );
}

#[test]
fn build_project_source_file_supports_direct_dependency_public_struct_methods() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-source-file-public-struct-method");
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
        "pub struct Box { value: Int }\n\nimpl Box {\n    pub fn read(self) -> Int {\n        return self.value\n    }\n}\n\npub fn make_box() -> Box { return Box { value: 7 } }\n",
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
        "use dep.make_box as make\n\nfn main() -> Int {\n    let value = make()\n    return value.read()\n}\n",
    );

    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let app_output = project_root.join("target/ql/debug/main.ll");
    let interface_output = dep_root.join("dep.qi");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&app_main).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` direct project source file dependency public struct method",
    );
    let (stdout, stderr) = expect_success(
        "project-build-source-file-public-struct-method",
        "direct project source file dependency public struct method json build",
        &output,
    )
    .expect(
        "direct project source file `ql build --json` should support direct dependency public struct methods",
    );
    expect_empty_stderr(
        "project-build-source-file-public-struct-method",
        "direct project source file dependency public struct method json build",
        &stderr,
    )
    .expect("direct dependency public struct method json build should not print stderr");

    let json = parse_json_output("project-build-source-file-public-struct-method", &stdout);
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
        .expect("dependency public struct method json build should expose built targets");
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
        "project-build-source-file-public-struct-method",
        &dep_output,
        "dependency package artifact",
        "direct project source file dependency public struct method json build",
    )
    .expect("direct dependency public struct method json build should emit dependency artifacts");
    expect_file_exists(
        "project-build-source-file-public-struct-method",
        &app_output,
        "selected package artifact",
        "direct project source file dependency public struct method json build",
    )
    .expect(
        "direct dependency public struct method json build should emit the selected package artifact",
    );
    expect_file_exists(
        "project-build-source-file-public-struct-method",
        &interface_output,
        "synced dependency interface",
        "direct project source file dependency public struct method json build",
    )
    .expect(
        "direct dependency public struct method json build should keep dependency interface sync",
    );
}
