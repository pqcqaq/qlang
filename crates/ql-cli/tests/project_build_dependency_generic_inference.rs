mod support;

use serde_json::Value as JsonValue;
use support::{
    TempDir, expect_empty_stderr, expect_exit_code, expect_file_exists, expect_success, ql_command,
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
