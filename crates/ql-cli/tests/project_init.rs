mod support;

use std::fs;
use std::path::{Path, PathBuf};

use ql_driver::{ToolchainOptions, discover_toolchain};
use serde_json::Value as JsonValue;
use support::{
    TempDir, executable_output_path, expect_empty_stderr, expect_empty_stdout, expect_exit_code,
    expect_file_exists, expect_silent_output, expect_stderr_contains, expect_stdout_contains_all,
    expect_success, ql_command, read_normalized_file, run_command_capture,
    static_library_output_path, workspace_root,
};

fn toolchain_available(context: &str) -> bool {
    let Ok(_toolchain) = discover_toolchain(&ToolchainOptions::default()) else {
        eprintln!(
            "skipping {context}: no clang-style compiler found via ql-driver toolchain discovery"
        );
        return false;
    };
    true
}

fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&stdout.replace("\r\n", "\n"))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
}

fn json_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn assert_stdlib_check_json(
    context: &str,
    check_json: &JsonValue,
    scope: &str,
    project_manifest: &Path,
    checked_files: &[PathBuf],
    stdlib_root: &Path,
) {
    assert_eq!(check_json["schema"], "ql.check.v1");
    assert_eq!(check_json["scope"], scope);
    assert_eq!(check_json["status"], "ok");
    assert_eq!(
        check_json["project_manifest_path"],
        json_path(project_manifest)
    );
    assert_eq!(check_json["diagnostic_files"], serde_json::json!([]));
    assert_eq!(check_json["failing_manifests"], serde_json::json!([]));
    assert_eq!(check_json["sync_interfaces"], false);
    assert_eq!(check_json["written_interfaces"], serde_json::json!([]));
    assert_eq!(
        check_json["checked_files"],
        serde_json::json!(
            checked_files
                .iter()
                .map(|path| json_path(path))
                .collect::<Vec<_>>()
        ),
        "{context} should report the initialized package sources"
    );
    assert_eq!(
        check_json["loaded_interfaces"],
        serde_json::json!([
            json_path(&stdlib_root.join("packages/array/std.array.qi")),
            json_path(&stdlib_root.join("packages/core/std.core.qi")),
            json_path(&stdlib_root.join("packages/option/std.option.qi")),
            json_path(&stdlib_root.join("packages/result/std.result.qi")),
            json_path(&stdlib_root.join("packages/test/std.test.qi")),
        ]),
        "{context} should load every initialized stdlib dependency interface"
    );
}

fn assert_stdlib_dependency_build_targets(context: &str, build_json: &JsonValue) {
    let built_targets = build_json["built_targets"]
        .as_array()
        .unwrap_or_else(|| panic!("{context} should expose built targets: {build_json}"));
    for package_name in [
        "std.array",
        "std.core",
        "std.option",
        "std.result",
        "std.test",
    ] {
        assert!(
            built_targets.iter().any(|target| {
                target["package_name"] == package_name
                    && target["dependency_only"] == true
                    && target["kind"] == "lib"
                    && target["selected"] == false
            }),
            "{context} should include dependency target `{package_name}`: {build_json}"
        );
    }
}

fn assert_build_json_includes_target(context: &str, build_json: &JsonValue, expected: JsonValue) {
    let built_targets = build_json["built_targets"]
        .as_array()
        .unwrap_or_else(|| panic!("{context} should expose built targets: {build_json}"));
    assert!(
        built_targets.iter().any(|target| target == &expected),
        "{context} should include target {expected}: {build_json}"
    );
}

fn write_repo_stdlib_fixture(temp: &TempDir, repo_root: &Path) -> PathBuf {
    let source_root = repo_root.join("stdlib");
    for relative in [
        "qlang.toml",
        "packages/core/qlang.toml",
        "packages/core/src/lib.ql",
        "packages/core/tests/smoke.ql",
        "packages/array/qlang.toml",
        "packages/array/src/lib.ql",
        "packages/array/tests/smoke.ql",
        "packages/option/qlang.toml",
        "packages/option/src/lib.ql",
        "packages/option/tests/smoke.ql",
        "packages/result/qlang.toml",
        "packages/result/src/lib.ql",
        "packages/result/tests/smoke.ql",
        "packages/test/qlang.toml",
        "packages/test/src/lib.ql",
        "packages/test/tests/smoke.ql",
        "examples/starter/qlang.toml",
        "examples/starter/src/lib.ql",
        "examples/starter/src/main.ql",
        "examples/starter/tests/smoke.ql",
    ] {
        let source_path = source_root.join(relative);
        let contents = fs::read_to_string(&source_path).unwrap_or_else(|error| {
            panic!("read stdlib fixture `{}`: {error}", source_path.display())
        });
        temp.write(&format!("stdlib/{relative}"), &contents);
    }
    temp.path().join("stdlib")
}

fn expect_stdlib_starter_source(source: &str, context: &str) {
    for needle in [
        "use std.array.repeat_array as repeat_array",
        "use std.option.Option as Option",
        "use std.result.error_to_option as result_error_to_option",
        "use std.result.ok_or as result_ok_or",
        "use std.result.to_option as result_to_option",
        "let repeated: [Int; 3] = repeat_array(1)",
        "let failed: Result[Int, Int] = result_ok_or(missing, 7)",
    ] {
        assert!(
            source.contains(needle),
            "{context} should contain `{needle}`\n{source}"
        );
    }
    for legacy in [
        "repeat3_array",
        "reverse3_array",
        "some_int",
        "ok_int",
        "unwrap_result_or as result_unwrap_result_or",
    ] {
        assert!(
            !source.contains(legacy),
            "{context} should not contain legacy API `{legacy}`\n{source}"
        );
    }
}

fn expect_stdlib_starter_main_source(source: &str, context: &str) {
    for needle in [
        "use std.array.repeat_array as repeat_array",
        "use std.result.to_option as result_to_option",
        "let repeated_false: [Bool; 3] = repeat_array(false)",
        "let repeated_enabled: [Bool; 3] = [option_unwrap_or(enabled, false); 3]",
    ] {
        assert!(
            source.contains(needle),
            "{context} should contain `{needle}`\n{source}"
        );
    }
    for legacy in ["repeat3_array", "reverse3_array", "some_bool", "ok_bool"] {
        assert!(
            !source.contains(legacy),
            "{context} should not contain legacy API `{legacy}`\n{source}"
        );
    }
}

fn expect_stdlib_starter_smoke_source(source: &str, context: &str) {
    for needle in [
        "use std.array.repeat_array as repeat_array",
        "use std.result.ok_or as result_ok_or",
        "use std.result.to_option as result_to_option",
        "use std.test.expect_array_eq as expect_array_eq",
        "use std.test.expect_array_reverse as expect_array_reverse",
        "use std.test.expect_eq as expect_eq",
        "use std.test.expect_option_none as expect_option_none",
        "use std.test.expect_option_some as expect_option_some",
        "use std.test.expect_result_err as expect_result_err",
        "use std.test.expect_result_ok as expect_result_ok",
        "let repeated: [Int; 3] = repeat_array(2)",
        "let array_check = expect_array_eq(repeated, [2, 2, 2]) + expect_array_reverse(numbers, [3, 2, 1])",
        "let result_value: Result[Int, Int] = result_ok_or(option_value, 9)",
        "let failed: Result[Int, Int] = result_ok_or(missing, 4)",
        "let option_check = expect_option_some(option_value, 6) + expect_option_none(missing)",
        "let result_check = expect_result_ok(result_value, 6) + expect_result_err(failed, 4)",
    ] {
        assert!(
            source.contains(needle),
            "{context} should contain `{needle}`\n{source}"
        );
    }
    assert!(
        !source.contains("result_error_to_option"),
        "{context} should not use conversion-only result assertions\n{source}"
    );
    for legacy in ["repeat3_array", "reverse3_array", "some_int", "ok_int"] {
        assert!(
            !source.contains(legacy),
            "{context} should not contain legacy API `{legacy}`\n{source}"
        );
    }
}

fn expect_stdlib_starter_interface(source: &str, package_name: &str, context: &str) {
    for needle in &[
        "// qlang interface v1".to_owned(),
        format!("// package: {package_name}"),
        "// source: src/lib.ql".to_owned(),
        "use std.array.repeat_array as repeat_array".to_owned(),
        "use std.option.Option as Option".to_owned(),
        "use std.result.Result as Result".to_owned(),
        "use std.result.ok_or as result_ok_or".to_owned(),
        "pub fn run() -> Int".to_owned(),
    ] {
        assert!(
            source.contains(needle),
            "{context} should contain `{needle}`\n{source}"
        );
    }
    for legacy in ["repeat3_array", "reverse3_array", "some_int", "ok_int"] {
        assert!(
            !source.contains(legacy),
            "{context} should not contain legacy API `{legacy}`\n{source}"
        );
    }
}

fn expect_emit_interface_check_ok(
    case_name: &str,
    workspace_root: &Path,
    project_root: &Path,
    package_name: Option<&str>,
    interface_path: &Path,
    description: &str,
) {
    let mut command = ql_command(workspace_root);
    command
        .args(["project", "emit-interface", "--check"])
        .arg(project_root);
    if let Some(package_name) = package_name {
        command.args(["--package", package_name]);
    }
    let output = run_command_capture(&mut command, description);
    let (stdout, stderr) = expect_success(
        case_name,
        "emit interface check initialized scaffold",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        case_name,
        "emit interface check initialized scaffold",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        case_name,
        &stdout.replace('\\', "/"),
        &[&format!(
            "ok interface: {}",
            interface_path.display().to_string().replace('\\', "/")
        )],
    )
    .unwrap();
}

struct AppCoreWorkspaceFixture {
    app_manifest_path: PathBuf,
    app_member_dir: PathBuf,
}

