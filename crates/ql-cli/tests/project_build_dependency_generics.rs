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
fn build_package_path_json_supports_dependency_generic_array_length_function() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-generic-array-length-function");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dep source tree for generic array length function");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for generic array length function");

    temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "dep/src/lib.ql",
        r#"
pub fn first[T, N](values: [T; N]) -> T {
    return values[0]
}

pub fn last[T, N](values: [T; N]) -> T {
    var selected = values[0]
    for value in values {
        selected = value
    }
    return selected
}

pub fn at_or[T, N](values: [T; N], index: Int, fallback: T) -> T {
    var current_index = 0
    for value in values {
        if current_index == index {
            return value
        };
        current_index = current_index + 1
    }
    return fallback
}

pub fn contains[T, N](values: [T; N], needle: T) -> Bool {
    for value in values {
        if value == needle {
            return true
        }
    }
    return false
}

pub fn count[T, N](values: [T; N], needle: T) -> Int {
    var total = 0
    for value in values {
        if value == needle {
            total = total + 1
        }
    }
    return total
}

pub fn len[T, N](values: [T; N]) -> Int {
    return N
}

pub fn mirror[T, N](values: [T; N]) -> [T; N] {
    return values
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
use dep.first as first
use dep.last as last
use dep.at_or as at_or
use dep.contains as contains
use dep.count as count
use dep.len as len
use dep.mirror as mirror

fn bool_score(value: Bool) -> Int {
    if value {
        return 4
    }
    return 0
}

fn hidden_value() -> Int {
    return 8
}

fn main() -> Int {
    let number: Int = first([7, 8, 9])
    let flag: Bool = last([false, true, true, false])
    let picked: Int = at_or([3, 4, 5, 6], 2, 0)
    let present = contains(["red", "blue", "green"], "blue")
    let matches = count([1, 2, 1, 1, 3], 1)
    let length = len([10, 20, 30, 40])
    let mirrored: [Int; 3] = mirror([1, hidden_value(), at_or([8, 9, 10], 1, 0)])
    return number + bool_score(flag) + picked + bool_score(present) + matches + length + mirrored[1]
}
"#,
    );

    let app_output = project_root.join("target/ql/debug/main.ll");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` dependency generic array length function",
    );
    let (stdout, stderr) = expect_success(
        "project-build-package-json-generic-array-length-function",
        "package build json dependency generic array length function",
        &output,
    )
    .expect(
        "package-path `ql build --json` should support dependency generic array length functions",
    );
    expect_empty_stderr(
        "project-build-package-json-generic-array-length-function",
        "package build json dependency generic array length function",
        &stderr,
    )
    .expect("generic array length function json build should not print stderr");

    let json = parse_json_output(
        "project-build-package-json-generic-array-length-function",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "ok");
    expect_file_exists(
        "project-build-package-json-generic-array-length-function",
        &app_output,
        "selected package artifact",
        "package build json dependency generic array length function",
    )
    .expect("generic array length function build should emit the selected artifact");
}

#[test]
fn build_package_path_json_supports_dependency_generic_wrapper_calling_generic_helper() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-generic-wrapper-helper");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dep source tree for generic wrapper helper");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for generic wrapper helper");

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
pub fn first[T, N](values: [T; N]) -> T {
    return values[0]
}

pub fn first_wrapped[T, N](values: [T; N]) -> T {
    return first(values)
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
use dep.first_wrapped as first_wrapped

fn main() -> Int {
    return first_wrapped([7, 8, 9])
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
        "`ql build --json` dependency generic wrapper calling generic helper",
    );
    let (stdout, stderr) = expect_success(
        "project-build-package-json-generic-wrapper-helper",
        "package build json dependency generic wrapper calling generic helper",
        &output,
    )
    .expect(
        "package-path `ql build --json` should specialize generic wrappers and their generic helpers",
    );
    expect_empty_stderr(
        "project-build-package-json-generic-wrapper-helper",
        "package build json dependency generic wrapper calling generic helper",
        &stderr,
    )
    .expect("generic wrapper helper json build should not print stderr");

    let json = parse_json_output("project-build-package-json-generic-wrapper-helper", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(json["status"], "ok");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    let built_targets = json["built_targets"]
        .as_array()
        .expect("generic wrapper helper json should expose built_targets");
    assert_eq!(built_targets.len(), 2);
    assert_eq!(
        built_targets[0]["manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    expect_file_exists(
        "project-build-package-json-generic-wrapper-helper",
        &dep_output,
        "dependency package artifact",
        "package build json dependency generic wrapper calling generic helper",
    )
    .expect("generic wrapper helper build should preserve the dependency artifact");
    expect_file_exists(
        "project-build-package-json-generic-wrapper-helper",
        &app_output,
        "selected package artifact",
        "package build json dependency generic wrapper calling generic helper",
    )
    .expect("generic wrapper helper build should emit the selected artifact");
}

#[test]
fn build_package_path_json_supports_dependency_generic_wrapper_calling_imported_generic_helper() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-generic-imported-helper");
    let helper_root = temp.path().join("helper");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(helper_root.join("src"))
        .expect("create helper source tree for imported generic helper");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dep source tree for imported generic helper");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for imported generic helper");

    let helper_manifest = temp.write(
        "helper/qlang.toml",
        r#"
[package]
name = "helper"
"#,
    );
    temp.write(
        "helper/src/lib.ql",
        r#"
pub fn reverse_array[T, N](values: [T; N]) -> [T; N] {
    var result = values
    var index = 0
    for value in values {
        result[index] = values[N - index - 1];
        index = index + 1
    }
    return result
}
"#,
    );
    let dep_manifest = temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"

[dependencies]
helper = "../helper"
"#,
    );
    temp.write(
        "dep/src/lib.ql",
        r#"
use helper.reverse_array as reverse_array

pub fn reverse_wrapped[T, N](values: [T; N]) -> [T; N] {
    return reverse_array(values)
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
use dep.reverse_wrapped as reverse_wrapped

fn main() -> Int {
    let reversed: [Int; 3] = reverse_wrapped([7, 8, 9])
    return reversed[0] + reversed[1] + reversed[2]
}
"#,
    );

    let helper_output = static_library_output_path(&helper_root.join("target/ql/debug"), "lib");
    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let app_output = project_root.join("target/ql/debug/main.ll");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` dependency generic wrapper calling imported generic helper",
    );
    let (stdout, stderr) = expect_success(
        "project-build-package-json-generic-imported-helper",
        "package build json dependency generic wrapper calling imported generic helper",
        &output,
    )
    .expect("package-path `ql build --json` should specialize imported generic helper wrappers");
    expect_empty_stderr(
        "project-build-package-json-generic-imported-helper",
        "package build json dependency generic wrapper calling imported generic helper",
        &stderr,
    )
    .expect("imported generic helper json build should not print stderr");

    let json = parse_json_output(
        "project-build-package-json-generic-imported-helper",
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
        .expect("imported generic helper json should expose built_targets");
    assert_eq!(built_targets.len(), 3);
    assert_eq!(
        built_targets[0]["manifest_path"],
        helper_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        built_targets[1]["manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    expect_file_exists(
        "project-build-package-json-generic-imported-helper",
        &helper_output,
        "transitive helper package artifact",
        "package build json dependency generic wrapper calling imported generic helper",
    )
    .expect("imported generic helper build should preserve the helper artifact");
    expect_file_exists(
        "project-build-package-json-generic-imported-helper",
        &dep_output,
        "dependency package artifact",
        "package build json dependency generic wrapper calling imported generic helper",
    )
    .expect("imported generic helper build should preserve the dependency artifact");
    expect_file_exists(
        "project-build-package-json-generic-imported-helper",
        &app_output,
        "selected package artifact",
        "package build json dependency generic wrapper calling imported generic helper",
    )
    .expect("imported generic helper build should emit the selected artifact");
}

#[test]
fn build_package_path_json_supports_local_generic_array_length_wrapper_function() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-local-generic-wrapper");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for local generic wrapper");

    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "app/src/lib.ql",
        r#"
