mod support;

use serde_json::Value as JsonValue;
use support::{
    TempDir, expect_empty_stderr, expect_exit_code, expect_file_exists, expect_success, ql_command,
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

#[test]
fn build_package_path_json_infers_dependency_generic_function_from_named_arguments() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-generic-public-function-named-args");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dep source tree for dependency generic function named args");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for dependency generic function named args");

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
pub fn choose[T](fallback: T, value: T) -> T {
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
use dep.choose as choose

fn bool_score(value: Bool) -> Int {
    if value {
        return 6
    }
    return 0
}

fn main() -> Int {
    let number: Int = choose(value: 7, fallback: 0)
    let flag: Bool = choose(value: true, fallback: false)
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
        "`ql build --json` direct dependency generic public function named arguments",
    );
    let (stdout, stderr) = expect_success(
        "project-build-package-json-generic-public-function-named-args",
        "package build json dependency generic public function named arguments",
        &output,
    )
    .expect("package-path `ql build --json` should infer generic function instantiations from named arguments");
    expect_empty_stderr(
        "project-build-package-json-generic-public-function-named-args",
        "package build json dependency generic public function named arguments",
        &stderr,
    )
    .expect("named-argument generic function instantiation json build should not print stderr");

    let json = parse_json_output(
        "project-build-package-json-generic-public-function-named-args",
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
        .expect("named-argument generic function json should expose built_targets");
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
        "project-build-package-json-generic-public-function-named-args",
        &dep_output,
        "dependency package artifact",
        "package build json dependency generic public function named arguments",
    )
    .expect(
        "named-argument generic function instantiation should preserve the dependency artifact",
    );
    expect_file_exists(
        "project-build-package-json-generic-public-function-named-args",
        &app_output,
        "selected package artifact",
        "package build json dependency generic public function named arguments",
    )
    .expect("named-argument generic function instantiation should emit the selected artifact");
}

#[test]
fn build_package_path_json_infers_dependency_generic_function_from_expressions() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-generic-public-function-expressions");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dep source tree for dependency generic function expressions");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for dependency generic function expressions");

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

pub fn choose[T](fallback: T, value: T) -> T {
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
use dep.choose as choose
use dep.identity as identity

fn main() -> Int {
    let number: Int = identity(1 + 2)
    let flag: Bool = choose(value: !(false || false), fallback: false)
    let ordered: Bool = identity(1 < 2)
    let pair: (Int, Bool) = identity((number, flag))
    let values: [Int; 3] = identity([number, 2 + 3, 4])
    if pair[1] && ordered {
        return values[0] + values[1] + values[2]
    }
    return 0
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
        "`ql build --json` direct dependency generic public function expression arguments",
    );
    let (stdout, stderr) = expect_success(
        "project-build-package-json-generic-public-function-expressions",
        "package build json dependency generic public function expression arguments",
        &output,
    )
    .expect("package-path `ql build --json` should infer generic function instantiations from expression arguments");
    expect_empty_stderr(
        "project-build-package-json-generic-public-function-expressions",
        "package build json dependency generic public function expression arguments",
        &stderr,
    )
    .expect(
        "expression-argument generic function instantiation json build should not print stderr",
    );

    let json = parse_json_output(
        "project-build-package-json-generic-public-function-expressions",
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
        .expect("expression-argument generic function json should expose built_targets");
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
        "project-build-package-json-generic-public-function-expressions",
        &dep_output,
        "dependency package artifact",
        "package build json dependency generic public function expression arguments",
    )
    .expect("expression-argument generic function instantiation should preserve the dependency artifact");
    expect_file_exists(
        "project-build-package-json-generic-public-function-expressions",
        &app_output,
        "selected package artifact",
        "package build json dependency generic public function expression arguments",
    )
    .expect("expression-argument generic function instantiation should emit the selected artifact");
}