fn write_app_core_workspace_fixture(
    temp: &TempDir,
    app_manifest_source: &str,
) -> AppCoreWorkspaceFixture {
    let project_root = temp.path().join("workspace");
    let app_member_dir = project_root.join("packages/app");
    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\"]\n",
    );
    let app_manifest_path = temp.write("workspace/packages/app/qlang.toml", app_manifest_source);
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int {\n    return 0\n}\n",
    );
    temp.write(
        "workspace/packages/core/src/lib.ql",
        "pub fn core() -> Int {\n    return 1\n}\n",
    );

    AppCoreWorkspaceFixture {
        app_manifest_path,
        app_member_dir,
    }
}

#[test]
fn project_init_with_stdlib_copies_starter_template_from_stdlib_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-stdlib-template-copy");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);
    temp.write(
        "stdlib/examples/starter/src/lib.ql",
        "pub fn run() -> Int {\n    return 17\n}\n",
    );
    temp.write(
        "stdlib/examples/starter/src/main.ql",
        "fn main() -> Int {\n    return 18\n}\n",
    );
    temp.write(
        "stdlib/examples/starter/tests/smoke.ql",
        "fn main() -> Int {\n    return 19\n}\n",
    );
    let project_root = temp.path().join("demo-package");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--stdlib",
        &stdlib_root.to_string_lossy(),
    ]);
    let output = run_command_capture(&mut init, "`ql project init --stdlib` template copy");
    let (_stdout, stderr) = expect_success(
        "project-init-stdlib-template-copy",
        "stdlib package template copy",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-template-copy",
        "stdlib package template copy",
        &stderr,
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("src/lib.ql"),
            "copied stdlib package source"
        ),
        "pub fn run() -> Int {\n    return 17\n}\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("src/main.ql"),
            "copied stdlib package main source"
        ),
        "fn main() -> Int {\n    return 18\n}\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("tests/smoke.ql"),
            "copied stdlib package smoke test"
        ),
        "fn main() -> Int {\n    return 19\n}\n"
    );
}

#[test]
fn project_init_workspace_with_missing_stdlib_starter_fails_without_partial_manifest() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-stdlib-missing-starter");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);
    fs::remove_dir_all(stdlib_root.join("examples").join("starter"))
        .expect("remove stdlib starter template from fixture");
    let project_root = temp.path().join("demo-workspace");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
        "--stdlib",
        &stdlib_root.to_string_lossy(),
    ]);
    let output = run_command_capture(
        &mut init,
        "`ql project init --workspace --stdlib` missing starter",
    );
    let (_stdout, stderr) = expect_exit_code(
        "project-init-stdlib-missing-starter",
        "missing stdlib starter workspace init",
        &output,
        1,
    )
    .unwrap();
    expect_stderr_contains(
        "project-init-stdlib-missing-starter",
        "missing stdlib starter workspace init",
        &stderr,
        "stdlib starter template",
    )
    .unwrap();
    assert!(
        !project_root.join("qlang.toml").exists(),
        "workspace init should not write a partial root manifest when the stdlib starter is missing"
    );
}

