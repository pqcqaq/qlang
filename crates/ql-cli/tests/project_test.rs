mod support;

use std::process::Stdio;
use std::time::{Duration, Instant};

use ql_driver::{ToolchainOptions, discover_toolchain};
use serde_json::Value as JsonValue;
use support::{
    TempDir, assert_no_build_lock_directories, executable_output_path, expect_empty_stderr,
    expect_exit_code, expect_stderr_contains, expect_stdout_contains_all, expect_success,
    ql_command, run_command_capture, sleep_program_source, wait_for_path_exists, workspace_root,
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

fn normalize_output_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&normalize_output_text(stdout))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
}

#[test]
fn test_package_path_runs_discovered_tests() {
    if !toolchain_available("`ql test` package test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source root");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("app/tests/smoke.ql", "fn main() -> Int { return 0 }\n");
    temp.write("app/tests/api/basic.ql", "fn main() -> Int { return 0 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test` package path");
    let (stdout, stderr) = expect_success("project-test-package", "package smoke tests", &output)
        .expect("package-path `ql test` should succeed");
    expect_empty_stderr("project-test-package", "package smoke tests", &stderr)
        .expect("package-path `ql test` should not print stderr");
    expect_stdout_contains_all(
        "project-test-package",
        &stdout.replace('\\', "/"),
        &[
            "test tests/api/basic.ql ... ok",
            "test tests/smoke.ql ... ok",
            "test result: ok. 2 passed; 0 failed",
        ],
    )
    .expect("package-path `ql test` should run all discovered tests");
}

#[test]
fn test_workspace_path_prebuilds_selected_members_that_are_also_dependencies() {
    if !toolchain_available("`ql test` selected dependency member test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-selected-dependency-member");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/core/src"))
        .expect("create core package source tree");
    std::fs::create_dir_all(project_root.join("packages/core/tests"))
        .expect("create core package tests");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app package source tree");
    std::fs::create_dir_all(project_root.join("packages/app/tests"))
        .expect("create app package tests");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/core", "packages/app"]
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
        "workspace/packages/core/src/lib.ql",
        "pub fn answer() -> Int { return 42 }\n",
    );
    temp.write(
        "workspace/packages/core/tests/smoke.ql",
        "fn main() -> Int { return 0 }\n",
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
        "workspace/packages/app/src/lib.ql",
        "pub fn helper() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/app/tests/smoke.ql",
        "use core.answer as answer\n\nfn main() -> Int {\n    return answer() - 42\n}\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql test` selected dependency member workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-test-selected-dependency-member",
        "workspace selected dependency member test",
        &output,
    )
    .expect("workspace `ql test` should prebuild selected members that are also dependencies");
    expect_empty_stderr(
        "project-test-selected-dependency-member",
        "workspace selected dependency member test",
        &stderr,
    )
    .expect("selected dependency member test should not print stderr");
    expect_stdout_contains_all(
        "project-test-selected-dependency-member",
        &stdout.replace('\\', "/"),
        &[
            "test packages/core/tests/smoke.ql ... ok",
            "test packages/app/tests/smoke.ql ... ok",
            "test result: ok. 2 passed; 0 failed",
        ],
    )
    .expect("workspace selected dependency member test should report two passing tests");
}

#[test]
fn test_package_path_lists_discovered_tests_without_running_them() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-list");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source root");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("app/tests/smoke.ql", "fn main() -> Int { return nope }\n");
    temp.write("app/tests/api/basic.ql", "this is not valid qlang\n");
    temp.write(
        "app/tests/ui/type_error.ql",
        "fn main() -> Int { return nope }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root).arg("--list");
    let output = run_command_capture(&mut command, "`ql test --list` package path");
    let (stdout, stderr) = expect_success("project-test-list", "package test listing", &output)
        .expect("package-path `ql test --list` should succeed");
    expect_empty_stderr("project-test-list", "package test listing", &stderr)
        .expect("package-path `ql test --list` should not print stderr");
    expect_stdout_contains_all(
        "project-test-list",
        &stdout.replace('\\', "/"),
        &[
            "tests/api/basic.ql",
            "tests/smoke.ql",
            "tests/ui/type_error.ql",
            "test listing: 3 discovered",
        ],
    )
    .expect("package-path `ql test --list` should print discovered tests without building them");
}