pub fn sum_values[N](values: [Int; N]) -> Int {
    var total = 0
    for value in values {
        total = total + value
    }
    return total
}

pub fn sum_wrapped_values[N](values: [Int; N]) -> Int {
    return sum_values(values)
}

pub fn sample_total() -> Int {
    return sum_wrapped_values([1, 2, 3])
}
"#,
    );
    let interface_output = project_root.join("app.qi");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["build"])
        .arg(&project_root)
        .args(["--lib", "--emit-interface", "--json"]);
    let output = run_command_capture(
        &mut command,
        "`ql build --lib --json` local generic wrapper function",
    );
    let (stdout, stderr) = expect_success(
        "project-build-package-json-local-generic-wrapper",
        "package build json local generic wrapper function",
        &output,
    )
    .expect("package-path `ql build --lib --json` should specialize local generic wrappers");
    expect_empty_stderr(
        "project-build-package-json-local-generic-wrapper",
        "package build json local generic wrapper function",
        &stderr,
    )
    .expect("local generic wrapper json build should not print stderr");

    let json = parse_json_output("project-build-package-json-local-generic-wrapper", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "ok");
    expect_file_exists(
        "project-build-package-json-local-generic-wrapper",
        &interface_output,
        "package interface artifact",
        "package build json local generic wrapper function",
    )
    .expect("local generic wrapper json build should emit the package interface");

    let interface = read_normalized_file(&interface_output, "local generic wrapper interface");
    assert!(
        interface.contains("pub fn sum_values[N](values: [Int; N]) -> Int"),
        "local generic wrapper interface should preserve generic helper declaration:\n{interface}"
    );
    assert!(
        interface.contains("pub fn sum_wrapped_values[N](values: [Int; N]) -> Int"),
        "local generic wrapper interface should preserve generic wrapper declaration:\n{interface}"
    );
    assert!(
        interface.contains("pub fn sample_total() -> Int"),
        "local generic wrapper interface should preserve concrete caller declaration:\n{interface}"
    );
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