#[test]
fn project_init_creates_package_scaffold_and_check_succeeds() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-package");
    let project_root = temp.path().join("demo-package");

    let mut init = ql_command(&workspace_root);
    init.args(["project", "init", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut init, "`ql project init` package");
    let (stdout, stderr) = expect_success("project-init-package", "package init", &output).unwrap();
    expect_empty_stderr("project-init-package", "package init", &stderr).unwrap();
    expect_stdout_contains_all(
        "project-init-package",
        &stdout,
        &[
            &format!(
                "created: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("src")
                    .join("lib.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("src")
                    .join("main.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("tests")
                    .join("smoke.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(&project_root.join("qlang.toml"), "package manifest"),
        "[package]\nname = \"demo-package\"\n"
    );
    assert_eq!(
        read_normalized_file(&project_root.join("src/lib.ql"), "package source"),
        "pub fn run() -> Int {\n    return 0\n}\n"
    );
    assert_eq!(
        read_normalized_file(&project_root.join("src/main.ql"), "package main source"),
        "fn main() -> Int {\n    return 0\n}\n"
    );
    assert_eq!(
        read_normalized_file(&project_root.join("tests/smoke.ql"), "package smoke test"),
        "fn main() -> Int {\n    return 0\n}\n"
    );

    let mut check = ql_command(&workspace_root);
    check.args(["check", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut check, "`ql check` initialized package");
    let (stdout, stderr) =
        expect_success("project-init-package", "check initialized package", &output).unwrap();
    expect_empty_stderr("project-init-package", "check initialized package", &stderr).unwrap();
    expect_stdout_contains_all(
        "project-init-package",
        &stdout,
        &[&format!(
            "ok: {}",
            project_root.join("src").join("lib.ql").to_string_lossy()
        )],
    )
    .unwrap();
}

#[test]
fn project_init_with_stdlib_creates_consuming_package_scaffold_and_check_succeeds() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-stdlib-package");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);
    let project_root = temp.path().join("demo-package");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--stdlib",
        &stdlib_root.to_string_lossy(),
    ]);
    let output = run_command_capture(&mut init, "`ql project init --stdlib` package");
    let (_stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "stdlib package init",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "stdlib package init",
        &stderr,
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(&project_root.join("qlang.toml"), "stdlib package manifest"),
        "[package]\nname = \"demo-package\"\n\n[dependencies]\n\"std.core\" = \"../stdlib/packages/core\"\n\"std.option\" = \"../stdlib/packages/option\"\n\"std.result\" = \"../stdlib/packages/result\"\n\"std.array\" = \"../stdlib/packages/array\"\n\"std.test\" = \"../stdlib/packages/test\"\n"
    );
    let lib_source =
        read_normalized_file(&project_root.join("src/lib.ql"), "stdlib package source");
    expect_stdlib_starter_source(&lib_source, "stdlib package source");
    let main_source = read_normalized_file(
        &project_root.join("src/main.ql"),
        "stdlib package main source",
    );
    expect_stdlib_starter_main_source(&main_source, "stdlib package main source");
    let smoke_source = read_normalized_file(
        &project_root.join("tests/smoke.ql"),
        "stdlib package smoke test",
    );
    expect_stdlib_starter_smoke_source(&smoke_source, "stdlib package smoke test");

    let mut check = ql_command(&workspace_root);
    check.args(["check", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut check, "`ql check` initialized stdlib package");
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "check initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "check initialized stdlib package",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-package",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "ok: {}",
                project_root
                    .join("src")
                    .join("lib.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            "loaded interface:",
        ],
    )
    .unwrap();

    let mut check_json = ql_command(&workspace_root);
    check_json.args(["check", &project_root.to_string_lossy(), "--json"]);
    let output = run_command_capture(
        &mut check_json,
        "`ql check --json` initialized stdlib package",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "json check initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "json check initialized stdlib package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-package", &stdout);
    assert_stdlib_check_json(
        "initialized stdlib package check json",
        &actual,
        "package",
        &project_root.join("qlang.toml"),
        &[
            project_root.join("src/lib.ql"),
            project_root.join("src/main.ql"),
        ],
        &stdlib_root,
    );

    let package_interface = project_root.join("demo-package.qi");
    let mut emit_interface = ql_command(&workspace_root);
    emit_interface.args(["project", "emit-interface", &project_root.to_string_lossy()]);
    let output = run_command_capture(
        &mut emit_interface,
        "`ql project emit-interface` initialized stdlib package",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "emit interface initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "emit interface initialized stdlib package",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-package",
        &stdout.replace('\\', "/"),
        &[&format!(
            "wrote interface: {}",
            package_interface.display().to_string().replace('\\', "/")
        )],
    )
    .unwrap();
    let interface_source = read_normalized_file(
        &package_interface,
        "initialized stdlib package interface artifact",
    );
    expect_stdlib_starter_interface(
        &interface_source,
        "demo-package",
        "initialized stdlib package interface artifact",
    );
    expect_emit_interface_check_ok(
        "project-init-stdlib-package",
        &workspace_root,
        &project_root,
        None,
        &package_interface,
        "`ql project emit-interface --check` initialized stdlib package",
    );
}

#[test]
fn project_init_creates_runnable_package_scaffold() {
    if !toolchain_available("`ql project init` runnable package test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-package-run");
    let project_root = temp.path().join("demo-package");
    let output_path = executable_output_path(&project_root.join("target/ql/debug"), "main");

    let mut init = ql_command(&workspace_root);
    init.args(["project", "init", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut init, "`ql project init` runnable package");
    let (_stdout, stderr) = expect_success(
        "project-init-package-run",
        "package init for runnable scaffold",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-package-run",
        "package init for runnable scaffold",
        &stderr,
    )
    .unwrap();

    let mut run = ql_command(&workspace_root);
    run.current_dir(temp.path());
    run.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut run, "`ql run` initialized package");
    let (stdout, stderr) = expect_exit_code(
        "project-init-package-run",
        "run initialized package",
        &output,
        0,
    )
    .unwrap();
    expect_silent_output(
        "project-init-package-run",
        "run initialized package",
        &stdout,
        &stderr,
    )
    .unwrap();
    expect_file_exists(
        "project-init-package-run",
        &output_path,
        "initialized package executable",
        "run initialized package",
    )
    .unwrap();
}

#[test]
fn project_init_with_stdlib_creates_runnable_and_testable_package_scaffold() {
    if !toolchain_available("`ql project init --stdlib` runnable package test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-stdlib-package-run");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);
    let project_root = temp.path().join("demo-package");
    let package_manifest = project_root.join("qlang.toml");
    let package_library_output =
        static_library_output_path(&project_root.join("target/ql/debug"), "lib");
    let package_build_output = project_root.join("target/ql/debug/main.ll");
    let package_run_output = executable_output_path(&project_root.join("target/ql/debug"), "main");
    let package_interface_output = project_root.join("demo-package.qi");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--stdlib",
        &stdlib_root.to_string_lossy(),
    ]);
    let output = run_command_capture(&mut init, "`ql project init --stdlib` runnable package");
    let (_stdout, stderr) = expect_success(
        "project-init-stdlib-package-run",
        "stdlib package init for runnable scaffold",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package-run",
        "stdlib package init for runnable scaffold",
        &stderr,
    )
    .unwrap();

    let mut build_json = ql_command(&workspace_root);
    build_json.current_dir(temp.path());
    build_json.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut build_json,
        "`ql build --json` initialized stdlib package",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package-run",
        "json build initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package-run",
        "json build initialized stdlib package",
        &stderr,
    )
    .unwrap();

    let build_json = parse_json_output("project-init-stdlib-package-run", &stdout);
    assert_eq!(build_json["schema"], "ql.build.v1");
    assert_eq!(
        build_json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(build_json["scope"], "project");
    assert_eq!(
        build_json["project_manifest_path"],
        package_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(build_json["requested_emit"], "llvm-ir");
    assert_eq!(build_json["requested_profile"], "debug");
    assert_eq!(build_json["profile_overridden"], false);
    assert_eq!(build_json["emit_interface"], false);
    assert_eq!(build_json["status"], "ok");
    assert_eq!(build_json["failure"], JsonValue::Null);
    assert_eq!(
        build_json["interfaces"],
        serde_json::json!([
            {
                "manifest_path": package_manifest.display().to_string().replace('\\', "/"),
                "package_name": "demo-package",
                "selected": true,
                "status": "wrote",
                "path": package_interface_output.display().to_string().replace('\\', "/"),
            }
        ])
    );
    assert_stdlib_dependency_build_targets("initialized stdlib package build json", &build_json);
    assert_build_json_includes_target(
        "initialized stdlib package build json",
        &build_json,
        serde_json::json!({
            "manifest_path": package_manifest.display().to_string().replace('\\', "/"),
            "package_name": "demo-package",
            "selected": true,
            "dependency_only": false,
            "kind": "lib",
            "path": "src/lib.ql",
            "emit": "staticlib",
            "profile": "debug",
            "artifact_path": package_library_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        }),
    );
    assert_build_json_includes_target(
        "initialized stdlib package build json",
        &build_json,
        serde_json::json!({
            "manifest_path": package_manifest.display().to_string().replace('\\', "/"),
            "package_name": "demo-package",
            "selected": true,
            "dependency_only": false,
            "kind": "bin",
            "path": "src/main.ql",
            "emit": "llvm-ir",
            "profile": "debug",
            "artifact_path": package_build_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        }),
    );
    expect_file_exists(
        "project-init-stdlib-package-run",
        &package_library_output,
        "initialized stdlib package library artifact",
        "json build initialized stdlib package",
    )
    .unwrap();
    expect_file_exists(
        "project-init-stdlib-package-run",
        &package_build_output,
        "initialized stdlib package build artifact",
        "json build initialized stdlib package",
    )
    .unwrap();
    expect_file_exists(
        "project-init-stdlib-package-run",
        &package_interface_output,
        "initialized stdlib package interface artifact",
        "json build initialized stdlib package",
    )
    .unwrap();

    let mut run = ql_command(&workspace_root);
    run.current_dir(temp.path());
    run.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut run, "`ql run` initialized stdlib package");
    let (stdout, stderr) = expect_exit_code(
        "project-init-stdlib-package-run",
        "run initialized stdlib package",
        &output,
        0,
    )
    .unwrap();
    expect_silent_output(
        "project-init-stdlib-package-run",
        "run initialized stdlib package",
        &stdout,
        &stderr,
    )
    .unwrap();

    let mut run_json = ql_command(&workspace_root);
    run_json.current_dir(temp.path());
    run_json.args(["run"]).arg(&project_root).arg("--json");
    let output = run_command_capture(&mut run_json, "`ql run --json` initialized stdlib package");
    let (stdout, stderr) = expect_exit_code(
        "project-init-stdlib-package-run",
        "json run initialized stdlib package",
        &output,
        0,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package-run",
        "json run initialized stdlib package",
        &stderr,
    )
    .unwrap();

    let run_json = parse_json_output("project-init-stdlib-package-run", &stdout);
    assert_eq!(run_json["schema"], "ql.run.v1");
    assert_eq!(
        run_json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(run_json["scope"], "project");
    assert_eq!(
        run_json["project_manifest_path"],
        package_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(run_json["requested_profile"], "debug");
    assert_eq!(run_json["profile_overridden"], false);
    assert_eq!(run_json["program_args"], serde_json::json!([]));
    assert_eq!(run_json["status"], "completed");
    assert_eq!(run_json["failure"], JsonValue::Null);
    assert_eq!(
        run_json["built_target"],
        serde_json::json!({
            "manifest_path": package_manifest.display().to_string().replace('\\', "/"),
            "package_name": "demo-package",
            "selected": true,
            "dependency_only": false,
            "kind": "bin",
            "path": "src/main.ql",
            "emit": "exe",
            "profile": "debug",
            "artifact_path": package_run_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        })
    );
    assert_eq!(
        run_json["execution"],
        serde_json::json!({
            "exit_code": 0,
            "stdout": "",
            "stderr": "",
        })
    );
    expect_file_exists(
        "project-init-stdlib-package-run",
        &package_run_output,
        "initialized stdlib package executable",
        "json run initialized stdlib package",
    )
    .unwrap();

    let mut test = ql_command(&workspace_root);
    test.current_dir(temp.path());
    test.args(["test"]).arg(&project_root);
    let output = run_command_capture(&mut test, "`ql test` initialized stdlib package");
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package-run",
        "test initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package-run",
        "test initialized stdlib package",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-package-run",
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .unwrap();

    let mut test_json = ql_command(&workspace_root);
    test_json.current_dir(temp.path());
    test_json.args(["test", "--json"]).arg(&project_root);
    let output = run_command_capture(
        &mut test_json,
        "`ql test --json` initialized stdlib package",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package-run",
        "json test initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package-run",
        "json test initialized stdlib package",
        &stderr,
    )
    .unwrap();

    let actual = parse_json_output("project-init-stdlib-package-run", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.test.v1",
        "path": project_root.display().to_string().replace('\\', "/"),
        "requested_profile": "debug",
        "profile_overridden": false,
        "package_name": JsonValue::Null,
        "filter": JsonValue::Null,
        "list_only": false,
        "status": "ok",
        "discovered_total": 1,
        "selected_total": 1,
        "targets": [
            {
                "path": "tests/smoke.ql",
                "kind": "smoke",
                "profile": "debug",
            }
        ],
        "passed": 1,
        "failed": 0,
        "failures": [],
    });
    assert_eq!(
        actual, expected,
        "initialized stdlib package should keep a stable test json contract"
    );
}

#[test]
fn project_init_creates_workspace_scaffold_and_graph_succeeds() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-workspace");
    let project_root = temp.path().join("demo-workspace");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut init, "`ql project init --workspace`");
    let (stdout, stderr) =
        expect_success("project-init-workspace", "workspace init", &output).unwrap();
    expect_empty_stderr("project-init-workspace", "workspace init", &stderr).unwrap();
    expect_stdout_contains_all(
        "project-init-workspace",
        &stdout,
        &[
            &format!(
                "created: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("packages")
                    .join("app")
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("packages")
                    .join("app")
                    .join("src")
                    .join("main.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("packages")
                    .join("app")
                    .join("src")
                    .join("lib.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("packages")
                    .join("app")
                    .join("tests")
                    .join("smoke.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(&project_root.join("qlang.toml"), "workspace manifest"),
        "[workspace]\nmembers = [\"packages/app\"]\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace member manifest"
        ),
        "[package]\nname = \"app\"\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/src/main.ql"),
            "workspace member main source"
        ),
        "fn main() -> Int {\n    return 0\n}\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/tests/smoke.ql"),
            "workspace member smoke test"
        ),
        "fn main() -> Int {\n    return 0\n}\n"
    );

    let mut graph = ql_command(&workspace_root);
    graph.args(["project", "graph", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut graph, "`ql project graph` initialized workspace");
    let (stdout, stderr) = expect_success(
        "project-init-workspace",
        "graph initialized workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-workspace",
        "graph initialized workspace",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-workspace",
        &stdout,
        &[
            "package: <none>",
            "workspace_members:",
            "  - packages/app",
            "workspace_packages:",
            "  - member: packages/app",
            "    package: app",
            "    status: missing",
        ],
    )
    .unwrap();
}

#[test]
fn project_init_with_stdlib_creates_consuming_workspace_scaffold_and_check_succeeds() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-stdlib-workspace");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);
    let project_root = temp.path().join("demo-workspace");
    let member_root = project_root.join("packages/app");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
        "--stdlib",
        &stdlib_root.to_string_lossy(),
    ]);
    let output = run_command_capture(&mut init, "`ql project init --workspace --stdlib`");
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "stdlib workspace init",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "stdlib workspace init",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-workspace",
        &stdout,
        &[
            &format!(
                "created: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                member_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                member_root
                    .join("tests/smoke.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("qlang.toml"),
            "stdlib workspace manifest"
        ),
        "[workspace]\nmembers = [\"packages/app\"]\n"
    );
    assert_eq!(
        read_normalized_file(
            &member_root.join("qlang.toml"),
            "stdlib workspace member manifest"
        ),
        "[package]\nname = \"app\"\n\n[dependencies]\n\"std.core\" = \"../../../stdlib/packages/core\"\n\"std.option\" = \"../../../stdlib/packages/option\"\n\"std.result\" = \"../../../stdlib/packages/result\"\n\"std.array\" = \"../../../stdlib/packages/array\"\n\"std.test\" = \"../../../stdlib/packages/test\"\n"
    );
    let lib_source = read_normalized_file(
        &member_root.join("src/lib.ql"),
        "stdlib workspace member source",
    );
    expect_stdlib_starter_source(&lib_source, "stdlib workspace member source");
    let main_source = read_normalized_file(
        &member_root.join("src/main.ql"),
        "stdlib workspace member main source",
    );
    expect_stdlib_starter_main_source(&main_source, "stdlib workspace member main source");
    let smoke_source = read_normalized_file(
        &member_root.join("tests/smoke.ql"),
        "stdlib workspace member smoke test",
    );
    expect_stdlib_starter_smoke_source(&smoke_source, "stdlib workspace member smoke test");

    let mut check = ql_command(&workspace_root);
    check.args(["check", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut check, "`ql check` initialized stdlib workspace");
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "check initialized stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "check initialized stdlib workspace",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-workspace",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "ok: {}",
                member_root
                    .join("src/lib.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            "loaded interface:",
        ],
    )
    .unwrap();

    let mut check_json = ql_command(&workspace_root);
    check_json.args(["check", &project_root.to_string_lossy(), "--json"]);
    let output = run_command_capture(
        &mut check_json,
        "`ql check --json` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "json check initialized stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "json check initialized stdlib workspace",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-workspace", &stdout);
    assert_stdlib_check_json(
        "initialized stdlib workspace check json",
        &actual,
        "workspace",
        &project_root.join("qlang.toml"),
        &[
            member_root.join("src/lib.ql"),
            member_root.join("src/main.ql"),
        ],
        &stdlib_root,
    );

    let member_interface = member_root.join("app.qi");
    let mut emit_interface = ql_command(&workspace_root);
    emit_interface.args([
        "project",
        "emit-interface",
        &project_root.to_string_lossy(),
        "--package",
        "app",
    ]);
    let output = run_command_capture(
        &mut emit_interface,
        "`ql project emit-interface --package app` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "emit interface initialized stdlib workspace package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "emit interface initialized stdlib workspace package",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-workspace",
        &stdout.replace('\\', "/"),
        &[&format!(
            "wrote interface: {}",
            member_interface.display().to_string().replace('\\', "/")
        )],
    )
    .unwrap();
    let interface_source = read_normalized_file(
        &member_interface,
        "initialized stdlib workspace member interface artifact",
    );
    expect_stdlib_starter_interface(
        &interface_source,
        "app",
        "initialized stdlib workspace member interface artifact",
    );
    expect_emit_interface_check_ok(
        "project-init-stdlib-workspace",
        &workspace_root,
        &project_root,
        Some("app"),
        &member_interface,
        "`ql project emit-interface --check --package app` initialized stdlib workspace",
    );
}

#[test]
fn project_init_with_stdlib_creates_runnable_and_testable_workspace_scaffold() {
    if !toolchain_available("`ql project init --workspace --stdlib` runnable workspace test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-stdlib-workspace-run");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);
    let project_root = temp.path().join("demo-workspace");
    let member_root = project_root.join("packages/app");
    let app_manifest = member_root.join("qlang.toml");
    let app_library_output =
        static_library_output_path(&member_root.join("target/ql/debug"), "lib");
    let app_build_output = member_root.join("target/ql/debug/main.ll");
    let app_output = executable_output_path(&member_root.join("target/ql/debug"), "main");
    let app_interface_output = member_root.join("app.qi");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
        "--stdlib",
        &stdlib_root.to_string_lossy(),
    ]);
    let output = run_command_capture(
        &mut init,
        "`ql project init --workspace --stdlib` runnable workspace",
    );
    let (_stdout, stderr) = expect_success(
        "project-init-stdlib-workspace-run",
        "stdlib workspace init for runnable scaffold",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace-run",
        "stdlib workspace init for runnable scaffold",
        &stderr,
    )
    .unwrap();

    let mut build_json = ql_command(&workspace_root);
    build_json.current_dir(temp.path());
    build_json
        .args(["build"])
        .arg(&project_root)
        .args(["--package", "app", "--json"]);
    let output = run_command_capture(
        &mut build_json,
        "`ql build --json --package app` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace-run",
        "json build initialized stdlib workspace package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace-run",
        "json build initialized stdlib workspace package",
        &stderr,
    )
    .unwrap();

    let build_json = parse_json_output("project-init-stdlib-workspace-run", &stdout);
    assert_eq!(build_json["schema"], "ql.build.v1");
    assert_eq!(
        build_json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(build_json["scope"], "project");
    assert_eq!(
        build_json["project_manifest_path"],
        project_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    assert_eq!(build_json["requested_emit"], "llvm-ir");
    assert_eq!(build_json["requested_profile"], "debug");
    assert_eq!(build_json["profile_overridden"], false);
    assert_eq!(build_json["emit_interface"], false);
    assert_eq!(build_json["status"], "ok");
    assert_eq!(build_json["failure"], JsonValue::Null);
    assert_eq!(
        build_json["interfaces"],
        serde_json::json!([
            {
                "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "selected": true,
                "status": "wrote",
                "path": app_interface_output.display().to_string().replace('\\', "/"),
            }
        ])
    );
    assert_stdlib_dependency_build_targets("initialized stdlib workspace build json", &build_json);
    assert_build_json_includes_target(
        "initialized stdlib workspace build json",
        &build_json,
        serde_json::json!({
            "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
            "package_name": "app",
            "selected": true,
            "dependency_only": false,
            "kind": "lib",
            "path": "src/lib.ql",
            "emit": "staticlib",
            "profile": "debug",
            "artifact_path": app_library_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        }),
    );
    assert_build_json_includes_target(
        "initialized stdlib workspace build json",
        &build_json,
        serde_json::json!({
            "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
            "package_name": "app",
            "selected": true,
            "dependency_only": false,
            "kind": "bin",
            "path": "src/main.ql",
            "emit": "llvm-ir",
            "profile": "debug",
            "artifact_path": app_build_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        }),
    );
    expect_file_exists(
        "project-init-stdlib-workspace-run",
        &app_library_output,
        "initialized stdlib workspace library artifact",
        "json build initialized stdlib workspace package",
    )
    .unwrap();
    expect_file_exists(
        "project-init-stdlib-workspace-run",
        &app_build_output,
        "initialized stdlib workspace build artifact",
        "json build initialized stdlib workspace package",
    )
    .unwrap();
    expect_file_exists(
        "project-init-stdlib-workspace-run",
        &app_interface_output,
        "initialized stdlib workspace interface artifact",
        "json build initialized stdlib workspace package",
    )
    .unwrap();

    let mut run = ql_command(&workspace_root);
    run.current_dir(temp.path());
    run.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut run, "`ql run` initialized stdlib workspace");
    let (stdout, stderr) = expect_exit_code(
        "project-init-stdlib-workspace-run",
        "run initialized stdlib workspace",
        &output,
        0,
    )
    .unwrap();
    expect_silent_output(
        "project-init-stdlib-workspace-run",
        "run initialized stdlib workspace",
        &stdout,
        &stderr,
    )
    .unwrap();

    let mut run_json = ql_command(&workspace_root);
    run_json.current_dir(temp.path());
    run_json
        .args(["run"])
        .arg(&project_root)
        .args(["--package", "app", "--json"]);
    let output = run_command_capture(
        &mut run_json,
        "`ql run --json --package app` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-init-stdlib-workspace-run",
        "json run initialized stdlib workspace package",
        &output,
        0,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace-run",
        "json run initialized stdlib workspace package",
        &stderr,
    )
    .unwrap();
    let run_json = parse_json_output("project-init-stdlib-workspace-run", &stdout);
    assert_eq!(run_json["schema"], "ql.run.v1");
    assert_eq!(
        run_json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(run_json["scope"], "project");
    assert_eq!(
        run_json["project_manifest_path"],
        project_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    assert_eq!(run_json["requested_profile"], "debug");
    assert_eq!(run_json["profile_overridden"], false);
    assert_eq!(run_json["program_args"], serde_json::json!([]));
    assert_eq!(run_json["status"], "completed");
    assert_eq!(run_json["failure"], JsonValue::Null);
    assert_eq!(
        run_json["built_target"],
        serde_json::json!({
            "manifest_path": member_root.join("qlang.toml").display().to_string().replace('\\', "/"),
            "package_name": "app",
            "selected": true,
            "dependency_only": false,
            "kind": "bin",
            "path": "src/main.ql",
            "emit": "exe",
            "profile": "debug",
            "artifact_path": app_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        })
    );
    assert_eq!(
        run_json["execution"],
        serde_json::json!({
            "exit_code": 0,
            "stdout": "",
            "stderr": "",
        })
    );
    expect_file_exists(
        "project-init-stdlib-workspace-run",
        &app_output,
        "initialized stdlib workspace executable",
        "json run initialized stdlib workspace package",
    )
    .unwrap();

    let mut test = ql_command(&workspace_root);
    test.current_dir(temp.path());
    test.args(["test"]).arg(&project_root);
    let output = run_command_capture(&mut test, "`ql test` initialized stdlib workspace");
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace-run",
        "test initialized stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace-run",
        "test initialized stdlib workspace",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-workspace-run",
        &stdout.replace('\\', "/"),
        &[
            "test packages/app/tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .unwrap();

    let mut test_json = ql_command(&workspace_root);
    test_json.current_dir(temp.path());
    test_json
        .args(["test", "--json"])
        .arg(&project_root)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut test_json,
        "`ql test --json --package app` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace-run",
        "json test initialized stdlib workspace package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace-run",
        "json test initialized stdlib workspace package",
        &stderr,
    )
    .unwrap();

    let actual = parse_json_output("project-init-stdlib-workspace-run", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.test.v1",
        "path": project_root.display().to_string().replace('\\', "/"),
        "requested_profile": "debug",
        "profile_overridden": false,
        "package_name": "app",
        "filter": JsonValue::Null,
        "list_only": false,
        "status": "ok",
        "discovered_total": 1,
        "selected_total": 1,
        "targets": [
            {
                "path": "packages/app/tests/smoke.ql",
                "kind": "smoke",
                "profile": "debug",
            }
        ],
        "passed": 1,
        "failed": 0,
        "failures": [],
    });
    assert_eq!(
        actual, expected,
        "initialized stdlib workspace should keep a stable package-selected test json contract"
    );
}

#[test]
fn project_init_refuses_to_overwrite_existing_manifest() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-conflict");
    let project_root = temp.path().join("demo-conflict");
    temp.write(
        "demo-conflict/qlang.toml",
        "[package]\nname = \"already-there\"\n",
    );

    let mut init = ql_command(&workspace_root);
    init.args(["project", "init", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut init, "`ql project init` conflicting manifest");
    let (stdout, stderr) = support::expect_exit_code(
        "project-init-conflict",
        "conflicting package init",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout("project-init-conflict", "conflicting package init", &stdout).unwrap();
    expect_stderr_contains(
        "project-init-conflict",
        "conflicting package init",
        &stderr,
        &format!(
            "error: `ql project init` would overwrite existing path `{}`",
            project_root
                .join("qlang.toml")
                .to_string_lossy()
                .replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_add_creates_workspace_member_from_member_source_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-success");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/app/src/main.ql");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut init, "`ql project init` workspace for add");
    let (_stdout, stderr) =
        expect_success("project-add-success", "workspace init for add", &output).unwrap();
    expect_empty_stderr("project-add-success", "workspace init for add", &stderr).unwrap();

    let mut add_core = ql_command(&workspace_root);
    add_core.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(&mut add_core, "`ql project add` workspace core member");
    let (_stdout, stderr) =
        expect_success("project-add-success", "add workspace core member", &output).unwrap();
    expect_empty_stderr("project-add-success", "add workspace core member", &stderr).unwrap();

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "add",
        &request_path.to_string_lossy(),
        "--name",
        "tools",
        "--dependency",
        "app",
        "--dependency",
        "core",
    ]);
    let output = run_command_capture(&mut add, "`ql project add` workspace member source path");
    let (stdout, stderr) =
        expect_success("project-add-success", "add workspace member", &output).unwrap();
    expect_empty_stderr("project-add-success", "add workspace member", &stderr).unwrap();
    expect_stdout_contains_all(
        "project-add-success",
        &stdout,
        &[
            &format!(
                "updated: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("packages/tools/qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("packages/tools/src/lib.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("packages/tools/src/main.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("packages/tools/tests/smoke.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    let workspace_manifest = read_normalized_file(
        &project_root.join("qlang.toml"),
        "workspace manifest after add",
    );
    assert!(
        workspace_manifest.contains("packages/app"),
        "workspace manifest should keep existing member entry"
    );
    assert!(
        workspace_manifest.contains("packages/tools"),
        "workspace manifest should add the new member entry"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/tools/qlang.toml"),
            "added workspace member manifest"
        ),
        "[package]\nname = \"tools\"\n\n[dependencies]\napp = \"../app\"\ncore = \"../core\"\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/tools/src/lib.ql"),
            "added workspace member lib"
        ),
        "pub fn run() -> Int {\n    return 0\n}\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/tools/tests/smoke.ql"),
            "added workspace member smoke test"
        ),
        "fn main() -> Int {\n    return 0\n}\n"
    );

    let mut graph = ql_command(&workspace_root);
    graph.args(["project", "graph", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut graph, "`ql project graph` after add");
    let (stdout, stderr) =
        expect_success("project-add-success", "graph workspace after add", &output).unwrap();
    expect_empty_stderr("project-add-success", "graph workspace after add", &stderr).unwrap();
    expect_stdout_contains_all(
        "project-add-success",
        &stdout,
        &[
            "workspace_members:",
            "  - packages/app",
            "  - packages/core",
            "  - packages/tools",
            "  - member: packages/core",
            "    package: core",
            "  - member: packages/tools",
            "    package: tools",
            "    status: missing",
            "    references:",
            "      - ../app",
            "      - ../core",
        ],
    )
    .unwrap();
}

