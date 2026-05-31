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
fn build_package_path_json_supports_dependency_generic_function_from_typed_values() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-generic-public-function-typed-values");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dep source tree for dependency generic function typed values");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for dependency generic function typed values");

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

const ROOT_VALUE: Int = 5

fn via_param(value: Int) -> Int {
    return identity(value)
}

fn main() -> Int {
    let value: Int = 7
    let mirror = value
    return via_param(value) + identity(mirror) + identity(ROOT_VALUE)
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
        "`ql build --json` direct dependency generic public function typed values",
    );
    let (stdout, stderr) = expect_success(
        "project-build-package-json-generic-public-function-typed-values",
        "package build json dependency generic public function typed values",
        &output,
    )
    .expect("package-path `ql build --json` should infer generic function instantiations from typed values");
    expect_empty_stderr(
        "project-build-package-json-generic-public-function-typed-values",
        "package build json dependency generic public function typed values",
        &stderr,
    )
    .expect("typed-value generic function instantiation json build should not print stderr");

    let json = parse_json_output(
        "project-build-package-json-generic-public-function-typed-values",
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
        .expect("typed-value generic function json should expose built_targets");
    assert_eq!(built_targets.len(), 2);
    assert_eq!(
        built_targets[0]["manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(built_targets[0]["package_name"], "dep");
    assert_eq!(built_targets[0]["dependency_only"], true);
    assert_eq!(
        built_targets[1]["artifact_path"],
        app_output.display().to_string().replace('\\', "/")
    );
    expect_file_exists(
        "project-build-package-json-generic-public-function-typed-values",
        &dep_output,
        "dependency package artifact",
        "package build json dependency generic public function typed values",
    )
    .expect("typed-value generic function instantiation should preserve the dependency artifact");
    expect_file_exists(
        "project-build-package-json-generic-public-function-typed-values",
        &app_output,
        "selected package artifact",
        "package build json dependency generic public function typed values",
    )
    .expect("typed-value generic function instantiation should emit the selected artifact");
}

#[test]
fn build_package_path_json_supports_multiple_dependency_generic_function_instantiations() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-multiple-generic-instantiations");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dep source tree for multiple generic instantiations");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for multiple generic instantiations");

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

fn bool_score(value: Bool) -> Int {
    if value {
        return 4
    }
    return 0
}

fn main() -> Int {
    let number: Int = identity(7)
    let flag: Bool = identity(true)
    return number + bool_score(flag)
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
        "`ql build --json` multiple direct dependency generic function instantiations",
    );
    let (stdout, stderr) = expect_success(
        "project-build-package-json-multiple-generic-instantiations",
        "package build json multiple dependency generic function instantiations",
        &output,
    )
    .expect(
        "package-path `ql build --json` should support multiple concrete generic instantiations",
    );
    expect_empty_stderr(
        "project-build-package-json-multiple-generic-instantiations",
        "package build json multiple dependency generic function instantiations",
        &stderr,
    )
    .expect("multiple generic instantiation build should not print stderr");

    let json = parse_json_output(
        "project-build-package-json-multiple-generic-instantiations",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(json["status"], "ok");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    let built_targets = json["built_targets"]
        .as_array()
        .expect("multiple generic instantiation json should expose built_targets");
    assert_eq!(built_targets.len(), 2);
    assert_eq!(
        built_targets[0]["manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        built_targets[1]["artifact_path"],
        app_output.display().to_string().replace('\\', "/")
    );
    expect_file_exists(
        "project-build-package-json-multiple-generic-instantiations",
        &dep_output,
        "dependency package artifact",
        "package build json multiple dependency generic function instantiations",
    )
    .expect("multiple generic instantiation should preserve the dependency artifact");
    expect_file_exists(
        "project-build-package-json-multiple-generic-instantiations",
        &app_output,
        "selected package artifact",
        "package build json multiple dependency generic function instantiations",
    )
    .expect("multiple generic instantiation should emit the selected artifact");
}