#[test]
fn build_package_path_json_scans_dependency_generic_calls_in_nested_expressions() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-generic-nested-expressions");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dep source tree for dependency generic nested expressions");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for dependency generic nested expressions");

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

struct Pair {
    number: Int,
    flag: Bool,
}

fn main() -> Int {
    let values: [Int; 3] = [2, 3, 4]
    let pair: Pair = Pair {
        number: identity(1) + values[identity(0)],
        flag: match true {
            _ if identity(true) => identity(true),
            _ => false,
        },
    }
    if pair.flag {
        return pair.number
    }
    return 0
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
        "`ql build --json` nested dependency generic expression calls",
    );
    let (stdout, stderr) = expect_success(
        "project-build-package-json-generic-nested-expressions",
        "package build json dependency generic nested expression calls",
        &output,
    )
    .expect(
        "package-path `ql build --json` should scan nested dependency generic expression calls",
    );
    expect_empty_stderr(
        "project-build-package-json-generic-nested-expressions",
        "package build json dependency generic nested expression calls",
        &stderr,
    )
    .expect("nested expression generic function json build should not print stderr");

    let json = parse_json_output(
        "project-build-package-json-generic-nested-expressions",
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
        .expect("nested expression generic function json should expose built_targets");
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
        "project-build-package-json-generic-nested-expressions",
        &dep_output,
        "dependency package artifact",
        "package build json dependency generic nested expression calls",
    )
    .expect("nested expression generic function scan should preserve dependency artifact");
    expect_file_exists(
        "project-build-package-json-generic-nested-expressions",
        &app_output,
        "selected package artifact",
        "package build json dependency generic nested expression calls",
    )
    .expect("nested expression generic function scan should emit selected artifact");
}

#[test]
fn build_package_path_json_supports_dependency_generic_function_from_generic_carriers() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-generic-public-function-carriers");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dep source tree for dependency generic function carriers");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for dependency generic function carriers");

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

pub fn identity[T](value: T) -> T {
    return value
}

pub fn keep_box[T](value: Box[T]) -> Box[T] {
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
use dep.Box as Box
use dep.identity as identity
use dep.keep_box as keep_box

fn via_param(value: Box[Int]) -> Int {
    let kept: Box[Int] = identity(value)
    let nested: Box[Int] = keep_box(kept)
    return nested.value
}

fn main() -> Int {
    let value: Box[Int] = Box { value: 9 }
    return via_param(value)
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
        "`ql build --json` direct dependency generic public function carriers",
    );
    let (stdout, stderr) = expect_success(
        "project-build-package-json-generic-public-function-carriers",
        "package build json dependency generic public function carriers",
        &output,
    )
    .expect("package-path `ql build --json` should infer generic function instantiations from generic carrier values");
    expect_empty_stderr(
        "project-build-package-json-generic-public-function-carriers",
        "package build json dependency generic public function carriers",
        &stderr,
    )
    .expect("generic carrier function instantiation json build should not print stderr");

    let json = parse_json_output(
        "project-build-package-json-generic-public-function-carriers",
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
        .expect("generic carrier function json should expose built_targets");
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
        "project-build-package-json-generic-public-function-carriers",
        &dep_output,
        "dependency package artifact",
        "package build json dependency generic public function carriers",
    )
    .expect("generic carrier function instantiation should preserve the dependency artifact");
    expect_file_exists(
        "project-build-package-json-generic-public-function-carriers",
        &app_output,
        "selected package artifact",
        "package build json dependency generic public function carriers",
    )
    .expect("generic carrier function instantiation should emit the selected artifact");
}

#[test]
fn build_package_path_json_infers_dependency_generic_function_from_variant_call_carrier() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-generic-public-function-variant-call");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dep source tree for dependency generic function variant call");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for dependency generic function variant call");

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
pub enum Option[T] {
    Some(T),
    None,
}