#[test]
fn project_add_refuses_duplicate_workspace_package_name() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-duplicate");
    let project_root = temp.path().join("workspace");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut init, "`ql project init` workspace for duplicate add");
    let (_stdout, stderr) = expect_success(
        "project-add-duplicate",
        "workspace init for duplicate add",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-duplicate",
        "workspace init for duplicate add",
        &stderr,
    )
    .unwrap();

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut add, "`ql project add` duplicate package");
    let (stdout, stderr) = expect_exit_code(
        "project-add-duplicate",
        "duplicate workspace package add",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-duplicate",
        "duplicate workspace package add",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-duplicate",
        "duplicate workspace package add",
        &stderr,
        "already declares member `packages/app`",
    )
    .unwrap();
}

#[test]
fn project_add_refuses_to_overwrite_existing_member_directory() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-conflict");
    let project_root = temp.path().join("workspace");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut init, "`ql project init` workspace for conflict add");
    let (_stdout, stderr) = expect_success(
        "project-add-conflict",
        "workspace init for conflict add",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-conflict",
        "workspace init for conflict add",
        &stderr,
    )
    .unwrap();

    temp.write("workspace/packages/tools/placeholder.txt", "already-here");

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "tools",
    ]);
    let output = run_command_capture(&mut add, "`ql project add` conflicting member directory");
    let (stdout, stderr) = expect_exit_code(
        "project-add-conflict",
        "conflicting workspace member add",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-conflict",
        "conflicting workspace member add",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-conflict",
        "conflicting workspace member add",
        &stderr,
        &format!(
            "error: `ql project add` would overwrite existing path `{}`",
            project_root
                .join("packages/tools")
                .to_string_lossy()
                .replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_add_rejects_ambiguous_existing_workspace_package_name() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-ambiguous-package");
    let project_root = temp.path().join("workspace");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/a\", \"packages/b\"]\n",
    );
    temp.write(
        "workspace/packages/a/qlang.toml",
        "[package]\nname = \"util\"\n",
    );
    temp.write(
        "workspace/packages/b/qlang.toml",
        "[package]\nname = \"util\"\n",
    );

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "util",
    ]);
    let output = run_command_capture(&mut add, "`ql project add` ambiguous existing package");
    let (stdout, stderr) = expect_exit_code(
        "project-add-ambiguous-package",
        "add workspace member with ambiguous existing package name",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-ambiguous-package",
        "add workspace member with ambiguous existing package name",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-ambiguous-package",
        "add workspace member with ambiguous existing package name",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project add` workspace manifest `{}` contains multiple members for package `util`: packages/a, packages/b",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
    assert!(
        !project_root.join("packages/util").exists(),
        "ambiguous package add should not create the new workspace member directory"
    );
}

#[test]
fn project_add_existing_workspace_member_from_source_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-existing");
    let project_root = temp.path().join("workspace");
    let existing_request_path = project_root.join("vendor/core/src/main.ql");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut init, "`ql project init` workspace for existing add");
    let (_stdout, stderr) = expect_success(
        "project-add-existing",
        "workspace init for existing add",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-existing",
        "workspace init for existing add",
        &stderr,
    )
    .unwrap();

    temp.write(
        "workspace/vendor/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );
    temp.write(
        "workspace/vendor/core/src/main.ql",
        "fn main() -> Int {\n    return 0\n}\n",
    );

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--existing",
        &existing_request_path.to_string_lossy(),
    ]);
    let output = run_command_capture(&mut add, "`ql project add --existing` source path");
    let (stdout, stderr) = expect_success(
        "project-add-existing",
        "add existing workspace member from source path",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-existing",
        "add existing workspace member from source path",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-add-existing",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "updated: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "added: {}",
                project_root
                    .join("vendor/core")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("qlang.toml"),
            "workspace manifest after existing member add"
        ),
        "[workspace]\nmembers = [\"packages/app\", \"vendor/core\"]\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("vendor/core/qlang.toml"),
            "existing package manifest after workspace add"
        ),
        "[package]\nname = \"core\"\n"
    );
}

#[test]
fn project_add_existing_refuses_name_override() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-existing-name");
    let project_root = temp.path().join("workspace");
    let existing_member_root = project_root.join("vendor/core");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(
        &mut init,
        "`ql project init` workspace for existing add name conflict",
    );
    let (_stdout, stderr) = expect_success(
        "project-add-existing-name",
        "workspace init for existing add name conflict",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-existing-name",
        "workspace init for existing add name conflict",
        &stderr,
    )
    .unwrap();

    temp.write(
        "workspace/vendor/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--existing",
        &existing_member_root.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(&mut add, "`ql project add --existing --name`");
    let (stdout, stderr) = expect_exit_code(
        "project-add-existing-name",
        "add existing workspace member with explicit name override",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-existing-name",
        "add existing workspace member with explicit name override",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-existing-name",
        "add existing workspace member with explicit name override",
        &stderr,
        "error: `ql project add --existing` does not accept `--name`; package name comes from the existing manifest",
    )
    .unwrap();
}

#[test]
fn project_add_existing_rejects_ambiguous_existing_workspace_package_name() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-existing-ambiguous-package");
    let project_root = temp.path().join("workspace");
    let existing_member_root = project_root.join("vendor/util");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/a\", \"packages/b\"]\n",
    );
    temp.write(
        "workspace/packages/a/qlang.toml",
        "[package]\nname = \"util\"\n",
    );
    temp.write(
        "workspace/packages/b/qlang.toml",
        "[package]\nname = \"util\"\n",
    );
    temp.write(
        "workspace/vendor/util/qlang.toml",
        "[package]\nname = \"util\"\n",
    );

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--existing",
        &existing_member_root.to_string_lossy(),
    ]);
    let output = run_command_capture(
        &mut add,
        "`ql project add --existing` ambiguous existing package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-add-existing-ambiguous-package",
        "add existing workspace member with ambiguous existing package name",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-existing-ambiguous-package",
        "add existing workspace member with ambiguous existing package name",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-existing-ambiguous-package",
        "add existing workspace member with ambiguous existing package name",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project add` workspace manifest `{}` contains multiple members for package `util`: packages/a, packages/b",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_add_refuses_unknown_workspace_dependency() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-missing-dependency");
    let project_root = temp.path().join("workspace");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(
        &mut init,
        "`ql project init` workspace for missing dependency add",
    );
    let (_stdout, stderr) = expect_success(
        "project-add-missing-dependency",
        "workspace init for missing dependency add",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-missing-dependency",
        "workspace init for missing dependency add",
        &stderr,
    )
    .unwrap();

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "tools",
        "--dependency",
        "missing",
    ]);
    let output = run_command_capture(&mut add, "`ql project add` missing dependency");
    let (stdout, stderr) = expect_exit_code(
        "project-add-missing-dependency",
        "workspace member add with missing dependency",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-missing-dependency",
        "workspace member add with missing dependency",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-missing-dependency",
        "workspace member add with missing dependency",
        &stderr,
        &format!(
            "error: `ql project add` workspace manifest `{}` does not contain package `missing`",
            project_root
                .join("qlang.toml")
                .to_string_lossy()
                .replace('\\', "/")
        ),
    )
    .unwrap();
    assert!(
        !project_root.join("packages/tools").exists(),
        "missing dependency add should not create the new workspace member directory"
    );
}