#[test]
fn test_package_path_reports_json_success() {
    if !toolchain_available("`ql test --json` package test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-json-success");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source root");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("app/tests/smoke.ql", "fn main() -> Int { return 0 }\n");
    temp.write("app/tests/api/basic.ql", "fn main() -> Int { return 0 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test", "--json"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test --json` package path");
    let (stdout, stderr) = expect_success(
        "project-test-json-success",
        "package test json success",
        &output,
    )
    .expect("package-path `ql test --json` should succeed");
    expect_empty_stderr(
        "project-test-json-success",
        "package test json success",
        &stderr,
    )
    .expect("package-path `ql test --json` should not print stderr");

    let actual = parse_json_output("project-test-json-success", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.test.v1",
        "path": project_root.display().to_string().replace('\\', "/"),
        "requested_profile": "debug",
        "profile_overridden": false,
        "package_name": JsonValue::Null,
        "filter": JsonValue::Null,
        "list_only": false,
        "status": "ok",
        "discovered_total": 2,
        "selected_total": 2,
        "targets": [
            {
                "path": "tests/api/basic.ql",
                "kind": "smoke",
                "profile": "debug",
            },
            {
                "path": "tests/smoke.ql",
                "kind": "smoke",
                "profile": "debug",
            }
        ],
        "passed": 2,
        "failed": 0,
        "failures": [],
    });
    assert_eq!(
        actual, expected,
        "package-path `ql test --json` should match the stable success contract"
    );
}

#[test]
fn test_workspace_path_runs_member_tests_and_skips_members_without_tests() {
    if !toolchain_available("`ql test` workspace test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-workspace");
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
        "pub fn helper() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        "pub fn helper() -> Int { return 2 }\n",
    );
    temp.write(
        "workspace/packages/app/tests/smoke.ql",
        "fn main() -> Int { return 0 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test` workspace path");
    let (stdout, stderr) =
        expect_success("project-test-workspace", "workspace smoke tests", &output)
            .expect("workspace-path `ql test` should succeed");
    expect_empty_stderr("project-test-workspace", "workspace smoke tests", &stderr)
        .expect("workspace-path `ql test` should not print stderr");
    expect_stdout_contains_all(
        "project-test-workspace",
        &stdout.replace('\\', "/"),
        &[
            "test packages/app/tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .expect("workspace-path `ql test` should run member tests and skip members without tests");
}

#[test]
fn test_package_path_reports_failing_test_process() {
    if !toolchain_available("`ql test` failing-test case") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-failure");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source root");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("app/tests/fail.ql", "fn main() -> Int { return 7 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test` failing package");
    let (stdout, stderr) = expect_exit_code(
        "project-test-failure",
        "failing package smoke tests",
        &output,
        1,
    )
    .expect("`ql test` should report failing test processes");
    expect_stdout_contains_all(
        "project-test-failure",
        &stdout.replace('\\', "/"),
        &["test tests/fail.ql ... FAILED"],
    )
    .expect("failing package smoke tests should mark the test as failed on stdout");
    expect_stderr_contains(
        "project-test-failure",
        "failing package smoke tests",
        &stderr,
        "reason: test process exited with code 7",
    )
    .expect("failing package smoke tests should report the child exit code");
    expect_stderr_contains(
        "project-test-failure",
        "failing package smoke tests",
        &stderr,
        "test result: FAILED. 0 passed; 1 failed",
    )
    .expect("failing package smoke tests should print the failed summary");
}

#[test]
fn test_package_path_reports_json_build_failure_without_stderr_noise() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-json-build-failure");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source root");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("app/tests/broken.ql", "fn main() -> Int { return nope }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["test", "--json"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql test --json` build failure");
    let (stdout, stderr) = expect_exit_code(
        "project-test-json-build-failure",
        "package test json build failure",
        &output,
        1,
    )
    .expect("package-path `ql test --json` should surface build failures in json");
    expect_empty_stderr(
        "project-test-json-build-failure",
        "package test json build failure",
        &stderr,
    )
    .expect("package-path `ql test --json` build failures should not print stderr noise");

    let actual = parse_json_output("project-test-json-build-failure", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.test.v1",
        "path": project_root.display().to_string().replace('\\', "/"),
        "requested_profile": "debug",
        "profile_overridden": false,
        "package_name": JsonValue::Null,
        "filter": JsonValue::Null,
        "list_only": false,
        "status": "failed",
        "discovered_total": 1,
        "selected_total": 1,
        "targets": [
            {
                "path": "tests/broken.ql",
                "kind": "smoke",
                "profile": "debug",
            }
        ],
        "passed": 0,
        "failed": 1,
        "failures": [
            {
                "path": "tests/broken.ql",
                "kind": "build",
            }
        ],
    });
    assert_eq!(
        actual, expected,
        "package-path `ql test --json` should report build failures via the stable json contract"
    );
}