pub fn is_some[T](value: Option[T]) -> Bool {
    return match value {
        Option.Some(_) => true,
        Option.None => false,
    }
}

pub fn or_option[T](value: Option[T], fallback: Option[T]) -> Option[T] {
    return match value {
        Option.Some(inner) => Option.Some(inner),
        Option.None => fallback,
    }
}

pub fn unwrap_or[T](value: Option[T], fallback: T) -> T {
    return match value {
        Option.Some(inner) => inner,
        Option.None => fallback,
    }
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
use dep.Option as Option
use dep.is_some as is_some
use dep.or_option as or_option
use dep.unwrap_or as unwrap_or

fn main() -> Int {
    let choice = or_option(Option.Some(7), Option.None)
    if is_some(Option.Some(42)) {
        return unwrap_or(choice, 0)
    }
    return 1
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
        "`ql build --json` direct dependency generic public function variant call carrier",
    );
    let (stdout, stderr) = expect_success(
        "project-build-package-json-generic-public-function-variant-call",
        "package build json dependency generic public function variant call carrier",
        &output,
    )
    .expect("package-path `ql build --json` should infer generic function instantiations from single-field variant calls");
    expect_empty_stderr(
        "project-build-package-json-generic-public-function-variant-call",
        "package build json dependency generic public function variant call carrier",
        &stderr,
    )
    .expect("variant-call generic function instantiation json build should not print stderr");

    let json = parse_json_output(
        "project-build-package-json-generic-public-function-variant-call",
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
        .expect("variant-call generic function json should expose built_targets");
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
        "project-build-package-json-generic-public-function-variant-call",
        &dep_output,
        "dependency package artifact",
        "package build json dependency generic public function variant call carrier",
    )
    .expect("variant-call generic function instantiation should preserve the dependency artifact");
    expect_file_exists(
        "project-build-package-json-generic-public-function-variant-call",
        &app_output,
        "selected package artifact",
        "package build json dependency generic public function variant call carrier",
    )
    .expect("variant-call generic function instantiation should emit the selected artifact");
}

#[test]
fn build_package_path_json_infers_dependency_generic_function_from_result_context() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-generic-public-function-result-context");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dep source tree for dependency generic function result context");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for dependency generic function result context");

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
pub enum Result[T, E] {
    Ok(T),
    Err(E),
}

pub fn ok[T, E](value: T) -> Result[T, E] {
    return Result.Ok(value)
}

pub fn err[T, E](error: E) -> Result[T, E] {
    return Result.Err(error)
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
use dep.Result as Result
use dep.ok as result_ok
use dep.err as result_err

fn make_ok() -> Result[Int, Int] {
    return result_ok(7)
}

fn ok_status(value: Result[Int, Int]) -> Int {
    return match value {
        Result.Ok(inner) => inner,
        Result.Err(_) => 0,
    }
}

fn err_status(value: Result[Int, Int]) -> Int {
    return match value {
        Result.Ok(_) => 0,
        Result.Err(error) => error,
    }
}

fn main() -> Int {
    let failed: Result[Int, Int] = result_err(3)
    return ok_status(make_ok()) + err_status(failed)
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
        "`ql build --json` direct dependency generic public function result context",
    );
    let (stdout, stderr) = expect_success(
        "project-build-package-json-generic-public-function-result-context",
        "package build json dependency generic public function result context",
        &output,
    )
    .expect("package-path `ql build --json` should infer generic function instantiations from explicit result context");
    expect_empty_stderr(
        "project-build-package-json-generic-public-function-result-context",
        "package build json dependency generic public function result context",
        &stderr,
    )
    .expect("result-context generic function instantiation json build should not print stderr");

    let json = parse_json_output(
        "project-build-package-json-generic-public-function-result-context",
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
        .expect("result-context generic function json should expose built_targets");
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
        "project-build-package-json-generic-public-function-result-context",
        &dep_output,
        "dependency package artifact",
        "package build json dependency generic public function result context",
    )
    .expect(
        "result-context generic function instantiation should preserve the dependency artifact",
    );
    expect_file_exists(
        "project-build-package-json-generic-public-function-result-context",
        &app_output,
        "selected package artifact",
        "package build json dependency generic public function result context",
    )
    .expect("result-context generic function instantiation should emit the selected artifact");
}