#[test]
fn project_add_refuses_ambiguous_workspace_dependency_package() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-ambiguous-dependency");
    let project_root = temp.path().join("workspace");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/a\", \"packages/b\"]\n",
    );
    temp.write(
        "workspace/packages/a/qlang.toml",
        "[package]\nname = \"util\"\n",
    );
    temp.write(
        "workspace/packages/b/qlang.toml",
        "[package]\nname = \"util\"\n",
    );

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "tools",
        "--dependency",
        "util",
    ]);
    let output = run_command_capture(&mut add, "`ql project add` ambiguous dependency");
    let (stdout, stderr) = expect_exit_code(
        "project-add-ambiguous-dependency",
        "workspace member add with ambiguous dependency package",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-ambiguous-dependency",
        "workspace member add with ambiguous dependency package",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-ambiguous-dependency",
        "workspace member add with ambiguous dependency package",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project add` workspace manifest `{}` contains multiple members for package `util`: packages/a, packages/b",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
    assert!(
        !project_root.join("packages/tools").exists(),
        "ambiguous dependency add should not create the new workspace member directory"
    );
}

#[test]
fn project_add_dependency_updates_existing_package_manifest_from_member_source_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-dependency-success");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/app/src/main.ql");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut init, "`ql project init` workspace for add-dependency");
    let (_stdout, stderr) = expect_success(
        "project-add-dependency-success",
        "workspace init for add-dependency",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-success",
        "workspace init for add-dependency",
        &stderr,
    )
    .unwrap();

    let mut add_core = ql_command(&workspace_root);
    add_core.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut add_core,
        "`ql project add` workspace member for add-dependency",
    );
    let (_stdout, stderr) = expect_success(
        "project-add-dependency-success",
        "add workspace member for add-dependency",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-success",
        "add workspace member for add-dependency",
        &stderr,
    )
    .unwrap();

    let mut add_dependency = ql_command(&workspace_root);
    add_dependency.args([
        "project",
        "add-dependency",
        &request_path.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut add_dependency,
        "`ql project add-dependency` workspace member source path",
    );
    let (stdout, stderr) = expect_success(
        "project-add-dependency-success",
        "add dependency to existing package manifest",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-success",
        "add dependency to existing package manifest",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-add-dependency-success",
        &stdout,
        &[&format!(
            "updated: {}",
            project_root
                .join("packages/app/qlang.toml")
                .to_string_lossy()
                .replace('\\', "/")
        )],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace member manifest after add-dependency"
        ),
        "[dependencies]\ncore = \"../core\"\n\n[package]\nname = \"app\"\n"
    );
}

#[test]
fn project_add_dependency_updates_existing_package_manifest_from_member_directory() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-dependency-member-dir");
    let fixture = write_app_core_workspace_fixture(&temp, "[package]\nname = \"app\"\n");

    let mut add_dependency = ql_command(&workspace_root);
    add_dependency.args([
        "project",
        "add-dependency",
        &fixture.app_member_dir.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut add_dependency,
        "`ql project add-dependency` workspace member directory",
    );
    let (stdout, stderr) = expect_success(
        "project-add-dependency-member-dir",
        "add dependency to existing package manifest from member directory",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-member-dir",
        "add dependency to existing package manifest from member directory",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-add-dependency-member-dir",
        &stdout.replace('\\', "/"),
        &[&format!(
            "updated: {}",
            fixture
                .app_manifest_path
                .to_string_lossy()
                .replace('\\', "/")
        )],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &fixture.app_manifest_path,
            "workspace member manifest after member directory add-dependency"
        ),
        "[dependencies]\ncore = \"../core\"\n\n[package]\nname = \"app\"\n"
    );
}

#[test]
fn project_add_dependency_supports_workspace_root_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-dependency-selector");
    let project_root = temp.path().join("workspace");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(
        &mut init,
        "`ql project init` workspace for selected add-dependency",
    );
    let (_stdout, stderr) = expect_success(
        "project-add-dependency-selector",
        "workspace init for selected add-dependency",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-selector",
        "workspace init for selected add-dependency",
        &stderr,
    )
    .unwrap();

    let mut add_core = ql_command(&workspace_root);
    add_core.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut add_core,
        "`ql project add` workspace member for selected add-dependency",
    );
    let (_stdout, stderr) = expect_success(
        "project-add-dependency-selector",
        "add workspace member for selected add-dependency",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-selector",
        "add workspace member for selected add-dependency",
        &stderr,
    )
    .unwrap();

    let mut add_dependency = ql_command(&workspace_root);
    add_dependency.args([
        "project",
        "add-dependency",
        &project_root.to_string_lossy(),
        "--package",
        "app",
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut add_dependency,
        "`ql project add-dependency --package` workspace root",
    );
    let (stdout, stderr) = expect_success(
        "project-add-dependency-selector",
        "add dependency from workspace root with package selector",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-selector",
        "add dependency from workspace root with package selector",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-add-dependency-selector",
        &stdout,
        &[&format!(
            "updated: {}",
            project_root
                .join("packages/app/qlang.toml")
                .to_string_lossy()
                .replace('\\', "/")
        )],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace member manifest after selected add-dependency"
        ),
        "[dependencies]\ncore = \"../core\"\n\n[package]\nname = \"app\"\n"
    );
}

#[test]
fn project_add_dependency_supports_external_local_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-dependency-path");
    let project_root = temp.path().join("workspace");
    let vendor_source_path = project_root.join("vendor/core/src/lib.ql");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(
        &mut init,
        "`ql project init` workspace for path add-dependency",
    );
    let (_stdout, stderr) = expect_success(
        "project-add-dependency-path",
        "workspace init for path add-dependency",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-path",
        "workspace init for path add-dependency",
        &stderr,
    )
    .unwrap();

    temp.write(
        "workspace/vendor/core/qlang.toml",
        "[package]\nname = \"vendor.core\"\n",
    );
    temp.write(
        "workspace/vendor/core/src/lib.ql",
        "pub fn helper() -> Int {\n    return 1\n}\n",
    );

    let mut add_dependency = ql_command(&workspace_root);
    add_dependency.args([
        "project",
        "add-dependency",
        &project_root.to_string_lossy(),
        "--package",
        "app",
        "--path",
        &vendor_source_path.to_string_lossy(),
    ]);
    let output = run_command_capture(
        &mut add_dependency,
        "`ql project add-dependency --path` external local package",
    );
    let (stdout, stderr) = expect_success(
        "project-add-dependency-path",
        "add external local dependency by path",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-path",
        "add external local dependency by path",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-add-dependency-path",
        &stdout,
        &[&format!(
            "updated: {}",
            project_root
                .join("packages/app/qlang.toml")
                .to_string_lossy()
                .replace('\\', "/")
        )],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace member manifest after path add-dependency"
        ),
        "[dependencies]\n\"vendor.core\" = \"../../vendor/core\"\n\n[package]\nname = \"app\"\n"
    );
}

#[test]
fn project_add_dependency_refuses_name_and_path_together() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-dependency-conflict");
    let project_root = temp.path().join("workspace");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(
        &mut init,
        "`ql project init` workspace for conflicting add-dependency selectors",
    );
    let (_stdout, stderr) = expect_success(
        "project-add-dependency-conflict",
        "workspace init for conflicting add-dependency selectors",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-conflict",
        "workspace init for conflicting add-dependency selectors",
        &stderr,
    )
    .unwrap();

    let mut add_dependency = ql_command(&workspace_root);
    add_dependency.args([
        "project",
        "add-dependency",
        &project_root.to_string_lossy(),
        "--package",
        "app",
        "--name",
        "core",
        "--path",
        &project_root.join("vendor/core").to_string_lossy(),
    ]);
    let output = run_command_capture(
        &mut add_dependency,
        "`ql project add-dependency` conflicting selectors",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-add-dependency-conflict",
        "add dependency with conflicting selectors",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-dependency-conflict",
        "add dependency with conflicting selectors",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-dependency-conflict",
        "add dependency with conflicting selectors",
        &stderr,
        "error: `ql project add-dependency` accepts either `--name <package>` or `--path <file-or-dir>`, not both",
    )
    .unwrap();
}

#[test]
fn project_add_dependency_refuses_missing_workspace_package() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-dependency-missing");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/app/src/main.ql");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(
        &mut init,
        "`ql project init` workspace for missing add-dependency",
    );
    let (_stdout, stderr) = expect_success(
        "project-add-dependency-missing",
        "workspace init for missing add-dependency",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-missing",
        "workspace init for missing add-dependency",
        &stderr,
    )
    .unwrap();

    let mut add_dependency = ql_command(&workspace_root);
    add_dependency.args([
        "project",
        "add-dependency",
        &request_path.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut add_dependency,
        "`ql project add-dependency` missing workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-add-dependency-missing",
        "add dependency with missing workspace package",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-dependency-missing",
        "add dependency with missing workspace package",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-dependency-missing",
        "add dependency with missing workspace package",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project add-dependency` workspace manifest `{}` does not contain package `core`",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_add_dependency_refuses_ambiguous_workspace_package() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-dependency-ambiguous");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/app/src/main.ql");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/a\", \"packages/b\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n",
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int {\n    return 0\n}\n",
    );
    temp.write(
        "workspace/packages/a/qlang.toml",
        "[package]\nname = \"util\"\n",
    );
    temp.write(
        "workspace/packages/b/qlang.toml",
        "[package]\nname = \"util\"\n",
    );

    let mut add_dependency = ql_command(&workspace_root);
    add_dependency.args([
        "project",
        "add-dependency",
        &request_path.to_string_lossy(),
        "--name",
        "util",
    ]);
    let output = run_command_capture(
        &mut add_dependency,
        "`ql project add-dependency` ambiguous workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-add-dependency-ambiguous",
        "add dependency with ambiguous workspace package",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-dependency-ambiguous",
        "add dependency with ambiguous workspace package",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-dependency-ambiguous",
        "add dependency with ambiguous workspace package",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project add-dependency` workspace manifest `{}` contains multiple members for package `util`: packages/a, packages/b",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_add_dependency_refuses_unresolved_workspace_member_metadata() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-dependency-broken-member");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/app/src/main.ql");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\", \"packages/broken\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n",
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int {\n    return 0\n}\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );
    temp.write(
        "workspace/packages/broken/qlang.toml",
        "[package]\nversion = \"0.1.0\"\n",
    );

    let mut add_dependency = ql_command(&workspace_root);
    add_dependency.args([
        "project",
        "add-dependency",
        &request_path.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut add_dependency,
        "`ql project add-dependency` broken workspace member metadata",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-add-dependency-broken-member",
        "add dependency with broken workspace member metadata",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-dependency-broken-member",
        "add dependency with broken workspace member metadata",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-dependency-broken-member",
        "add dependency with broken workspace member metadata",
        &stderr.replace('\\', "/"),
        "error: `ql project add-dependency` failed to inspect workspace member `packages/broken`: manifest",
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-dependency-broken-member",
        "add dependency with broken workspace member metadata",
        &stderr,
        "does not declare `[package].name`",
    )
    .unwrap();
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace member manifest after rejected add-dependency"
        ),
        "[package]\nname = \"app\"\n"
    );
}

#[test]
fn project_remove_dependency_updates_existing_package_manifest() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-success");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/app/src/main.ql");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\ncore = \"../core\"\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int {\n    return 0\n}\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &request_path.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency` existing package manifest",
    );
    let (stdout, stderr) = expect_success(
        "project-remove-dependency-success",
        "remove dependency from existing package manifest",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependency-success",
        "remove dependency from existing package manifest",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-remove-dependency-success",
        &stdout,
        &[&format!(
            "updated: {}",
            project_root
                .join("packages/app/qlang.toml")
                .to_string_lossy()
                .replace('\\', "/")
        )],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace member manifest after remove-dependency"
        ),
        "[package]\nname = \"app\"\n"
    );
}