#[test]
fn test_project_path_json_holds_executable_lock_while_smoke_test_runs() {
    if !toolchain_available("`ql test --json` executable lock during smoke execution test") {
        return;
    }

    let temp = TempDir::new("ql-project-test-json-executable-lock-during-execution");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for executable lock test");
    std::fs::create_dir_all(project_root.join("tests"))
        .expect("create package test tree for executable lock test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("app/tests/smoke.ql", &sleep_program_source(900));
    let smoke_output = executable_output_path(&project_root.join("target/ql/debug/tests"), "smoke");
    let workspace_root = workspace_root();

    let mut first = ql_command(&workspace_root);
    first.current_dir(temp.path());
    first.args(["test", "--json"]).arg(&project_root);
    first.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut first = first
        .spawn()
        .expect("spawn first long-running `ql test --json`");
    wait_for_path_exists(
        "project-test-json-executable-lock-during-execution",
        "first long-running smoke test executable",
        &smoke_output,
        Duration::from_secs(20),
    )
    .expect("first long-running smoke test should create the executable before execution");
    std::thread::sleep(Duration::from_millis(50));
    assert!(
        first
            .try_wait()
            .expect("poll first long-running `ql test --json`")
            .is_none(),
        "first long-running smoke test should still hold the executable while the second test starts"
    );

    let mut second = ql_command(&workspace_root);
    second.current_dir(temp.path());
    second.args(["test", "--json"]).arg(&project_root);
    second.stdout(Stdio::piped()).stderr(Stdio::piped());
    let second_started = Instant::now();
    let second = second
        .spawn()
        .expect("spawn second `ql test --json` against same smoke executable");

    let second_output = second
        .wait_with_output()
        .expect("wait for second executable-lock `ql test --json`");
    let second_elapsed = second_started.elapsed();
    let first_output = first
        .wait_with_output()
        .expect("wait for first executable-lock `ql test --json`");

    for (label, output) in [("first", first_output), ("second", second_output)] {
        let (stdout, stderr) = expect_success(
            "project-test-json-executable-lock-during-execution",
            &format!("{label} long-running executable-lock test"),
            &output,
        )
        .unwrap_or_else(|message| panic!("{message}"));
        expect_empty_stderr(
            "project-test-json-executable-lock-during-execution",
            &format!("{label} long-running executable-lock test"),
            &stderr,
        )
        .unwrap_or_else(|message| panic!("{message}"));
        let json = parse_json_output(
            "project-test-json-executable-lock-during-execution",
            &stdout,
        );
        assert_eq!(json["schema"], "ql.test.v1");
        assert_eq!(json["status"], "ok");
        assert_eq!(json["passed"], 1);
        assert_eq!(json["failed"], 0);
    }

    assert!(
        second_elapsed >= Duration::from_millis(1000),
        "second test should wait for the first execution lock before rebuilding the same executable; elapsed {second_elapsed:?}"
    );
    assert_no_build_lock_directories(
        "project-test-json-executable-lock-during-execution",
        &project_root,
    );
}