#[test]
fn build_package_path_json_infers_dependency_zero_argument_generic_function_from_context() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-zero-arg-generic-public-function");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dep source tree for zero-argument dependency generic function");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for zero-argument dependency generic function");

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
pub enum Option[T] {
    Some(T),
    None,
}

pub fn none_option[T]() -> Option[T] {
    return Option.None
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
use dep.Option as Option
use dep.none_option as option_none

fn make_none() -> Option[Int] {
    return option_none()
}

fn none_status(value: Option[Int]) -> Int {
    return match value {
        Option.Some(_) => 1,
        Option.None => 0,
    }
}

fn main() -> Int {
    let value: Option[Int] = option_none()
    return none_status(value) + none_status(make_none())
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
        "`ql build --json` direct dependency zero-argument generic public function context",
    );
    let (stdout, stderr) = expect_success(
        "project-build-package-json-zero-arg-generic-public-function",
        "package build json dependency zero-argument generic public function context",
        &output,
    )
    .expect("package-path `ql build --json` should infer zero-argument generic functions from explicit context");
    expect_empty_stderr(
        "project-build-package-json-zero-arg-generic-public-function",
        "package build json dependency zero-argument generic public function context",
        &stderr,
    )
    .expect("zero-argument generic function json build should not print stderr");

    let json = parse_json_output(
        "project-build-package-json-zero-arg-generic-public-function",
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
        .expect("zero-argument generic function json should expose built_targets");
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
        "project-build-package-json-zero-arg-generic-public-function",
        &dep_output,
        "dependency package artifact",
        "package build json dependency zero-argument generic public function context",
    )
    .expect("zero-argument generic function instantiation should preserve the dependency artifact");
    expect_file_exists(
        "project-build-package-json-zero-arg-generic-public-function",
        &app_output,
        "selected package artifact",
        "package build json dependency zero-argument generic public function context",
    )
    .expect("zero-argument generic function instantiation should emit the selected artifact");
}

#[test]
fn build_package_path_json_reports_uninferred_direct_dependency_generic_public_function_import() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-uninferred-generic-public-function");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dep source tree for uninferred dependency generic function import");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for uninferred dependency generic function import");

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
pub fn make[T]() -> T {
    return 0
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
use dep.make as make

fn main() -> Int {
    let value = make()
    return 0
}
"#,
    );

    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` uninferred direct dependency generic public function import",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-json-uninferred-generic-public-function",
        "package build json uninferred dependency generic public function import",
        &output,
        1,
    )
    .expect("package-path `ql build --json` should reject uninferred generic function imports explicitly");
    expect_empty_stderr(
        "project-build-package-json-uninferred-generic-public-function",
        "package build json uninferred dependency generic public function import",
        &stderr,
    )
    .expect("uninferred generic function bridge diagnostics should stay on stdout in json mode");

    let json = parse_json_output(
        "project-build-package-json-uninferred-generic-public-function",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(json["status"], "failed");
    assert_eq!(
        json["failure"]["manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["failure"]["error_kind"],
        "dependency-function-unsupported-generic"
    );
    assert_eq!(json["failure"]["symbol"], "make");
    assert_eq!(json["failure"]["dependency_package"], "dep");
    assert_eq!(
        json["failure"]["dependency_manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    expect_file_exists(
        "project-build-package-json-uninferred-generic-public-function",
        &dep_output,
        "dependency package artifact",
        "package build json uninferred dependency generic public function import",
    )
    .expect("uninferred generic function import should preserve the dependency package artifact");
}