#[test]
fn project_remove_dependency_updates_existing_package_manifest_from_member_directory() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-member-dir");
    let fixture = write_app_core_workspace_fixture(
        &temp,
        "[package]\nname = \"app\"\n\n[dependencies]\ncore = \"../core\"\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &fixture.app_member_dir.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency` workspace member directory",
    );
    let (stdout, stderr) = expect_success(
        "project-remove-dependency-member-dir",
        "remove dependency from existing package manifest from member directory",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependency-member-dir",
        "remove dependency from existing package manifest from member directory",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-remove-dependency-member-dir",
        &stdout.replace('\\', "/"),
        &[&format!(
            "updated: {}",
            fixture
                .app_manifest_path
                .to_string_lossy()
                .replace('\\', "/")
        )],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &fixture.app_manifest_path,
            "workspace member manifest after member directory remove-dependency"
        ),
        "[package]\nname = \"app\"\n"
    );
}

#[test]
fn project_remove_dependency_supports_workspace_root_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-selector");
    let project_root = temp.path().join("workspace");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\ncore = \"../core\"\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &project_root.to_string_lossy(),
        "--package",
        "app",
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency --package` workspace root",
    );
    let (stdout, stderr) = expect_success(
        "project-remove-dependency-selector",
        "remove dependency from workspace root with package selector",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependency-selector",
        "remove dependency from workspace root with package selector",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-remove-dependency-selector",
        &stdout,
        &[&format!(
            "updated: {}",
            project_root
                .join("packages/app/qlang.toml")
                .to_string_lossy()
                .replace('\\', "/")
        )],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace member manifest after selected remove-dependency"
        ),
        "[package]\nname = \"app\"\n"
    );
}

#[test]
fn project_remove_dependency_removes_legacy_reference_entry() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-legacy");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/app/src/main.ql");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n\n[references]\npackages = [\"../core\"]\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int {\n    return 0\n}\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &request_path.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency` legacy reference entry",
    );
    let (stdout, stderr) = expect_success(
        "project-remove-dependency-legacy",
        "remove legacy reference dependency from existing package manifest",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependency-legacy",
        "remove legacy reference dependency from existing package manifest",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-remove-dependency-legacy",
        &stdout,
        &[&format!(
            "updated: {}",
            project_root
                .join("packages/app/qlang.toml")
                .to_string_lossy()
                .replace('\\', "/")
        )],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace member manifest after legacy remove-dependency"
        ),
        "[package]\nname = \"app\"\n"
    );
}

