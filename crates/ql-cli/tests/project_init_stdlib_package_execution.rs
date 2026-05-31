mod support;

use serde_json::Value as JsonValue;
use support::project_init_stdlib::{
    assert_build_json_includes_target, assert_stdlib_dependency_build_targets, parse_json_output,
    toolchain_available, write_repo_stdlib_fixture,
};
use support::{
    TempDir, executable_output_path, expect_empty_stderr, expect_exit_code, expect_file_exists,
    expect_silent_output, expect_stdout_contains_all, expect_success, ql_command,
    run_command_capture, static_library_output_path, workspace_root,
};

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