#[test]
fn project_remove_dependency_all_refuses_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-all-package");
    let project_root = temp.path().join("workspace");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\ncore = \"../core\"\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &project_root.to_string_lossy(),
        "--package",
        "app",
        "--name",
        "core",
        "--all",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency --all --package` workspace root",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-remove-dependency-all-package",
        "remove dependency all with package selector",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-remove-dependency-all-package",
        "remove dependency all with package selector",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-remove-dependency-all-package",
        "remove dependency all with package selector",
        &stderr,
        "error: `ql project remove-dependency --all` does not accept `--package`; bulk cleanup already targets all dependents of `--name`",
    )
    .unwrap();
}

#[test]
fn project_remove_dependency_all_updates_all_workspace_dependents() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-all");
    let project_root = temp.path().join("workspace");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/tools\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\ncore = \"../core\"\n",
    );
    temp.write(
        "workspace/packages/tools/qlang.toml",
        "[package]\nname = \"tools\"\n\n[references]\npackages = [\"../core\"]\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &project_root.to_string_lossy(),
        "--name",
        "core",
        "--all",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency --all` workspace dependents",
    );
    let (stdout, stderr) = expect_success(
        "project-remove-dependency-all",
        "remove dependency from all workspace dependents",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependency-all",
        "remove dependency from all workspace dependents",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-remove-dependency-all",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "updated: {}",
                project_root
                    .join("packages/app/qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "updated: {}",
                project_root
                    .join("packages/tools/qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace app manifest after remove-dependency --all"
        ),
        "[package]\nname = \"app\"\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/tools/qlang.toml"),
            "workspace tools manifest after remove-dependency --all"
        ),
        "[package]\nname = \"tools\"\n"
    );
}

#[test]
fn project_remove_dependency_all_derives_package_name_from_member_source_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-all-derived-name");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/core/src/main.ql");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/tools\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\ncore = \"../core\"\n",
    );
    temp.write(
        "workspace/packages/tools/qlang.toml",
        "[package]\nname = \"tools\"\n\n[references]\npackages = [\"../core\"]\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );
    temp.write(
        "workspace/packages/core/src/main.ql",
        "fn main() -> Int {\n    return 0\n}\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &request_path.to_string_lossy(),
        "--all",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency --all` derived package name",
    );
    let (stdout, stderr) = expect_success(
        "project-remove-dependency-all-derived-name",
        "remove dependency from all workspace dependents with derived package name",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependency-all-derived-name",
        "remove dependency from all workspace dependents with derived package name",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-remove-dependency-all-derived-name",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "updated: {}",
                project_root
                    .join("packages/app/qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "updated: {}",
                project_root
                    .join("packages/tools/qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace app manifest after derived remove-dependency --all"
        ),
        "[package]\nname = \"app\"\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/tools/qlang.toml"),
            "workspace tools manifest after derived remove-dependency --all"
        ),
        "[package]\nname = \"tools\"\n"
    );
}

#[test]
fn project_remove_dependency_all_requires_name_for_workspace_root() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-all-derived-name-missing");
    let project_root = temp.path().join("workspace");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\ncore = \"../core\"\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &project_root.to_string_lossy(),
        "--all",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency --all` ambiguous workspace root",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-remove-dependency-all-derived-name-missing",
        "remove dependency from all workspace dependents with ambiguous workspace root",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-remove-dependency-all-derived-name-missing",
        "remove dependency from all workspace dependents with ambiguous workspace root",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-remove-dependency-all-derived-name-missing",
        "remove dependency from all workspace dependents with ambiguous workspace root",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project remove-dependency --all` could not derive a package name from `{}`; rerun with `--name <package>`",
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_remove_dependency_all_refuses_workspace_package_without_dependents() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-all-empty");
    let project_root = temp.path().join("workspace");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &project_root.to_string_lossy(),
        "--name",
        "core",
        "--all",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency --all` package without dependents",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-remove-dependency-all-empty",
        "remove dependency from workspace package without dependents",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-remove-dependency-all-empty",
        "remove dependency from workspace package without dependents",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-remove-dependency-all-empty",
        "remove dependency from workspace package without dependents",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project remove-dependency` workspace package `core` does not have any dependent members to update in workspace manifest `{}`",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_remove_updates_workspace_members_from_member_source_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-success");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/tools/src/main.ql");
    let removed_member_root = project_root.join("packages/tools");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut init, "`ql project init` workspace for remove");
    let (_stdout, stderr) = expect_success(
        "project-remove-success",
        "workspace init for remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-success",
        "workspace init for remove",
        &stderr,
    )
    .unwrap();

    let mut add_core = ql_command(&workspace_root);
    add_core.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(&mut add_core, "`ql project add` core for remove");
    let (_stdout, stderr) = expect_success(
        "project-remove-success",
        "workspace core add for remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-success",
        "workspace core add for remove",
        &stderr,
    )
    .unwrap();

    let mut add_tools = ql_command(&workspace_root);
    add_tools.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "tools",
        "--dependency",
        "app",
        "--dependency",
        "core",
    ]);
    let output = run_command_capture(&mut add_tools, "`ql project add` tools for remove");
    let (_stdout, stderr) = expect_success(
        "project-remove-success",
        "workspace tools add for remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-success",
        "workspace tools add for remove",
        &stderr,
    )
    .unwrap();

    let mut remove = ql_command(&workspace_root);
    remove.args([
        "project",
        "remove",
        &request_path.to_string_lossy(),
        "--name",
        "tools",
    ]);
    let output = run_command_capture(
        &mut remove,
        "`ql project remove` workspace member source path",
    );
    let (stdout, stderr) =
        expect_success("project-remove-success", "remove workspace member", &output).unwrap();
    expect_empty_stderr("project-remove-success", "remove workspace member", &stderr).unwrap();
    expect_stdout_contains_all(
        "project-remove-success",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "updated: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "removed: {}",
                removed_member_root.to_string_lossy().replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    let workspace_manifest = read_normalized_file(
        &project_root.join("qlang.toml"),
        "workspace manifest after remove",
    );
    assert!(
        workspace_manifest.contains("packages/app"),
        "workspace manifest should keep existing members after remove"
    );
    assert!(
        workspace_manifest.contains("packages/core"),
        "workspace manifest should keep unrelated members after remove"
    );
    assert!(
        !workspace_manifest.contains("packages/tools"),
        "workspace manifest should drop the removed member entry"
    );
    assert!(
        removed_member_root.is_dir(),
        "project remove should keep the removed member files on disk"
    );

    let mut graph = ql_command(&workspace_root);
    graph.args(["project", "graph", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut graph, "`ql project graph` after remove");
    let (stdout, stderr) = expect_success(
        "project-remove-success",
        "graph workspace after remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-success",
        "graph workspace after remove",
        &stderr,
    )
    .unwrap();
    let normalized_stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        "project-remove-success",
        &normalized_stdout,
        &[
            "workspace_members:",
            "  - packages/app",
            "  - packages/core",
        ],
    )
    .unwrap();
    assert!(
        !normalized_stdout.contains("packages/tools"),
        "workspace graph should not include the removed member, got:\n{stdout}"
    );
}

#[test]
fn project_remove_cascade_updates_dependents_and_workspace_members() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-cascade");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/core/src/main.ql");
    let removed_member_root = project_root.join("packages/core");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut init, "`ql project init` workspace for cascade remove");
    let (_stdout, stderr) = expect_success(
        "project-remove-cascade",
        "workspace init for cascade remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-cascade",
        "workspace init for cascade remove",
        &stderr,
    )
    .unwrap();

    let mut add_core = ql_command(&workspace_root);
    add_core.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(&mut add_core, "`ql project add` core for cascade remove");
    let (_stdout, stderr) = expect_success(
        "project-remove-cascade",
        "workspace core add for cascade remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-cascade",
        "workspace core add for cascade remove",
        &stderr,
    )
    .unwrap();

    let mut add_tools = ql_command(&workspace_root);
    add_tools.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "tools",
        "--dependency",
        "core",
    ]);
    let output = run_command_capture(&mut add_tools, "`ql project add` tools for cascade remove");
    let (_stdout, stderr) = expect_success(
        "project-remove-cascade",
        "workspace tools add for cascade remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-cascade",
        "workspace tools add for cascade remove",
        &stderr,
    )
    .unwrap();

    let mut remove = ql_command(&workspace_root);
    remove.args([
        "project",
        "remove",
        &request_path.to_string_lossy(),
        "--name",
        "core",
        "--cascade",
    ]);
    let output = run_command_capture(
        &mut remove,
        "`ql project remove --cascade` workspace member with dependents",
    );
    let (stdout, stderr) = expect_success(
        "project-remove-cascade",
        "remove workspace member with cascading dependency cleanup",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-cascade",
        "remove workspace member with cascading dependency cleanup",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-remove-cascade",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "updated: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "updated: {}",
                project_root
                    .join("packages/tools/qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "removed: {}",
                removed_member_root.to_string_lossy().replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    let workspace_manifest = read_normalized_file(
        &project_root.join("qlang.toml"),
        "workspace manifest after cascade remove",
    );
    assert!(
        workspace_manifest.contains("packages/app"),
        "workspace manifest should keep unrelated members after cascade remove"
    );
    assert!(
        workspace_manifest.contains("packages/tools"),
        "workspace manifest should keep dependents after cascade remove"
    );
    assert!(
        !workspace_manifest.contains("packages/core"),
        "workspace manifest should drop the removed member entry after cascade remove"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/tools/qlang.toml"),
            "dependent manifest after cascade remove"
        ),
        "[package]\nname = \"tools\"\n"
    );
    assert!(
        removed_member_root.is_dir(),
        "project remove --cascade should keep the removed member files on disk"
    );
}

#[test]
fn project_remove_refuses_workspace_member_with_dependents() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependent");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/core/src/main.ql");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(
        &mut init,
        "`ql project init` workspace for dependent remove",
    );
    let (_stdout, stderr) = expect_success(
        "project-remove-dependent",
        "workspace init for dependent remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependent",
        "workspace init for dependent remove",
        &stderr,
    )
    .unwrap();

    let mut add_core = ql_command(&workspace_root);
    add_core.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(&mut add_core, "`ql project add` core for dependent remove");
    let (_stdout, stderr) = expect_success(
        "project-remove-dependent",
        "workspace core add for dependent remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependent",
        "workspace core add for dependent remove",
        &stderr,
    )
    .unwrap();

    let mut add_tools = ql_command(&workspace_root);
    add_tools.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "tools",
        "--dependency",
        "core",
    ]);
    let output = run_command_capture(
        &mut add_tools,
        "`ql project add` tools for dependent remove",
    );
    let (_stdout, stderr) = expect_success(
        "project-remove-dependent",
        "workspace tools add for dependent remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependent",
        "workspace tools add for dependent remove",
        &stderr,
    )
    .unwrap();

    let mut remove = ql_command(&workspace_root);
    remove.args([
        "project",
        "remove",
        &request_path.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut remove,
        "`ql project remove` workspace member with dependents",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-remove-dependent",
        "remove workspace member with dependents",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-remove-dependent",
        "remove workspace member with dependents",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-remove-dependent",
        "remove workspace member with dependents",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project remove` cannot remove member package `core` from workspace manifest `{}` because other members still depend on it: packages/tools (tools); remove those edges first with `ql project remove-dependency <member> --name core` or rerun with `ql project remove <file-or-dir> --name core --cascade`",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();

    let workspace_manifest = read_normalized_file(
        &project_root.join("qlang.toml"),
        "workspace manifest after refused dependent remove",
    );
    assert!(
        workspace_manifest.contains("packages/core"),
        "workspace manifest should keep dependent member after refused remove"
    );
    assert!(
        project_root.join("packages/core").is_dir(),
        "refused remove should keep member directory on disk"
    );
}

#[test]
fn project_remove_refuses_unknown_workspace_member_package() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-missing");
    let project_root = temp.path().join("workspace");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut init, "`ql project init` workspace for missing remove");
    let (_stdout, stderr) = expect_success(
        "project-remove-missing",
        "workspace init for missing remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-missing",
        "workspace init for missing remove",
        &stderr,
    )
    .unwrap();

    let mut remove = ql_command(&workspace_root);
    remove.args([
        "project",
        "remove",
        &project_root.to_string_lossy(),
        "--name",
        "tools",
    ]);
    let output = run_command_capture(&mut remove, "`ql project remove` missing package");
    let (stdout, stderr) = expect_exit_code(
        "project-remove-missing",
        "remove missing workspace member package",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-remove-missing",
        "remove missing workspace member package",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-remove-missing",
        "remove missing workspace member package",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project remove` workspace manifest `{}` does not contain member package `tools`",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_remove_rejects_ambiguous_workspace_member_package_names() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-ambiguous");
    let project_root = temp.path().join("workspace");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/a\", \"packages/b\"]\n",
    );
    temp.write(
        "workspace/packages/a/qlang.toml",
        "[package]\nname = \"util\"\n",
    );
    temp.write(
        "workspace/packages/b/qlang.toml",
        "[package]\nname = \"util\"\n",
    );

    let mut remove = ql_command(&workspace_root);
    remove.args([
        "project",
        "remove",
        &project_root.to_string_lossy(),
        "--name",
        "util",
    ]);
    let output = run_command_capture(&mut remove, "`ql project remove` ambiguous package");
    let (stdout, stderr) = expect_exit_code(
        "project-remove-ambiguous",
        "remove ambiguous workspace member package",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-remove-ambiguous",
        "remove ambiguous workspace member package",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-remove-ambiguous",
        "remove ambiguous workspace member package",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project remove` workspace manifest `{}` contains multiple members for package `util`: packages/a, packages/b",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}
